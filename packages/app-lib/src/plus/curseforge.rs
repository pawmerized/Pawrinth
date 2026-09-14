//! CurseForge support through the official REST API. API responses are never
//! stored: the only thing kept is which project and file an installed file came
//! from, which update checks and rollback need.

use super::curseforge_key::{self as key_store, ApiKey};
#[cfg(debug_assertions)]
use super::curseforge_samples as samples;
use super::manifest::{ContentProvider, ManifestEntry};
use super::operations::{self, OperationKind};
use crate::ErrorKind;
use crate::state::instances::ContentSourceKind;
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::instances::commands::{InstallContent, install_stored_file};
use crate::state::{ModLoader, ProjectType, State};
use anyhow::bail;
use futures::StreamExt;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

const API_BASE: &str = "https://api.curseforge.com/v1";
#[cfg(debug_assertions)]
const LOCAL_API_BASE_ENV: &str = "PAWRINTH_CURSEFORGE_API_BASE";
pub(super) const MINECRAFT_GAME_ID: u32 = 432;
const MAX_ATTEMPTS: u32 = 3;
const MAX_DEPENDENCY_DEPTH: usize = 8;
const MAX_CONCURRENT_INSTALLED_LOOKUPS: usize = 4;
const RELATION_REQUIRED: u32 = 3;
const RELATION_OPTIONAL: u32 = 2;
const RELATION_INCOMPATIBLE: u32 = 5;
pub(super) const HASH_SHA1: u32 = 1;
const RELEASE_TYPE_RELEASE: u32 = 1;
const RELEASE_TYPE_BETA: u32 = 2;
/// How many ids one batch request asks for.
const BATCH_SIZE: usize = 500;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Asset {
    pub url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Author {
    pub id: Option<u64>,
    pub name: String,
    pub url: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Category {
    pub id: u32,
    pub name: String,
    pub slug: String,
    pub class_id: Option<u32>,
    pub is_class: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Links {
    pub website_url: Option<String>,
    pub source_url: Option<String>,
    pub wiki_url: Option<String>,
    pub issues_url: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Project {
    pub id: u64,
    pub name: String,
    pub slug: String,
    pub summary: String,
    pub links: Links,
    pub class_id: Option<u32>,
    pub download_count: f64,
    pub thumbs_up_count: u64,
    pub logo: Option<Asset>,
    pub screenshots: Vec<Asset>,
    pub authors: Vec<Author>,
    pub categories: Vec<Category>,
    pub date_created: Option<String>,
    pub date_modified: Option<String>,
    pub date_released: Option<String>,
    /// `None` means the API didn't say, which is treated as not allowed.
    pub allow_mod_distribution: Option<bool>,
    pub is_available: Option<bool>,
    pub latest_files_indexes: Vec<LatestFileIndex>,
    pub latest_files: Vec<LatestFile>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LatestFileIndex {
    pub game_version: String,
    pub mod_loader: Option<u32>,
}

/// Only the tag list is kept; it says which sides a project runs on.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LatestFile {
    pub game_versions: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileDependency {
    pub mod_id: u64,
    pub relation_type: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileHash {
    pub value: String,
    pub algo: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectFile {
    pub id: u64,
    pub mod_id: u64,
    pub display_name: String,
    pub file_name: String,
    pub release_type: u32,
    pub file_date: String,
    pub file_length: u64,
    pub download_count: u64,
    pub download_url: Option<String>,
    pub game_versions: Vec<String>,
    pub dependencies: Vec<FileDependency>,
    pub hashes: Vec<FileHash>,
    pub is_available: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Pagination {
    pub index: u32,
    pub page_size: u32,
    pub result_count: u32,
    pub total_count: u64,
}

#[derive(Deserialize)]
struct DataResponse<T> {
    data: T,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResults {
    pub data: Vec<Project>,
    #[serde(default)]
    pub pagination: Pagination,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchQuery {
    pub class_id: Option<u32>,
    pub search_filter: Option<String>,
    pub game_version: Option<String>,
    pub mod_loader_type: Option<u32>,
    pub category_id: Option<u32>,
    pub author_id: Option<u64>,
    pub sort_field: Option<u32>,
    pub sort_order: Option<String>,
    pub index: u32,
    pub page_size: u32,
}

/// Development builds can point at a local stand-in, to test what otherwise needs an
/// authorized key. Release builds only ever talk to CurseForge.
#[cfg(debug_assertions)]
fn local_api_base() -> Option<String> {
    std::env::var(LOCAL_API_BASE_ENV)
        .ok()
        .filter(|base| base.starts_with("http://127.0.0.1:"))
}

pub(super) fn api_base() -> String {
    #[cfg(debug_assertions)]
    if let Some(base) = local_api_base() {
        return base;
    }
    API_BASE.to_string()
}

/// Release builds only ever use the client that refuses anything but HTTPS.
pub(super) fn http_client() -> &'static reqwest::Client {
    #[cfg(debug_assertions)]
    if local_api_base().is_some() {
        return &crate::util::fetch::INSECURE_REQWEST_CLIENT;
    }
    &crate::util::fetch::REQWEST_CLIENT
}

pub(super) async fn require_key() -> crate::Result<ApiKey> {
    match key_store::resolve().await {
        Some((key, _)) => Ok(key),
        None => Err(ErrorKind::InputError(
            "CurseForge needs an API key for this. Add one in Settings, under Pawrinth and API configuration"
                .to_string(),
        )
        .into()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataMode {
    Live,
    /// Labelled made-up projects, which exist only in development builds.
    #[cfg(debug_assertions)]
    Sample,
    NeedsKey,
    KeyRejected,
}

#[cfg(debug_assertions)]
const MODE_WITHOUT_KEY: DataMode = DataMode::Sample;
#[cfg(not(debug_assertions))]
const MODE_WITHOUT_KEY: DataMode = DataMode::NeedsKey;

/// If CurseForge can't be reached the answer is left open, so a network problem shows
/// as an error rather than as a bad key.
pub async fn data_mode() -> DataMode {
    let Some((key, _)) = key_store::resolve().await else {
        return MODE_WITHOUT_KEY;
    };
    match key_store::validity(&key).await {
        Some(false) => DataMode::KeyRejected,
        _ => DataMode::Live,
    }
}

/// Endpoints the API serves without a key.
async fn get_public<T: DeserializeOwned>(
    path_and_query: &str,
    state: &State,
) -> crate::Result<T> {
    let url = format!("{}{path_and_query}", api_base());
    let _permit = state.api_semaphore.0.acquire().await?;
    let response = http_client()
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|_| {
            ErrorKind::InputError("could not reach CurseForge".to_string())
        })?;
    if !response.status().is_success() {
        bail!(ErrorKind::InputError(format!(
            "CurseForge returned status {}",
            response.status().as_u16()
        )));
    }
    let body: DataResponse<T> =
        serde_json::from_slice(&response.bytes().await?)?;
    Ok(body.data)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameVersion {
    pub version_string: String,
    pub game_version_type_id: Option<u32>,
    pub approved: Option<bool>,
}

/// Minecraft versions from CurseForge's public endpoint, newest first.
pub async fn game_versions() -> crate::Result<Vec<GameVersion>> {
    let state = State::get().await?;
    get_public("/minecraft/version", &state).await
}

/// Only these hosts ever receive the key.
pub(super) fn is_curseforge_host(url: &str) -> bool {
    #[cfg(debug_assertions)]
    if local_api_base().is_some_and(|base| url.starts_with(&base)) {
        return true;
    }
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    parsed.scheme() == "https"
        && parsed.host_str().is_some_and(|host| {
            host == "api.curseforge.com"
                || host == "edge.forgecdn.net"
                || host == "mediafilez.forgecdn.net"
        })
}

async fn send_get(
    url: &str,
    key: &ApiKey,
    state: &State,
) -> crate::Result<bytes::Bytes> {
    send_request(url, key, None, state).await
}

/// Separate from the shared fetch helper, whose tracing span records its header
/// argument. That would write the key into log files. With a body it's a POST.
async fn send_request(
    url: &str,
    key: &ApiKey,
    body: Option<&serde_json::Value>,
    state: &State,
) -> crate::Result<bytes::Bytes> {
    if !is_curseforge_host(url) {
        bail!(ErrorKind::InputError(
            "refusing to send the CurseForge API key to another host"
                .to_string()
        ));
    }
    let _permit = state.api_semaphore.0.acquire().await?;
    let mut last_error = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        let request = match body {
            Some(body) => http_client().post(url).json(body),
            None => http_client().get(url),
        };
        let response = request
            .header("x-api-key", key.header_value()?)
            .header("Accept", "application/json")
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                return Ok(response.bytes().await?);
            }
            Ok(response) if response.status().is_server_error() => {
                last_error =
                    format!("CurseForge returned {}", response.status());
            }
            Ok(response) => {
                let status = response.status().as_u16();
                let from_api = url.starts_with(&api_base());
                if from_api && matches!(status, 401 | 403) {
                    key_store::mark_rejected().await;
                }
                bail!(ErrorKind::InputError(match status {
                    401 | 403 if from_api => {
                        "CurseForge rejected the API key".to_string()
                    }
                    401 | 403 => "CurseForge refused that download".to_string(),
                    404 => "CurseForge couldn't find that".to_string(),
                    other => format!("CurseForge returned status {other}"),
                }));
            }
            Err(_) => {
                // The error's text can include request details, so it isn't kept.
                last_error = "could not reach CurseForge".to_string();
            }
        }
        if attempt < MAX_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(500 * u64::from(attempt)))
                .await;
        }
    }
    bail!(ErrorKind::InputError(last_error))
}

async fn get<T: DeserializeOwned>(
    path_and_query: &str,
    state: &State,
) -> crate::Result<T> {
    let key = require_key().await?;
    let bytes =
        send_get(&format!("{}{path_and_query}", api_base()), &key, state)
            .await?;
    Ok(serde_json::from_slice(&bytes)?)
}

async fn get_data<T: DeserializeOwned>(
    path_and_query: &str,
    state: &State,
) -> crate::Result<T> {
    Ok(get::<DataResponse<T>>(path_and_query, state).await?.data)
}

async fn post_data<T: DeserializeOwned>(
    path: &str,
    body: serde_json::Value,
    state: &State,
) -> crate::Result<T> {
    let key = require_key().await?;
    let bytes = send_request(
        &format!("{}{path}", api_base()),
        &key,
        Some(&body),
        state,
    )
    .await?;
    Ok(serde_json::from_slice::<DataResponse<T>>(&bytes)?.data)
}

/// Files by id, a batch at a time. Ids CurseForge doesn't know are left out.
pub(super) async fn fetch_files_by_id(
    ids: &[u64],
    state: &State,
) -> crate::Result<Vec<ProjectFile>> {
    let mut files = Vec::new();
    for batch in ids.chunks(BATCH_SIZE) {
        files.extend(
            post_data::<Vec<ProjectFile>>(
                "/mods/files",
                serde_json::json!({ "fileIds": batch }),
                state,
            )
            .await?,
        );
    }
    Ok(files)
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct FingerprintMatches {
    exact_matches: Vec<FingerprintMatch>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct FingerprintMatch {
    file: FingerprintFile,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct FingerprintFile {
    id: u64,
    mod_id: u64,
    file_fingerprint: u64,
}

/// The CurseForge project and file for each fingerprint CurseForge recognises,
/// a batch at a time.
pub(super) async fn fetch_fingerprint_matches(
    fingerprints: &[u32],
    state: &State,
) -> crate::Result<std::collections::HashMap<u32, (u64, u64)>> {
    let mut matches = std::collections::HashMap::new();
    for batch in fingerprints.chunks(BATCH_SIZE) {
        let found: FingerprintMatches = post_data(
            &format!("/fingerprints/{MINECRAFT_GAME_ID}"),
            serde_json::json!({ "fingerprints": batch }),
            state,
        )
        .await?;
        for item in found.exact_matches {
            if let Ok(fingerprint) = u32::try_from(item.file.file_fingerprint) {
                matches.insert(fingerprint, (item.file.mod_id, item.file.id));
            }
        }
    }
    Ok(matches)
}

/// Projects by id, a batch at a time. Ids CurseForge doesn't know are left out.
pub(super) async fn fetch_projects_by_id(
    ids: &[u64],
    state: &State,
) -> crate::Result<Vec<Project>> {
    let mut projects = Vec::new();
    for batch in ids.chunks(BATCH_SIZE) {
        projects.extend(
            post_data::<Vec<Project>>(
                "/mods",
                serde_json::json!({ "modIds": batch, "filterPcOnly": true }),
                state,
            )
            .await?,
        );
    }
    Ok(projects)
}

fn search_path(query: &SearchQuery) -> String {
    let mut url = url::form_urlencoded::Serializer::new(String::new());
    url.append_pair("gameId", &MINECRAFT_GAME_ID.to_string());
    if let Some(class_id) = query.class_id {
        url.append_pair("classId", &class_id.to_string());
    }
    if let Some(filter) = query
        .search_filter
        .as_deref()
        .map(str::trim)
        .filter(|filter| !filter.is_empty())
    {
        url.append_pair("searchFilter", filter);
    }
    if let Some(version) = query.game_version.as_deref() {
        url.append_pair("gameVersion", version);
    }
    if let Some(loader) = query.mod_loader_type {
        url.append_pair("modLoaderType", &loader.to_string());
    }
    if let Some(category) = query.category_id {
        url.append_pair("categoryId", &category.to_string());
    }
    if let Some(author) = query.author_id {
        url.append_pair("authorId", &author.to_string());
    }
    if let Some(sort) = query.sort_field {
        url.append_pair("sortField", &sort.to_string());
        url.append_pair(
            "sortOrder",
            query.sort_order.as_deref().unwrap_or("desc"),
        );
    }
    let page_size = query.page_size.clamp(1, 50);
    // The API rejects anything past index + pageSize = 10000.
    let index = query.index.min(10_000 - page_size);
    url.append_pair("index", &index.to_string());
    url.append_pair("pageSize", &page_size.to_string());
    format!("/mods/search?{}", url.finish())
}

pub async fn search(query: SearchQuery) -> crate::Result<SearchResults> {
    match data_mode().await {
        DataMode::Live => {}
        #[cfg(debug_assertions)]
        DataMode::Sample => return Ok(samples::search(&query)),
        DataMode::NeedsKey => {
            require_key().await?;
        }
        DataMode::KeyRejected => bail!(ErrorKind::InputError(
            "CurseForge rejected the API key".to_string()
        )),
    }
    let state = State::get().await?;
    get(&search_path(&query), &state).await
}

const CLASSES_PATH: &str = "/categories?gameId=432&classesOnly=true";

/// Project classes, such as mods and shaders, as the API defines them.
pub async fn classes() -> crate::Result<Vec<Category>> {
    let state = State::get().await?;
    get_public(CLASSES_PATH, &state).await
}

/// Categories for one class. The API only filters by class with a key, so this reads
/// the public full list and filters it here.
pub async fn categories(class_id: u32) -> crate::Result<Vec<Category>> {
    let state = State::get().await?;
    let all: Vec<Category> =
        get_public(&format!("/categories?gameId={MINECRAFT_GAME_ID}"), &state)
            .await?;
    Ok(all
        .into_iter()
        .filter(|category| {
            category.class_id == Some(class_id)
                && category.is_class != Some(true)
        })
        .collect())
}

pub(super) async fn fetch_project(
    id: u64,
    state: &State,
) -> crate::Result<Project> {
    #[cfg(debug_assertions)]
    if samples::is_sample_id(id) {
        return samples::project(id);
    }
    get_data(&format!("/mods/{id}"), state).await
}

pub(super) async fn fetch_file(
    project_id: u64,
    file_id: u64,
    state: &State,
) -> crate::Result<ProjectFile> {
    #[cfg(debug_assertions)]
    if samples::is_sample_id(project_id) {
        return samples::file(project_id, file_id);
    }
    get_data(&format!("/mods/{project_id}/files/{file_id}"), state).await
}

pub(super) async fn fetch_files(
    project_id: u64,
    game_version: Option<&str>,
    loader_type: Option<u32>,
    state: &State,
) -> crate::Result<Vec<ProjectFile>> {
    #[cfg(debug_assertions)]
    if samples::is_sample_id(project_id) {
        return samples::files(project_id);
    }
    get_data(&files_path(project_id, game_version, loader_type), state).await
}

pub async fn project(id: u64) -> crate::Result<Project> {
    let state = State::get().await?;
    fetch_project(id, &state).await
}

/// HTML from CurseForge. The frontend sanitizes it before display.
pub async fn description(id: u64) -> crate::Result<String> {
    #[cfg(debug_assertions)]
    if samples::is_sample_id(id) {
        return Ok(samples::DESCRIPTION.to_string());
    }
    let state = State::get().await?;
    get_data(&format!("/mods/{id}/description"), &state).await
}

pub async fn changelog(project_id: u64, file_id: u64) -> crate::Result<String> {
    #[cfg(debug_assertions)]
    if samples::is_sample_id(project_id) {
        return Ok(samples::CHANGELOG.to_string());
    }
    let state = State::get().await?;
    get_data(
        &format!("/mods/{project_id}/files/{file_id}/changelog"),
        &state,
    )
    .await
}

fn files_path(
    project_id: u64,
    game_version: Option<&str>,
    loader_type: Option<u32>,
) -> String {
    let mut url = url::form_urlencoded::Serializer::new(String::new());
    if let Some(version) = game_version {
        url.append_pair("gameVersion", version);
    }
    if let Some(loader) = loader_type {
        url.append_pair("modLoaderType", &loader.to_string());
    }
    url.append_pair("pageSize", "50");
    format!("/mods/{project_id}/files?{}", url.finish())
}

pub async fn files(
    project_id: u64,
    game_version: Option<String>,
    loader_type: Option<u32>,
) -> crate::Result<Vec<ProjectFile>> {
    let state = State::get().await?;
    fetch_files(project_id, game_version.as_deref(), loader_type, &state).await
}

fn loader_type(loader: ModLoader) -> Option<u32> {
    match loader {
        ModLoader::Forge => Some(1),
        ModLoader::Fabric => Some(4),
        ModLoader::Quilt => Some(5),
        ModLoader::NeoForge => Some(6),
        _ => None,
    }
}

fn loader_tag(loader: ModLoader) -> Option<&'static str> {
    match loader {
        ModLoader::Forge => Some("Forge"),
        ModLoader::Fabric => Some("Fabric"),
        ModLoader::Quilt => Some("Quilt"),
        ModLoader::NeoForge => Some("NeoForge"),
        _ => None,
    }
}

const KNOWN_LOADER_TAGS: [&str; 6] = [
    "Forge",
    "Fabric",
    "Quilt",
    "NeoForge",
    "Cauldron",
    "LiteLoader",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status", content = "reason")]
pub enum Compatibility {
    Compatible,
    /// Some metadata is missing, so this can't be confirmed either way.
    Unknown(String),
    Incompatible(String),
}

/// Files list game versions and loaders together in `gameVersions`.
pub fn evaluate_compatibility(
    file: &ProjectFile,
    project_type: ProjectType,
    game_version: &str,
    loader: ModLoader,
) -> Compatibility {
    let looks_like_version = |value: &&String| {
        value.chars().next().is_some_and(|c| c.is_ascii_digit())
    };
    let versions: Vec<&String> = file
        .game_versions
        .iter()
        .filter(looks_like_version)
        .collect();
    if !versions.is_empty()
        && !versions.iter().any(|version| *version == game_version)
    {
        return Compatibility::Incompatible(format!(
            "this file isn't marked for Minecraft {game_version}"
        ));
    }

    let mut unknown = versions
        .is_empty()
        .then(|| "this file lists no Minecraft versions".to_string());

    if project_type == ProjectType::Mod {
        let listed: Vec<&String> = file
            .game_versions
            .iter()
            .filter(|value| KNOWN_LOADER_TAGS.contains(&value.as_str()))
            .collect();
        match loader_tag(loader) {
            None => {
                return Compatibility::Incompatible(
                    "this instance has no mod loader".to_string(),
                );
            }
            Some(_) if listed.is_empty() => {
                unknown = Some("this file lists no mod loaders".to_string());
            }
            Some(tag) if !listed.iter().any(|value| *value == tag) => {
                return Compatibility::Incompatible(format!(
                    "this file isn't marked for {tag}"
                ));
            }
            Some(_) => {}
        }
    }
    unknown.map_or(Compatibility::Compatible, Compatibility::Unknown)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyStatus {
    WillInstall,
    AlreadyInstalled,
    /// The author doesn't allow third-party downloads, or no compatible file exists.
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlannedDependency {
    pub project_id: u64,
    pub name: String,
    pub website_url: Option<String>,
    pub status: DependencyStatus,
    pub file: Option<ProjectFile>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallPlan {
    pub project: Project,
    pub file: ProjectFile,
    pub compatibility: Compatibility,
    pub can_download: bool,
    pub required: Vec<PlannedDependency>,
    pub optional: Vec<PlannedDependency>,
    /// Installed CurseForge projects this file declares itself incompatible with.
    pub conflicts: Vec<String>,
}

pub(super) fn can_download(project: &Project, file: &ProjectFile) -> bool {
    project.allow_mod_distribution == Some(true)
        && file.is_available != Some(false)
        && file.download_url.is_some()
}

fn newest_file<'a>(
    files: &'a [ProjectFile],
    project_type: ProjectType,
    game_version: &str,
    loader: ModLoader,
) -> Option<&'a ProjectFile> {
    let compatible = files.iter().filter(|file| {
        !matches!(
            evaluate_compatibility(file, project_type, game_version, loader),
            Compatibility::Incompatible(_)
        ) && file.is_available != Some(false)
    });
    let mut candidates: Vec<&ProjectFile> = compatible.collect();
    candidates.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    candidates
        .iter()
        .find(|file| file.release_type == 1)
        .or(candidates.first())
        .copied()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileOption {
    pub file: ProjectFile,
    pub compatibility: Compatibility,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallOptions {
    pub files: Vec<FileOption>,
    /// `None` means nothing available fits this instance.
    pub recommended_file_id: Option<u64>,
}

/// What gets installed unless someone chooses otherwise: never an incompatible file,
/// a file known to fit before one that can't be confirmed, and a stable release before
/// a beta or alpha, however new those are. The newest wins among equals.
pub(super) fn recommended_file(options: &[FileOption]) -> Option<&ProjectFile> {
    options
        .iter()
        .filter(|option| {
            !matches!(option.compatibility, Compatibility::Incompatible(_))
                && option.file.is_available != Some(false)
        })
        .min_by_key(|option| {
            (
                u8::from(!matches!(
                    option.compatibility,
                    Compatibility::Compatible
                )),
                match option.file.release_type {
                    RELEASE_TYPE_RELEASE => 0,
                    RELEASE_TYPE_BETA => 1,
                    _ => 2,
                },
                std::cmp::Reverse(option.file.file_date.as_str()),
            )
        })
        .map(|option| &option.file)
}

struct TargetInstance {
    game_version: String,
    loader: ModLoader,
    entries: Vec<ManifestEntry>,
}

async fn target_instance(
    instance_id: &str,
    loader_override: Option<&str>,
) -> crate::Result<TargetInstance> {
    let manifest = operations::instance_manifest(instance_id).await?;
    Ok(TargetInstance {
        game_version: manifest.game_version,
        loader: loader_override.map_or(manifest.loader, ModLoader::from_string),
        entries: manifest.entries,
    })
}

pub(super) fn curseforge_ref(project_id: u64) -> String {
    format!("curseforge:{project_id}")
}

fn installed(target: &TargetInstance, project_id: u64) -> bool {
    let wanted = curseforge_ref(project_id);
    target
        .entries
        .iter()
        .any(|entry| entry.project_id.as_deref() == Some(wanted.as_str()))
}

fn project_type_for_class(slug: &str) -> Option<ProjectType> {
    match slug {
        "mc-mods" => Some(ProjectType::Mod),
        "texture-packs" => Some(ProjectType::ResourcePack),
        "shaders" => Some(ProjectType::ShaderPack),
        "data-packs" => Some(ProjectType::DataPack),
        _ => None,
    }
}

/// The project classes Pawrinth can install, by class id, with the class slug of
/// modpacks alongside.
pub(super) async fn class_kinds(
    state: &State,
) -> crate::Result<(std::collections::HashMap<u32, ProjectType>, Option<u32>)> {
    let classes: Vec<Category> = get_public(CLASSES_PATH, state).await?;
    let modpacks = classes
        .iter()
        .find(|class| class.slug == "modpacks")
        .map(|class| class.id);
    Ok((
        classes
            .iter()
            .filter_map(|class| {
                Some((class.id, project_type_for_class(&class.slug)?))
            })
            .collect(),
        modpacks,
    ))
}

async fn project_type_of(
    project: &Project,
    state: &State,
) -> crate::Result<ProjectType> {
    let class_id = project.class_id.ok_or_else(|| {
        ErrorKind::InputError(format!(
            "CurseForge doesn't say what kind of project {} is",
            project.name
        ))
    })?;
    let classes: Vec<Category> = get_public(CLASSES_PATH, state).await?;
    match classes
        .iter()
        .find(|class| class.id == class_id)
        .and_then(|class| project_type_for_class(&class.slug))
    {
        Some(project_type) => Ok(project_type),
        None => bail!(ErrorKind::InputError(format!(
            "installing this kind of CurseForge project isn't supported yet: {}",
            project.name
        ))),
    }
}

async fn plan_dependency(
    project_id: u64,
    target: &TargetInstance,
    state: &State,
) -> crate::Result<PlannedDependency> {
    let project = fetch_project(project_id, state).await?;
    let website_url = project.links.website_url.clone();
    let name = project.name.clone();
    if installed(target, project_id) {
        return Ok(PlannedDependency {
            project_id,
            name,
            website_url,
            status: DependencyStatus::AlreadyInstalled,
            file: None,
            reason: None,
        });
    }
    let project_type = project_type_of(&project, state).await?;
    let files = fetch_files(
        project_id,
        Some(&target.game_version),
        loader_type(target.loader),
        state,
    )
    .await?;
    let Some(file) =
        newest_file(&files, project_type, &target.game_version, target.loader)
    else {
        return Ok(PlannedDependency {
            project_id,
            name,
            website_url,
            status: DependencyStatus::Unavailable,
            file: None,
            reason: Some(format!(
                "no file for Minecraft {} on this loader",
                target.game_version
            )),
        });
    };
    if !can_download(&project, file) {
        return Ok(PlannedDependency {
            project_id,
            name,
            website_url,
            status: DependencyStatus::Unavailable,
            file: None,
            reason: Some(
                "its author doesn't allow downloads through other apps"
                    .to_string(),
            ),
        });
    }
    Ok(PlannedDependency {
        project_id,
        name,
        website_url,
        status: DependencyStatus::WillInstall,
        file: Some(file.clone()),
        reason: None,
    })
}

/// Everything that can be installed into an instance, each with how well it fits, and
/// the one to use by default. `loader_override` is for when someone had to say which
/// loader an instance uses. Only with `include_incompatible` are files that don't fit
/// listed, so they can be shown but never picked by default.
pub async fn install_options(
    instance_id: &str,
    project_id: u64,
    include_incompatible: bool,
    loader_override: Option<&str>,
) -> crate::Result<InstallOptions> {
    let state = State::get().await?;
    let target = target_instance(instance_id, loader_override).await?;
    let project = fetch_project(project_id, &state).await?;
    let project_type = project_type_of(&project, &state).await?;
    let (game_version, loader) = if include_incompatible {
        (None, None)
    } else {
        (
            Some(target.game_version.as_str()),
            if project_type == ProjectType::Mod {
                loader_type(target.loader)
            } else {
                None
            },
        )
    };
    let mut files: Vec<FileOption> =
        fetch_files(project_id, game_version, loader, &state)
            .await?
            .into_iter()
            .map(|file| FileOption {
                compatibility: evaluate_compatibility(
                    &file,
                    project_type,
                    &target.game_version,
                    target.loader,
                ),
                file,
            })
            .collect();
    if !include_incompatible {
        files.retain(|option| {
            !matches!(option.compatibility, Compatibility::Incompatible(_))
        });
    }
    let recommended_file_id = recommended_file(&files).map(|file| file.id);
    Ok(InstallOptions {
        files,
        recommended_file_id,
    })
}

/// Works out what installing a file involves, including required dependencies of
/// dependencies. Nothing is changed. Dependencies only ever come from CurseForge;
/// a similarly named Modrinth project is never substituted.
pub async fn plan_install(
    instance_id: &str,
    project_id: u64,
    file_id: u64,
    loader_override: Option<&str>,
) -> crate::Result<InstallPlan> {
    let state = State::get().await?;
    let target = target_instance(instance_id, loader_override).await?;
    let project = fetch_project(project_id, &state).await?;
    let file = fetch_file(project_id, file_id, &state).await?;
    let project_type = project_type_of(&project, &state).await?;
    let compatibility = evaluate_compatibility(
        &file,
        project_type,
        &target.game_version,
        target.loader,
    );

    let mut required = Vec::new();
    let mut seen = HashSet::from([project_id]);
    let mut queue: Vec<(u64, usize)> = file
        .dependencies
        .iter()
        .filter(|dependency| dependency.relation_type == RELATION_REQUIRED)
        .map(|dependency| (dependency.mod_id, 1))
        .collect();
    while let Some((dependency_id, depth)) = queue.pop() {
        if !seen.insert(dependency_id) {
            continue;
        }
        let planned = plan_dependency(dependency_id, &target, &state).await?;
        if depth < MAX_DEPENDENCY_DEPTH
            && let Some(dependency_file) = &planned.file
        {
            queue.extend(
                dependency_file
                    .dependencies
                    .iter()
                    .filter(|d| d.relation_type == RELATION_REQUIRED)
                    .map(|d| (d.mod_id, depth + 1)),
            );
        }
        required.push(planned);
    }

    let mut optional = Vec::new();
    for dependency in file
        .dependencies
        .iter()
        .filter(|dependency| dependency.relation_type == RELATION_OPTIONAL)
    {
        optional
            .push(plan_dependency(dependency.mod_id, &target, &state).await?);
    }

    let conflicts = file
        .dependencies
        .iter()
        .filter(|dependency| dependency.relation_type == RELATION_INCOMPATIBLE)
        .filter(|dependency| installed(&target, dependency.mod_id))
        .map(|dependency| curseforge_ref(dependency.mod_id))
        .collect();

    Ok(InstallPlan {
        can_download: can_download(&project, &file),
        project,
        file,
        compatibility,
        required,
        optional,
        conflicts,
    })
}

async fn download(
    file: &ProjectFile,
    key: &ApiKey,
    state: &State,
) -> crate::Result<tempfile::TempPath> {
    let url = file.download_url.as_deref().ok_or_else(|| {
        ErrorKind::InputError(format!(
            "{} can't be downloaded through other apps",
            file.file_name
        ))
    })?;
    let bytes = send_get(url, key, state).await?;
    if file.file_length > 0 && bytes.len() as u64 != file.file_length {
        bail!(ErrorKind::InputError(format!(
            "{} downloaded with the wrong size",
            file.file_name
        )));
    }
    if let Some(expected) =
        file.hashes.iter().find(|hash| hash.algo == HASH_SHA1)
    {
        use sha1_smol::Sha1;
        let actual = Sha1::from(&bytes).digest().to_string();
        if !actual.eq_ignore_ascii_case(&expected.value) {
            bail!(ErrorKind::InputError(format!(
                "{} failed its integrity check",
                file.file_name
            )));
        }
    }
    let temp_dir = state.directories.caches_dir();
    crate::util::io::create_dir_all(&temp_dir).await?;
    let temp = tempfile::NamedTempFile::new_in(&temp_dir)?;
    tokio::fs::write(temp.path(), &bytes).await?;
    Ok(temp.into_temp_path())
}

async fn install_file(
    instance_id: &str,
    project: &Project,
    file: &ProjectFile,
    previous_path: Option<&str>,
    state: &State,
) -> crate::Result<String> {
    if !can_download(project, file) {
        bail!(ErrorKind::InputError(format!(
            "the author of {} doesn't allow downloads through other apps",
            project.name
        )));
    }
    if !path_util::is_safe_file_name(&file.file_name) {
        bail!(ErrorKind::InputError(
            "CurseForge returned an unsafe file name".to_string()
        ));
    }
    let project_type = project_type_of(project, state).await?;
    let key = require_key().await?;
    let temp = download(file, &key, state).await?;
    let stored = state.content_store.store_file(&temp).await?;
    let requested = format!("{}/{}", project_type.get_folder(), file.file_name);
    let path = install_stored_file(
        instance_id,
        InstallContent {
            requested_path: &requested,
            stored_file: &stored,
            project_type,
            source_kind: ContentSourceKind::Local,
            origin: None,
            enabled_override: None,
            previous_path,
        },
        state,
    )
    .await?;
    if let Some(previous_path) = previous_path
        && previous_path != path
    {
        forget_origin(instance_id, previous_path, state).await?;
    }
    record_origin(
        instance_id,
        &path,
        &curseforge_ref(project.id),
        &file.id.to_string(),
        state,
    )
    .await?;
    Ok(path)
}

pub(crate) async fn record_origin(
    instance_id: &str,
    relative_path: &str,
    project_ref: &str,
    file_ref: &str,
    state: &State,
) -> crate::Result<()> {
    sqlx::query!(
        "
        INSERT INTO plus_content_origins
            (instance_id, relative_path, provider, project_ref, file_ref)
        VALUES ($1, $2, 'curseforge', $3, $4)
        ON CONFLICT (instance_id, relative_path) DO UPDATE SET
            project_ref = excluded.project_ref,
            file_ref = excluded.file_ref
        ",
        instance_id,
        relative_path,
        project_ref,
        file_ref,
    )
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub(crate) async fn forget_origin(
    instance_id: &str,
    relative_path: &str,
    state: &State,
) -> crate::Result<()> {
    sqlx::query!(
        "DELETE FROM plus_content_origins WHERE instance_id = $1 AND relative_path = $2",
        instance_id,
        relative_path,
    )
    .execute(&state.pool)
    .await?;
    Ok(())
}

/// Installed-file origins for one instance, keyed by path.
pub(crate) async fn origins(
    instance_id: &str,
    state: &State,
) -> crate::Result<std::collections::HashMap<String, (String, String)>> {
    let rows = sqlx::query!(
        "SELECT relative_path, project_ref, file_ref FROM plus_content_origins WHERE instance_id = $1",
        instance_id,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.relative_path, (row.project_ref, row.file_ref)))
        .collect())
}

/// Whether a recorded file is still in the instance, turned on or off. Removing a
/// file doesn't remove where it came from, so this is checked on disk.
async fn still_present(instance_id: &str, relative_path: &str) -> bool {
    let Ok(dir) = crate::api::instance::get_full_path(instance_id).await else {
        return false;
    };
    dir.join(relative_path).exists()
        || dir.join(format!("{relative_path}.disabled")).exists()
}

/// Instances that have a file from this CurseForge project.
pub async fn installed_in(project_id: u64) -> crate::Result<Vec<String>> {
    let state = State::get().await?;
    let project_ref = curseforge_ref(project_id);
    let rows = sqlx::query!(
        "SELECT instance_id, relative_path FROM plus_content_origins WHERE project_ref = $1",
        project_ref,
    )
    .fetch_all(&state.pool)
    .await?;
    let mut instances = std::collections::BTreeSet::new();
    for row in rows {
        if !instances.contains(&row.instance_id)
            && still_present(&row.instance_id, &row.relative_path).await
        {
            instances.insert(row.instance_id);
        }
    }
    Ok(instances.into_iter().collect())
}

/// CurseForge projects with a file in this instance.
pub async fn installed_projects(instance_id: &str) -> crate::Result<Vec<u64>> {
    let state = State::get().await?;
    let mut projects = std::collections::BTreeSet::new();
    for (relative_path, (project_ref, _)) in
        origins(instance_id, &state).await?
    {
        let Some(project_id) = project_ref
            .strip_prefix("curseforge:")
            .and_then(|id| id.parse::<u64>().ok())
        else {
            continue;
        };
        if !projects.contains(&project_id)
            && still_present(instance_id, &relative_path).await
        {
            projects.insert(project_id);
        }
    }
    Ok(projects.into_iter().collect())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InstallReport {
    pub installed: Vec<String>,
    pub already_installed: Vec<String>,
    pub unavailable: Vec<String>,
}

/// Installs a file and its required dependencies. It stops before changing anything
/// if the file is incompatible, unless `allow_incompatible` is set, or if a required
/// dependency can't be installed, unless `skip_unavailable` is set.
pub async fn install(
    instance_id: &str,
    project_id: u64,
    file_id: u64,
    allow_incompatible: bool,
    skip_unavailable: bool,
    loader_override: Option<&str>,
) -> crate::Result<InstallReport> {
    #[cfg(debug_assertions)]
    if samples::is_sample_id(project_id) {
        bail!(ErrorKind::InputError(
            "this is sample data, so there is nothing to install".to_string()
        ));
    }
    let plan =
        plan_install(instance_id, project_id, file_id, loader_override).await?;
    if !plan.can_download {
        bail!(ErrorKind::InputError(format!(
            "the author of {} doesn't allow downloads through other apps",
            plan.project.name
        )));
    }
    if let Compatibility::Incompatible(reason) = &plan.compatibility
        && !allow_incompatible
    {
        bail!(ErrorKind::InputError(format!(
            "{} can't be installed here: {reason}",
            plan.project.name
        )));
    }
    let unavailable: Vec<String> = plan
        .required
        .iter()
        .filter(|d| d.status == DependencyStatus::Unavailable)
        .map(|d| d.name.clone())
        .collect();
    if !unavailable.is_empty() && !skip_unavailable {
        bail!(ErrorKind::InputError(format!(
            "required mods aren't available: {}",
            unavailable.join(", ")
        )));
    }

    let state = State::get().await?;
    let mut report = InstallReport {
        unavailable,
        ..InstallReport::default()
    };
    for dependency in &plan.required {
        match (&dependency.status, &dependency.file) {
            (DependencyStatus::WillInstall, Some(file)) => {
                let project =
                    fetch_project(dependency.project_id, &state).await?;
                install_file(instance_id, &project, file, None, &state).await?;
                report.installed.push(dependency.name.clone());
            }
            (DependencyStatus::AlreadyInstalled, _) => {
                report.already_installed.push(dependency.name.clone());
            }
            _ => {}
        }
    }
    install_file(instance_id, &plan.project, &plan.file, None, &state).await?;
    report.installed.push(plan.project.name.clone());
    crate::event::emit::emit_instance(
        instance_id,
        crate::event::InstancePayloadType::Edited,
    )
    .await?;
    Ok(report)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AvailableUpdate {
    pub relative_path: String,
    pub project: Project,
    pub current_file_name: String,
    pub current_file_id: u64,
    pub file: ProjectFile,
    pub can_download: bool,
}

/// Picks the newest compatible file, preferring releases, for each CurseForge file
/// in the instance.
pub async fn check_updates(
    instance_id: &str,
) -> crate::Result<Vec<AvailableUpdate>> {
    let state = State::get().await?;
    let target = target_instance(instance_id, None).await?;
    let mut updates = Vec::new();
    for entry in &target.entries {
        if entry.provider != ContentProvider::CurseForge {
            continue;
        }
        let Some(project_id) = entry
            .project_id
            .as_deref()
            .and_then(|id| id.strip_prefix("curseforge:"))
            .and_then(|id| id.parse::<u64>().ok())
        else {
            continue;
        };
        let current_file_id = entry
            .version_id
            .as_deref()
            .and_then(|id| id.parse::<u64>().ok())
            .unwrap_or_default();
        let project = fetch_project(project_id, &state).await?;
        let files = fetch_files(
            project_id,
            Some(&target.game_version),
            if entry.project_type == ProjectType::Mod {
                loader_type(target.loader)
            } else {
                None
            },
            &state,
        )
        .await?;
        let Some(file) = newest_file(
            &files,
            entry.project_type,
            &target.game_version,
            target.loader,
        ) else {
            continue;
        };
        if file.id != current_file_id {
            updates.push(AvailableUpdate {
                relative_path: entry.relative_path.clone(),
                can_download: can_download(&project, file),
                project,
                current_file_name: entry.file_name.clone(),
                current_file_id,
                file: file.clone(),
            });
        }
    }
    Ok(updates)
}

/// What's known about one file installed from CurseForge. Where it came from is stored
/// locally, so `project` is only missing when CurseForge can't be asked.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstalledFile {
    pub relative_path: String,
    pub project_id: u64,
    pub file_id: u64,
    pub project: Option<Project>,
    pub current_file: Option<ProjectFile>,
    pub update: Option<ProjectFile>,
}

async fn installed_file(
    relative_path: String,
    project_id: u64,
    file_id: u64,
    target: &TargetInstance,
    state: &State,
) -> InstalledFile {
    let mut installed = InstalledFile {
        relative_path,
        project_id,
        file_id,
        project: None,
        current_file: None,
        update: None,
    };
    let Ok(project) = fetch_project(project_id, state).await else {
        return installed;
    };
    installed.current_file = fetch_file(project_id, file_id, state).await.ok();
    let project_type =
        ProjectType::get_from_parent_folder(&installed.relative_path)
            .unwrap_or(ProjectType::Mod);
    let files = fetch_files(
        project_id,
        Some(&target.game_version),
        if project_type == ProjectType::Mod {
            loader_type(target.loader)
        } else {
            None
        },
        state,
    )
    .await
    .unwrap_or_default();
    installed.update =
        newest_file(&files, project_type, &target.game_version, target.loader)
            .filter(|newest| {
                newest.id != file_id
                    && can_download(&project, newest)
                    && installed.current_file.as_ref().is_none_or(|current| {
                        newest.file_date > current.file_date
                    })
            })
            .cloned();
    installed.project = Some(project);
    installed
}

/// Every file in the instance that was installed from CurseForge.
pub async fn installed_files(
    instance_id: &str,
) -> crate::Result<Vec<InstalledFile>> {
    let state = State::get().await?;
    let origins = origins(instance_id, &state).await?;
    if origins.is_empty() {
        return Ok(Vec::new());
    }
    let parsed = origins.into_iter().filter_map(
        |(relative_path, (project_ref, file_ref))| {
            let project_id = project_ref
                .strip_prefix("curseforge:")?
                .parse::<u64>()
                .ok()?;
            Some((relative_path, project_id, file_ref.parse::<u64>().ok()?))
        },
    );
    if data_mode().await != DataMode::Live {
        return Ok(parsed
            .map(|(relative_path, project_id, file_id)| InstalledFile {
                relative_path,
                project_id,
                file_id,
                project: None,
                current_file: None,
                update: None,
            })
            .collect());
    }
    let target = target_instance(instance_id, None).await?;
    let target = &target;
    let state = &state;
    Ok(futures::stream::iter(parsed)
        .map(|(relative_path, project_id, file_id)| {
            installed_file(relative_path, project_id, file_id, target, state)
        })
        .buffered(MAX_CONCURRENT_INSTALLED_LOOKUPS)
        .collect()
        .await)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateSelection {
    pub relative_path: String,
    pub project_id: u64,
    pub file_id: u64,
}

/// Applies updates as one recorded operation, so they show in Update history and
/// can be rolled back together. Old files are kept by the content store.
pub async fn apply_updates(
    instance_id: &str,
    selections: Vec<UpdateSelection>,
) -> crate::Result<String> {
    let state = State::get().await?;
    if crate::state::instance_has_running_process(instance_id, &state).await? {
        bail!(ErrorKind::InputError(
            "stop this instance before updating its content".to_string()
        ));
    }
    if instance_rows::get_instance_by_id(instance_id, &state.pool)
        .await?
        .is_none()
    {
        bail!(ErrorKind::InputError("unknown instance".to_string()));
    }
    let operation_id = uuid::Uuid::new_v4().to_string();
    let recording = operations::begin_update(
        &operation_id,
        instance_id,
        OperationKind::UpdateAll,
        &state,
    )
    .await?;

    let mut failure = None;
    for selection in &selections {
        let result: crate::Result<()> = async {
            let project = fetch_project(selection.project_id, &state).await?;
            let file =
                fetch_file(selection.project_id, selection.file_id, &state)
                    .await?;
            install_file(
                instance_id,
                &project,
                &file,
                Some(&selection.relative_path),
                &state,
            )
            .await?;
            Ok(())
        }
        .await;
        if let Err(error) = result {
            failure = Some(error);
            break;
        }
    }

    if recording {
        operations::finish_update(
            &operation_id,
            instance_id,
            failure.is_none(),
            &state,
        )
        .await?;
    }
    crate::event::emit::emit_instance(
        instance_id,
        crate::event::InstancePayloadType::Edited,
    )
    .await?;
    match failure {
        Some(error) => Err(error),
        None => Ok(operation_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(versions: &[&str]) -> ProjectFile {
        ProjectFile {
            id: 1,
            game_versions: versions.iter().map(|v| v.to_string()).collect(),
            ..ProjectFile::default()
        }
    }

    #[test]
    fn parses_the_documented_response_shape() {
        let json = r#"{
            "data": [{
                "id": 123, "gameId": 432, "name": "Example", "slug": "example",
                "links": { "websiteUrl": "https://example.invalid", "wikiUrl": null },
                "summary": "text", "downloadCount": 12.0, "classId": 6,
                "authors": [{ "id": 1, "name": "someone", "url": "https://example.invalid" }],
                "logo": { "id": 1, "modId": 123, "thumbnailUrl": "https://example.invalid/t.png", "url": "https://example.invalid/l.png" },
                "allowModDistribution": null, "isAvailable": true,
                "latestFiles": [], "unknownField": { "ignored": true }
            }],
            "pagination": { "index": 0, "pageSize": 20, "resultCount": 1, "totalCount": 1 }
        }"#;
        let parsed: SearchResults = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.data[0].id, 123);
        assert_eq!(parsed.data[0].allow_mod_distribution, None);
        assert_eq!(parsed.pagination.total_count, 1);
    }

    #[test]
    fn compatible_when_version_and_loader_match() {
        assert_eq!(
            evaluate_compatibility(
                &file(&["1.21.1", "Fabric", "Client"]),
                ProjectType::Mod,
                "1.21.1",
                ModLoader::Fabric
            ),
            Compatibility::Compatible
        );
    }

    #[test]
    fn wrong_game_version_is_incompatible() {
        assert!(matches!(
            evaluate_compatibility(
                &file(&["1.20.1", "Fabric"]),
                ProjectType::Mod,
                "1.21.1",
                ModLoader::Fabric
            ),
            Compatibility::Incompatible(_)
        ));
    }

    #[test]
    fn forge_file_on_fabric_is_incompatible() {
        assert!(matches!(
            evaluate_compatibility(
                &file(&["1.21.1", "Forge"]),
                ProjectType::Mod,
                "1.21.1",
                ModLoader::Fabric
            ),
            Compatibility::Incompatible(_)
        ));
    }

    #[test]
    fn neoforge_is_not_mistaken_for_forge() {
        assert!(matches!(
            evaluate_compatibility(
                &file(&["1.21.1", "NeoForge"]),
                ProjectType::Mod,
                "1.21.1",
                ModLoader::Forge
            ),
            Compatibility::Incompatible(_)
        ));
    }

    #[test]
    fn missing_loader_metadata_is_unknown_not_compatible() {
        assert!(matches!(
            evaluate_compatibility(
                &file(&["1.21.1"]),
                ProjectType::Mod,
                "1.21.1",
                ModLoader::Fabric
            ),
            Compatibility::Unknown(_)
        ));
    }

    #[test]
    fn resource_packs_do_not_need_a_loader() {
        assert_eq!(
            evaluate_compatibility(
                &file(&["1.21.1"]),
                ProjectType::ResourcePack,
                "1.21.1",
                ModLoader::Vanilla
            ),
            Compatibility::Compatible
        );
    }

    #[test]
    fn mods_are_incompatible_with_vanilla() {
        assert!(matches!(
            evaluate_compatibility(
                &file(&["1.21.1", "Fabric"]),
                ProjectType::Mod,
                "1.21.1",
                ModLoader::Vanilla
            ),
            Compatibility::Incompatible(_)
        ));
    }

    #[test]
    fn distribution_needs_an_explicit_yes_and_a_url() {
        let allowed = Project {
            allow_mod_distribution: Some(true),
            ..Project::default()
        };
        let mut downloadable = file(&[]);
        downloadable.download_url =
            Some("https://edge.forgecdn.net/files/1/2/a.jar".to_string());
        assert!(can_download(&allowed, &downloadable));
        assert!(!can_download(&allowed, &file(&[])));
        for value in [None, Some(false)] {
            let denied = Project {
                allow_mod_distribution: value,
                ..Project::default()
            };
            assert!(!can_download(&denied, &downloadable));
        }
    }

    #[test]
    fn the_key_only_goes_to_curseforge_over_https() {
        assert!(is_curseforge_host("https://api.curseforge.com/v1/games"));
        assert!(is_curseforge_host(
            "https://edge.forgecdn.net/files/1/2/a.jar"
        ));
        assert!(!is_curseforge_host("http://api.curseforge.com/v1/games"));
        assert!(!is_curseforge_host("https://api.curseforge.com.evil.test/"));
        assert!(!is_curseforge_host("https://evil.test/edge.forgecdn.net"));
        assert!(!is_curseforge_host("not a url"));
    }

    #[test]
    fn newest_prefers_releases_over_newer_betas() {
        let mut release = file(&["1.21.1", "Fabric"]);
        release.id = 1;
        release.release_type = 1;
        release.file_date = "2026-01-01T00:00:00Z".to_string();
        let mut beta = file(&["1.21.1", "Fabric"]);
        beta.id = 2;
        beta.release_type = 2;
        beta.file_date = "2026-06-01T00:00:00Z".to_string();
        let files = [beta, release];
        let picked =
            newest_file(&files, ProjectType::Mod, "1.21.1", ModLoader::Fabric)
                .unwrap();
        assert_eq!(picked.id, 1);
    }

    #[test]
    fn search_clamps_paging_to_api_limits() {
        let path = search_path(&SearchQuery {
            index: 20_000,
            page_size: 500,
            search_filter: Some(" a&b ".to_string()),
            ..SearchQuery::default()
        });
        assert!(path.contains("pageSize=50"));
        assert!(path.contains("index=9950"));
        assert!(path.contains("searchFilter=a%26b"));
    }

    fn option(
        id: u64,
        release_type: u32,
        date: &str,
        compatibility: Compatibility,
    ) -> FileOption {
        FileOption {
            file: ProjectFile {
                id,
                release_type,
                file_date: date.to_string(),
                ..ProjectFile::default()
            },
            compatibility,
        }
    }

    #[test]
    fn a_stable_release_beats_a_newer_beta() {
        let options = [
            option(
                1,
                RELEASE_TYPE_BETA,
                "2026-03-01",
                Compatibility::Compatible,
            ),
            option(
                2,
                RELEASE_TYPE_RELEASE,
                "2026-01-01",
                Compatibility::Compatible,
            ),
        ];
        assert_eq!(recommended_file(&options).unwrap().id, 2);
    }

    #[test]
    fn the_newest_release_wins_among_releases() {
        let options = [
            option(
                1,
                RELEASE_TYPE_RELEASE,
                "2026-01-01",
                Compatibility::Compatible,
            ),
            option(
                2,
                RELEASE_TYPE_RELEASE,
                "2026-02-01",
                Compatibility::Compatible,
            ),
        ];
        assert_eq!(recommended_file(&options).unwrap().id, 2);
    }

    #[test]
    fn a_beta_is_used_when_there_is_no_release() {
        let options = [
            option(1, 3, "2026-03-01", Compatibility::Compatible),
            option(
                2,
                RELEASE_TYPE_BETA,
                "2026-01-01",
                Compatibility::Compatible,
            ),
        ];
        assert_eq!(recommended_file(&options).unwrap().id, 2);
    }

    #[test]
    fn a_file_known_to_fit_beats_one_that_cant_be_confirmed() {
        let options = [
            option(
                1,
                RELEASE_TYPE_RELEASE,
                "2026-03-01",
                Compatibility::Unknown("lists no loaders".to_string()),
            ),
            option(
                2,
                RELEASE_TYPE_BETA,
                "2026-01-01",
                Compatibility::Compatible,
            ),
        ];
        assert_eq!(recommended_file(&options).unwrap().id, 2);
    }

    #[test]
    fn an_incompatible_file_is_never_recommended_however_new() {
        let options = [option(
            1,
            RELEASE_TYPE_RELEASE,
            "2026-03-01",
            Compatibility::Incompatible("wrong version".to_string()),
        )];
        assert!(recommended_file(&options).is_none());
    }

    #[test]
    fn an_unavailable_file_is_never_recommended() {
        let mut unavailable = option(
            1,
            RELEASE_TYPE_RELEASE,
            "2026-03-01",
            Compatibility::Compatible,
        );
        unavailable.file.is_available = Some(false);
        assert!(recommended_file(&[unavailable]).is_none());
    }

    #[test]
    fn searching_by_author_asks_for_that_author() {
        let path = search_path(&SearchQuery {
            author_id: Some(42),
            page_size: 20,
            ..SearchQuery::default()
        });
        assert!(path.contains("authorId=42"));
    }

    #[test]
    fn data_modes_have_the_names_the_frontend_expects() {
        assert_eq!(
            serde_json::to_string(&DataMode::Live).unwrap(),
            r#""live""#
        );
        assert_eq!(
            serde_json::to_string(&DataMode::NeedsKey).unwrap(),
            r#""needs_key""#
        );
        assert_eq!(
            serde_json::to_string(&DataMode::KeyRejected).unwrap(),
            r#""key_rejected""#
        );
    }

    #[test]
    fn the_key_only_goes_to_curseforge() {
        assert!(!is_curseforge_host("http://127.0.0.1:9000/v1/games"));
        assert!(!is_curseforge_host("https://example.invalid/v1/games"));
    }
}
