//! Looks up a player's current skin through Mojang's public profile services,
//! the ones the game itself relies on. NameMC, SkinMC and MinecraftSkins are
//! only ever linked to: NameMC and MinecraftSkins forbid automated access in
//! their terms, and SkinMC documents no API.

use crate::ErrorKind;
use anyhow::bail;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;

const PROFILE_BY_NAME_URL: &str =
    "https://api.minecraftservices.com/minecraft/profile/lookup/name";
const PROFILE_BY_ID_URL: &str =
    "https://sessionserver.mojang.com/session/minecraft/profile";
const TEXTURE_HOST: &str = "textures.minecraft.net";
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_PROFILE_BYTES: usize = 64 * 1024;
const MAX_TEXTURE_BYTES: usize = 1024 * 1024;
const MAX_NAME_LENGTH: usize = 16;
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkinModel {
    Classic,
    Slim,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlayerSkin {
    pub id: String,
    pub name: String,
    pub model: SkinModel,
    /// A `data:` URL of the skin, or `None` for a player on a default skin.
    pub texture: Option<String>,
    pub texture_key: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SkinLookup {
    Found { player: PlayerSkin },
    NotFound,
    RateLimited,
    Unavailable,
}

#[derive(Debug, Eq, PartialEq)]
enum PlayerQuery {
    Name(String),
    Id(String),
}

#[derive(Deserialize)]
struct ProfileId {
    id: String,
}

#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
    #[serde(default)]
    properties: Vec<ProfileProperty>,
}

#[derive(Deserialize)]
struct ProfileProperty {
    name: String,
    value: String,
}

#[derive(Deserialize)]
struct TexturesProperty {
    textures: Textures,
}

#[derive(Deserialize)]
struct Textures {
    #[serde(rename = "SKIN")]
    skin: Option<SkinTexture>,
}

#[derive(Deserialize)]
struct SkinTexture {
    url: String,
    metadata: Option<SkinMetadata>,
}

#[derive(Deserialize)]
struct SkinMetadata {
    model: Option<String>,
}

enum LookupFailure {
    NotFound,
    RateLimited,
    Unavailable,
}

/// Development builds can point at a local server to test failures.
#[cfg(debug_assertions)]
fn local_api_base() -> Option<String> {
    std::env::var("PAWRINTH_MOJANG_API_BASE")
        .ok()
        .filter(|url| url.starts_with("http://127.0.0.1:"))
}

fn name_url(name: &str) -> String {
    #[cfg(debug_assertions)]
    if let Some(base) = local_api_base() {
        return format!("{base}/lookup/{name}");
    }
    format!("{PROFILE_BY_NAME_URL}/{name}")
}

fn profile_url(id: &str) -> String {
    #[cfg(debug_assertions)]
    if let Some(base) = local_api_base() {
        return format!("{base}/profile/{id}");
    }
    format!("{PROFILE_BY_ID_URL}/{id}")
}

fn http_client() -> &'static reqwest::Client {
    #[cfg(debug_assertions)]
    if local_api_base().is_some() {
        return &crate::util::fetch::INSECURE_REQWEST_CLIENT;
    }
    &crate::util::fetch::REQWEST_CLIENT
}

fn parse_query(query: &str) -> Option<PlayerQuery> {
    let query = query.trim();
    if let Ok(id) = Uuid::try_parse(query) {
        return Some(PlayerQuery::Id(id.simple().to_string()));
    }
    let valid_name = !query.is_empty()
        && query.len() <= MAX_NAME_LENGTH
        && query
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || letter == '_');
    valid_name.then(|| PlayerQuery::Name(query.to_string()))
}

/// Profiles are signed by Mojang but read here without trusting where the URL
/// inside points, since it decides what this app downloads next.
fn texture_url(url: &str) -> Option<String> {
    let mut parsed = url::Url::parse(url).ok()?;
    #[cfg(debug_assertions)]
    if local_api_base().is_some() && parsed.host_str() == Some("127.0.0.1") {
        return Some(parsed.to_string());
    }
    if parsed.host_str() != Some(TEXTURE_HOST)
        || !parsed.path().starts_with("/texture/")
    {
        return None;
    }
    parsed.set_scheme("https").ok()?;
    Some(parsed.to_string())
}

/// `Ok(None)` is a player on a default skin, which profiles simply leave out.
fn skin_in_profile(
    profile: &Profile,
) -> Result<Option<(String, SkinModel)>, LookupFailure> {
    let Some(property) = profile
        .properties
        .iter()
        .find(|property| property.name == "textures")
    else {
        return Ok(None);
    };
    let decoded = BASE64_STANDARD
        .decode(&property.value)
        .map_err(|_| LookupFailure::Unavailable)?;
    let textures: TexturesProperty = serde_json::from_slice(&decoded)
        .map_err(|_| LookupFailure::Unavailable)?;
    let Some(skin) = textures.textures.skin else {
        return Ok(None);
    };
    let url = texture_url(&skin.url).ok_or(LookupFailure::Unavailable)?;
    let slim = skin
        .metadata
        .and_then(|metadata| metadata.model)
        .is_some_and(|model| model == "slim");
    Ok(Some((
        url,
        if slim {
            SkinModel::Slim
        } else {
            SkinModel::Classic
        },
    )))
}

async fn fetch_limited(
    url: &str,
    limit: usize,
) -> Result<bytes::Bytes, LookupFailure> {
    let response = http_client()
        .get(url)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .map_err(|_| LookupFailure::Unavailable)?;
    match response.status() {
        StatusCode::OK => {}
        StatusCode::NOT_FOUND | StatusCode::NO_CONTENT => {
            return Err(LookupFailure::NotFound);
        }
        StatusCode::TOO_MANY_REQUESTS => {
            return Err(LookupFailure::RateLimited);
        }
        _ => return Err(LookupFailure::Unavailable),
    }
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(LookupFailure::Unavailable);
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| LookupFailure::Unavailable)?;
    if bytes.len() > limit {
        return Err(LookupFailure::Unavailable);
    }
    Ok(bytes)
}

async fn find_player(query: PlayerQuery) -> Result<PlayerSkin, LookupFailure> {
    let id = match query {
        PlayerQuery::Id(id) => id,
        PlayerQuery::Name(name) => {
            let bytes =
                fetch_limited(&name_url(&name), MAX_PROFILE_BYTES).await?;
            let found: ProfileId = serde_json::from_slice(&bytes)
                .map_err(|_| LookupFailure::Unavailable)?;
            found.id
        }
    };
    let bytes = fetch_limited(&profile_url(&id), MAX_PROFILE_BYTES).await?;
    let profile: Profile = serde_json::from_slice(&bytes)
        .map_err(|_| LookupFailure::Unavailable)?;
    let skin = skin_in_profile(&profile)?;
    let Some((url, model)) = skin else {
        return Ok(PlayerSkin {
            id: profile.id,
            name: profile.name,
            model: SkinModel::Classic,
            texture: None,
            texture_key: None,
        });
    };
    let texture =
        fetch_limited(&url, MAX_TEXTURE_BYTES)
            .await
            .map_err(|failure| match failure {
                LookupFailure::RateLimited => LookupFailure::RateLimited,
                _ => LookupFailure::Unavailable,
            })?;
    if !texture.starts_with(PNG_SIGNATURE) {
        return Err(LookupFailure::Unavailable);
    }
    Ok(PlayerSkin {
        id: profile.id,
        name: profile.name,
        model,
        texture: Some(format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(&texture)
        )),
        texture_key: url.rsplit('/').next().map(String::from),
    })
}

/// Only ever runs when someone asks, because Mojang rate limits these services.
pub async fn lookup(query: &str) -> SkinLookup {
    let Some(query) = parse_query(query) else {
        return SkinLookup::NotFound;
    };
    match find_player(query).await {
        Ok(player) => SkinLookup::Found { player },
        Err(LookupFailure::NotFound) => SkinLookup::NotFound,
        Err(LookupFailure::RateLimited) => SkinLookup::RateLimited,
        Err(LookupFailure::Unavailable) => SkinLookup::Unavailable,
    }
}

fn validate_for_saving(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let is_png = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
    if !is_png {
        return Err("skins are saved as .png files".to_string());
    }
    if bytes.len() > MAX_TEXTURE_BYTES || !bytes.starts_with(PNG_SIGNATURE) {
        return Err("that isn't a skin image".to_string());
    }
    Ok(())
}

pub async fn save_png(path: &Path, bytes: &[u8]) -> crate::Result<()> {
    if let Err(reason) = validate_for_saving(path, bytes) {
        bail!(ErrorKind::InputError(reason));
    }
    crate::util::io::write(path, bytes).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_with(textures_json: &str) -> Profile {
        Profile {
            id: "069a79f444e94726a5befca90e38aaf5".to_string(),
            name: "Tester".to_string(),
            properties: vec![ProfileProperty {
                name: "textures".to_string(),
                value: BASE64_STANDARD.encode(textures_json),
            }],
        }
    }

    #[test]
    fn a_username_is_looked_up_by_name() {
        assert_eq!(
            parse_query("  Some_Player1 "),
            Some(PlayerQuery::Name("Some_Player1".to_string()))
        );
    }

    #[test]
    fn a_uuid_skips_the_name_lookup() {
        let id = "069a79f444e94726a5befca90e38aaf5".to_string();
        assert_eq!(parse_query(&id), Some(PlayerQuery::Id(id.clone())));
        assert_eq!(
            parse_query("069a79f4-44e9-4726-a5be-fca90e38aaf5"),
            Some(PlayerQuery::Id(id))
        );
    }

    #[test]
    fn text_that_cannot_be_a_name_is_never_sent() {
        for query in [
            "",
            "   ",
            "two words",
            "seventeen_letters",
            "../session",
            "name?x=1",
            "émile",
        ] {
            assert_eq!(parse_query(query), None, "{query}");
        }
    }

    #[test]
    fn textures_only_come_from_mojang_and_over_https() {
        assert_eq!(
            texture_url("http://textures.minecraft.net/texture/abc").as_deref(),
            Some("https://textures.minecraft.net/texture/abc")
        );
        assert_eq!(texture_url("https://example.com/texture/abc"), None);
        assert_eq!(
            texture_url("https://textures.minecraft.net/other/abc"),
            None
        );
        assert_eq!(texture_url("not a url"), None);
    }

    #[test]
    fn reads_a_classic_skin() {
        let profile = profile_with(
            r#"{"textures":{"SKIN":{"url":"http://textures.minecraft.net/texture/abc"}}}"#,
        );
        let skin = skin_in_profile(&profile).ok().flatten().unwrap();
        assert_eq!(skin.0, "https://textures.minecraft.net/texture/abc");
        assert_eq!(skin.1, SkinModel::Classic);
    }

    #[test]
    fn reads_a_slim_skin() {
        let profile = profile_with(
            r#"{"textures":{"SKIN":{"url":"http://textures.minecraft.net/texture/abc","metadata":{"model":"slim"}}}}"#,
        );
        let skin = skin_in_profile(&profile).ok().flatten().unwrap();
        assert_eq!(skin.1, SkinModel::Slim);
    }

    #[test]
    fn a_player_without_a_skin_is_not_an_error() {
        let profile = profile_with(r#"{"textures":{}}"#);
        assert!(matches!(skin_in_profile(&profile), Ok(None)));

        let bare = Profile {
            id: "a".to_string(),
            name: "b".to_string(),
            properties: Vec::new(),
        };
        assert!(matches!(skin_in_profile(&bare), Ok(None)));
    }

    #[test]
    fn a_broken_profile_is_reported_rather_than_guessed_at() {
        let mut profile = profile_with("{}");
        assert!(skin_in_profile(&profile).is_err());

        profile.properties[0].value = "not base64!".to_string();
        assert!(skin_in_profile(&profile).is_err());

        let elsewhere = profile_with(
            r#"{"textures":{"SKIN":{"url":"https://example.com/texture/abc"}}}"#,
        );
        assert!(skin_in_profile(&elsewhere).is_err());
    }

    #[test]
    fn only_png_files_are_saved() {
        let png = [PNG_SIGNATURE, b"rest"].concat();
        assert!(validate_for_saving(Path::new("a.png"), &png).is_ok());
        assert!(validate_for_saving(Path::new("a.PNG"), &png).is_ok());
        assert!(validate_for_saving(Path::new("a.exe"), &png).is_err());
        assert!(validate_for_saving(Path::new("a"), &png).is_err());
        assert!(validate_for_saving(Path::new("a.png"), b"not a png").is_err());
        let huge = [PNG_SIGNATURE, &vec![0; MAX_TEXTURE_BYTES]].concat();
        assert!(validate_for_saving(Path::new("a.png"), &huge).is_err());
    }
}
