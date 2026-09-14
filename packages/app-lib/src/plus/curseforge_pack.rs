//! CurseForge modpacks. A pack is turned into an `.mrpack` that lists CurseForge's own
//! download addresses and checksums, then installs through the same jobs as a Modrinth
//! pack, with the same progress, pausing, cancelling, retrying and rollback.
//!
//! Files whose authors don't allow downloads through other apps are never fetched
//! another way. They're reported before anything is installed, and a pack is only
//! installed without them when someone chooses to.

use super::curseforge::{
    self as api, Compatibility, FileOption, Project, ProjectFile,
};
use crate::ErrorKind;
use crate::api::pack::install_from::{
    CreatePackLocation, PackDependency, PackFile, PackFileHash, PackFormat,
};
use crate::event::LoadingBarType;
use crate::event::emit::{emit_loading, init_loading};
use crate::install::{InstallJobSnapshot, InstallPostInstallEdit};
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::{InstanceLink, ModLoader, State};
use crate::util::fetch::{DownloadedFile, FetchProgressFn};
use anyhow::bail;
use path_util::SafeRelativeUtf8UnixPathBuf;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::future::Future;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

const MANIFEST_NAME: &str = "manifest.json";
const INDEX_NAME: &str = "modrinth.index.json";
const RECORD_NAME: &str = "pawrinth-curseforge.json";
const ICON_NAME: &str = "icon.png";
const PACKS_FOLDER: &str = "curseforge-packs";
const PACK_OVERRIDES: &str = "overrides";
/// Converted packs are kept this long, so a failed install can be retried.
const KEEP_CONVERTED_PACKS: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    minecraft: ManifestMinecraft,
    #[serde(default)]
    manifest_type: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    files: Vec<ManifestFile>,
    #[serde(default)]
    overrides: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<ManifestLoader>,
}

#[derive(Deserialize)]
struct ManifestLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct ManifestFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default = "required_by_default")]
    required: bool,
}

fn required_by_default() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingReason {
    /// The author doesn't allow downloads through other apps.
    NotDistributable,
    /// CurseForge doesn't offer the file, or doesn't give what's needed to check it.
    Unavailable,
    /// A kind of project Pawrinth doesn't know where to put.
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MissingFile {
    pub project_id: u64,
    pub file_id: u64,
    pub name: String,
    pub file_name: Option<String>,
    /// Where someone can download it themselves, when CurseForge says.
    pub download_page: Option<String>,
    pub reason: MissingReason,
}

/// A pack ready to install, and what installing it leaves out.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedPack {
    pub token: String,
    pub project_id: u64,
    pub file_id: u64,
    pub name: String,
    pub version: String,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
    pub missing: Vec<MissingFile>,
    pub optional_skipped: usize,
    /// For an update: files someone changed or added, which the update leaves alone.
    pub kept: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecordFile {
    path: String,
    project_id: u64,
    file_id: u64,
}

/// Written into the converted pack, so finishing an install knows where each file
/// came from.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PackRecord {
    project_id: u64,
    file_id: u64,
    name: String,
    version: String,
    file_name: String,
    for_instance: Option<String>,
    files: Vec<RecordFile>,
    /// Every override in this version of the pack, with its SHA-1.
    overrides: BTreeMap<String, String>,
    missing: Vec<MissingFile>,
    kept: Vec<String>,
    /// Projects someone had turned off, which stay off after an update.
    keep_disabled: Vec<String>,
}

/// What's known about an instance installed from a CurseForge modpack.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackInfo {
    pub instance_id: String,
    pub project_id: u64,
    pub file_id: u64,
    pub name: String,
    pub version: String,
    pub missing: Vec<MissingFile>,
    pub kept: Vec<String>,
}

struct OverrideEntry {
    index: usize,
    relative: String,
    sha1: String,
}

struct ReadPack {
    manifest: Manifest,
    overrides: Vec<OverrideEntry>,
}

/// Reads loader ids such as `forge-47.4.0`, `neoforge-21.1.90`,
/// `neoforge-1.20.1-47.1.106` and `fabric-0.16.5`.
pub(super) fn parse_loader(
    id: &str,
    game_version: &str,
) -> Option<(ModLoader, String)> {
    let (name, version) = id.split_once('-')?;
    let loader = match name.to_ascii_lowercase().as_str() {
        "forge" => ModLoader::Forge,
        "neoforge" => ModLoader::NeoForge,
        "fabric" => ModLoader::Fabric,
        "quilt" => ModLoader::Quilt,
        _ => return None,
    };
    let version = version
        .strip_prefix(&format!("{game_version}-"))
        .unwrap_or(version);
    (!version.is_empty()).then(|| (loader, version.to_string()))
}

fn pack_loader(
    manifest: &Manifest,
) -> crate::Result<(ModLoader, Option<String>)> {
    let loaders = &manifest.minecraft.mod_loaders;
    let Some(chosen) = loaders
        .iter()
        .find(|loader| loader.primary)
        .or(loaders.first())
    else {
        return Ok((ModLoader::Vanilla, None));
    };
    match parse_loader(&chosen.id, &manifest.minecraft.version) {
        Some((loader, version)) => Ok((loader, Some(version))),
        None => bail!(ErrorKind::InputError(format!(
            "this modpack uses a loader Pawrinth doesn't know: {}",
            chosen.id
        ))),
    }
}

fn pack_dependencies(
    game_version: &str,
    loader: ModLoader,
    loader_version: Option<&str>,
) -> HashMap<PackDependency, String> {
    let mut dependencies =
        HashMap::from([(PackDependency::Minecraft, game_version.to_string())]);
    let key = match loader {
        ModLoader::Forge => Some(PackDependency::Forge),
        ModLoader::NeoForge => Some(PackDependency::NeoForge),
        ModLoader::Fabric => Some(PackDependency::FabricLoader),
        ModLoader::Quilt => Some(PackDependency::QuiltLoader),
        _ => None,
    };
    if let (Some(key), Some(version)) = (key, loader_version) {
        dependencies.insert(key, version.to_string());
    }
    dependencies
}

/// The world folder an override belongs to, for paths under `saves/`.
fn world_of(relative: &str) -> Option<&str> {
    let mut parts = relative.strip_prefix("saves/")?.split('/');
    let world = parts.next().filter(|world| !world.is_empty())?;
    parts.next().map(|_| world)
}

fn sha1_hex(mut reader: impl Read) -> std::io::Result<String> {
    let mut hasher = sha1_smol::Sha1::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(hasher.digest().to_string());
        }
        hasher.update(&buffer[..read]);
    }
}

fn file_sha1(path: &Path) -> Option<String> {
    sha1_hex(std::io::BufReader::new(std::fs::File::open(path).ok()?)).ok()
}

fn display_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        "modpack".to_string()
    } else {
        cleaned
    }
}

fn packs_dir(state: &State) -> PathBuf {
    state.directories.caches_dir().join(PACKS_FOLDER)
}

fn valid_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 96
        && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
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

/// The newest release that can be downloaded, or a beta or alpha when there's no
/// release.
async fn newest_pack_file(
    project: &Project,
    state: &State,
) -> crate::Result<ProjectFile> {
    let options: Vec<FileOption> =
        api::fetch_files(project.id, None, None, state)
            .await?
            .into_iter()
            .filter(|file| api::can_download(project, file))
            .map(|file| FileOption {
                file,
                compatibility: Compatibility::Compatible,
            })
            .collect();
    api::recommended_file(&options).cloned().ok_or_else(|| {
        ErrorKind::InputError(format!(
            "{} has no version that can be downloaded",
            project.name
        ))
        .into()
    })
}

async fn download_pack(
    project: &Project,
    file: &ProjectFile,
    state: &State,
) -> crate::Result<DownloadedFile> {
    let url = file.download_url.as_deref().unwrap_or_default();
    if !api::is_curseforge_host(url) {
        bail!(ErrorKind::InputError(
            "CurseForge gave a download address on another host".to_string()
        ));
    }
    let Some(sha1) = file
        .hashes
        .iter()
        .find(|hash| hash.algo == api::HASH_SHA1)
        .map(|hash| hash.value.to_ascii_lowercase())
    else {
        bail!(ErrorKind::InputError(
            "CurseForge didn't give a checksum for this modpack".to_string()
        ));
    };
    let total = file.file_length.max(1);
    let bar = init_loading(
        LoadingBarType::PackImport {
            pack_name: project.name.clone(),
        },
        total as f64,
        "Downloading the modpack",
    )
    .await
    .ok();
    let mut reported = 0_u64;
    let mut progress =
        |current: u64,
         _total: u64|
         -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send>> {
            let current = current.min(total);
            if let Some(bar) = &bar
                && current > reported
            {
                let _ = emit_loading(bar, (current - reported) as f64, None);
                reported = current;
            }
            Box::pin(async { Ok(()) })
        };
    let staging = state.directories.caches_dir();
    crate::util::io::create_dir_all(&staging).await?;
    let downloaded = crate::util::fetch::fetch_file_with_client(
        url,
        &sha1,
        &state.fetch_semaphore,
        &state.pool,
        api::http_client(),
        Some(&mut progress as &mut FetchProgressFn<'_>),
        &staging,
    )
    .await?;
    if file.file_length > 0 && downloaded.size != file.file_length {
        bail!(ErrorKind::InputError(
            "the modpack downloaded with the wrong size".to_string()
        ));
    }
    Ok(downloaded)
}

fn read_pack(path: &Path) -> crate::Result<ReadPack> {
    let mut archive = zip::ZipArchive::new(std::io::BufReader::new(
        std::fs::File::open(path)?,
    ))
    .map_err(|_| {
        ErrorKind::InputError(
            "this file isn't a CurseForge modpack".to_string(),
        )
    })?;
    let manifest: Manifest = {
        let mut entry = archive.by_name(MANIFEST_NAME).map_err(|_| {
            ErrorKind::InputError(
                "this file isn't a CurseForge modpack".to_string(),
            )
        })?;
        let mut text = String::new();
        entry.read_to_string(&mut text)?;
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(
            |error| {
                ErrorKind::InputError(format!(
                    "this modpack's manifest can't be read: {error}"
                ))
            },
        )?
    };
    if manifest
        .manifest_type
        .as_deref()
        .is_some_and(|kind| kind != "minecraftModpack")
    {
        bail!(ErrorKind::InputError(
            "this file isn't a Minecraft modpack".to_string()
        ));
    }
    let prefix = format!(
        "{}/",
        manifest
            .overrides
            .as_deref()
            .unwrap_or(PACK_OVERRIDES)
            .trim_matches('/')
    );
    let mut overrides = Vec::new();
    let mut seen = HashSet::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name()?.replace('\\', "/");
        let Some(relative) = name.strip_prefix(&prefix) else {
            continue;
        };
        if relative.is_empty() || relative.ends_with('/') {
            continue;
        }
        if SafeRelativeUtf8UnixPathBuf::try_from(relative.to_string()).is_err()
        {
            bail!(ErrorKind::InputError(format!(
                "this modpack contains an unsafe file path: {relative}"
            )));
        }
        if !seen.insert(relative.to_lowercase()) {
            continue;
        }
        let relative = relative.to_string();
        let sha1 = sha1_hex(entry)?;
        overrides.push(OverrideEntry {
            index,
            relative,
            sha1,
        });
    }
    Ok(ReadPack {
        manifest,
        overrides,
    })
}

struct Resolved {
    files: Vec<PackFile>,
    record_files: Vec<RecordFile>,
    missing: Vec<MissingFile>,
    optional_skipped: usize,
}

async fn resolve_files(
    manifest: &Manifest,
    state: &State,
) -> crate::Result<Resolved> {
    let mut wanted = Vec::new();
    let mut seen_files = HashSet::new();
    let mut optional_skipped = 0;
    for entry in &manifest.files {
        if !entry.required {
            optional_skipped += 1;
        } else if seen_files.insert(entry.file_id) {
            wanted.push(entry);
        }
    }
    let file_ids: Vec<u64> = wanted.iter().map(|entry| entry.file_id).collect();
    let files: HashMap<u64, ProjectFile> =
        api::fetch_files_by_id(&file_ids, state)
            .await?
            .into_iter()
            .map(|file| (file.id, file))
            .collect();
    let project_ids: Vec<u64> = wanted
        .iter()
        .map(|entry| entry.project_id)
        .chain(files.values().map(|file| file.mod_id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let projects: HashMap<u64, Project> =
        api::fetch_projects_by_id(&project_ids, state)
            .await?
            .into_iter()
            .map(|project| (project.id, project))
            .collect();
    let (kinds, _) = api::class_kinds(state).await?;

    let mut resolved = Resolved {
        files: Vec::new(),
        record_files: Vec::new(),
        missing: Vec::new(),
        optional_skipped,
    };
    let mut seen_paths = HashSet::new();
    for entry in wanted {
        let file = files.get(&entry.file_id);
        let project =
            projects.get(&file.map_or(entry.project_id, |f| f.mod_id));
        let missing = |reason| MissingFile {
            project_id: entry.project_id,
            file_id: entry.file_id,
            name: project.map_or_else(
                || format!("CurseForge project {}", entry.project_id),
                |project| project.name.clone(),
            ),
            file_name: file.map(|file| file.file_name.clone()),
            download_page: project
                .and_then(|project| project.links.website_url.as_deref())
                .map(|url| {
                    format!(
                        "{}/files/{}",
                        url.trim_end_matches('/'),
                        entry.file_id
                    )
                }),
            reason,
        };
        let (Some(file), Some(project)) = (file, project) else {
            resolved.missing.push(missing(MissingReason::Unavailable));
            continue;
        };
        let Some(kind) = project.class_id.and_then(|id| kinds.get(&id)) else {
            resolved.missing.push(missing(MissingReason::Unsupported));
            continue;
        };
        if file.is_available == Some(false) {
            resolved.missing.push(missing(MissingReason::Unavailable));
            continue;
        }
        if !api::can_download(project, file) {
            resolved
                .missing
                .push(missing(MissingReason::NotDistributable));
            continue;
        }
        let url = file.download_url.clone().unwrap_or_default();
        let sha1 = file
            .hashes
            .iter()
            .find(|hash| hash.algo == api::HASH_SHA1)
            .map(|hash| hash.value.to_ascii_lowercase());
        let (true, Some(sha1), true) = (
            api::is_curseforge_host(&url),
            sha1,
            path_util::is_safe_file_name(&file.file_name),
        ) else {
            resolved.missing.push(missing(MissingReason::Unavailable));
            continue;
        };
        let path = format!("{}/{}", kind.get_folder(), file.file_name);
        if !seen_paths.insert(path.to_lowercase()) {
            continue;
        }
        let Ok(safe_path) = SafeRelativeUtf8UnixPathBuf::try_from(path.clone())
        else {
            resolved.missing.push(missing(MissingReason::Unavailable));
            continue;
        };
        resolved.files.push(PackFile {
            path: safe_path,
            hashes: HashMap::from([(PackFileHash::Sha1, sha1)]),
            env: None,
            downloads: vec![url],
            file_size: u32::try_from(file.file_length).unwrap_or(u32::MAX),
        });
        resolved.record_files.push(RecordFile {
            path,
            project_id: project.id,
            file_id: file.id,
        });
    }
    Ok(resolved)
}

struct Previous {
    overrides: BTreeMap<String, String>,
    instance_dir: PathBuf,
    keep_disabled: Vec<String>,
}

async fn previous_install(
    instance_id: &str,
    project_id: u64,
    state: &State,
) -> crate::Result<Previous> {
    let row = sqlx::query!(
        "SELECT project_id, overrides FROM plus_curseforge_packs WHERE instance_id = $1",
        instance_id,
    )
    .fetch_optional(&state.pool)
    .await?;
    let Some(row) = row else {
        bail!(ErrorKind::InputError(
            "this instance wasn't installed from a CurseForge modpack"
                .to_string()
        ));
    };
    if u64::try_from(row.project_id).ok() != Some(project_id) {
        bail!(ErrorKind::InputError(
            "this instance was installed from a different modpack".to_string()
        ));
    }
    if crate::state::instance_has_running_process(instance_id, state).await? {
        bail!(ErrorKind::InputError(
            "stop this instance before updating it".to_string()
        ));
    }
    let origins = api::origins(instance_id, state).await?;
    let keep_disabled = crate::state::instances::commands::list_project_files(
        instance_id,
        state,
    )
    .await?
    .into_iter()
    .filter(|file| !file.enabled)
    .filter_map(|file| {
        origins
            .get(&file.relative_path)
            .map(|(project, _)| project.clone())
    })
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect();
    Ok(Previous {
        overrides: serde_json::from_str(&row.overrides).unwrap_or_default(),
        instance_dir: crate::api::instance::get_full_path(instance_id).await?,
        keep_disabled,
    })
}

/// Which overrides an update leaves out, so nothing someone changed or made is
/// replaced. Worlds the instance already has are never touched, and other files
/// are only replaced when they're still what the last version of the pack put
/// there. Content folders belong to the pack, which replaces them on every
/// update.
fn overrides_to_keep(
    overrides: &[OverrideEntry],
    previous: &Previous,
) -> (HashSet<usize>, Vec<String>) {
    let mut skipped = HashSet::new();
    let mut kept = BTreeSet::new();
    for entry in overrides {
        if crate::state::content_store::is_managed_content_path(&entry.relative)
        {
            continue;
        }
        if let Some(world) = world_of(&entry.relative)
            && previous.instance_dir.join("saves").join(world).exists()
        {
            skipped.insert(entry.index);
            kept.insert(format!("saves/{world}"));
            continue;
        }
        let target = previous.instance_dir.join(&entry.relative);
        if !target.exists() {
            continue;
        }
        let current = file_sha1(&target);
        if current.as_deref() == Some(entry.sha1.as_str()) {
            skipped.insert(entry.index);
        } else if current.is_none()
            || previous.overrides.get(&entry.relative) != current.as_ref()
        {
            skipped.insert(entry.index);
            kept.insert(entry.relative.clone());
        }
    }
    (skipped, kept.into_iter().collect())
}

async fn pack_icon(project: &Project, state: &State) -> Option<bytes::Bytes> {
    let url = project.logo.as_ref().and_then(|logo| {
        logo.thumbnail_url.as_deref().or(logo.url.as_deref())
    })?;
    if !url.starts_with("https://") {
        return None;
    }
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

#[allow(clippy::too_many_arguments)]
fn write_pack(
    source: &Path,
    destination_dir: &Path,
    file_name: &str,
    index: &PackFormat,
    record: &PackRecord,
    icon: Option<&[u8]>,
    overrides: &[OverrideEntry],
    skipped: &HashSet<usize>,
) -> crate::Result<PathBuf> {
    std::fs::create_dir_all(destination_dir)?;
    let mut archive = zip::ZipArchive::new(std::io::BufReader::new(
        std::fs::File::open(source)?,
    ))?;
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
    for entry in overrides {
        if skipped.contains(&entry.index) {
            continue;
        }
        writer.raw_copy_file_rename(
            archive.by_index_raw(entry.index)?,
            format!("{PACK_OVERRIDES}/{}", entry.relative),
        )?;
    }
    writer.finish()?.flush()?;
    let destination = destination_dir.join(file_name);
    crate::util::io::persist_replacing(
        temporary.into_temp_path(),
        &destination,
    )?;
    Ok(destination)
}

/// Downloads and checks a CurseForge modpack and gets it ready to install. Nothing
/// is installed yet, so whatever can't be downloaded can be shown first. With an
/// instance, it's prepared as an update of the pack that instance already has.
pub async fn prepare(
    project_id: u64,
    file_id: Option<u64>,
    instance_id: Option<String>,
) -> crate::Result<PreparedPack> {
    api::require_key().await?;
    let state = State::get().await?;
    remove_old_packs(&state).await;
    let project = api::fetch_project(project_id, &state).await?;
    let (_, modpack_class) = api::class_kinds(&state).await?;
    if project.class_id.is_none() || project.class_id != modpack_class {
        bail!(ErrorKind::InputError(format!(
            "{} isn't a modpack",
            project.name
        )));
    }
    if project.allow_mod_distribution != Some(true) {
        bail!(ErrorKind::InputError(format!(
            "the author of {} doesn't allow downloads through other apps, so it can only be downloaded from CurseForge's website",
            project.name
        )));
    }
    let file = match file_id {
        Some(file_id) => api::fetch_file(project_id, file_id, &state).await?,
        None => newest_pack_file(&project, &state).await?,
    };
    if file.mod_id != project_id {
        bail!(ErrorKind::InputError(
            "that file belongs to a different project".to_string()
        ));
    }
    if !api::can_download(&project, &file) {
        bail!(ErrorKind::InputError(format!(
            "the author of {} doesn't allow downloads through other apps, so it can only be downloaded from CurseForge's website",
            project.name
        )));
    }
    let previous = match &instance_id {
        Some(instance_id) => {
            Some(previous_install(instance_id, project_id, &state).await?)
        }
        None => None,
    };

    let downloaded = download_pack(&project, &file, &state).await?;
    let source = downloaded.path().to_path_buf();
    let read =
        tokio::task::spawn_blocking(move || read_pack(&source)).await??;
    let (loader, loader_version) = pack_loader(&read.manifest)?;
    let game_version = read.manifest.minecraft.version.clone();
    if game_version.trim().is_empty() {
        bail!(ErrorKind::InputError(
            "this modpack doesn't say which Minecraft version it needs"
                .to_string()
        ));
    }
    let resolved = resolve_files(&read.manifest, &state).await?;
    let (skipped, kept) = match &previous {
        Some(previous) => overrides_to_keep(&read.overrides, previous),
        None => (HashSet::new(), Vec::new()),
    };
    let icon = match previous {
        Some(_) => None,
        None => pack_icon(&project, &state).await,
    };

    let name = read
        .manifest
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| project.name.clone());
    let version = read
        .manifest
        .version
        .clone()
        .filter(|version| !version.trim().is_empty())
        .unwrap_or_else(|| file.display_name.clone());
    let index = PackFormat {
        game: "minecraft".to_string(),
        format_version: 1,
        version_id: version.clone(),
        name: name.clone(),
        summary: Some(project.summary.clone()),
        files: resolved.files,
        dependencies: pack_dependencies(
            &game_version,
            loader,
            loader_version.as_deref(),
        ),
    };
    let record = PackRecord {
        project_id,
        file_id: file.id,
        name: name.clone(),
        version: version.clone(),
        file_name: file.file_name.clone(),
        for_instance: instance_id.clone(),
        files: resolved.record_files,
        overrides: read
            .overrides
            .iter()
            .map(|entry| (entry.relative.clone(), entry.sha1.clone()))
            .collect(),
        missing: resolved.missing.clone(),
        kept: kept.clone(),
        keep_disabled: previous
            .map(|previous| previous.keep_disabled)
            .unwrap_or_default(),
    };
    let token = match &instance_id {
        Some(instance_id) => format!(
            "{project_id}-{}-{}",
            file.id,
            instance_id
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .take(40)
                .collect::<String>()
        ),
        None => format!("{project_id}-{}", file.id),
    };
    let destination_dir = packs_dir(&state).join(&token);
    let file_count = index.files.len();
    let total_bytes = index
        .files
        .iter()
        .map(|file| u64::from(file.file_size))
        .sum();
    let pack_file_name = format!("{}.mrpack", display_name(&name));
    let source = downloaded.path().to_path_buf();
    let written_overrides = read.overrides;
    tokio::task::spawn_blocking(move || {
        write_pack(
            &source,
            &destination_dir,
            &pack_file_name,
            &index,
            &record,
            icon.as_deref(),
            &written_overrides,
            &skipped,
        )
    })
    .await??;

    Ok(PreparedPack {
        token,
        project_id,
        file_id: file.id,
        name,
        version,
        game_version,
        loader,
        loader_version,
        file_count,
        total_bytes,
        missing: resolved.missing,
        optional_skipped: resolved.optional_skipped,
        kept,
    })
}

async fn prepared_pack_path(
    token: &str,
    state: &State,
) -> crate::Result<PathBuf> {
    if !valid_token(token) {
        bail!(ErrorKind::InputError("unknown modpack".to_string()));
    }
    let mut entries = tokio::fs::read_dir(packs_dir(state).join(token))
        .await
        .map_err(|_| {
        ErrorKind::InputError(
            "this modpack isn't ready to install any more, so start again"
                .to_string(),
        )
    })?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "mrpack")
        {
            return Ok(path);
        }
    }
    bail!(ErrorKind::InputError(
        "this modpack isn't ready to install any more, so start again"
            .to_string()
    ))
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
    token: &str,
    instance_id: Option<String>,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let path = prepared_pack_path(token, &state).await?;
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

fn exists_maybe_disabled(instance_dir: &Path, relative: &str) -> bool {
    instance_dir.join(relative).exists()
        || instance_dir.join(format!("{relative}.disabled")).exists()
}

/// Runs at the end of an install job. For a converted CurseForge pack it records
/// where each file came from, turns back off what someone had turned off, and
/// remembers the installed version. Other packs are left alone.
pub(crate) async fn finish_install(
    instance_id: &str,
    location: &CreatePackLocation,
    state: &State,
) -> crate::Result<()> {
    let CreatePackLocation::FromFile { path } = location else {
        return Ok(());
    };
    if !path.starts_with(packs_dir(state)) {
        return Ok(());
    }
    let record = read_record(path).await?;
    let instance_dir = crate::api::instance::get_full_path(instance_id).await?;

    for relative_path in api::origins(instance_id, state).await?.into_keys() {
        if !exists_maybe_disabled(&instance_dir, &relative_path) {
            api::forget_origin(instance_id, &relative_path, state).await?;
        }
    }
    for file in &record.files {
        if exists_maybe_disabled(&instance_dir, &file.path) {
            api::record_origin(
                instance_id,
                &file.path,
                &api::curseforge_ref(file.project_id),
                &file.file_id.to_string(),
                state,
            )
            .await?;
        }
    }
    if !record.keep_disabled.is_empty() {
        let turned_off: HashSet<&str> =
            record.keep_disabled.iter().map(String::as_str).collect();
        let origins = api::origins(instance_id, state).await?;
        for file in crate::state::instances::commands::list_project_files(
            instance_id,
            state,
        )
        .await?
        {
            let off =
                origins
                    .get(&file.relative_path)
                    .is_some_and(|(project, _)| {
                        turned_off.contains(project.as_str())
                    });
            if file.enabled && off {
                crate::state::instances::commands::toggle_disable_project(
                    instance_id,
                    &file.relative_path,
                    Some(false),
                    state,
                )
                .await?;
            }
        }
    }

    let project_id = record.project_id as i64;
    let file_id = record.file_id as i64;
    let missing = serde_json::to_string(&record.missing)?;
    let kept = serde_json::to_string(&record.kept)?;
    let overrides = serde_json::to_string(&record.overrides)?;
    sqlx::query!(
        "
        INSERT INTO plus_curseforge_packs
            (instance_id, project_id, file_id, name, version, missing, kept, overrides)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (instance_id) DO UPDATE SET
            project_id = excluded.project_id,
            file_id = excluded.file_id,
            name = excluded.name,
            version = excluded.version,
            missing = excluded.missing,
            kept = excluded.kept,
            overrides = excluded.overrides
        ",
        instance_id,
        project_id,
        file_id,
        record.name,
        record.version,
        missing,
        kept,
        overrides,
    )
    .execute(&state.pool)
    .await?;
    Ok(())
}

/// Every instance installed from a CurseForge modpack. What's remembered about
/// instances that have since been deleted is forgotten here.
pub async fn installed_packs() -> crate::Result<Vec<PackInfo>> {
    let state = State::get().await?;
    let rows = sqlx::query!(
        "SELECT instance_id, project_id, file_id, name, version, missing, kept FROM plus_curseforge_packs"
    )
    .fetch_all(&state.pool)
    .await?;
    let mut packs = Vec::new();
    for row in rows {
        if instance_rows::get_instance_by_id(&row.instance_id, &state.pool)
            .await?
            .is_none()
        {
            sqlx::query!(
                "DELETE FROM plus_curseforge_packs WHERE instance_id = $1",
                row.instance_id,
            )
            .execute(&state.pool)
            .await?;
            continue;
        }
        let (Ok(project_id), Ok(file_id)) =
            (u64::try_from(row.project_id), u64::try_from(row.file_id))
        else {
            continue;
        };
        packs.push(PackInfo {
            instance_id: row.instance_id,
            project_id,
            file_id,
            name: row.name,
            version: row.version,
            missing: serde_json::from_str(&row.missing).unwrap_or_default(),
            kept: serde_json::from_str(&row.kept).unwrap_or_default(),
        });
    }
    Ok(packs)
}

/// Downloads one file of a converted CurseForge pack. CurseForge publishes SHA-1
/// checksums rather than SHA-512, so the download is checked against that before
/// it's kept in the content store.
pub(crate) async fn fetch_pack_file(
    state: &State,
    mirrors: &[&str],
    sha1: &str,
    progress: Option<&mut FetchProgressFn<'_>>,
) -> crate::Result<DownloadedFile> {
    let Some(url) = mirrors
        .iter()
        .copied()
        .find(|url| api::is_curseforge_host(url))
    else {
        bail!(ErrorKind::InputError(
            "a file in this pack has no SHA-512 checksum".to_string()
        ));
    };
    let staging = state.directories.caches_dir();
    crate::util::io::create_dir_all(&staging).await?;
    let downloaded = crate::util::fetch::fetch_file_with_client(
        url,
        &sha1.to_ascii_lowercase(),
        &state.fetch_semaphore,
        &state.pool,
        api::http_client(),
        progress,
        &staging,
    )
    .await?;
    let stored = state.content_store.store_file(downloaded.path()).await?;
    Ok(DownloadedFile::from_stored_file(stored, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_each_loader_id() {
        assert_eq!(
            parse_loader("forge-47.4.0", "1.20.1"),
            Some((ModLoader::Forge, "47.4.0".to_string()))
        );
        assert_eq!(
            parse_loader("neoforge-21.1.90", "1.21.1"),
            Some((ModLoader::NeoForge, "21.1.90".to_string()))
        );
        assert_eq!(
            parse_loader("neoforge-1.20.1-47.1.106", "1.20.1"),
            Some((ModLoader::NeoForge, "47.1.106".to_string()))
        );
        assert_eq!(
            parse_loader("fabric-0.16.5", "1.21.1"),
            Some((ModLoader::Fabric, "0.16.5".to_string()))
        );
        assert_eq!(
            parse_loader("quilt-0.26.4", "1.21.1"),
            Some((ModLoader::Quilt, "0.26.4".to_string()))
        );
        assert_eq!(parse_loader("rift-1.0", "1.13"), None);
        assert_eq!(parse_loader("forge", "1.20.1"), None);
    }

    fn manifest(json: &str) -> Manifest {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn the_primary_loader_is_used() {
        let parsed = manifest(
            r#"{"minecraft":{"version":"1.20.1","modLoaders":[
                {"id":"fabric-0.15.0","primary":false},
                {"id":"forge-47.2.0","primary":true}]}}"#,
        );
        assert_eq!(
            pack_loader(&parsed).unwrap(),
            (ModLoader::Forge, Some("47.2.0".to_string()))
        );
    }

    #[test]
    fn a_pack_without_loaders_is_vanilla() {
        let parsed =
            manifest(r#"{"minecraft":{"version":"1.20.1","modLoaders":[]}}"#);
        assert_eq!(pack_loader(&parsed).unwrap(), (ModLoader::Vanilla, None));
    }

    #[test]
    fn an_unknown_loader_is_refused_rather_than_guessed() {
        let parsed = manifest(
            r#"{"minecraft":{"version":"1.12.2","modLoaders":[{"id":"liteloader-1.0","primary":true}]}}"#,
        );
        assert!(pack_loader(&parsed).is_err());
    }

    #[test]
    fn files_are_required_unless_the_manifest_says_otherwise() {
        let parsed = manifest(
            r#"{"minecraft":{"version":"1.20.1"},"files":[
                {"projectID":1,"fileID":2},
                {"projectID":3,"fileID":4,"required":false}]}"#,
        );
        assert!(parsed.files[0].required);
        assert!(!parsed.files[1].required);
    }

    #[test]
    fn dependencies_use_the_mrpack_names() {
        let dependencies =
            pack_dependencies("1.21.1", ModLoader::NeoForge, Some("21.1.90"));
        let json = serde_json::to_value(&dependencies).unwrap();
        assert_eq!(json["minecraft"], "1.21.1");
        assert_eq!(json["neoforge"], "21.1.90");
        let vanilla = pack_dependencies("1.21.1", ModLoader::Vanilla, None);
        assert_eq!(vanilla.len(), 1);
    }

    #[test]
    fn world_folders_are_found_only_inside_saves() {
        assert_eq!(world_of("saves/Lobby/level.dat"), Some("Lobby"));
        assert_eq!(world_of("saves/Lobby"), None);
        assert_eq!(world_of("config/saves/x"), None);
    }

    #[test]
    fn tokens_are_only_letters_digits_and_dashes() {
        assert!(valid_token("123-456"));
        assert!(valid_token("123-456-abc"));
        assert!(!valid_token("../123"));
        assert!(!valid_token("123/456"));
        assert!(!valid_token(""));
    }

    fn entry(index: usize, relative: &str, sha1: &str) -> OverrideEntry {
        OverrideEntry {
            index,
            relative: relative.to_string(),
            sha1: sha1.to_string(),
        }
    }

    #[test]
    fn an_update_keeps_what_someone_changed_or_made() {
        let dir = tempfile::tempdir().unwrap();
        let write = |relative: &str, contents: &str| {
            let path = dir.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        };
        let hash = |contents: &str| {
            sha1_smol::Sha1::from(contents.as_bytes())
                .digest()
                .to_string()
        };
        write("config/untouched.toml", "pack v1");
        write("config/edited.toml", "edited by the player");
        write("config/made-by-player.toml", "mine");
        write("saves/Lobby/level.dat", "played in");
        write("mods/custom.jar", "old jar");
        let previous = Previous {
            overrides: BTreeMap::from([
                ("config/untouched.toml".to_string(), hash("pack v1")),
                ("config/edited.toml".to_string(), hash("pack v1")),
            ]),
            instance_dir: dir.path().to_path_buf(),
            keep_disabled: Vec::new(),
        };
        let overrides = [
            entry(0, "config/untouched.toml", &hash("pack v2")),
            entry(1, "config/edited.toml", &hash("pack v2")),
            entry(2, "config/made-by-player.toml", &hash("pack v2")),
            entry(3, "saves/Lobby/level.dat", &hash("fresh world")),
            entry(4, "saves/Lobby/region/r.0.0.mca", &hash("region")),
            entry(5, "config/new.toml", &hash("new")),
            entry(6, "mods/custom.jar", &hash("new jar")),
        ];
        let (skipped, kept) = overrides_to_keep(&overrides, &previous);
        assert_eq!(skipped, HashSet::from([1, 2, 3, 4]));
        assert_eq!(
            kept,
            vec![
                "config/edited.toml".to_string(),
                "config/made-by-player.toml".to_string(),
                "saves/Lobby".to_string(),
            ]
        );
    }

    #[test]
    fn an_unchanged_file_is_not_rewritten_or_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("config")).unwrap();
        std::fs::write(dir.path().join("config/same.toml"), "same").unwrap();
        let previous = Previous {
            overrides: BTreeMap::new(),
            instance_dir: dir.path().to_path_buf(),
            keep_disabled: Vec::new(),
        };
        let same = sha1_smol::Sha1::from(b"same".as_slice())
            .digest()
            .to_string();
        let (skipped, kept) = overrides_to_keep(
            &[entry(0, "config/same.toml", &same)],
            &previous,
        );
        assert_eq!(skipped, HashSet::from([0]));
        assert!(kept.is_empty());
    }

    fn cf_zip(entries: &[(&str, &[u8])]) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut writer = zip::ZipWriter::new(file.reopen().unwrap());
        for (name, contents) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
        file
    }

    #[test]
    fn overrides_with_unix_extra_fields_are_copied() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut writer = zip::ZipWriter::new(file.reopen().unwrap());
        writer
            .start_file(
                "manifest.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer
            .write_all(br#"{"minecraft":{"version":"1.20.1"}}"#)
            .unwrap();
        let mut options = zip::write::FullFileOptions::default();
        options.add_extra_field(0x756e, vec![0; 14], false).unwrap();
        writer
            .start_file("overrides/config/unix.toml", options)
            .unwrap();
        writer.write_all(b"unix").unwrap();
        writer.finish().unwrap();

        let read = read_pack(file.path()).unwrap();
        let out = tempfile::tempdir().unwrap();
        let index = PackFormat {
            game: "minecraft".to_string(),
            format_version: 1,
            version_id: "1.0".to_string(),
            name: "Test".to_string(),
            summary: None,
            files: Vec::new(),
            dependencies: pack_dependencies("1.20.1", ModLoader::Vanilla, None),
        };
        let record = PackRecord {
            project_id: 1,
            file_id: 2,
            name: "Test".to_string(),
            version: "1.0".to_string(),
            file_name: "test.zip".to_string(),
            for_instance: None,
            files: Vec::new(),
            overrides: BTreeMap::new(),
            missing: Vec::new(),
            kept: Vec::new(),
            keep_disabled: Vec::new(),
        };
        let written = write_pack(
            file.path(),
            out.path(),
            "Test.mrpack",
            &index,
            &record,
            None,
            &read.overrides,
            &HashSet::new(),
        )
        .unwrap();
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(&written).unwrap())
                .unwrap();
        let mut text = String::new();
        archive
            .by_name("overrides/config/unix.toml")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "unix");
    }

    #[test]
    fn reads_the_manifest_and_overrides() {
        let pack = cf_zip(&[
            (
                "manifest.json",
                br#"{"minecraft":{"version":"1.20.1","modLoaders":[{"id":"forge-47.2.0","primary":true}]},
                    "manifestType":"minecraftModpack","name":"Test","version":"1.0","files":[],"overrides":"extra"}"#,
            ),
            ("extra/config/a.toml", b"a"),
            ("extra\\config\\b.toml", b"b"),
            ("overrides/not-used.txt", b"x"),
            ("modlist.html", b"<ul></ul>"),
        ]);
        let read = read_pack(pack.path()).unwrap();
        let relative: Vec<&str> =
            read.overrides.iter().map(|o| o.relative.as_str()).collect();
        assert_eq!(relative, vec!["config/a.toml", "config/b.toml"]);
        assert_eq!(
            read.overrides[0].sha1,
            sha1_smol::Sha1::from(b"a".as_slice()).digest().to_string()
        );
    }

    #[test]
    fn an_unsafe_override_path_is_refused() {
        let pack = cf_zip(&[
            ("manifest.json", br#"{"minecraft":{"version":"1.20.1"}}"#),
            ("overrides/../../evil.txt", b"x"),
        ]);
        assert!(read_pack(pack.path()).is_err());
    }

    #[test]
    fn a_zip_without_a_manifest_is_not_a_modpack() {
        let pack = cf_zip(&[("overrides/config/a.toml", b"a")]);
        assert!(read_pack(pack.path()).is_err());
    }

    #[test]
    fn the_converted_pack_has_the_index_record_and_overrides() {
        let source = cf_zip(&[
            ("manifest.json", br#"{"minecraft":{"version":"1.20.1"}}"#),
            ("overrides/config/a.toml", b"a"),
            ("overrides/config/b.toml", b"b"),
        ]);
        let read = read_pack(source.path()).unwrap();
        let out = tempfile::tempdir().unwrap();
        let index = PackFormat {
            game: "minecraft".to_string(),
            format_version: 1,
            version_id: "1.0".to_string(),
            name: "Test".to_string(),
            summary: None,
            files: Vec::new(),
            dependencies: pack_dependencies("1.20.1", ModLoader::Vanilla, None),
        };
        let record = PackRecord {
            project_id: 1,
            file_id: 2,
            name: "Test".to_string(),
            version: "1.0".to_string(),
            file_name: "test.zip".to_string(),
            for_instance: None,
            files: Vec::new(),
            overrides: BTreeMap::new(),
            missing: Vec::new(),
            kept: Vec::new(),
            keep_disabled: Vec::new(),
        };
        let skipped = HashSet::from([read.overrides[1].index]);
        let written = write_pack(
            source.path(),
            out.path(),
            "Test.mrpack",
            &index,
            &record,
            Some(b"icon"),
            &read.overrides,
            &skipped,
        )
        .unwrap();
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(&written).unwrap())
                .unwrap();
        let mut names: Vec<String> = archive
            .file_names()
            .map(|name| name.unwrap().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "icon.png",
                "modrinth.index.json",
                "overrides/config/a.toml",
                "pawrinth-curseforge.json",
            ]
        );
        let mut text = String::new();
        archive
            .by_name("overrides/config/a.toml")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "a");
        let mut index_text = String::new();
        archive
            .by_name(INDEX_NAME)
            .unwrap()
            .read_to_string(&mut index_text)
            .unwrap();
        let parsed: PackFormat = serde_json::from_str(&index_text).unwrap();
        assert_eq!(parsed.name, "Test");
    }

    #[test]
    fn display_names_are_safe_file_names() {
        assert_eq!(
            display_name("All the Mods 9: To the Sky"),
            "All the Mods 9_ To the Sky"
        );
        assert!(!display_name("../..").contains(['/', '\\']));
        assert!(!display_name("../..").starts_with('.'));
        assert_eq!(display_name(""), "modpack");
    }
}
