//! Technic modpacks, from the Technic Platform's public API. A pack is downloaded the
//! way the Technic launcher downloads it, as one zip or as the mod zips its Solder
//! server lists, then turned into an `.mrpack` whose overrides are the pack's own files,
//! and installed through the same jobs as any other pack. The mod loader and its version
//! come from the pack's own `bin/` folder.
//!
//! Technic packs are hosted by their creators. Downloads over plain HTTP are only made
//! when someone agrees to it for that pack, because nothing else checks those files.

use super::curseforge_pack::{
    OverrideEntry, Previous, display_name, file_sha1, overrides_to_keep,
    pack_dependencies, sha1_hex,
};
use crate::ErrorKind;
use crate::api::pack::install_from::{CreatePackLocation, PackFormat};
use crate::event::LoadingBarType;
use crate::event::emit::{emit_loading, init_loading};
use crate::install::{InstallJobSnapshot, InstallPostInstallEdit};
use crate::state::{InstanceLink, ModLoader, State};
use crate::util::fetch::{
    DownloadedFile, FetchProgressFn, INSECURE_REQWEST_CLIENT, REQWEST_CLIENT,
};
use anyhow::bail;
use futures::StreamExt;
use path_util::SafeRelativeUtf8UnixPathBuf;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

const API_BASE: &str = "https://api.technicpack.net";
/// The API asks which launcher build is calling. Pawrinth names itself.
const LAUNCHER_BUILD: &str = "pawrinth";
const INDEX_NAME: &str = "modrinth.index.json";
const RECORD_NAME: &str = "pawrinth-technic.json";
const ICON_NAME: &str = "icon.png";
const PACKS_FOLDER: &str = "technic-packs";
/// Where what's known about each installed pack is kept, beside the instances.
const INSTALLED_FOLDER: &str = "technic-packs";
const PACK_OVERRIDES: &str = "overrides";
/// A converted pack holds the whole modpack, so it's removed once it's installed, and
/// one that never got installed is removed after this long.
const KEEP_CONVERTED_PACKS: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_ATTEMPTS: u32 = 3;
/// Technic answers too many requests in a row with 429, so a list is shown a page at a
/// time, and only that page's packs are looked up, a couple at a time, and remembered.
const MAX_PAGE_SIZE: usize = 20;
/// Search returns 15 packs unless asked for more, and never more than 50. Trending is
/// always 20, and the API has no list of every pack.
const SEARCH_RESULTS: &str = "50";
/// With no list of every pack, the list people browse is gathered from broad
/// searches. Each returns a different 50 of Technic's packs, so these find about 550.
const CATALOG_SEARCHES: [&str; 12] =
    ["a", "e", "i", "o", "u", "r", "s", "t", "n", "l", "c", "d"];
const CATALOG_CONCURRENCY: usize = 3;
const CATALOG_KEPT_FOR: Duration = Duration::from_secs(60 * 60);
/// A list missing some searches, because Technic limited requests, is gathered again
/// sooner.
const PARTIAL_CATALOG_KEPT_FOR: Duration = Duration::from_secs(5 * 60);
const DETAIL_CONCURRENCY: usize = 2;
const DETAILS_KEPT_FOR: Duration = Duration::from_secs(60 * 60);
const DOWNLOAD_CONCURRENCY: usize = 4;
/// A file that doesn't match its listed MD5 is downloaded once more before giving up.
const CHECKSUM_ATTEMPTS: u32 = 2;
const MAX_QUERY_CHARS: usize = 100;
const MAX_SLUG_CHARS: usize = 100;
const MAX_SUMMARY_CHARS: usize = 300;
const MAX_UPDATES: usize = 10;
/// A loader profile is a few megabytes at most, so anything far bigger isn't one, and
/// a profile inside the jar is never unpacked past this.
const MAX_PROFILE_BYTES: u64 = 64 * 1024 * 1024;

fn lenient_number<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.trim().parse().ok(),
        _ => None,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SearchResponse {
    modpacks: Vec<SearchHit>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SearchHit {
    /// Technic numbers packs as they're made, so a higher id is a newer pack.
    id: Option<String>,
    name: String,
    slug: String,
    icon_url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct Image {
    url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct ApiUpdate {
    user: Option<String>,
    #[serde(deserialize_with = "lenient_number")]
    date: Option<u64>,
    content: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ApiPack {
    #[serde(deserialize_with = "lenient_number")]
    id: Option<u64>,
    /// The pack's slug, which Solder servers also know it by.
    name: String,
    display_name: String,
    user: Option<String>,
    url: Option<String>,
    platform_url: Option<String>,
    minecraft: Option<String>,
    #[serde(deserialize_with = "lenient_number")]
    ratings: Option<u64>,
    #[serde(deserialize_with = "lenient_number")]
    installs: Option<u64>,
    #[serde(deserialize_with = "lenient_number")]
    runs: Option<u64>,
    description: Option<String>,
    tags: Option<String>,
    is_official: bool,
    version: Option<String>,
    feed: Vec<ApiUpdate>,
    icon: Option<Image>,
    logo: Option<Image>,
    background: Option<Image>,
    solder: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SolderPack {
    recommended: Option<String>,
    latest: Option<String>,
    builds: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SolderBuild {
    minecraft: Option<String>,
    /// The memory the creator recommends, in megabytes. `0` means none.
    #[serde(deserialize_with = "lenient_number")]
    memory: Option<u64>,
    mods: Vec<SolderMod>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SolderMod {
    name: String,
    url: String,
    md5: Option<String>,
    #[serde(deserialize_with = "lenient_number")]
    filesize: Option<u64>,
}

/// A pack in a list.
#[derive(Clone, Debug, Serialize)]
pub struct PackSummary {
    pub slug: String,
    pub name: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub icon_url: Option<String>,
    pub game_version: Option<String>,
    pub installs: Option<u64>,
    pub ratings: Option<u64>,
    pub tags: Vec<String>,
}

/// One page of a list, and how many packs the whole list has.
#[derive(Clone, Debug, Serialize)]
pub struct PackPage {
    pub packs: Vec<PackSummary>,
    pub total: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct PackUpdate {
    pub author: Option<String>,
    /// Seconds since the Unix epoch.
    pub date: Option<u64>,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PackDetails {
    pub slug: String,
    pub name: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub background_url: Option<String>,
    pub game_version: Option<String>,
    pub version: Option<String>,
    pub installs: Option<u64>,
    pub runs: Option<u64>,
    pub ratings: Option<u64>,
    pub official: bool,
    pub tags: Vec<String>,
    pub page_url: Option<String>,
    pub updates: Vec<PackUpdate>,
    /// Whether Technic lists anything to download.
    pub downloadable: bool,
    /// Whether the pack downloads over plain HTTP.
    pub insecure: bool,
}

/// A pack ready to install.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedPack {
    pub token: String,
    pub slug: String,
    pub name: String,
    pub version: String,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
    /// Files that came over HTTPS but never matched the MD5 their Solder server lists,
    /// usually because the creator replaced a file without updating the list.
    pub unverified: Vec<String>,
    /// The memory the pack's creator recommends, which the new instance starts with
    /// when this PC can spare it.
    pub recommended_memory_mb: Option<u32>,
    /// For an update: files someone changed or added, which the update leaves alone.
    pub kept: Vec<String>,
}

/// Written into the converted pack, so installing it knows what it is.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PackRecord {
    slug: String,
    name: String,
    version: String,
    file_name: String,
    #[serde(default)]
    memory_mb: Option<u32>,
    #[serde(default)]
    for_instance: Option<String>,
    /// Every file in this version of the pack, with its SHA-1.
    #[serde(default)]
    files: BTreeMap<String, String>,
    /// For an update: files the last version put there, which this version dropped
    /// and nobody changed, so they're removed.
    #[serde(default)]
    stale: Vec<String>,
    /// For an update: mods someone had turned off, which stay off.
    #[serde(default)]
    keep_disabled: Vec<String>,
    #[serde(default)]
    allowed_insecure: bool,
}

/// What's remembered about an instance installed from a Technic pack, so the pack
/// can be updated without replacing what someone changed.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct InstalledPack {
    instance_id: String,
    slug: String,
    name: String,
    version: String,
    files: BTreeMap<String, String>,
    /// Whether someone agreed to this pack downloading over plain HTTP.
    #[serde(default)]
    allowed_insecure: bool,
}

/// An instance installed from a Technic pack, as the app shows it.
#[derive(Clone, Debug, Serialize)]
pub struct InstalledInfo {
    pub instance_id: String,
    pub slug: String,
    pub name: String,
    pub version: String,
}

struct UpdatePlan {
    skipped: HashSet<usize>,
    kept: Vec<String>,
    stale: Vec<String>,
    keep_disabled: Vec<String>,
}

#[derive(Clone)]
struct Source {
    url: String,
    md5: Option<String>,
    size: Option<u64>,
    label: String,
}

struct Plan {
    version: String,
    game_version: Option<String>,
    memory_mb: Option<u32>,
    sources: Vec<Source>,
}

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    source: usize,
    index: usize,
    relative: String,
    size: u64,
}

#[derive(Debug, Default)]
struct Collected {
    entries: Vec<Entry>,
    /// Lowercased `bin/` paths, and where the last copy of each is.
    bin: HashMap<String, (usize, usize)>,
    has_mods: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct VersionProfile {
    id: Option<String>,
    inherits_from: Option<String>,
    libraries: Vec<Library>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Library {
    name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct InstallProfile {
    version_info: Option<VersionProfile>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Transport {
    Secure,
    Insecure,
    Refused,
}

fn transport(url: &str) -> Transport {
    match url::Url::parse(url).map(|url| url.scheme().to_string()) {
        Ok(scheme) if scheme == "https" => Transport::Secure,
        Ok(scheme) if scheme == "http" => Transport::Insecure,
        _ => Transport::Refused,
    }
}

fn secure_image(image: Option<&Image>) -> Option<String> {
    image
        .and_then(|image| image.url.clone())
        .filter(|url| transport(url) == Transport::Secure)
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn shortened(text: &str, max: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max {
        return text;
    }
    let cut: String = text.chars().take(max).collect();
    format!("{}…", cut.trim_end())
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= MAX_SLUG_CHARS
        && slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn api_url(path: &str, params: &[(&str, &str)]) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in params {
        query.append_pair(key, value);
    }
    query.append_pair("build", LAUNCHER_BUILD);
    format!("{API_BASE}{path}?{}", query.finish())
}

fn pack_url(slug: &str) -> String {
    api_url(&format!("/modpack/{}", urlencoding::encode(slug)), &[])
}

/// Dropbox share links open a web page unless they ask for the file itself.
fn direct_download(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return url.to_string();
    };
    let is_dropbox = parsed.host_str().is_some_and(|host| {
        host == "dropbox.com" || host.ends_with(".dropbox.com")
    });
    if !is_dropbox {
        return url.to_string();
    }
    let pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(key, _)| key != "dl")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    parsed
        .query_pairs_mut()
        .clear()
        .extend_pairs(pairs)
        .append_pair("dl", "1");
    parsed.to_string()
}

#[derive(Debug)]
enum Fetched<T> {
    Ok(T),
    Limited,
    Failed(String),
}

async fn fetch_json<T: DeserializeOwned>(
    url: &str,
    service: &str,
    state: &State,
) -> Fetched<T> {
    let client = match transport(url) {
        Transport::Secure => &*REQWEST_CLIENT,
        Transport::Insecure => &*INSECURE_REQWEST_CLIENT,
        Transport::Refused => {
            return Fetched::Failed(format!(
                "{service} gave an address Pawrinth can't use"
            ));
        }
    };
    let Ok(_permit) = state.api_semaphore.0.acquire().await else {
        return Fetched::Failed(format!("could not reach {service}"));
    };
    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await;
    match response {
        Ok(response) if response.status().is_success() => {
            match response.bytes().await {
                Ok(bytes) => match serde_json::from_slice(&bytes) {
                    Ok(value) => Fetched::Ok(value),
                    Err(_) => Fetched::Failed(format!(
                        "{service} sent something Pawrinth couldn't read"
                    )),
                },
                Err(_) => Fetched::Failed(format!("could not reach {service}")),
            }
        }
        Ok(response) if response.status().as_u16() == 429 => Fetched::Limited,
        Ok(response) if response.status().as_u16() == 404 => {
            Fetched::Failed(format!("{service} couldn't find that modpack"))
        }
        Ok(response) => Fetched::Failed(format!(
            "{service} returned status {}",
            response.status().as_u16()
        )),
        // The error's text can include request details, so it isn't kept.
        Err(_) => Fetched::Failed(format!("could not reach {service}")),
    }
}

/// Retries what's worth retrying, waiting longer each time Technic asks it to slow down.
async fn get_json<T: DeserializeOwned>(
    url: &str,
    service: &str,
    state: &State,
) -> crate::Result<T> {
    let mut last_error = format!("could not reach {service}");
    for attempt in 1..=MAX_ATTEMPTS {
        match fetch_json(url, service, state).await {
            Fetched::Ok(value) => return Ok(value),
            Fetched::Limited => {
                last_error = format!(
                    "{service} is limiting requests, so try again in a minute"
                );
                tokio::time::sleep(Duration::from_secs(2 * u64::from(attempt)))
                    .await;
            }
            Fetched::Failed(error) if error.contains("couldn't find") => {
                bail!(ErrorKind::InputError(error))
            }
            Fetched::Failed(error) => {
                last_error = error;
                if attempt < MAX_ATTEMPTS {
                    tokio::time::sleep(Duration::from_millis(
                        500 * u64::from(attempt),
                    ))
                    .await;
                }
            }
        }
    }
    bail!(ErrorKind::InputError(last_error))
}

static DETAILS: LazyLock<Mutex<HashMap<String, (Instant, ApiPack)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn remembered(slug: &str) -> Option<ApiPack> {
    let details = DETAILS.lock().ok()?;
    let (fetched, pack) = details.get(slug)?;
    (fetched.elapsed() < DETAILS_KEPT_FOR).then(|| pack.clone())
}

fn remember(slug: &str, pack: &ApiPack) {
    if let Ok(mut details) = DETAILS.lock() {
        details.retain(|_, (fetched, _)| fetched.elapsed() < DETAILS_KEPT_FOR);
        details.insert(slug.to_string(), (Instant::now(), pack.clone()));
    }
}

async fn fresh_details(slug: &str, state: &State) -> crate::Result<ApiPack> {
    if !valid_slug(slug) {
        bail!(ErrorKind::InputError("unknown Technic modpack".to_string()));
    }
    let pack: ApiPack = get_json(&pack_url(slug), "Technic", state).await?;
    if pack.name.is_empty() {
        bail!(ErrorKind::InputError(
            "Technic couldn't find that modpack".to_string()
        ));
    }
    remember(slug, &pack);
    Ok(pack)
}

fn summary_of(hit: &SearchHit, details: Option<&ApiPack>) -> PackSummary {
    let name = details
        .and_then(|pack| non_empty(Some(&pack.display_name)))
        .or_else(|| non_empty(Some(&hit.name)))
        .unwrap_or_else(|| hit.slug.clone());
    PackSummary {
        slug: hit.slug.clone(),
        name,
        author: details.and_then(|pack| non_empty(pack.user.as_deref())),
        summary: details
            .and_then(|pack| non_empty(pack.description.as_deref()))
            .map(|text| shortened(&text, MAX_SUMMARY_CHARS)),
        icon_url: details
            .and_then(|pack| secure_image(pack.icon.as_ref()))
            .or_else(|| {
                hit.icon_url
                    .clone()
                    .filter(|url| transport(url) == Transport::Secure)
            }),
        game_version: details
            .and_then(|pack| non_empty(pack.minecraft.as_deref())),
        installs: details.and_then(|pack| pack.installs),
        ratings: details.and_then(|pack| pack.ratings),
        tags: details.map(pack_tags).unwrap_or_default(),
    }
}

fn pack_tags(pack: &ApiPack) -> Vec<String> {
    pack.tags
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

/// Details for a pack in a list, from memory or from Technic while it isn't limiting
/// requests. Lists only look up their first few packs.
async fn list_details(
    slug: String,
    limited: Arc<AtomicBool>,
    state: Arc<State>,
) -> Option<ApiPack> {
    if let Some(pack) = remembered(&slug) {
        return Some(pack);
    }
    if limited.load(Ordering::Relaxed) {
        return None;
    }
    match fetch_json::<ApiPack>(&pack_url(&slug), "Technic", &state).await {
        Fetched::Ok(pack) if !pack.name.is_empty() => {
            remember(&slug, &pack);
            Some(pack)
        }
        Fetched::Limited => {
            limited.store(true, Ordering::Relaxed);
            None
        }
        _ => None,
    }
}

static CATALOG: LazyLock<Mutex<Option<(Instant, bool, Vec<SearchHit>)>>> =
    LazyLock::new(|| Mutex::new(None));

/// Each pack once, in the order given, leaving out any with an unusable slug.
fn merged(lists: impl IntoIterator<Item = Vec<SearchHit>>) -> Vec<SearchHit> {
    let mut seen = std::collections::HashSet::new();
    lists
        .into_iter()
        .flatten()
        .filter(|hit| valid_slug(&hit.slug) && seen.insert(hit.slug.clone()))
        .collect()
}

/// `newest` and `name` reorder the list. Anything else keeps Technic's own order:
/// trending packs first, then each search's best matches.
fn sorted(mut hits: Vec<SearchHit>, sort: Option<&str>) -> Vec<SearchHit> {
    match sort {
        Some("newest") => hits.sort_by_key(|hit| {
            std::cmp::Reverse(
                hit.id.as_deref().and_then(|id| id.parse::<u64>().ok()),
            )
        }),
        Some("name") => {
            hits.sort_by_cached_key(|hit| hit.name.trim().to_lowercase())
        }
        _ => {}
    }
    hits
}

/// Owns its state, because a Tauri command's future can't hold borrowed data across
/// the stream.
async fn catalog_search(
    query: String,
    state: Arc<State>,
) -> Option<SearchResponse> {
    let url = api_url("/search", &[("q", &query), ("limit", SEARCH_RESULTS)]);
    get_json(&url, "Technic", &state).await.ok()
}

/// Technic's trending packs, then everything the broad searches found, remembered for
/// an hour so only the first visit waits for it.
async fn catalog(state: Arc<State>) -> crate::Result<Vec<SearchHit>> {
    if let Ok(cached) = CATALOG.lock()
        && let Some((gathered, complete, hits)) = cached.as_ref()
        && gathered.elapsed()
            < if *complete {
                CATALOG_KEPT_FOR
            } else {
                PARTIAL_CATALOG_KEPT_FOR
            }
    {
        return Ok(hits.clone());
    }
    let trending_url = api_url("/trending", &[]);
    let trending: SearchResponse =
        get_json(&trending_url, "Technic", &state).await?;
    let searches: Vec<Option<SearchResponse>> =
        futures::stream::iter(CATALOG_SEARCHES.map(String::from))
            .map(|query| catalog_search(query, state.clone()))
            .buffered(CATALOG_CONCURRENCY)
            .collect()
            .await;
    let complete = searches.iter().all(Option::is_some);
    let hits = merged(
        std::iter::once(trending.modpacks)
            .chain(searches.into_iter().flatten().map(|found| found.modpacks)),
    );
    if let Ok(mut cached) = CATALOG.lock() {
        *cached = Some((Instant::now(), complete, hits.clone()));
    }
    Ok(hits)
}

/// A page of the packs matching a search, or of the packs Pawrinth gathered from
/// Technic when there's no search, in the order `sort` names.
pub async fn browse(
    query: Option<String>,
    offset: usize,
    limit: usize,
    sort: Option<String>,
) -> crate::Result<PackPage> {
    let state = State::get().await?;
    let query = query
        .map(|query| query.trim().chars().take(MAX_QUERY_CHARS).collect())
        .filter(|query: &String| !query.is_empty());
    let hits = match &query {
        Some(query) => {
            let url =
                api_url("/search", &[("q", query), ("limit", SEARCH_RESULTS)]);
            let response: SearchResponse =
                get_json(&url, "Technic", &state).await?;
            merged([response.modpacks])
        }
        None => sorted(catalog(state.clone()).await?, sort.as_deref()),
    };
    let total = hits.len();
    let page: Vec<SearchHit> = hits
        .into_iter()
        .skip(offset)
        .take(limit.clamp(1, MAX_PAGE_SIZE))
        .collect();

    let limited = Arc::new(AtomicBool::new(false));
    let details: Vec<Option<ApiPack>> = futures::stream::iter(page.clone())
        .map(|hit| list_details(hit.slug, limited.clone(), state.clone()))
        .buffered(DETAIL_CONCURRENCY)
        .collect()
        .await;

    Ok(PackPage {
        packs: page
            .iter()
            .zip(details.iter())
            .map(|(hit, details)| summary_of(hit, details.as_ref()))
            .collect(),
        total,
    })
}

fn is_insecure(pack: &ApiPack) -> bool {
    [pack.url.as_deref(), pack.solder.as_deref()]
        .into_iter()
        .flatten()
        .filter(|url| !url.trim().is_empty())
        .any(|url| transport(url) == Transport::Insecure)
}

fn details_of(slug: &str, pack: &ApiPack) -> PackDetails {
    PackDetails {
        slug: slug.to_string(),
        name: non_empty(Some(&pack.display_name))
            .unwrap_or_else(|| pack.name.clone()),
        author: non_empty(pack.user.as_deref()),
        description: non_empty(pack.description.as_deref()),
        icon_url: secure_image(pack.icon.as_ref())
            .or_else(|| secure_image(pack.logo.as_ref())),
        background_url: secure_image(pack.background.as_ref()),
        game_version: non_empty(pack.minecraft.as_deref()),
        version: non_empty(pack.version.as_deref()),
        installs: pack.installs,
        runs: pack.runs,
        ratings: pack.ratings,
        official: pack.is_official,
        tags: pack_tags(pack),
        page_url: pack
            .platform_url
            .clone()
            .filter(|url| transport(url) == Transport::Secure),
        updates: pack
            .feed
            .iter()
            .filter_map(|update| {
                Some(PackUpdate {
                    author: non_empty(update.user.as_deref()),
                    date: update.date,
                    text: non_empty(update.content.as_deref())?,
                })
            })
            .take(MAX_UPDATES)
            .collect(),
        downloadable: non_empty(pack.solder.as_deref()).is_some()
            || non_empty(pack.url.as_deref()).is_some(),
        insecure: is_insecure(pack),
    }
}

pub async fn pack(slug: String) -> crate::Result<PackDetails> {
    let state = State::get().await?;
    let pack = match remembered(&slug) {
        Some(pack) => pack,
        None => fresh_details(&slug, &state).await?,
    };
    Ok(details_of(&slug, &pack))
}

fn check_transport(
    url: &str,
    allow_insecure: bool,
    name: &str,
) -> crate::Result<()> {
    match transport(url) {
        Transport::Secure => Ok(()),
        Transport::Insecure if allow_insecure => Ok(()),
        Transport::Insecure => bail!(ErrorKind::InputError(format!(
            "{name} downloads over an insecure connection, so it needs your OK first"
        ))),
        Transport::Refused => bail!(ErrorKind::InputError(format!(
            "{name} has a download address Pawrinth can't use"
        ))),
    }
}

/// The build a Solder server wants installed: its recommended one, or its newest.
/// Returns the pack's address on that server too.
async fn solder_build(
    pack: &ApiPack,
    name: &str,
    solder: &str,
    allow_insecure: bool,
    state: &State,
) -> crate::Result<(String, String)> {
    check_transport(solder, allow_insecure, name)?;
    let base = format!("{}/", solder.trim().trim_end_matches('/'));
    let slug = urlencoding::encode(&pack.name).into_owned();
    let address = format!("{base}modpack/{slug}");
    let listing: SolderPack =
        get_json(&address, "the pack's server", state).await?;
    let Some(build) = non_empty(listing.recommended.as_deref())
        .or_else(|| non_empty(listing.latest.as_deref()))
        .or_else(|| listing.builds.last().cloned())
    else {
        bail!(ErrorKind::InputError(format!(
            "{name} has no build to download"
        )));
    };
    Ok((address, build))
}

async fn solder_plan(
    pack: &ApiPack,
    name: &str,
    solder: &str,
    allow_insecure: bool,
    state: &State,
) -> crate::Result<Plan> {
    let (address, build) =
        solder_build(pack, name, solder, allow_insecure, state).await?;
    let build_address = format!("{address}/{}", urlencoding::encode(&build));
    let contents: SolderBuild =
        get_json(&build_address, "the pack's server", state).await?;
    if contents.mods.is_empty() {
        bail!(ErrorKind::InputError(format!(
            "{name}'s build {build} has no files"
        )));
    }
    let mut sources = Vec::new();
    for item in contents.mods {
        check_transport(&item.url, allow_insecure, name)?;
        sources.push(Source {
            url: item.url,
            md5: item
                .md5
                .map(|md5| md5.trim().to_ascii_lowercase())
                .filter(|md5| md5.len() == 32),
            size: item.filesize.filter(|size| *size > 0),
            label: item.name,
        });
    }
    Ok(Plan {
        version: build,
        game_version: non_empty(contents.minecraft.as_deref()),
        memory_mb: contents
            .memory
            .filter(|memory| *memory > 0)
            .and_then(|memory| u32::try_from(memory).ok()),
        sources,
    })
}

fn zip_plan(
    pack: &ApiPack,
    name: &str,
    url: &str,
    allow_insecure: bool,
) -> crate::Result<Plan> {
    check_transport(url, allow_insecure, name)?;
    Ok(Plan {
        version: non_empty(pack.version.as_deref())
            .unwrap_or_else(|| "1".to_string()),
        game_version: non_empty(pack.minecraft.as_deref()),
        memory_mb: None,
        sources: vec![Source {
            url: direct_download(url.trim()),
            md5: None,
            size: None,
            label: name.to_string(),
        }],
    })
}

fn md5_of(path: &Path) -> std::io::Result<String> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut context = md5::Context::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(format!("{:x}", context.finalize()));
        }
        context.consume(&buffer[..read]);
    }
}

/// Downloads one source, and says whether it matched its listed MD5.
async fn download_source(
    source: Source,
    share: f64,
    bar: Option<crate::event::LoadingBarId>,
    state: Arc<State>,
) -> crate::Result<(DownloadedFile, bool)> {
    let bar = bar.as_ref();
    let staging = state.directories.caches_dir();
    crate::util::io::create_dir_all(&staging).await?;
    let client = match transport(&source.url) {
        Transport::Secure => &*REQWEST_CLIENT,
        _ => &*INSECURE_REQWEST_CLIENT,
    };
    let mut reported = 0.0_f64;
    let mut attempt = 0;
    loop {
        attempt += 1;
        let mut progress = |current: u64,
                            total: u64|
         -> Pin<
            Box<dyn Future<Output = crate::Result<()>> + Send>,
        > {
            let total = source.size.unwrap_or(total);
            if let Some(bar) = bar
                && total > 0
            {
                let done = share * (current.min(total) as f64 / total as f64);
                if done > reported {
                    let _ = emit_loading(bar, done - reported, None);
                    reported = done;
                }
            }
            Box::pin(async { Ok(()) })
        };
        let downloaded = crate::util::fetch::fetch_file_with_client(
            &source.url,
            None,
            &state.fetch_semaphore,
            &state.pool,
            client,
            Some(&mut progress as &mut FetchProgressFn<'_>),
            &staging,
        )
        .await
        .map_err(|error| {
            ErrorKind::InputError(format!(
                "couldn't download {}: {error}",
                source.label
            ))
        })?;
        if let Some(bar) = bar
            && share > reported
        {
            let _ = emit_loading(bar, share - reported, None);
            reported = share;
        }
        let Some(expected) = &source.md5 else {
            if let Some(size) = source.size
                && downloaded.size != size
            {
                bail!(ErrorKind::InputError(format!(
                    "{} downloaded with the wrong size",
                    source.label
                )));
            }
            return Ok((downloaded, true));
        };
        let path = downloaded.path().to_path_buf();
        let actual =
            tokio::task::spawn_blocking(move || md5_of(&path)).await??;
        if &actual == expected {
            return Ok((downloaded, true));
        }
        if attempt < CHECKSUM_ATTEMPTS {
            continue;
        }
        if mismatch_is_tolerable(&source.url) {
            return Ok((downloaded, false));
        }
        bail!(ErrorKind::InputError(format!(
            "{} didn't match the checksum the pack's server lists, and it came over an insecure connection",
            source.label
        )));
    }
}

/// A listed MD5 comes from the same place as the file, so it only catches damaged
/// downloads. Over HTTPS the connection already does that, and a file that still
/// doesn't match was most likely replaced by its creator. Over plain HTTP the MD5 is
/// the only check there is.
fn mismatch_is_tolerable(url: &str) -> bool {
    transport(url) == Transport::Secure
}

async fn download_sources(
    sources: &[Source],
    name: &str,
    state: &Arc<State>,
) -> crate::Result<Vec<(DownloadedFile, bool)>> {
    let all_sizes_known = sources.iter().all(|source| source.size.is_some());
    let weights: Vec<f64> = sources
        .iter()
        .map(|source| match (all_sizes_known, source.size) {
            (true, Some(size)) => size.max(1) as f64,
            _ => 1.0,
        })
        .collect();
    let total_weight: f64 = weights.iter().sum::<f64>().max(1.0);
    let bar = init_loading(
        LoadingBarType::PackImport {
            pack_name: name.to_string(),
        },
        100.0,
        "Downloading the modpack",
    )
    .await
    .ok();
    let results: Vec<crate::Result<(DownloadedFile, bool)>> =
        futures::stream::iter(sources.iter().cloned().zip(weights))
            .map(|(source, weight)| {
                download_source(
                    source,
                    100.0 * weight / total_weight,
                    bar.clone(),
                    state.clone(),
                )
            })
            .buffered(DOWNLOAD_CONCURRENCY)
            .collect()
            .await;
    results.into_iter().collect()
}

fn is_clutter(relative: &str) -> bool {
    let lower = relative.to_ascii_lowercase();
    lower.starts_with("__macosx/")
        || lower.ends_with("/.ds_store")
        || lower == ".ds_store"
        || lower.ends_with("thumbs.db")
}

/// Every file the sources would leave in the pack folder, with later sources replacing
/// earlier ones the way unzipping them in order would.
fn collect_entries(paths: &[PathBuf]) -> crate::Result<Collected> {
    let mut collected = Collected::default();
    let mut positions: HashMap<String, usize> = HashMap::new();
    for (source, path) in paths.iter().enumerate() {
        let mut archive = zip::ZipArchive::new(std::io::BufReader::new(
            std::fs::File::open(path)?,
        ))
        .map_err(|_| {
            ErrorKind::InputError(
                "a download of this pack isn't a zip file, so the pack may need to be downloaded from its own website"
                    .to_string(),
            )
        })?;
        for index in 0..archive.len() {
            let entry = archive.by_index_raw(index)?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name()?.replace('\\', "/");
            let relative = name.trim_start_matches("./").to_string();
            if relative.is_empty() || is_clutter(&relative) {
                continue;
            }
            let lower = relative.to_lowercase();
            if lower.starts_with("bin/") {
                collected.bin.insert(lower, (source, index));
                continue;
            }
            if SafeRelativeUtf8UnixPathBuf::try_from(relative.clone()).is_err()
            {
                bail!(ErrorKind::InputError(format!(
                    "this pack contains an unsafe file path: {relative}"
                )));
            }
            if lower.starts_with("mods/")
                && (lower.ends_with(".jar") || lower.ends_with(".zip"))
            {
                collected.has_mods = true;
            }
            let item = Entry {
                source,
                index,
                relative,
                size: entry.size(),
            };
            match positions.get(&lower) {
                Some(&position) => collected.entries[position] = item,
                None => {
                    positions.insert(lower, collected.entries.len());
                    collected.entries.push(item);
                }
            }
        }
    }
    Ok(collected)
}

/// The SHA-1 of every entry, in the same order, so an update can tell what someone
/// changed since the pack put it there.
fn hash_entries(
    paths: &[PathBuf],
    entries: &[Entry],
) -> crate::Result<Vec<String>> {
    let mut archives = paths
        .iter()
        .map(|path| {
            Ok(zip::ZipArchive::new(std::io::BufReader::new(
                std::fs::File::open(path)?,
            ))?)
        })
        .collect::<crate::Result<Vec<_>>>()?;
    entries
        .iter()
        .map(|entry| {
            Ok(sha1_hex(archives[entry.source].by_index(entry.index)?)?)
        })
        .collect()
}

/// What updating leaves alone and cleans up. Files someone changed stay, worlds the
/// instance already has are never touched, and a file the last version put there is
/// only removed when this version dropped it and it's still exactly what the pack
/// installed. Jars in content folders belong to the pack, which replaces them.
fn plan_update(
    entries: &[Entry],
    hashes: &[String],
    installed: &InstalledPack,
    instance_dir: &Path,
) -> UpdatePlan {
    let overrides: Vec<OverrideEntry> = entries
        .iter()
        .zip(hashes)
        .enumerate()
        .map(|(index, (entry, sha1))| OverrideEntry {
            index,
            relative: entry.relative.clone(),
            sha1: sha1.clone(),
        })
        .collect();
    let previous = Previous {
        overrides: installed.files.clone(),
        instance_dir: instance_dir.to_path_buf(),
        keep_disabled: Vec::new(),
    };
    let (skipped, kept) = overrides_to_keep(&overrides, &previous);
    let current: HashSet<String> = entries
        .iter()
        .map(|entry| entry.relative.to_lowercase())
        .collect();
    let managed = crate::state::content_store::is_managed_content_path;
    let stale = installed
        .files
        .iter()
        .filter(|(path, sha1)| {
            !current.contains(&path.to_lowercase())
                && !managed(path)
                && !path.starts_with("saves/")
                && file_sha1(&instance_dir.join(path)).as_deref()
                    == Some(sha1.as_str())
        })
        .map(|(path, _)| path.clone())
        .collect();
    let keep_disabled = installed
        .files
        .keys()
        .filter(|path| {
            managed(path)
                && current.contains(&path.to_lowercase())
                && !instance_dir.join(path).exists()
                && instance_dir.join(format!("{path}.disabled")).exists()
        })
        .cloned()
        .collect();
    UpdatePlan {
        skipped,
        kept,
        stale,
        keep_disabled,
    }
}

fn read_entry(
    paths: &[PathBuf],
    (source, index): (usize, usize),
) -> crate::Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::io::BufReader::new(
        std::fs::File::open(&paths[source])?,
    ))?;
    let entry = archive.by_index(index)?;
    if entry.size() > MAX_PROFILE_BYTES {
        bail!(ErrorKind::InputError(
            "this pack's loader file is too large to read".to_string()
        ));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.take(MAX_PROFILE_BYTES).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn profile_from_jar(jar: &[u8]) -> crate::Result<Option<VersionProfile>> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(jar)).map_err(|_| {
            ErrorKind::InputError(
                "this pack's bin/modpack.jar can't be read".to_string(),
            )
        })?;
    let mut text = String::new();
    if let Ok(entry) = archive.by_name("version.json") {
        entry.take(MAX_PROFILE_BYTES).read_to_string(&mut text)?;
        return Ok(
            serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
        );
    }
    if let Ok(entry) = archive.by_name("install_profile.json") {
        entry.take(MAX_PROFILE_BYTES).read_to_string(&mut text)?;
        let profile: Option<InstallProfile> =
            serde_json::from_str(text.trim_start_matches('\u{feff}')).ok();
        return Ok(profile.and_then(|profile| profile.version_info));
    }
    Ok(None)
}

/// The launch profile in the pack's `bin/` folder. `Ok(None)` means the pack has none.
fn read_profile(
    paths: &[PathBuf],
    collected: &Collected,
) -> crate::Result<Option<VersionProfile>> {
    if let Some(&location) = collected.bin.get("bin/version.json") {
        let bytes = read_entry(paths, location)?;
        let text = String::from_utf8_lossy(&bytes);
        return Ok(
            serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
        );
    }
    if let Some(&location) = collected.bin.get("bin/modpack.jar") {
        let jar = read_entry(paths, location)?;
        return match profile_from_jar(&jar)? {
            Some(profile) => Ok(Some(profile)),
            None => bail!(ErrorKind::InputError(
                "this pack changes Minecraft's own files instead of using a mod loader, which Pawrinth can't install"
                    .to_string()
            )),
        };
    }
    Ok(None)
}

/// Turns `1.12.2-14.23.5.2860` or `1.7.10-10.13.4.1614-1.7.10` into the loader's
/// own version.
fn loader_version(raw: &str, game_version: &str) -> Option<String> {
    let mut version = raw.trim();
    if !game_version.is_empty() {
        version = version
            .strip_prefix(&format!("{game_version}-"))
            .unwrap_or(version);
        version = version
            .strip_suffix(&format!("-{game_version}"))
            .unwrap_or(version);
    }
    (!version.is_empty()).then(|| version.to_string())
}

const LOADER_LIBRARIES: [(&str, &str, ModLoader); 6] = [
    ("org.quiltmc", "quilt-loader", ModLoader::Quilt),
    ("net.neoforged", "neoforge", ModLoader::NeoForge),
    ("net.neoforged", "forge", ModLoader::NeoForge),
    ("net.fabricmc", "fabric-loader", ModLoader::Fabric),
    ("net.minecraftforge", "forge", ModLoader::Forge),
    ("net.minecraftforge", "minecraftforge", ModLoader::Forge),
];

fn loader_from_profile(
    profile: &VersionProfile,
    game_version: &str,
) -> Option<(ModLoader, String)> {
    for (group, artifact, loader) in LOADER_LIBRARIES {
        for library in &profile.libraries {
            let mut parts = library.name.split(':');
            if parts.next() == Some(group)
                && parts.next() == Some(artifact)
                && let Some(version) = parts
                    .next()
                    .and_then(|raw| loader_version(raw, game_version))
            {
                return Some((loader, version));
            }
        }
    }
    let id = profile.id.as_deref()?.trim();
    let lower = id.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("fabric-loader-") {
        return loader_version(rest, game_version)
            .map(|version| (ModLoader::Fabric, version));
    }
    if let Some(position) = lower.find("neoforge") {
        let rest = id[position + "neoforge".len()..].trim_start_matches('-');
        return loader_version(rest, game_version)
            .map(|version| (ModLoader::NeoForge, version));
    }
    if let Some(position) = lower.find("forge") {
        let rest = id[position + "forge".len()..].trim_start_matches('-');
        return loader_version(rest, game_version)
            .map(|version| (ModLoader::Forge, version));
    }
    None
}

fn packs_dir(state: &State) -> PathBuf {
    state.directories.caches_dir().join(PACKS_FOLDER)
}

fn valid_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 96
        && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn token_for(slug: &str, version: &str) -> String {
    let clean = |text: &str, max: usize| -> String {
        text.chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .take(max)
            .collect()
    };
    format!("technic-{}-{}", clean(slug, 48), clean(version, 32))
}

async fn remove_old_packs(state: &State) {
    let Ok(mut entries) = tokio::fs::read_dir(packs_dir(state)).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let old = entry
            .metadata()
            .await
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > KEEP_CONVERTED_PACKS);
        if old {
            let _ = tokio::fs::remove_dir_all(entry.path()).await;
        }
    }
}

async fn pack_icon(url: Option<&str>, state: &State) -> Option<bytes::Bytes> {
    let url = url.filter(|url| transport(url) == Transport::Secure)?;
    crate::util::fetch::fetch(
        url,
        None,
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await
    .ok()
}

fn write_pack(
    paths: &[PathBuf],
    entries: &[Entry],
    index: &PackFormat,
    record: &PackRecord,
    icon: Option<&[u8]>,
    destination_dir: &Path,
) -> crate::Result<PathBuf> {
    std::fs::create_dir_all(destination_dir)?;
    let mut archives = paths
        .iter()
        .map(|path| {
            Ok(zip::ZipArchive::new(std::io::BufReader::new(
                std::fs::File::open(path)?,
            ))?)
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let temporary = tempfile::NamedTempFile::new_in(destination_dir)?;
    let mut writer = zip::ZipWriter::new(std::io::BufWriter::new(
        temporary.as_file().try_clone()?,
    ));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer.start_file(INDEX_NAME, options)?;
    writer.write_all(&serde_json::to_vec_pretty(index)?)?;
    writer.start_file(RECORD_NAME, options)?;
    writer.write_all(&serde_json::to_vec_pretty(record)?)?;
    if let Some(icon) = icon {
        writer.start_file(ICON_NAME, options)?;
        writer.write_all(icon)?;
    }
    for entry in entries {
        writer.raw_copy_file_rename(
            archives[entry.source].by_index_raw(entry.index)?,
            format!("{PACK_OVERRIDES}/{}", entry.relative),
        )?;
    }
    writer.finish()?.flush()?;
    let destination = destination_dir.join(&record.file_name);
    crate::util::io::persist_replacing(
        temporary.into_temp_path(),
        &destination,
    )?;
    Ok(destination)
}

fn installed_path(instance_id: &str, state: &State) -> PathBuf {
    state
        .directories
        .config_dir
        .join(INSTALLED_FOLDER)
        .join(format!(
            "{}.json",
            super::world_backups::instance_folder(instance_id)
        ))
}

async fn read_installed(path: &Path) -> Option<InstalledPack> {
    serde_json::from_slice(&tokio::fs::read(path).await.ok()?).ok()
}

async fn previous_install(
    instance_id: &str,
    slug: &str,
    state: &State,
) -> crate::Result<(InstalledPack, PathBuf)> {
    let Some(installed) =
        read_installed(&installed_path(instance_id, state)).await
    else {
        bail!(ErrorKind::InputError(
            "this instance wasn't installed from a Technic modpack Pawrinth can update"
                .to_string()
        ));
    };
    if installed.instance_id != instance_id || installed.slug != slug {
        bail!(ErrorKind::InputError(
            "this instance was installed from a different modpack".to_string()
        ));
    }
    if crate::state::instance_has_running_process(instance_id, state).await? {
        bail!(ErrorKind::InputError(
            "stop this instance before updating it".to_string()
        ));
    }
    Ok((
        installed,
        crate::api::instance::get_full_path(instance_id).await?,
    ))
}

/// Downloads a Technic pack and gets it ready to install. Packs that download over
/// plain HTTP need `allow_insecure`. With an instance, it's prepared as an update of
/// the pack that instance already has.
pub async fn prepare(
    slug: String,
    allow_insecure: bool,
    instance_id: Option<String>,
) -> crate::Result<PreparedPack> {
    let state = State::get().await?;
    remove_old_packs(&state).await;
    let pack = fresh_details(&slug, &state).await?;
    let name = non_empty(Some(&pack.display_name))
        .unwrap_or_else(|| pack.name.clone());
    let previous = match &instance_id {
        Some(instance_id) => {
            Some(previous_install(instance_id, &pack.name, &state).await?)
        }
        None => None,
    };
    let plan = match (
        non_empty(pack.solder.as_deref()),
        non_empty(pack.url.as_deref()),
    ) {
        (Some(solder), _) => {
            solder_plan(&pack, &name, &solder, allow_insecure, &state).await?
        }
        (None, Some(url)) => zip_plan(&pack, &name, &url, allow_insecure)?,
        (None, None) => bail!(ErrorKind::InputError(format!(
            "{name} has nothing to download on Technic"
        ))),
    };

    let downloads = download_sources(&plan.sources, &name, &state).await?;
    let unverified: Vec<String> = plan
        .sources
        .iter()
        .zip(&downloads)
        .filter(|(_, (_, verified))| !verified)
        .map(|(source, _)| source.label.clone())
        .collect();
    let paths: Vec<PathBuf> = downloads
        .iter()
        .map(|(download, _)| download.path().to_path_buf())
        .collect();
    let read_paths = paths.clone();
    let (collected, profile, hashes, update) =
        tokio::task::spawn_blocking(move || {
            let collected = collect_entries(&read_paths)?;
            let profile = read_profile(&read_paths, &collected)?;
            let hashes = hash_entries(&read_paths, &collected.entries)?;
            let update = previous.map(|(installed, instance_dir)| {
                plan_update(
                    &collected.entries,
                    &hashes,
                    &installed,
                    &instance_dir,
                )
            });
            crate::Result::Ok((collected, profile, hashes, update))
        })
        .await??;

    let Some(game_version) = plan.game_version.clone().or_else(|| {
        profile
            .as_ref()
            .and_then(|profile| non_empty(profile.inherits_from.as_deref()))
    }) else {
        bail!(ErrorKind::InputError(format!(
            "{name} doesn't say which Minecraft version it needs"
        )));
    };
    let (loader, loader_version) = match &profile {
        Some(profile) => match loader_from_profile(profile, &game_version) {
            Some((loader, version)) => (loader, Some(version)),
            None => bail!(ErrorKind::InputError(format!(
                "Pawrinth couldn't tell which mod loader {name} uses"
            ))),
        },
        None if collected.has_mods => bail!(ErrorKind::InputError(format!(
            "{name} has mods but doesn't include its mod loader"
        ))),
        None => (ModLoader::Vanilla, None),
    };

    let icon = pack_icon(
        secure_image(pack.icon.as_ref())
            .or_else(|| secure_image(pack.logo.as_ref()))
            .as_deref(),
        &state,
    )
    .await;
    let token = match &instance_id {
        Some(instance_id) => format!(
            "{}-{}",
            token_for(&pack.name, &plan.version)
                .chars()
                .take(54)
                .collect::<String>(),
            instance_id
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .take(40)
                .collect::<String>()
        ),
        None => token_for(&pack.name, &plan.version),
    };
    let (skipped, kept, stale, keep_disabled) = match update {
        Some(update) => (
            update.skipped,
            update.kept,
            update.stale,
            update.keep_disabled,
        ),
        None => Default::default(),
    };
    let record = PackRecord {
        slug: pack.name.clone(),
        name: name.clone(),
        version: plan.version.clone(),
        file_name: format!("{}.mrpack", display_name(&name)),
        memory_mb: plan.memory_mb,
        for_instance: instance_id.clone(),
        files: collected
            .entries
            .iter()
            .zip(&hashes)
            .map(|(entry, sha1)| (entry.relative.clone(), sha1.clone()))
            .collect(),
        stale,
        keep_disabled,
        allowed_insecure: allow_insecure && is_insecure(&pack),
    };
    let written_entries: Vec<Entry> = collected
        .entries
        .iter()
        .enumerate()
        .filter(|(index, _)| !skipped.contains(index))
        .map(|(_, entry)| entry.clone())
        .collect();
    let index = PackFormat {
        game: "minecraft".to_string(),
        format_version: 1,
        version_id: plan.version.clone(),
        name: name.clone(),
        summary: non_empty(pack.description.as_deref())
            .map(|text| shortened(&text, MAX_SUMMARY_CHARS)),
        files: Vec::new(),
        dependencies: pack_dependencies(
            &game_version,
            loader,
            loader_version.as_deref(),
        ),
    };
    let file_count = collected.entries.len();
    let total_bytes = collected
        .entries
        .iter()
        .map(|entry| entry.size)
        .sum::<u64>();
    let destination_dir = packs_dir(&state).join(&token);
    let written_record = record.clone();
    tokio::task::spawn_blocking(move || {
        write_pack(
            &paths,
            &written_entries,
            &index,
            &written_record,
            icon.as_deref(),
            &destination_dir,
        )
    })
    .await??;
    drop(downloads);

    Ok(PreparedPack {
        token,
        slug: pack.name,
        name,
        version: plan.version,
        game_version,
        loader,
        loader_version,
        file_count,
        total_bytes,
        unverified,
        recommended_memory_mb: plan.memory_mb,
        kept,
    })
}

async fn prepared_pack_path(
    token: &str,
    state: &State,
) -> crate::Result<PathBuf> {
    let gone = || {
        ErrorKind::InputError(
            "this modpack isn't ready to install any more, so start again"
                .to_string(),
        )
    };
    if !valid_token(token) {
        bail!(ErrorKind::InputError("unknown modpack".to_string()));
    }
    let mut entries = tokio::fs::read_dir(packs_dir(state).join(token))
        .await
        .map_err(|_| gone())?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "mrpack")
        {
            return Ok(path);
        }
    }
    Err(gone().into())
}

async fn read_record(path: &Path) -> crate::Result<PackRecord> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || -> crate::Result<PackRecord> {
        let mut archive = zip::ZipArchive::new(std::io::BufReader::new(
            std::fs::File::open(&path)?,
        ))?;
        let mut entry = archive.by_name(RECORD_NAME)?;
        let mut text = String::new();
        entry.read_to_string(&mut text)?;
        Ok(serde_json::from_str(&text)?)
    })
    .await?
}

/// Starts installing a prepared pack, as a new instance or as an update of the
/// instance it was prepared for. Progress shows in the download manager.
pub async fn install(
    token: String,
    instance_id: Option<String>,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let path = prepared_pack_path(&token, &state).await?;
    let record = read_record(&path).await?;
    if record.for_instance != instance_id {
        bail!(ErrorKind::InputError(
            "this modpack was prepared for a different instance".to_string()
        ));
    }
    let link = InstanceLink::ImportedModpack {
        project_id: None,
        version_id: None,
        name: Some(record.name.clone()),
        version_number: Some(record.version.clone()),
        filename: Some(record.file_name.clone()),
    };
    let location = CreatePackLocation::FromFile { path };
    match instance_id {
        None => {
            crate::install::create_modpack_instance(
                location,
                Some(InstallPostInstallEdit {
                    name: Some(record.name),
                    icon_path: None,
                    link: Some(link),
                }),
            )
            .await
        }
        Some(instance_id) => {
            crate::install::install_pack_to_existing_instance(
                instance_id,
                location,
                Some(InstallPostInstallEdit {
                    name: None,
                    icon_path: None,
                    link: Some(link),
                }),
            )
            .await
        }
    }
}

/// After a pack is installed or updated: removes what the new version dropped, turns
/// back off what someone had turned off, and remembers what the pack installed.
async fn settle(
    instance_id: &str,
    record: PackRecord,
    state: &State,
) -> crate::Result<()> {
    let instance_dir = crate::api::instance::get_full_path(instance_id).await?;
    for path in &record.stale {
        if SafeRelativeUtf8UnixPathBuf::try_from(path.clone()).is_ok() {
            let _ = tokio::fs::remove_file(instance_dir.join(path)).await;
        }
    }
    for path in &record.keep_disabled {
        if instance_dir.join(path).exists() {
            crate::state::instances::commands::toggle_disable_project(
                instance_id,
                path,
                Some(false),
                state,
            )
            .await?;
        }
    }
    let installed = InstalledPack {
        instance_id: instance_id.to_string(),
        slug: record.slug,
        name: record.name,
        version: record.version,
        files: record.files,
        allowed_insecure: record.allowed_insecure,
    };
    let path = installed_path(instance_id, state);
    if let Some(folder) = path.parent() {
        tokio::fs::create_dir_all(folder).await?;
    }
    tokio::fs::write(&path, serde_json::to_vec(&installed)?).await?;
    Ok(())
}

/// Every instance installed from a Technic pack that can be updated. What's
/// remembered about instances that have since been deleted is forgotten here.
pub async fn installed_packs() -> crate::Result<Vec<InstalledInfo>> {
    let state = State::get().await?;
    let folder = state.directories.config_dir.join(INSTALLED_FOLDER);
    let mut packs = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(&folder).await else {
        return Ok(packs);
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Some(installed) = read_installed(&entry.path()).await else {
            continue;
        };
        if crate::state::get_instance(&installed.instance_id, &state.pool)
            .await?
            .is_none()
        {
            let _ = tokio::fs::remove_file(entry.path()).await;
            continue;
        }
        packs.push(InstalledInfo {
            instance_id: installed.instance_id,
            slug: installed.slug,
            name: installed.name,
            version: installed.version,
        });
    }
    Ok(packs)
}

/// The version Technic would install now for the pack an instance has, or `None` when
/// it can't be found out. A pack on a plain HTTP server is only asked when someone
/// agreed to that server when installing it.
pub(super) async fn latest_version(instance_id: String) -> Option<String> {
    let state = State::get().await.ok()?;
    let installed =
        read_installed(&installed_path(&instance_id, &state)).await?;
    let pack = match remembered(&installed.slug) {
        Some(pack) => pack,
        None => fresh_details(&installed.slug, &state).await.ok()?,
    };
    match non_empty(pack.solder.as_deref()) {
        Some(solder) => solder_build(
            &pack,
            &installed.name,
            &solder,
            installed.allowed_insecure,
            &state,
        )
        .await
        .ok()
        .map(|(_, build)| build),
        None => non_empty(pack.version.as_deref()),
    }
}

/// The folder Pawrinth converted a pack into, when `path` is a pack in one. The path
/// comes from the webview, so `..` anywhere means it isn't, since `packs/..` would
/// otherwise pass as a folder inside the packs folder.
fn converted_pack_folder<'a>(path: &'a Path, packs: &Path) -> Option<&'a Path> {
    if path.components().any(|part| {
        !matches!(
            part,
            Component::Normal(_) | Component::Prefix(_) | Component::RootDir
        )
    }) {
        return None;
    }
    path.parent()
        .filter(|folder| folder.parent() == Some(packs))
}

/// Runs at the end of an install job. A converted Technic pack holds the whole modpack,
/// so it's removed once it's been installed. Other packs are left alone.
pub(crate) async fn finish_install(
    instance_id: &str,
    location: &CreatePackLocation,
    state: &State,
) {
    let CreatePackLocation::FromFile { path } = location else {
        return;
    };
    let packs = packs_dir(state);
    let Some(folder) = converted_pack_folder(path, &packs) else {
        return;
    };
    if let Ok(record) = read_record(path).await {
        if let Some(memory_mb) = record.memory_mb
            && let Err(error) = super::memory::recommend(
                instance_id,
                memory_mb,
                super::settings::MemorySource::Technic,
            )
            .await
        {
            tracing::warn!(
                "Couldn't apply the pack's recommended memory: {error}"
            );
        }
        if let Err(error) = settle(instance_id, record, state).await {
            tracing::warn!(
                "Couldn't remember what this Technic pack installed: {error}"
            );
        }
    }
    super::pack_updates::forget();
    let _ = tokio::fs::remove_dir_all(folder).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha1_of(text: &str) -> String {
        sha1_hex(text.as_bytes()).unwrap()
    }

    #[test]
    fn an_update_keeps_changes_and_removes_only_what_the_pack_dropped() {
        let instance = tempfile::tempdir().unwrap();
        let write = |relative: &str, text: &str| {
            let path = instance.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write("config/untouched.cfg", "v1");
        write("config/changed.cfg", "mine");
        write("config/dropped.cfg", "v1");
        write("config/dropped-but-changed.cfg", "mine");
        write("mods/off.jar.disabled", "jar v1");
        write("saves/World/level.dat", "my world");
        let installed = InstalledPack {
            instance_id: "local:a".to_string(),
            slug: "pack".to_string(),
            name: "Pack".to_string(),
            version: "1".to_string(),
            files: [
                ("config/untouched.cfg", "v1"),
                ("config/changed.cfg", "v1"),
                ("config/dropped.cfg", "v1"),
                ("config/dropped-but-changed.cfg", "v1"),
                ("mods/off.jar", "jar v1"),
                ("mods/gone.jar", "jar v1"),
            ]
            .into_iter()
            .map(|(path, text)| (path.to_string(), sha1_of(text)))
            .collect(),
            allowed_insecure: false,
        };
        let new_files = [
            ("config/untouched.cfg", "v2"),
            ("config/changed.cfg", "v2"),
            ("config/new.cfg", "v2"),
            ("mods/off.jar", "jar v2"),
            ("saves/World/level.dat", "pack world"),
        ];
        let entries: Vec<Entry> = new_files
            .iter()
            .enumerate()
            .map(|(index, (path, text))| Entry {
                source: 0,
                index,
                relative: path.to_string(),
                size: text.len() as u64,
            })
            .collect();
        let hashes: Vec<String> =
            new_files.iter().map(|(_, text)| sha1_of(text)).collect();

        let plan = plan_update(&entries, &hashes, &installed, instance.path());

        assert_eq!(plan.skipped, HashSet::from([1, 4]));
        assert_eq!(plan.kept, ["config/changed.cfg", "saves/World"]);
        assert_eq!(plan.stale, ["config/dropped.cfg"]);
        assert_eq!(plan.keep_disabled, ["mods/off.jar"]);
    }

    fn hit(id: Option<&str>, name: &str, slug: &str) -> SearchHit {
        SearchHit {
            id: id.map(str::to_string),
            name: name.to_string(),
            slug: slug.to_string(),
            icon_url: None,
        }
    }

    #[test]
    fn gathered_lists_keep_each_pack_once_in_order() {
        let trending = vec![hit(Some("5"), "Tekkit", "tekkit")];
        let search = vec![
            hit(Some("5"), "Tekkit", "tekkit"),
            hit(Some("9"), "Attack", "attack"),
            hit(None, "Bad", "../bad"),
        ];
        let slugs: Vec<_> = merged([trending, search])
            .into_iter()
            .map(|hit| hit.slug)
            .collect();
        assert_eq!(slugs, ["tekkit", "attack"]);
    }

    #[test]
    fn packs_sort_by_newest_or_name_or_keep_technics_order() {
        let packs = vec![
            hit(Some("5"), "tekkit", "tekkit"),
            hit(None, "Zed", "zed"),
            hit(Some("9"), "Attack", "attack"),
        ];
        let order = |sort| -> Vec<String> {
            sorted(packs.clone(), sort)
                .into_iter()
                .map(|hit| hit.slug)
                .collect()
        };
        assert_eq!(order(None), ["tekkit", "zed", "attack"]);
        assert_eq!(order(Some("newest")), ["attack", "tekkit", "zed"]);
        assert_eq!(order(Some("name")), ["attack", "tekkit", "zed"]);
    }

    #[test]
    fn only_a_folder_inside_the_packs_folder_is_removed() {
        let packs = std::env::temp_dir().join("caches").join("technic-packs");
        let pack = packs.join("abc").join("pack.mrpack");
        assert_eq!(
            converted_pack_folder(&pack, &packs),
            Some(packs.join("abc").as_path())
        );
        let escaping = packs.join("..").join("pack.mrpack");
        assert_eq!(converted_pack_folder(&escaping, &packs), None);
        let elsewhere = std::env::temp_dir().join("pack.mrpack");
        assert_eq!(converted_pack_folder(&elsewhere, &packs), None);
    }

    fn profile(id: Option<&str>, libraries: &[&str]) -> VersionProfile {
        VersionProfile {
            id: id.map(str::to_string),
            inherits_from: None,
            libraries: libraries
                .iter()
                .map(|name| Library {
                    name: name.to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn forge_versions_match_what_pawrinth_installs() {
        assert_eq!(
            loader_from_profile(
                &profile(
                    Some("1.12.2-forge-14.23.5.2860"),
                    &["net.minecraftforge:forge:1.12.2-14.23.5.2860"]
                ),
                "1.12.2"
            ),
            Some((ModLoader::Forge, "14.23.5.2860".to_string()))
        );
        assert_eq!(
            loader_from_profile(
                &profile(
                    Some("1.7.10-Forge10.13.4.1614-1.7.10"),
                    &[
                        "net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10",
                        "net.minecraft:launchwrapper:1.12"
                    ]
                ),
                "1.7.10"
            ),
            Some((ModLoader::Forge, "10.13.4.1614".to_string()))
        );
        assert_eq!(
            loader_from_profile(
                &profile(
                    None,
                    &["net.minecraftforge:minecraftforge:9.11.1.965"]
                ),
                "1.6.4"
            ),
            Some((ModLoader::Forge, "9.11.1.965".to_string()))
        );
    }

    #[test]
    fn other_loaders_are_recognised() {
        assert_eq!(
            loader_from_profile(
                &profile(
                    Some("fabric-loader-0.16.5-1.21.1"),
                    &[
                        "net.fabricmc:intermediary:1.21.1",
                        "net.fabricmc:fabric-loader:0.16.5"
                    ]
                ),
                "1.21.1"
            ),
            Some((ModLoader::Fabric, "0.16.5".to_string()))
        );
        assert_eq!(
            loader_from_profile(
                &profile(None, &["net.neoforged:neoforge:21.1.77"]),
                "1.21.1"
            ),
            Some((ModLoader::NeoForge, "21.1.77".to_string()))
        );
        assert_eq!(
            loader_from_profile(
                &profile(None, &["net.neoforged:forge:1.20.1-47.1.106"]),
                "1.20.1"
            ),
            Some((ModLoader::NeoForge, "47.1.106".to_string()))
        );
        assert_eq!(
            loader_from_profile(
                &profile(
                    None,
                    &[
                        "net.fabricmc:intermediary:1.20.1",
                        "org.quiltmc:quilt-loader:0.26.4"
                    ]
                ),
                "1.20.1"
            ),
            Some((ModLoader::Quilt, "0.26.4".to_string()))
        );
    }

    #[test]
    fn the_profile_name_is_a_fallback() {
        assert_eq!(
            loader_from_profile(
                &profile(Some("1.19.2-forge-43.2.0"), &[]),
                "1.19.2"
            ),
            Some((ModLoader::Forge, "43.2.0".to_string()))
        );
        assert_eq!(
            loader_from_profile(
                &profile(Some("fabric-loader-0.14.21-1.19.2"), &[]),
                "1.19.2"
            ),
            Some((ModLoader::Fabric, "0.14.21".to_string()))
        );
        assert_eq!(
            loader_from_profile(&profile(Some("1.12.2"), &[]), "1.12.2"),
            None
        );
    }

    #[test]
    fn insecure_downloads_need_consent() {
        assert_eq!(transport("https://example.com/a.zip"), Transport::Secure);
        assert_eq!(transport("http://example.com/a.zip"), Transport::Insecure);
        assert_eq!(transport("ftp://example.com/a.zip"), Transport::Refused);
        assert_eq!(transport("not a url"), Transport::Refused);
        assert!(
            check_transport("https://example.com/a.zip", false, "A").is_ok()
        );
        assert!(
            check_transport("http://example.com/a.zip", false, "A").is_err()
        );
        assert!(check_transport("http://example.com/a.zip", true, "A").is_ok());
        assert!(check_transport("file:///C:/a.zip", true, "A").is_err());
    }

    #[test]
    fn checksum_mismatches_are_only_tolerated_over_https() {
        assert!(mismatch_is_tolerable(
            "https://solder.example.com/mods/a/a-1.0.zip"
        ));
        assert!(!mismatch_is_tolerable(
            "http://solder.example.com/mods/a/a-1.0.zip"
        ));
    }

    #[test]
    fn a_pack_is_insecure_when_either_address_is() {
        let mut pack = ApiPack {
            url: Some("https://example.com/pack.zip".to_string()),
            ..ApiPack::default()
        };
        assert!(!is_insecure(&pack));
        pack.solder = Some("http://solder.example.com/api/".to_string());
        assert!(is_insecure(&pack));
    }

    #[test]
    fn dropbox_links_download_the_file() {
        assert_eq!(
            direct_download("https://www.dropbox.com/s/abc/Pack.zip?dl=0"),
            "https://www.dropbox.com/s/abc/Pack.zip?dl=1"
        );
        assert_eq!(
            direct_download(
                "https://www.dropbox.com/scl/fi/abc/Pack.zip?rlkey=xyz&dl=0"
            ),
            "https://www.dropbox.com/scl/fi/abc/Pack.zip?rlkey=xyz&dl=1"
        );
        assert_eq!(
            direct_download("https://example.com/Pack.zip?dl=0"),
            "https://example.com/Pack.zip?dl=0"
        );
    }

    #[test]
    fn requests_name_pawrinth_as_the_launcher() {
        assert_eq!(
            api_url("/search", &[("q", "tekkit legends")]),
            "https://api.technicpack.net/search?q=tekkit+legends&build=pawrinth"
        );
        assert_eq!(
            pack_url("shivaxi-rlcraft"),
            "https://api.technicpack.net/modpack/shivaxi-rlcraft?build=pawrinth"
        );
    }

    #[test]
    fn only_plain_slugs_are_looked_up() {
        assert!(valid_slug("the-1122-pack"));
        assert!(valid_slug("tekkit_main.2"));
        assert!(!valid_slug(""));
        assert!(!valid_slug("../modpack"));
        assert!(!valid_slug("a/b"));
        assert!(!valid_slug(&"a".repeat(MAX_SLUG_CHARS + 1)));
    }

    #[test]
    fn tokens_stay_safe() {
        let token = token_for("shivaxi-rlcraft", "2.9.3 Beta/1");
        assert_eq!(token, "technic-shivaxi-rlcraft-2-9-3-Beta-1");
        assert!(valid_token(&token));
        assert!(!valid_token("../x"));
    }

    #[test]
    fn solder_builds_can_recommend_memory() {
        let build: SolderBuild = serde_json::from_str(
            r#"{ "minecraft": "1.12.2", "memory": 5120, "mods": [] }"#,
        )
        .unwrap();
        assert_eq!(build.memory, Some(5120));
        let none: SolderBuild =
            serde_json::from_str(r#"{ "minecraft": "1.12.2", "memory": 0 }"#)
                .unwrap();
        assert_eq!(none.memory.filter(|memory| *memory > 0), None);
    }

    #[test]
    fn numbers_may_arrive_as_text() {
        let pack: ApiPack = serde_json::from_str(
            r#"{ "id": "735902", "name": "tekkit", "installs": 12, "ratings": null }"#,
        )
        .unwrap();
        assert_eq!(pack.id, Some(735902));
        assert_eq!(pack.installs, Some(12));
        assert_eq!(pack.ratings, None);
    }

    #[test]
    fn only_secure_images_reach_the_page() {
        let pack = ApiPack {
            name: "pack".to_string(),
            display_name: "Pack".to_string(),
            icon: Some(Image {
                url: Some("http://example.com/icon.png".to_string()),
            }),
            logo: Some(Image {
                url: Some("https://example.com/logo.png".to_string()),
            }),
            background: Some(Image {
                url: Some("https://example.com/background.jpg".to_string()),
            }),
            platform_url: Some("http://example.com/pack".to_string()),
            ..ApiPack::default()
        };
        let details = details_of("pack", &pack);
        assert_eq!(
            details.icon_url.as_deref(),
            Some("https://example.com/logo.png")
        );
        assert_eq!(
            details.background_url.as_deref(),
            Some("https://example.com/background.jpg")
        );
        assert_eq!(details.page_url, None);
        assert!(!details.downloadable);
    }

    #[test]
    fn long_descriptions_are_shortened_for_lists() {
        assert_eq!(shortened("a  b\n c", 10), "a b c");
        let long = shortened(&"word ".repeat(100), 20);
        assert!(long.ends_with('…'));
        assert!(long.chars().count() <= 21);
    }

    fn zip_with(dir: &Path, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join(name);
        let mut writer =
            zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (file, bytes) in files {
            writer.start_file(*file, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    fn forge_jar() -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file(
                "version.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer
            .write_all(
                br#"{ "id": "1.12.2-forge-14.23.5.2860", "inheritsFrom": "1.12.2",
                "libraries": [ { "name": "net.minecraftforge:forge:1.12.2-14.23.5.2860" } ] }"#,
            )
            .unwrap();
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn later_files_replace_earlier_ones_and_bin_stays_out() {
        let dir = tempfile::tempdir().unwrap();
        let jar = forge_jar();
        let first = zip_with(
            dir.path(),
            "core.zip",
            &[
                ("bin/modpack.jar", &jar),
                ("config/a.cfg", b"old"),
                ("__MACOSX/config/._a.cfg", b"junk"),
            ],
        );
        let second = zip_with(
            dir.path(),
            "mod.zip",
            &[("mods/a.jar", b"jar"), ("CONFIG/A.cfg", b"new")],
        );
        let paths = vec![first, second];
        let collected = collect_entries(&paths).unwrap();
        let relatives: Vec<&str> = collected
            .entries
            .iter()
            .map(|entry| entry.relative.as_str())
            .collect();
        assert_eq!(relatives, ["CONFIG/A.cfg", "mods/a.jar"]);
        assert_eq!(collected.entries[0].source, 1);
        assert!(collected.has_mods);
        assert!(collected.bin.contains_key("bin/modpack.jar"));

        let profile = read_profile(&paths, &collected).unwrap().unwrap();
        assert_eq!(profile.inherits_from.as_deref(), Some("1.12.2"));
        assert_eq!(
            loader_from_profile(&profile, "1.12.2"),
            Some((ModLoader::Forge, "14.23.5.2860".to_string()))
        );
    }

    #[test]
    fn unsafe_paths_stop_the_install() {
        let dir = tempfile::tempdir().unwrap();
        let bad = zip_with(dir.path(), "bad.zip", &[("../evil.txt", b"x")]);
        assert!(collect_entries(&[bad]).is_err());
    }

    #[test]
    fn a_jar_mod_without_a_loader_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let jar = {
            let mut writer =
                zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            writer
                .start_file(
                    "net/minecraft/client/Minecraft.class",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer.write_all(b"class").unwrap();
            writer.finish().unwrap().into_inner()
        };
        let pack =
            zip_with(dir.path(), "old.zip", &[("bin/modpack.jar", &jar)]);
        let paths = vec![pack];
        let collected = collect_entries(&paths).unwrap();
        assert!(read_profile(&paths, &collected).is_err());
    }

    #[test]
    fn a_converted_pack_holds_the_files_under_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let jar = forge_jar();
        let source = zip_with(
            dir.path(),
            "pack.zip",
            &[("bin/modpack.jar", &jar), ("mods/a.jar", b"jar")],
        );
        let paths = vec![source];
        let collected = collect_entries(&paths).unwrap();
        let record = PackRecord {
            slug: "pack".to_string(),
            name: "Pack".to_string(),
            version: "1.0".to_string(),
            file_name: "Pack.mrpack".to_string(),
            memory_mb: Some(5120),
            ..PackRecord::default()
        };
        let index = PackFormat {
            game: "minecraft".to_string(),
            format_version: 1,
            version_id: "1.0".to_string(),
            name: "Pack".to_string(),
            summary: None,
            files: Vec::new(),
            dependencies: pack_dependencies(
                "1.12.2",
                ModLoader::Forge,
                Some("14.23.5.2860"),
            ),
        };
        let out = dir.path().join("out");
        let written = write_pack(
            &paths,
            &collected.entries,
            &index,
            &record,
            Some(b"png"),
            &out,
        )
        .unwrap();
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(&written).unwrap())
                .unwrap();
        let mut names: Vec<String> = archive
            .file_names()
            .map(|name| name.unwrap().to_string())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "icon.png",
                "modrinth.index.json",
                "overrides/mods/a.jar",
                "pawrinth-technic.json"
            ]
        );
        let mut text = String::new();
        archive
            .by_name("modrinth.index.json")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert!(text.contains("\"forge\": \"14.23.5.2860\""));
        assert!(text.contains("\"minecraft\": \"1.12.2\""));
    }
}
