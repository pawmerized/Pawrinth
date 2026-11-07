//! Browses and searches community skins through MineSkin's documented API. Its
//! terms ask apps to identify themselves, which the shared client does, to
//! respect rate limits, and to use an API key for automated requests. Each user
//! can add their own key, which is kept in the system's credential store.

use super::curseforge_key::{ApiKey, KeyCheck, classify_status};
use super::skin_lookup::SkinModel;
use crate::{ErrorKind, State};
use reqwest::header::{AUTHORIZATION, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use uuid::Uuid;

const API_BASE: &str = "https://api.mineskin.org/v2";
const PAGE_SIZE: &str = "30";
const MAX_QUERY_CHARS: usize = 64;
const MAX_RESPONSE_BYTES: usize = 512 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);
const MIN_TEXTURE_HASH_LENGTH: usize = 32;
const MAX_TEXTURE_HASH_LENGTH: usize = 64;
/// A page can be nothing but parts of larger models, so a few more are tried.
const MAX_PAGES_PER_REQUEST: usize = 3;
const CREDENTIAL_ACCOUNT: &str = "mineskin-api-key";
/// Tools upload each limb of a large model as a skin of its own.
const MODEL_PART_MARKERS: [&str; 5] = [
    "_ARM_LEFT",
    "_ARM_RIGHT",
    "_LEG_LEFT",
    "_LEG_RIGHT",
    "_BASE",
];
const TEST_UPLOAD_PREFIX: &str = "mineskin-tester";
const GENERATED_NAME_PREFIX: &str = "archmc-";
const PLACEHOLDER_NAME: &str = "string";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GallerySkin {
    pub id: String,
    pub name: Option<String>,
    pub texture_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GalleryPage {
    Ok {
        skins: Vec<GallerySkin>,
        next: Option<String>,
    },
    RateLimited,
    KeyRejected,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GallerySkinDetails {
    pub model: Option<SkinModel>,
    pub tags: Vec<String>,
    pub views: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GalleryKeyStatus {
    pub hint: Option<String>,
    pub store_available: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct GalleryKeyOutcome {
    pub check: KeyCheck,
    pub saved: bool,
}

#[derive(Deserialize)]
struct RawList {
    #[serde(default)]
    skins: Vec<RawSkin>,
    pagination: Option<RawPagination>,
}

#[derive(Deserialize)]
struct RawPagination {
    next: Option<RawCursor>,
}

#[derive(Deserialize)]
struct RawCursor {
    after: Option<String>,
}

#[derive(Deserialize)]
struct RawSkin {
    uuid: String,
    name: Option<String>,
    texture: Option<String>,
}

#[derive(Deserialize)]
struct RawDetailsResponse {
    skin: RawDetails,
}

#[derive(Deserialize)]
struct RawDetails {
    variant: Option<String>,
    #[serde(default)]
    tags: Vec<RawTag>,
    views: Option<u64>,
}

#[derive(Deserialize)]
struct RawTag {
    tag: String,
}

enum FetchFailure {
    RateLimited,
    KeyRejected,
    Unavailable,
}

/// Development builds can point at a local server to test failures.
#[cfg(debug_assertions)]
fn local_api_base() -> Option<String> {
    std::env::var("PAWRINTH_MINESKIN_API_BASE")
        .ok()
        .filter(|url| url.starts_with("http://127.0.0.1:"))
}

fn api_base() -> String {
    #[cfg(debug_assertions)]
    if let Some(base) = local_api_base() {
        return base;
    }
    API_BASE.to_string()
}

fn http_client() -> &'static reqwest::Client {
    #[cfg(debug_assertions)]
    if local_api_base().is_some() {
        return &crate::util::fetch::INSECURE_REQWEST_CLIENT;
    }
    &crate::util::fetch::REQWEST_CLIENT
}

fn is_model_part(name: &str) -> bool {
    MODEL_PART_MARKERS
        .iter()
        .any(|marker| name.contains(marker))
}

fn is_test_upload(name: &str) -> bool {
    name.starts_with(TEST_UPLOAD_PREFIX)
}

/// Names tools made up say nothing to a person, so those skins count as unnamed.
fn readable_name(name: Option<String>) -> Option<String> {
    let name = name?.trim().to_string();
    let generated = name.is_empty()
        || name == PLACEHOLDER_NAME
        || name.starts_with(GENERATED_NAME_PREFIX);
    (!generated).then_some(name)
}

fn is_texture_hash(hash: &str) -> bool {
    (MIN_TEXTURE_HASH_LENGTH..=MAX_TEXTURE_HASH_LENGTH).contains(&hash.len())
        && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Keeps what someone could wear, once each, leaving out anything malformed.
fn usable_skins(raw: Vec<RawSkin>) -> Vec<GallerySkin> {
    let mut seen = HashSet::new();
    raw.into_iter()
        .filter_map(|skin| {
            let name = skin.name.as_deref().unwrap_or_default();
            if is_model_part(name) || is_test_upload(name) {
                return None;
            }
            let texture_hash = skin.texture?;
            let usable = is_texture_hash(&texture_hash)
                && Uuid::try_parse(&skin.uuid).is_ok()
                && seen.insert(texture_hash.clone());
            usable.then(|| GallerySkin {
                id: skin.uuid,
                name: readable_name(skin.name),
                texture_hash,
            })
        })
        .collect()
}

fn next_cursor(pagination: Option<RawPagination>) -> Option<String> {
    pagination?
        .next?
        .after
        .filter(|cursor| Uuid::try_parse(cursor).is_ok())
}

fn clean_query(query: Option<&str>) -> Option<String> {
    let query = query?.trim();
    (!query.is_empty()).then(|| query.chars().take(MAX_QUERY_CHARS).collect())
}

fn details_from(raw: RawDetails) -> GallerySkinDetails {
    let mut seen = HashSet::new();
    GallerySkinDetails {
        model: match raw.variant.as_deref() {
            Some("slim") => Some(SkinModel::Slim),
            Some("classic") => Some(SkinModel::Classic),
            _ => None,
        },
        tags: raw
            .tags
            .into_iter()
            .map(|tag| tag.tag)
            .filter(|tag| seen.insert(tag.clone()))
            .collect(),
        views: raw.views,
    }
}

fn credential(app_identifier: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(app_identifier, CREDENTIAL_ACCOUNT)
}

fn store_error(action: &str) -> crate::Error {
    ErrorKind::InputError(format!(
        "your system's credential store couldn't {action} the key"
    ))
    .into()
}

async fn stored_key() -> crate::Result<Option<ApiKey>> {
    let identifier = State::get().await?.directories.app_identifier.clone();
    let stored = tokio::task::spawn_blocking(move || {
        match credential(&identifier)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error),
        }
    })
    .await
    .map_err(|_| store_error("read"))?
    .map_err(|_: keyring::Error| store_error("read"))?;
    Ok(stored.and_then(|value| ApiKey::parse(&value)))
}

fn bearer_header(key: &ApiKey) -> Option<HeaderValue> {
    let mut value =
        HeaderValue::from_str(&format!("Bearer {}", key.expose())).ok()?;
    value.set_sensitive(true);
    Some(value)
}

async fn fetch(
    path: &str,
    query: &[(&str, &str)],
) -> Result<bytes::Bytes, FetchFailure> {
    let mut request = http_client()
        .get(format!("{}{path}", api_base()))
        .query(query)
        .timeout(FETCH_TIMEOUT);
    if let Ok(Some(key)) = stored_key().await
        && let Some(header) = bearer_header(&key)
    {
        request = request.header(AUTHORIZATION, header);
    }
    let response = request
        .send()
        .await
        .map_err(|_| FetchFailure::Unavailable)?;
    match classify_status(response.status().as_u16()) {
        KeyCheck::Valid => {}
        KeyCheck::Rejected => return Err(FetchFailure::KeyRejected),
        KeyCheck::RateLimited => return Err(FetchFailure::RateLimited),
        _ => return Err(FetchFailure::Unavailable),
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| FetchFailure::Unavailable)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(FetchFailure::Unavailable);
    }
    Ok(bytes)
}

/// One page of the newest skins, or of the skins matching a keyword.
pub async fn list(query: Option<&str>, after: Option<&str>) -> GalleryPage {
    let query = clean_query(query);
    let mut cursor = after
        .filter(|cursor| Uuid::try_parse(cursor).is_ok())
        .map(String::from);
    for _ in 0..MAX_PAGES_PER_REQUEST {
        let mut params = vec![("size", PAGE_SIZE)];
        if let Some(query) = &query {
            params.push(("filter", query));
        }
        if let Some(cursor) = &cursor {
            params.push(("after", cursor));
        }
        let bytes = match fetch("/skins", &params).await {
            Ok(bytes) => bytes,
            Err(FetchFailure::RateLimited) => return GalleryPage::RateLimited,
            Err(FetchFailure::KeyRejected) => return GalleryPage::KeyRejected,
            Err(FetchFailure::Unavailable) => return GalleryPage::Unavailable,
        };
        let Ok(raw) = serde_json::from_slice::<RawList>(&bytes) else {
            return GalleryPage::Unavailable;
        };
        let next = next_cursor(raw.pagination);
        let skins = usable_skins(raw.skins);
        if !skins.is_empty() || next.is_none() {
            return GalleryPage::Ok { skins, next };
        }
        cursor = next;
    }
    GalleryPage::Ok {
        skins: Vec::new(),
        next: cursor,
    }
}

/// `None` when the details couldn't be had, which never stops a skin being shown.
pub async fn details(id: &str) -> Option<GallerySkinDetails> {
    Uuid::try_parse(id).ok()?;
    let bytes = fetch(&format!("/skins/{id}"), &[]).await.ok()?;
    let raw: RawDetailsResponse = serde_json::from_slice(&bytes).ok()?;
    Some(details_from(raw.skin))
}

pub async fn key_status() -> GalleryKeyStatus {
    match stored_key().await {
        Ok(key) => GalleryKeyStatus {
            hint: key.map(|key| key.hint()),
            store_available: true,
        },
        Err(_) => GalleryKeyStatus {
            hint: None,
            store_available: false,
        },
    }
}

/// A key MineSkin definitely refuses isn't saved. One that can't be checked right
/// now is, so being offline doesn't lose it.
pub async fn key_save(raw: &str) -> crate::Result<GalleryKeyOutcome> {
    let malformed = GalleryKeyOutcome {
        check: KeyCheck::Malformed,
        saved: false,
    };
    let Some(key) = ApiKey::parse(raw) else {
        return Ok(malformed);
    };
    let Some(header) = bearer_header(&key) else {
        return Ok(malformed);
    };
    let response = http_client()
        .get(format!("{}/me", api_base()))
        .header(AUTHORIZATION, header)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await;
    let check = match response {
        Ok(response) => classify_status(response.status().as_u16()),
        // The error's text can include request details, so it isn't kept.
        Err(_) => KeyCheck::Unreachable,
    };
    if check == KeyCheck::Rejected {
        return Ok(GalleryKeyOutcome {
            check,
            saved: false,
        });
    }
    let identifier = State::get().await?.directories.app_identifier.clone();
    tokio::task::spawn_blocking(move || {
        credential(&identifier)?.set_password(key.expose())
    })
    .await
    .map_err(|_| store_error("save"))?
    .map_err(|_| store_error("save"))?;
    Ok(GalleryKeyOutcome { check, saved: true })
}

pub async fn key_remove() -> crate::Result<()> {
    let identifier = State::get().await?.directories.app_identifier.clone();
    tokio::task::spawn_blocking(move || {
        match credential(&identifier)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error),
        }
    })
    .await
    .map_err(|_| store_error("remove"))?
    .map_err(|_: keyring::Error| store_error("remove"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str =
        "5b0b78732cbf7f6a96925a299cc855ed1ec1e84189fee491f6b57255b43bf17e";
    const ID: &str = "ad321cbea8904f9c9bd96a3262584ad7";

    fn raw(name: Option<&str>, texture: &str) -> RawSkin {
        RawSkin {
            uuid: ID.to_string(),
            name: name.map(String::from),
            texture: Some(texture.to_string()),
        }
    }

    #[test]
    fn a_named_skin_keeps_its_name() {
        let skins = usable_skins(vec![raw(Some("Cat"), HASH)]);
        assert_eq!(
            skins,
            vec![GallerySkin {
                id: ID.to_string(),
                name: Some("Cat".to_string()),
                texture_hash: HASH.to_string(),
            }]
        );
    }

    #[test]
    fn made_up_names_count_as_no_name() {
        for name in ["archmc-4b4be79", "string", "", "   "] {
            let skins = usable_skins(vec![raw(Some(name), HASH)]);
            assert_eq!(skins[0].name, None, "{name}");
        }
    }

    #[test]
    fn parts_of_larger_models_are_left_out() {
        for name in [
            "xexido99_ARM_RIGHT_2",
            "kerotiramisu_LEG_LEFT_2",
            "Viviantte_ARM_LEFT_2",
            "someone_LEG_RIGHT_1",
            "MoshePerez_BASE",
        ] {
            assert!(usable_skins(vec![raw(Some(name), HASH)]).is_empty());
        }
    }

    #[test]
    fn test_uploads_are_left_out() {
        assert!(
            usable_skins(vec![raw(Some("mineskin-tester-1612"), HASH)])
                .is_empty()
        );
    }

    #[test]
    fn the_same_skin_appears_once() {
        let skins =
            usable_skins(vec![raw(Some("dog"), HASH), raw(Some("dog"), HASH)]);
        assert_eq!(skins.len(), 1);
    }

    #[test]
    fn malformed_entries_are_left_out() {
        assert!(usable_skins(vec![raw(None, "../../etc")]).is_empty());
        assert!(usable_skins(vec![raw(None, "abc")]).is_empty());
        assert!(usable_skins(vec![raw(None, &"a".repeat(65))]).is_empty());
        let missing = RawSkin {
            uuid: ID.to_string(),
            name: None,
            texture: None,
        };
        assert!(usable_skins(vec![missing]).is_empty());
        let bad_id = RawSkin {
            uuid: "not-an-id".to_string(),
            name: None,
            texture: Some(HASH.to_string()),
        };
        assert!(usable_skins(vec![bad_id]).is_empty());
    }

    #[test]
    fn textures_without_leading_zeros_are_still_valid() {
        assert_eq!(usable_skins(vec![raw(None, &HASH[1..])]).len(), 1);
    }

    #[test]
    fn only_a_real_cursor_continues() {
        let next = |after: Option<&str>| {
            next_cursor(Some(RawPagination {
                next: Some(RawCursor {
                    after: after.map(String::from),
                }),
            }))
        };
        assert_eq!(next(Some(ID)).as_deref(), Some(ID));
        assert_eq!(next(Some("../other")), None);
        assert_eq!(next(None), None);
        assert_eq!(next_cursor(None), None);
    }

    #[test]
    fn queries_are_trimmed_and_limited() {
        assert_eq!(clean_query(Some("  cat ")).as_deref(), Some("cat"));
        assert_eq!(clean_query(Some("   ")), None);
        assert_eq!(clean_query(None), None);
        let long = "a".repeat(MAX_QUERY_CHARS + 10);
        assert_eq!(
            clean_query(Some(&long)).unwrap().chars().count(),
            MAX_QUERY_CHARS
        );
    }

    #[test]
    fn details_repeat_each_tag_once() {
        let details = details_from(RawDetails {
            variant: Some("slim".to_string()),
            tags: ["cat", "cute", "cat"]
                .map(|tag| RawTag {
                    tag: tag.to_string(),
                })
                .into(),
            views: Some(6),
        });
        assert_eq!(details.model, Some(SkinModel::Slim));
        assert_eq!(details.tags, ["cat", "cute"]);
        assert_eq!(details.views, Some(6));
    }

    #[test]
    fn an_unknown_model_is_not_guessed() {
        let details = details_from(RawDetails {
            variant: Some("unknown".to_string()),
            tags: Vec::new(),
            views: None,
        });
        assert_eq!(details.model, None);
    }

    #[test]
    fn the_key_header_is_marked_sensitive() {
        let key = ApiKey::parse("example-key").unwrap();
        let header = bearer_header(&key).unwrap();
        assert!(header.is_sensitive());
        assert!(!format!("{header:?}").contains("example-key"));
    }
}
