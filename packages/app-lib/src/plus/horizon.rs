//! Horizon (<https://github.com/horizon-rust/horizon>), a fan-made rebuild of Minecraft:
//! Java Edition 26.3 in Rust, as another way to play a vanilla 26.3 instance.
//!
//! - **Setup** happens only when someone asks for it. It downloads Horizon's nightly build
//!   from its GitHub release and installs it only when Horizon's release key, built into
//!   Pawrinth, signed its checksum for exactly that build. Then it runs Horizon's own
//!   preparer on Mojang's 26.3 files. Horizon ships none of Minecraft's code or assets, so
//!   the preparer builds what the game needs from the player's own copy.
//! - **Every download** goes through the HTTPS-only client, so a redirect can't drop to http.
//! - **Mojang's files** come from Mojang's version manifest, because Horizon pins the
//!   SHA-1 of the version JSON and asset index, and the launcher meta's copies can differ.
//!   Asset objects Pawrinth already has are hard-linked instead of downloaded again.
//! - **Launching** uses Horizon's Launch API. The instance folder is the game folder, and
//!   the signed-in account goes in a session file that Horizon deletes once it's read.

use super::settings::PlusSettings;
use crate::instance::QuickPlayType;
use crate::state::{
    Credentials, InstanceInstallStage, InstanceLaunchContext, ModLoader,
    ProcessMetadata, Settings, WindowSize,
};
use crate::util::fetch;
use crate::{ErrorKind, State};
use anyhow::{Context, bail};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use bytes::Bytes;
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, PoisonError};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

/// The only Minecraft version Horizon plays.
pub const GAME_VERSION: &str = "26.3";
const RELEASE_URL: &str =
    "https://github.com/horizon-rust/horizon/releases/download/nightly/";
const RELEASE_API: &str =
    "https://api.github.com/repos/horizon-rust/horizon/releases/tags/nightly";
/// Horizon's release signing key, from `crates/horizon-release-sign/release-keys.txt` in its
/// repository. It's built in, so a different key can only arrive with a Pawrinth update.
const RELEASE_KEY: &str = "MU4oiIQcCxkZvbwSjjXs7SySCuM6ywRQ3nGaJYZG0gw=";
/// A nightly's signature for its own build (`horizon release nightly-<date>-<commit>`), so
/// neither another build's list nor the channel's can pass for it.
const SIGNATURE_FILE: &str = "SHA256SUMS.v2.sig";
// shortcut: only the Windows x64 build is set up and tested; add the macOS zips once someone can try them.
const ARCHIVE: Option<&str> = if cfg!(all(windows, target_arch = "x86_64")) {
    Some("horizon-windows-x64.zip")
} else {
    None
};
const MOJANG_MANIFEST: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const MOJANG_RESOURCES: &str = "https://resources.download.minecraft.net/";
const HORIZON_FOLDER: &str = "horizon";
const OBJECT_DOWNLOADS: usize = 16;
/// Windows' `CREATE_NO_WINDOW`: Horizon's programs are console programs.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug, Serialize)]
pub struct SetupProgress {
    pub message: String,
    pub fraction: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HorizonStatus {
    /// Whether Horizon has a build for this computer.
    pub supported: bool,
    /// The installed build, as Horizon names it.
    pub version: Option<String>,
    /// Whether the installed build has everything it needs to play.
    pub ready: bool,
    pub setting_up: Option<SetupProgress>,
    /// Whether this instance plays with Horizon.
    pub enabled: bool,
    /// Whether this instance can: vanilla 26.3, with nothing Horizon can't load.
    pub eligible: bool,
}

#[derive(Serialize, Deserialize)]
struct InstalledBuild {
    sha256: String,
    version: String,
}

static SETUP: Mutex<Option<SetupProgress>> = Mutex::new(None);

/// Marks a setup as running and clears it however the setup ends.
struct SetupGuard;

impl SetupGuard {
    fn start() -> crate::Result<Self> {
        let mut setup = SETUP.lock().unwrap_or_else(PoisonError::into_inner);
        if setup.is_some() {
            bail!(ErrorKind::InputError(
                "Horizon is already being set up".to_string()
            ));
        }
        *setup = Some(SetupProgress {
            message: "Starting".to_string(),
            fraction: Some(0.0),
        });
        Ok(Self)
    }
}

fn report(message: &str, fraction: f64) {
    *SETUP.lock().unwrap_or_else(PoisonError::into_inner) =
        Some(SetupProgress {
            message: message.to_string(),
            fraction: Some(fraction.clamp(0.0, 1.0)),
        });
}

fn report_message(message: &str) {
    if let Some(progress) = SETUP
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        message.clone_into(&mut progress.message);
    }
}

impl Drop for SetupGuard {
    fn drop(&mut self) {
        *SETUP.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

fn current_setup() -> Option<SetupProgress> {
    SETUP.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

struct Folders {
    root: PathBuf,
    game: PathBuf,
    resources: PathBuf,
    minecraft: PathBuf,
}

impl Folders {
    fn new(state: &State) -> Self {
        let root = state.directories.config_dir.join(HORIZON_FOLDER);
        Self {
            game: root.join("game"),
            resources: root.join("resources"),
            minecraft: root.join("minecraft"),
            root,
        }
    }

    fn installed_file(&self) -> PathBuf {
        self.root.join("game.json")
    }

    fn program(&self, name: &str) -> PathBuf {
        self.game
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    }
}

fn is_eligible(context: &InstanceLaunchContext) -> bool {
    context.applied_content_set.loader == ModLoader::Vanilla
        && context.applied_content_set.game_version == GAME_VERSION
}

async fn launch_context(
    instance_id: &str,
    state: &State,
) -> crate::Result<InstanceLaunchContext> {
    crate::state::instances::commands::get_instance_launch_context(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError(format!("no instance {instance_id}")).into()
    })
}

fn hidden(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Whether the prepared resources are complete and current for the installed build.
/// Horizon's check reads no game files, so it's quick.
async fn is_ready(folders: &Folders) -> bool {
    let program = folders.program("horizon-prepare");
    if !program.is_file() {
        return false;
    }
    hidden(&program)
        .arg("--check")
        .arg("--output")
        .arg(&folders.resources)
        .current_dir(&folders.game)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}

fn read_installed(folders: &Folders) -> Option<InstalledBuild> {
    let text = std::fs::read_to_string(folders.installed_file()).ok()?;
    serde_json::from_str(&text).ok()
}

pub async fn status(instance_id: &str) -> crate::Result<HorizonStatus> {
    let state = State::get().await?;
    let folders = Folders::new(&state);
    let context = launch_context(instance_id, &state).await?;
    let settings = PlusSettings::read(&state.pool).await?;
    let setting_up = current_setup();
    let ready = setting_up.is_none() && is_ready(&folders).await;
    Ok(HorizonStatus {
        supported: ARCHIVE.is_some(),
        version: read_installed(&folders).map(|build| build.version),
        ready,
        setting_up,
        enabled: settings.horizon.is_enabled(instance_id),
        eligible: is_eligible(&context),
    })
}

/// Records whether an instance plays with Horizon. It can be set before Horizon is set up,
/// and before a new instance has finished being created: launching checks both, and
/// ignores the choice for an instance that isn't vanilla 26.3.
pub async fn set_enabled(
    instance_id: &str,
    enabled: bool,
) -> crate::Result<()> {
    let state = State::get().await?;
    let value = if enabled {
        serde_json::Value::Bool(true)
    } else {
        serde_json::Value::Null
    };
    PlusSettings::merge_patch(
        &serde_json::json!({ "horizon": { "instances": { instance_id: value } } }),
        &state.pool,
    )
    .await?;
    Ok(())
}

/// Deletes Horizon and everything it prepared, and turns it off for every instance.
pub async fn remove() -> crate::Result<()> {
    let state = State::get().await?;
    let settings = PlusSettings::read(&state.pool).await?;
    for instance_id in settings.horizon.enabled_instances() {
        if crate::state::instance_has_running_process(instance_id, &state)
            .await?
        {
            bail!(ErrorKind::InputError(
                "Close the game before removing Horizon".to_string()
            ));
        }
    }
    let _setup = SetupGuard::start()?;
    let folders = Folders::new(&state);
    if folders.root.exists() {
        crate::util::io::remove_dir_all(&folders.root).await?;
    }
    PlusSettings::merge_patch(
        &serde_json::json!({ "horizon": { "instances": null } }),
        &state.pool,
    )
    .await?;
    Ok(())
}

/// Downloads or updates Horizon and prepares it from the instance's Minecraft 26.3.
pub async fn set_up(instance_id: &str) -> crate::Result<HorizonStatus> {
    let Some(archive) = ARCHIVE else {
        bail!(ErrorKind::InputError(
            "Horizon has no build for this computer yet".to_string()
        ));
    };
    let state = State::get().await?;
    let context = launch_context(instance_id, &state).await?;
    if !is_eligible(&context) {
        bail!(ErrorKind::InputError(format!(
            "Horizon only plays vanilla Minecraft {GAME_VERSION}"
        )));
    }
    if context.instance.install_stage != InstanceInstallStage::Installed {
        bail!(ErrorKind::InputError(
            "Wait for the instance to finish installing first".to_string()
        ));
    }
    let setup = SetupGuard::start()?;
    let folders = Folders::new(&state);
    crate::util::io::create_dir_all(&folders.root).await?;

    let new_build = install_build(archive, &folders, &state)
        .await
        .context("downloading Horizon")?;
    if !new_build && is_ready(&folders).await {
        drop(setup);
        return status(instance_id).await;
    }
    let inputs = gather_minecraft_files(&folders, &state)
        .await
        .context("getting Minecraft 26.3 from Mojang")?;
    let java = crate::launcher::resolve_java_for_launch(&context, &state)
        .await
        .context("finding Java for Horizon's preparer")?;
    let java_home = Path::new(&java.path)
        .parent()
        .and_then(Path::parent)
        .context("finding the Java folder")?
        .to_path_buf();
    prepare(&folders, &inputs, &java_home, &state).await?;
    drop(setup);
    status(instance_id).await
}

/// Installs the newest build, and says whether it's different from the one installed.
async fn install_build(
    archive: &str,
    folders: &Folders,
    state: &State,
) -> crate::Result<bool> {
    report("Checking for the newest Horizon build", 0.0);
    let release: Release =
        serde_json::from_slice(&download(RELEASE_API, None, state).await?)?;
    let version = signed_version(&release.name).with_context(|| {
        format!(
            "Horizon's nightly is named {:?}, not by its build",
            release.name
        )
    })?;
    let sums =
        download(&format!("{RELEASE_URL}SHA256SUMS"), None, state).await?;
    let signatures =
        download(&format!("{RELEASE_URL}{SIGNATURE_FILE}"), None, state)
            .await?;
    verify_signature(&version, &sums, &String::from_utf8_lossy(&signatures))?;
    let expected = expected_sha256(&String::from_utf8_lossy(&sums), archive)
        .context(
            "Horizon's signed checksums don't list this computer's build",
        )?;
    if read_installed(folders).is_some_and(|build| build.sha256 == expected)
        && folders.program("horizon-client").is_file()
    {
        return Ok(false);
    }

    report("Downloading Horizon", 0.0);
    let bytes =
        download_with_progress(&format!("{RELEASE_URL}{archive}")).await?;
    let actual: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual != expected {
        bail!(ErrorKind::LauncherError(format!(
            "Horizon's download doesn't match its signed checksum, so it wasn't installed ({actual})"
        )));
    }

    report("Unpacking Horizon", 0.25);
    let staging = folders.root.join("game.new");
    if staging.exists() {
        crate::util::io::remove_dir_all(&staging).await?;
    }
    {
        let staging = staging.clone();
        tokio::task::spawn_blocking(move || extract_build(&bytes, &staging))
            .await??;
    }
    if folders.game.exists() {
        crate::util::io::remove_dir_all(&folders.game)
            .await
            .context(
                "removing the old Horizon build (is Horizon still running?)",
            )?;
    }
    crate::util::io::rename_or_move(&staging, &folders.game).await?;

    let version = build_version(&folders.program("horizon-client"))
        .await
        .unwrap_or_else(|| "Nightly".to_string());
    crate::util::io::write(
        folders.installed_file(),
        serde_json::to_vec(&InstalledBuild {
            sha256: expected,
            version,
        })?,
    )
    .await?;
    Ok(true)
}

#[derive(Deserialize)]
struct Release {
    name: String,
}

/// A GET over the HTTPS-only client, checked against `sha1` when there is one.
async fn download(
    url: &str,
    sha1: Option<&str>,
    state: &State,
) -> crate::Result<Bytes> {
    fetch::fetch_advanced_with_client(
        reqwest::Method::GET,
        url,
        sha1,
        None,
        None,
        None,
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
        &fetch::REQWEST_CLIENT,
    )
    .await
}

/// Streams the build over the HTTPS-only client, reporting how much has arrived.
async fn download_with_progress(url: &str) -> crate::Result<Bytes> {
    let mut response = fetch::REQWEST_CLIENT
        .get(url)
        .send()
        .await?
        .error_for_status()?;
    let total = response.content_length().unwrap_or(0);
    let mut bytes = Vec::with_capacity(total as usize);
    while let Some(chunk) = response.chunk().await? {
        bytes.extend_from_slice(&chunk);
        if total > 0 {
            report(
                "Downloading Horizon",
                0.25 * bytes.len() as f64 / total as f64,
            );
        }
    }
    Ok(bytes.into())
}

/// The build a nightly's release title names ("Nightly 2026-11-13 · 9f42064"), as its
/// signature names it: `nightly-2026-11-13-9f42064`.
fn signed_version(title: &str) -> Option<String> {
    let (date, commit) = title.strip_prefix("Nightly ")?.split_once(" · ")?;
    let date_ok = date.len() == 10
        && date.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        });
    let commit_ok = (7..=40).contains(&commit.len())
        && commit
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    (date_ok && commit_ok).then(|| format!("nightly-{date}-{commit}"))
}

/// Checks that Horizon's release key signed `sums` for exactly `version`.
fn verify_signature(
    version: &str,
    sums: &[u8],
    signatures: &str,
) -> crate::Result<()> {
    let key = BASE64_STANDARD.decode(RELEASE_KEY)?;
    let key =
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key);
    let mut message = format!("horizon release {version}\n").into_bytes();
    message.extend_from_slice(sums);
    let signed = signatures
        .lines()
        .filter_map(|line| BASE64_STANDARD.decode(line.trim()).ok())
        .any(|signature| key.verify(&message, &signature).is_ok());
    if !signed {
        bail!(ErrorKind::LauncherError(format!(
            "{version} isn't signed with Horizon's release key, so it wasn't installed"
        )));
    }
    Ok(())
}

/// The SHA-256 a `sha256sum`-style list gives for one file.
fn expected_sha256(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.trim().split_once(char::is_whitespace)?;
        (name.trim().trim_start_matches('*') == file
            && hash.len() == 64
            && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| hash.to_ascii_lowercase())
    })
}

/// Unpacks the release archive without its top folder, refusing paths that leave it.
fn extract_build(archive: &[u8], into: &Path) -> crate::Result<()> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let name = entry.enclosed_name().ok_or_else(|| {
            ErrorKind::InputError(
                "Horizon's archive has a path outside its folder".to_string(),
            )
        })?;
        let relative: PathBuf = name.components().skip(1).collect();
        if relative.as_os_str().is_empty() {
            continue;
        }
        let out = into.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut file)?;
    }
    Ok(())
}

async fn build_version(client: &Path) -> Option<String> {
    let output = hidden(client)
        .arg("--launch-api")
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;
    let reply: serde_json::Value =
        serde_json::from_slice(&output.stdout).ok()?;
    reply.get("horizon")?.as_str().map(str::to_string)
}

struct MinecraftFiles {
    version_json: PathBuf,
    client_jar: PathBuf,
    server_jar: PathBuf,
    assets: PathBuf,
}

#[derive(Deserialize)]
struct VersionManifest {
    versions: Vec<ManifestVersion>,
}

#[derive(Deserialize)]
struct ManifestVersion {
    id: String,
    url: String,
    sha1: String,
}

#[derive(Deserialize)]
struct VersionJson {
    downloads: VersionDownloads,
    #[serde(rename = "assetIndex")]
    asset_index: AssetIndexRef,
}

#[derive(Deserialize)]
struct VersionDownloads {
    client: MojangFile,
    server: MojangFile,
}

#[derive(Deserialize)]
struct MojangFile {
    sha1: String,
    url: String,
}

#[derive(Deserialize)]
struct AssetIndexRef {
    id: String,
    sha1: String,
    url: String,
}

#[derive(Deserialize)]
struct AssetIndex {
    objects: std::collections::HashMap<String, AssetObject>,
}

#[derive(Deserialize)]
struct AssetObject {
    hash: String,
    size: u64,
}

async fn gather_minecraft_files(
    folders: &Folders,
    state: &Arc<State>,
) -> crate::Result<MinecraftFiles> {
    report("Getting Minecraft 26.3 from Mojang", 0.3);
    let manifest: VersionManifest =
        serde_json::from_slice(&download(MOJANG_MANIFEST, None, state).await?)?;
    let entry = manifest
        .versions
        .into_iter()
        .find(|version| version.id == GAME_VERSION)
        .context("Mojang's version list has no 26.3")?;

    let version_json = folders.minecraft.join(format!("{GAME_VERSION}.json"));
    ensure_file(&version_json, &entry.sha1, None, &entry.url, state).await?;
    let version: VersionJson =
        serde_json::from_slice(&tokio::fs::read(&version_json).await?)?;

    let client_jar = folders.minecraft.join("client.jar");
    let installed_jar = state
        .directories
        .version_dir(GAME_VERSION)
        .join(format!("{GAME_VERSION}.jar"));
    ensure_file(
        &client_jar,
        &version.downloads.client.sha1,
        Some(&installed_jar),
        &version.downloads.client.url,
        state,
    )
    .await?;

    let server_jar = folders.minecraft.join("server.jar");
    ensure_file(
        &server_jar,
        &version.downloads.server.sha1,
        None,
        &version.downloads.server.url,
        state,
    )
    .await?;

    let assets = folders.minecraft.join("assets");
    let index_file = assets
        .join("indexes")
        .join(format!("{}.json", version.asset_index.id));
    ensure_file(
        &index_file,
        &version.asset_index.sha1,
        None,
        &version.asset_index.url,
        state,
    )
    .await?;
    let index: AssetIndex =
        serde_json::from_slice(&tokio::fs::read(&index_file).await?)?;

    report("Collecting Minecraft's sounds and textures", 0.32);
    let objects = assets.join("objects");
    let store = state.directories.assets_dir().join("objects");
    let missing = tokio::task::spawn_blocking(move || {
        link_known_objects(index.objects.into_values(), &store, &objects)
    })
    .await??;
    let total = missing.len();
    futures::stream::iter(missing)
        .map(|(hash, destination)| {
            let state = Arc::clone(state);
            async move {
                let bytes = download(
                    &format!("{MOJANG_RESOURCES}{}/{hash}", &hash[..2]),
                    Some(&hash),
                    &state,
                )
                .await?;
                crate::util::io::write(&destination, bytes).await?;
                crate::Result::Ok(())
            }
        })
        .buffer_unordered(OBJECT_DOWNLOADS)
        .enumerate()
        .map(|(done, result)| {
            report_message(&format!(
                "Downloading Minecraft's sounds and textures ({}/{total})",
                done + 1
            ));
            result
        })
        .try_collect::<Vec<()>>()
        .await?;

    Ok(MinecraftFiles {
        version_json,
        client_jar,
        server_jar,
        assets,
    })
}

/// Makes `destination` the file with this SHA-1, from `local` when that copy matches,
/// else from `url`.
async fn ensure_file(
    destination: &Path,
    sha1: &str,
    local: Option<&Path>,
    url: &str,
    state: &State,
) -> crate::Result<()> {
    if has_sha1(destination, sha1).await {
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        crate::util::io::create_dir_all(parent).await?;
    }
    if let Some(local) = local
        && has_sha1(local, sha1).await
    {
        tokio::fs::copy(local, destination).await?;
        return Ok(());
    }
    let bytes = download(url, Some(sha1), state).await?;
    crate::util::io::write(destination, bytes).await?;
    Ok(())
}

async fn has_sha1(path: &Path, sha1: &str) -> bool {
    path.is_file()
        && fetch::sha1_file_async(path)
            .await
            .is_ok_and(|(_, hash)| hash == sha1)
}

/// Hard-links (or copies) each object Pawrinth's asset store already has into Horizon's
/// asset folder, and returns the ones it has to download. Store files are named by their
/// hash, so a matching size is taken as the same file; Horizon checks every SHA-1 it uses.
fn link_known_objects(
    objects: impl Iterator<Item = AssetObject>,
    store: &Path,
    into: &Path,
) -> crate::Result<Vec<(String, PathBuf)>> {
    let mut missing = Vec::new();
    for object in objects {
        if object.hash.len() < 2
            || !object.hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            bail!(ErrorKind::InputError(format!(
                "Mojang's asset index has a bad hash: {}",
                object.hash
            )));
        }
        let relative = Path::new(&object.hash[..2]).join(&object.hash);
        let destination = into.join(&relative);
        if size_is(&destination, object.size) {
            continue;
        }
        let source = store.join(&relative);
        if size_is(&source, object.size) {
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let _ = std::fs::remove_file(&destination);
            if std::fs::hard_link(&source, &destination).is_err() {
                std::fs::copy(&source, &destination)?;
            }
            continue;
        }
        missing.push((object.hash, destination));
    }
    Ok(missing)
}

fn size_is(path: &Path, size: u64) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.len() == size)
}

#[derive(Debug, PartialEq)]
enum PrepareLine {
    Message(String),
    Fraction(f64),
    Error(String),
}

/// Reads one line of `horizon-prepare --progress json`.
fn parse_prepare_line(line: &str) -> Option<PrepareLine> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if let Some(error) = value.get("error").and_then(|it| it.as_str()) {
        return Some(PrepareLine::Error(error.to_string()));
    }
    if let Some(fraction) = value.get("fraction").and_then(|it| it.as_f64()) {
        return Some(PrepareLine::Fraction(fraction));
    }
    value
        .get("message")
        .and_then(|it| it.as_str())
        .map(|message| PrepareLine::Message(message.to_string()))
}

async fn prepare(
    folders: &Folders,
    inputs: &MinecraftFiles,
    java_home: &Path,
    state: &State,
) -> crate::Result<()> {
    const START: f64 = 0.35;
    report("Preparing Horizon from your Minecraft files", START);
    let mut child = hidden(&folders.program("horizon-prepare"))
        .arg("--prepare")
        .arg("--complete")
        .args(["--progress", "json"])
        .arg("--output")
        .arg(&folders.resources)
        .arg("--server-jar")
        .arg(&inputs.server_jar)
        .arg("--version-json")
        .arg(&inputs.version_json)
        .arg("--client-jar")
        .arg(&inputs.client_jar)
        .arg("--libraries-dir")
        .arg(state.directories.libraries_dir())
        .arg("--assets-dir")
        .arg(&inputs.assets)
        .arg("--java-home")
        .arg(java_home)
        .current_dir(&folders.game)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("starting Horizon's preparer")?;

    let stderr = child.stderr.take();
    let stderr_text = tokio::spawn(async move {
        let mut text = String::new();
        if let Some(mut stderr) = stderr {
            let _ = stderr.read_to_string(&mut text).await;
        }
        text
    });

    let mut error = None;
    if let Some(stdout) = child.stdout.take() {
        let mut lines = BufReader::new(stdout).lines();
        while let Some(line) = lines.next_line().await? {
            match parse_prepare_line(&line) {
                Some(PrepareLine::Fraction(fraction)) => report(
                    "Preparing Horizon from your Minecraft files",
                    START + (1.0 - START) * fraction,
                ),
                Some(PrepareLine::Message(message)) => {
                    tracing::info!("Horizon preparer: {message}");
                }
                Some(PrepareLine::Error(message)) => error = Some(message),
                None => tracing::debug!("Horizon preparer: {line}"),
            }
        }
    }
    let exit = child.wait().await?;
    let stderr_text = stderr_text.await.unwrap_or_default();
    if !exit.success() {
        tracing::warn!("Horizon's preparer failed: {stderr_text}");
        let reason = error
            .or_else(|| stderr_text.lines().last().map(str::to_string))
            .unwrap_or_else(|| format!("exit code {:?}", exit.code()));
        bail!(ErrorKind::LauncherError(format!(
            "Horizon couldn't prepare its files: {reason}"
        )));
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
enum Window {
    Fullscreen,
    Size(u16, u16),
}

/// The Launch API command line, with the account in `session_file`, never in the arguments.
fn launch_args(
    resources: &Path,
    game_dir: &Path,
    session_file: &Path,
    window: &Window,
    quick_play: &QuickPlayType,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "--resources".into(),
        resources.into(),
        "--game-dir".into(),
        game_dir.into(),
        "--home".into(),
        game_dir.into(),
        "--session-file".into(),
        session_file.into(),
    ];
    match window {
        Window::Fullscreen => args.push("--fullscreen".into()),
        Window::Size(width, height) => {
            args.extend([
                "--width".into(),
                width.to_string().into(),
                "--height".into(),
                height.to_string().into(),
            ]);
        }
    }
    match quick_play {
        QuickPlayType::None => {}
        QuickPlayType::Singleplayer(world) => {
            args.extend(["--quickPlaySingleplayer".into(), world.into()]);
        }
        QuickPlayType::Server(address) => {
            let address = match address {
                crate::server_address::ServerAddress::Unresolved(address) => {
                    address.clone()
                }
                crate::server_address::ServerAddress::Resolved {
                    original_host,
                    original_port,
                    ..
                } => format!("{original_host}:{original_port}"),
            };
            args.extend(["--quickPlayMultiplayer".into(), address.into()]);
        }
    }
    args
}

fn session_json(credentials: &Credentials) -> serde_json::Value {
    serde_json::json!({
        "name": credentials.offline_profile.name,
        "uuid": credentials.offline_profile.id.simple().to_string(),
        "access_token": credentials.access_token,
        "user_type": "msa",
    })
}

/// Starts the instance with Horizon when it's turned on for it, and returns `None` when
/// it isn't, so the normal launch goes ahead.
pub(crate) async fn launch_if_enabled(
    context: &InstanceLaunchContext,
    credentials: &Credentials,
    quick_play: &QuickPlayType,
    state: &Arc<State>,
) -> crate::Result<Option<ProcessMetadata>> {
    let settings = PlusSettings::read(&state.pool).await?;
    if !settings.horizon.is_enabled(&context.instance.id)
        || !is_eligible(context)
    {
        return Ok(None);
    }
    let instance = &context.instance;
    if instance.install_stage != InstanceInstallStage::Installed {
        bail!(ErrorKind::LauncherError(
            "instance is not installed; start an install job first".to_string()
        ));
    }
    let folders = Folders::new(state);
    if current_setup().is_some() || !is_ready(&folders).await {
        bail!(ErrorKind::LauncherError(
            "Horizon isn't set up. Open this instance's settings and choose Horizon to set it up."
                .to_string()
        ));
    }
    if crate::state::instance_has_running_process(&instance.id, state).await? {
        bail!(ErrorKind::LauncherError(format!(
            "instance {} is already running",
            instance.id
        )));
    }

    let launcher_settings = Settings::get(&state.pool).await?;
    let window = if context
        .launch_overrides
        .force_fullscreen
        .unwrap_or(launcher_settings.force_fullscreen)
    {
        Window::Fullscreen
    } else {
        let WindowSize(width, height) = context
            .launch_overrides
            .game_resolution
            .unwrap_or(launcher_settings.game_resolution);
        Window::Size(width, height)
    };

    let game_dir = crate::util::io::canonicalize(
        state.directories.instances_dir().join(&instance.path),
    )?;
    let session = tempfile::TempDir::new()?;
    let session_file = session.path().join("session.json");
    crate::util::io::write(
        &session_file,
        serde_json::to_vec(&session_json(credentials))?,
    )
    .await?;

    let mut command = hidden(&folders.program("horizon-client"));
    command
        .args(launch_args(
            &folders.resources,
            &game_dir,
            &session_file,
            &window,
            quick_play,
        ))
        .current_dir(&game_dir);

    crate::state::instances::commands::set_instance_last_played(
        &instance.id,
        chrono::Utc::now(),
        &state.pool,
    )
    .await?;
    super::discord::show_playing_in_background(
        super::discord::PlayingInstance {
            name: instance.name.clone(),
            game_version: Some(GAME_VERSION.to_string()),
            loader: Some(ModLoader::Vanilla),
            started_at: Some(chrono::Utc::now().timestamp()),
        },
    );

    let rpc_server = crate::util::rpc::RpcServerBuilder::new().launch().await?;
    let process = state
        .process_manager
        .insert_new_process(
            &instance.id,
            &instance.path,
            &instance.name,
            command,
            None,
            Vec::new(),
            state.directories.instance_logs_dir(&instance.path),
            false,
            session,
            rpc_server,
            state,
            async |_, _| Ok(()),
        )
        .await?;
    Ok(Some(process))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server_address::ServerAddress;
    use std::io::Write;

    /// The real `SHA256SUMS` and `SHA256SUMS.v2.sig` of Horizon's nightly 2026-11-13.
    const NIGHTLY_SUMS: &str = concat!(
        "83d36c5a2f8b6690d4d77411dfc3f85753319184fffe738ee5f0a4c552a0eabe  horizon-linux-x86_64.tar.gz\n",
        "f47f9f988b7c30b727dfb35aed2b74f01428925b2eccc9bafff2352f4847f7d6  horizon-windows-x64.zip\n",
        "9cc4b7a72afba2df2a2c729f76a983d1e6cb82de44d1474fca04bc4d2a40d553  horizon-macos-arm64.zip\n",
        "f7d2d17d488ab2111fa0e7ce36a033fe6eb6db3ee6b74bc86aaa26dbab7e0144  horizon-macos-x64.zip\n",
        "f621e334c254d56bfeed16ed989500a36ef82a6a2e84ee6b861adcb774ced9b5  horizon-server-linux-x86_64.tar.gz\n",
        "aa9bf9383ae4e926d5f9c2ca8be997e843a6cf590fc3c8c04162a26718adeabd  horizon-source.tar.gz\n",
    );
    const NIGHTLY_SIGNATURE: &str = "W0WzX99BwYl5heDJXYjySOlYUWfQUSXVPll9VhrOwKehaHXbffIDAPpWG6U53k7aBfYXKD/FmJmBfjRiAioFCw==\n";
    /// The same list's `SHA256SUMS.sig`, signed for the channel rather than the build.
    const CHANNEL_SIGNATURE: &str = "aKtoYPO/dznYhh78vFORQvWnW/vzUvi6mM7w2PSNZcVwmMkuFpJzRbu+YvqMOJ0krL8Qkec/wNPYLZ6wzPmoCg==\n";

    #[test]
    fn a_nightly_installs_only_with_horizons_signature_for_that_build() {
        let build = "nightly-2026-11-13-9f42064";
        verify_signature(build, NIGHTLY_SUMS.as_bytes(), NIGHTLY_SIGNATURE)
            .unwrap();
        let tampered = NIGHTLY_SUMS.replace("f47f9f98", "f47f9f99");
        assert!(
            verify_signature(build, tampered.as_bytes(), NIGHTLY_SIGNATURE)
                .is_err()
        );
        assert!(
            verify_signature(
                "nightly-2026-11-12-9f42064",
                NIGHTLY_SUMS.as_bytes(),
                NIGHTLY_SIGNATURE
            )
            .is_err()
        );
        assert!(
            verify_signature(build, NIGHTLY_SUMS.as_bytes(), CHANNEL_SIGNATURE)
                .is_err()
        );
        assert!(verify_signature(build, NIGHTLY_SUMS.as_bytes(), "").is_err());
    }

    #[test]
    fn release_titles_name_the_build_they_are_signed_for() {
        assert_eq!(
            signed_version("Nightly 2026-11-13 · 9f42064").as_deref(),
            Some("nightly-2026-11-13-9f42064")
        );
        for title in [
            "Nightly 2026-11-13",
            "Snapshot 2026-11-13 · 9f42064",
            "Nightly 2026-1-13 · 9f42064",
            "Nightly 2026-11-13 · 9F42064",
            "Nightly 2026-11-13 · 9f42",
            "Nightly 2026-11-13 · 9f42064 extra",
        ] {
            assert_eq!(signed_version(title), None, "{title}");
        }
    }

    #[test]
    fn checksums_are_found_by_file_name() {
        let sums = "83d36c5a2f8b6690d4d77411dfc3f85753319184fffe738ee5f0a4c552a0eabe  horizon-linux-x86_64.tar.gz\n\
			F47F9F988B7C30B727DFB35AED2B74F01428925B2ECCC9BAFFF2352F4847F7D6 *horizon-windows-x64.zip\n";
        assert_eq!(
            expected_sha256(sums, "horizon-windows-x64.zip").as_deref(),
            Some(
                "f47f9f988b7c30b727dfb35aed2b74f01428925b2eccc9bafff2352f4847f7d6"
            )
        );
        assert_eq!(expected_sha256(sums, "horizon-windows-x64"), None);
        assert_eq!(expected_sha256("nothex  horizon.zip", "horizon.zip"), None);
    }

    #[test]
    fn preparer_progress_lines_are_read() {
        assert_eq!(
            parse_prepare_line(r#"{"fraction": 0.5}"#),
            Some(PrepareLine::Fraction(0.5))
        );
        assert_eq!(
            parse_prepare_line(r#"{"message": "exporting blocks"}"#),
            Some(PrepareLine::Message("exporting blocks".to_string()))
        );
        assert_eq!(
            parse_prepare_line(r#"{"error": "no Java 25"}"#),
            Some(PrepareLine::Error("no Java 25".to_string()))
        );
        assert_eq!(parse_prepare_line("plain text"), None);
    }

    #[test]
    fn the_account_never_goes_on_the_command_line() {
        let args = launch_args(
            Path::new("res"),
            Path::new("inst"),
            Path::new("tmp/session.json"),
            &Window::Size(854, 480),
            &QuickPlayType::None,
        );
        let args: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "--resources",
                "res",
                "--game-dir",
                "inst",
                "--home",
                "inst",
                "--session-file",
                "tmp/session.json",
                "--width",
                "854",
                "--height",
                "480",
            ]
        );
    }

    #[test]
    fn quick_play_and_fullscreen_use_the_launch_api_names() {
        let to_strings = |args: Vec<OsString>| -> Vec<String> {
            args.iter()
                .skip(8)
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect()
        };
        let world = launch_args(
            Path::new("r"),
            Path::new("g"),
            Path::new("s"),
            &Window::Fullscreen,
            &QuickPlayType::Singleplayer("My World".to_string()),
        );
        assert_eq!(
            to_strings(world),
            ["--fullscreen", "--quickPlaySingleplayer", "My World"]
        );
        let server = launch_args(
            Path::new("r"),
            Path::new("g"),
            Path::new("s"),
            &Window::Fullscreen,
            &QuickPlayType::Server(ServerAddress::Resolved {
                original_host: "play.example.net".to_string(),
                original_port: 25566,
                resolved_host: "10.0.0.1".to_string(),
                resolved_port: 25566,
            }),
        );
        assert_eq!(
            to_strings(server),
            [
                "--fullscreen",
                "--quickPlayMultiplayer",
                "play.example.net:25566"
            ]
        );
    }

    #[test]
    fn the_session_has_what_the_launch_api_requires() {
        let credentials = Credentials {
            offline_profile: crate::state::MinecraftProfile {
                id: uuid::Uuid::parse_str(
                    "069a79f4-44e9-4726-a5be-fca90e38aaf5",
                )
                .unwrap(),
                name: "Alex".to_string(),
                ..Default::default()
            },
            access_token: "token".to_string(),
            refresh_token: "refresh".to_string(),
            expires: chrono::Utc::now(),
            active: true,
        };
        assert_eq!(
            session_json(&credentials),
            serde_json::json!({
                "name": "Alex",
                "uuid": "069a79f444e94726a5befca90e38aaf5",
                "access_token": "token",
                "user_type": "msa",
            })
        );
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, data) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn the_build_is_unpacked_without_its_top_folder() {
        let folder = tempfile::tempdir().unwrap();
        let archive = zip_with(&[
            ("horizon-windows-x64/horizon-client.exe", b"client"),
            ("horizon-windows-x64/exporters/manifest.json", b"{}"),
        ]);
        extract_build(&archive, folder.path()).unwrap();
        assert_eq!(
            std::fs::read(folder.path().join("horizon-client.exe")).unwrap(),
            b"client"
        );
        assert!(folder.path().join("exporters/manifest.json").is_file());
    }

    #[test]
    fn paths_that_leave_the_folder_are_refused() {
        let folder = tempfile::tempdir().unwrap();
        let archive = zip_with(&[("../evil.exe", b"x")]);
        assert!(extract_build(&archive, &folder.path().join("game")).is_err());
        assert!(!folder.path().join("evil.exe").exists());
    }

    #[test]
    fn known_objects_are_linked_and_the_rest_downloaded() {
        let folder = tempfile::tempdir().unwrap();
        let store = folder.path().join("store");
        let into = folder.path().join("horizon");
        let known = "ab".repeat(20);
        let unknown = "cd".repeat(20);
        std::fs::create_dir_all(store.join("ab")).unwrap();
        std::fs::write(store.join("ab").join(&known), b"sound").unwrap();
        let missing = link_known_objects(
            [
                AssetObject {
                    hash: known.clone(),
                    size: 5,
                },
                AssetObject {
                    hash: unknown.clone(),
                    size: 3,
                },
            ]
            .into_iter(),
            &store,
            &into,
        )
        .unwrap();
        assert_eq!(
            std::fs::read(into.join("ab").join(&known)).unwrap(),
            b"sound"
        );
        assert_eq!(
            missing,
            [(unknown.clone(), into.join("cd").join(&unknown))]
        );
        assert!(
            link_known_objects(
                [AssetObject {
                    hash: "../../x".to_string(),
                    size: 1,
                }]
                .into_iter(),
                &store,
                &into,
            )
            .is_err()
        );
    }
}
