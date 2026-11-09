//! Instances already installed through the FTB App. Their files are copied as they are,
//! so nothing is downloaded again and the FTB App's own folder is never changed. Nothing
//! is fetched from FTB: this only reads what's on the computer.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    ErrorKind, State,
    install::{InstallPhaseDetails, InstallProgressReporter},
    prelude::ModLoader,
    state::{AppliedContentSetPatch, EditInstance, InstanceInstallStage},
    util::io,
};

use super::{finish_import, recache_icon};

const INSTANCE_FILE: &str = "instance.json";
const FTB_FOLDER: &str = ".ftbapp";

fn lenient_memory<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::Number(number) => {
            number.as_u64().and_then(|value| u32::try_from(value).ok())
        }
        serde_json::Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .filter(|memory| *memory > 0))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct FtbInstance {
    name: String,
    version: Option<String>,
    mc_version: String,
    mod_loader: Option<String>,
    #[serde(deserialize_with = "lenient_memory")]
    rec_memory: Option<u32>,
    #[serde(deserialize_with = "lenient_memory")]
    memory: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VersionTargets {
    targets: Vec<VersionTarget>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct VersionTarget {
    name: String,
    version: String,
}

/// An FTB App instance, as the import list shows it.
#[derive(Clone, Debug, Serialize)]
pub struct FtbAppInstance {
    pub folder: String,
    pub name: String,
    pub pack_version: Option<String>,
    pub game_version: String,
    pub loader: ModLoader,
}

fn ftb_root() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    #[cfg(target_os = "macos")]
    let home = home.join("Library").join("Application Support");
    Some(home.join(".ftba"))
}

/// The FTB App's instances folder, from its own settings when it was moved.
pub fn default_instances_path() -> Option<PathBuf> {
    let root = ftb_root()?;
    for settings in [
        root.join("storage").join("settings.json"),
        root.join("bin").join("settings.json"),
    ] {
        let Ok(text) = std::fs::read_to_string(&settings) else {
            continue;
        };
        let location = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|value| {
                value
                    .get("instanceLocation")
                    .and_then(serde_json::Value::as_str)
                    .map(PathBuf::from)
            });
        if let Some(location) = location
            && location.is_dir()
        {
            return Some(location);
        }
    }
    let fallback = root.join("instances");
    fallback.is_dir().then_some(fallback)
}

async fn read_instance(folder: &Path) -> Option<FtbInstance> {
    let (text, _) =
        io::read_any_encoding_to_string(&folder.join(INSTANCE_FILE))
            .await
            .ok()?;
    serde_json::from_str::<FtbInstance>(text.trim_start_matches('\u{feff}'))
        .ok()
        .filter(|instance| !instance.mc_version.trim().is_empty())
}

/// The pack's own name, when the instance has one.
pub(crate) async fn instance_name(folder: &Path) -> Option<String> {
    read_instance(folder)
        .await
        .map(|instance| instance.name.trim().to_string())
        .filter(|name| !name.is_empty())
}

pub async fn is_valid_ftb_app(instance_folder: PathBuf) -> bool {
    read_instance(&instance_folder).await.is_some()
}

fn loader_named(name: &str) -> Option<ModLoader> {
    match name.trim().to_ascii_lowercase().as_str() {
        "forge" => Some(ModLoader::Forge),
        "neoforge" => Some(ModLoader::NeoForge),
        "fabric" => Some(ModLoader::Fabric),
        "quilt" => Some(ModLoader::Quilt),
        _ => None,
    }
}

/// Strips the Minecraft version from loader versions such as `1.20.1-47.1.106`.
fn without_game_version(version: &str, game_version: &str) -> Option<String> {
    let version = version.trim();
    let version = version
        .strip_prefix(&format!("{game_version}-"))
        .unwrap_or(version);
    let version = version
        .strip_suffix(&format!("-{game_version}"))
        .unwrap_or(version);
    (!version.is_empty()).then(|| version.to_string())
}

/// Reads loader names such as `forge-47.2.0`, `neoforge-21.1.77` and
/// `fabric-0.15.11`.
fn parse_mod_loader(
    text: &str,
    game_version: &str,
) -> Option<(ModLoader, String)> {
    let (name, version) = text.trim().split_once('-')?;
    Some((
        loader_named(name)?,
        without_game_version(version, game_version)?,
    ))
}

/// Older instances name their loader in `version.json` instead.
async fn legacy_loader(
    folder: &Path,
    game_version: &str,
) -> Option<(ModLoader, String)> {
    for candidate in [
        folder.join(FTB_FOLDER).join("version.json"),
        folder.join("version.json"),
    ] {
        let Ok((text, _)) = io::read_any_encoding_to_string(&candidate).await
        else {
            continue;
        };
        let Ok(targets) = serde_json::from_str::<VersionTargets>(
            text.trim_start_matches('\u{feff}'),
        ) else {
            continue;
        };
        return targets.targets.iter().find_map(|target| {
            Some((
                loader_named(&target.name)?,
                without_game_version(&target.version, game_version)?,
            ))
        });
    }
    None
}

async fn instance_loader(
    folder: &Path,
    instance: &FtbInstance,
) -> Option<(ModLoader, String)> {
    let game_version = instance.mc_version.trim();
    match instance
        .mod_loader
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        Some(text) => parse_mod_loader(text, game_version),
        None => legacy_loader(folder, game_version).await,
    }
}

async fn has_mods(folder: &Path) -> bool {
    let Ok(mut entries) = tokio::fs::read_dir(folder.join("mods")).await else {
        return false;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".jar") || name.ends_with(".jar.disabled") {
            return true;
        }
    }
    false
}

/// Every FTB App instance in a folder, with the names people know them by.
pub async fn list_instances(
    instances_path: &Path,
) -> crate::Result<Vec<FtbAppInstance>> {
    let mut found = Vec::new();
    let mut entries = io::read_dir(instances_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let folder = entry.path();
        if !folder.is_dir() {
            continue;
        }
        let Some(instance) = read_instance(&folder).await else {
            continue;
        };
        let loader = instance_loader(&folder, &instance)
            .await
            .map_or(ModLoader::Vanilla, |(loader, _)| loader);
        found.push(FtbAppInstance {
            folder: entry.file_name().to_string_lossy().to_string(),
            name: if instance.name.trim().is_empty() {
                entry.file_name().to_string_lossy().to_string()
            } else {
                instance.name.trim().to_string()
            },
            pack_version: instance.version.clone(),
            game_version: instance.mc_version.trim().to_string(),
            loader,
        });
    }
    found.sort_by_key(|instance| instance.name.to_lowercase());
    Ok(found)
}

/// The FTB App keeps its logo as one byte for the image type (PNG, JPEG, GIF or WebP),
/// then the image itself.
fn logo_image(logo: &[u8]) -> Option<&[u8]> {
    match logo.split_first() {
        Some((0..=3, image)) if !image.is_empty() => Some(image),
        _ => None,
    }
}

async fn instance_icon(
    folder: &Path,
    state: &State,
) -> crate::Result<Option<PathBuf>> {
    let picture = folder.join("folder.jpg");
    if tokio::fs::try_exists(&picture).await.unwrap_or(false) {
        return recache_icon(picture).await;
    }
    let Ok(logo) = tokio::fs::read(folder.join(FTB_FOLDER).join("logo")).await
    else {
        return Ok(None);
    };
    match logo_image(&logo) {
        Some(image) => Ok(Some(
            crate::api::instance::cache_icon(
                bytes::Bytes::copy_from_slice(image),
                state,
            )
            .await?,
        )),
        None => Ok(None),
    }
}

pub async fn import_ftb_app(
    instance_folder: PathBuf,
    instance_id: &str,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
) -> crate::Result<()> {
    let instance = read_instance(&instance_folder).await.ok_or_else(|| {
        ErrorKind::InputError(
            "this folder isn't an FTB App instance".to_string(),
        )
    })?;
    let state = State::get().await?;
    let game_version = instance.mc_version.trim().to_string();
    let found_loader = instance_loader(&instance_folder, &instance).await;
    if found_loader.is_none() && has_mods(&instance_folder).await {
        return Err(ErrorKind::InputError(format!(
            "Pawrinth couldn't tell which mod loader {} uses",
            instance.name
        ))
        .into());
    }
    let (loader, loader_version) = match found_loader {
        Some((loader, version)) => {
            let resolved = crate::launcher::get_loader_version_from_profile(
                &game_version,
                loader,
                Some(&version),
                &state.directories,
            )
            .await?;
            (loader, resolved.map(|version| version.id))
        }
        None => (ModLoader::Vanilla, None),
    };
    let icon = instance_icon(&instance_folder, &state)
        .await
        .unwrap_or(None);
    let name = if instance.name.trim().is_empty() {
        format!(
            "FTB-{}",
            instance_folder
                .file_name()
                .map_or("Unknown".to_string(), |name| name
                    .to_string_lossy()
                    .to_string())
        )
    } else {
        instance.name.trim().to_string()
    };

    crate::api::instance::edit(
        instance_id,
        EditInstance {
            install_stage: (loader != ModLoader::Vanilla)
                .then_some(InstanceInstallStage::PackInstalling),
            name: Some(name),
            icon_path: Some(
                icon.map(|path| path.to_string_lossy().to_string()),
            ),
            content_set_patch: Some(AppliedContentSetPatch {
                source_kind: None,
                game_version: Some(game_version),
                protocol_version: Some(None),
                loader: Some(loader),
                loader_version: Some(loader_version),
            }),
            ..EditInstance::default()
        },
    )
    .await?;

    finish_import(instance_id, instance_folder, &state, reporter, details)
        .await?;

    if let Some(memory_mb) = instance.rec_memory.or(instance.memory)
        && let Err(error) = crate::plus::memory::recommend(
            instance_id,
            memory_mb,
            crate::plus::settings::MemorySource::FtbApp,
        )
        .await
    {
        tracing::warn!("Couldn't apply the FTB instance's memory: {error}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_names_are_read() {
        assert_eq!(
            parse_mod_loader("forge-47.2.0", "1.20.1"),
            Some((ModLoader::Forge, "47.2.0".to_string()))
        );
        assert_eq!(
            parse_mod_loader("neoforge-21.1.77", "1.21.1"),
            Some((ModLoader::NeoForge, "21.1.77".to_string()))
        );
        assert_eq!(
            parse_mod_loader("neoforge-1.20.1-47.1.106", "1.20.1"),
            Some((ModLoader::NeoForge, "47.1.106".to_string()))
        );
        assert_eq!(
            parse_mod_loader("fabric-0.15.11-1.20.1", "1.20.1"),
            Some((ModLoader::Fabric, "0.15.11".to_string()))
        );
        assert_eq!(parse_mod_loader("mystery-1.0", "1.20.1"), None);
        assert_eq!(parse_mod_loader("forge", "1.20.1"), None);
    }

    #[test]
    fn memory_fields_may_be_text_or_missing() {
        let instance: FtbInstance = serde_json::from_str(
            r#"{ "name": "A", "mcVersion": "1.20.1", "recMemory": "6144", "memory": 4096 }"#,
        )
        .unwrap();
        assert_eq!(instance.rec_memory, Some(6144));
        assert_eq!(instance.memory, Some(4096));
        let bare: FtbInstance =
            serde_json::from_str(r#"{ "name": "A", "mcVersion": "1.20.1" }"#)
                .unwrap();
        assert_eq!(bare.rec_memory, None);
        let zero: FtbInstance = serde_json::from_str(
            r#"{ "name": "A", "mcVersion": "1.20.1", "memory": 0 }"#,
        )
        .unwrap();
        assert_eq!(zero.memory, None);
    }

    #[tokio::test]
    async fn instances_are_listed_by_their_own_names() {
        let root = tempfile::tempdir().unwrap();
        let modern = root.path().join("1d2c3b4a-0000-0000-0000-000000000001");
        std::fs::create_dir_all(modern.join("mods")).unwrap();
        std::fs::write(
            modern.join(INSTANCE_FILE),
            r#"{ "uuid": "1d2c", "id": 100, "versionId": 2, "name": "FTB StoneBlock 4",
                "version": "1.4.0", "mcVersion": "1.21.1", "modLoader": "neoforge-21.1.77" }"#,
        )
        .unwrap();
        let legacy = root.path().join("legacy");
        std::fs::create_dir_all(legacy.join(FTB_FOLDER)).unwrap();
        std::fs::write(
            legacy.join(INSTANCE_FILE),
            r#"{ "name": "Old Pack", "mcVersion": "1.12.2" }"#,
        )
        .unwrap();
        std::fs::write(
            legacy.join(FTB_FOLDER).join("version.json"),
            r#"{ "targets": [
                { "name": "minecraft", "version": "1.12.2", "type": "game" },
                { "name": "forge", "version": "14.23.5.2860", "type": "modloader" }
            ] }"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.path().join("not-an-instance")).unwrap();

        let found = list_instances(root.path()).await.unwrap();
        let summary: Vec<(&str, &str, ModLoader)> = found
            .iter()
            .map(|instance| {
                (
                    instance.name.as_str(),
                    instance.game_version.as_str(),
                    instance.loader,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("FTB StoneBlock 4", "1.21.1", ModLoader::NeoForge),
                ("Old Pack", "1.12.2", ModLoader::Forge),
            ]
        );
        assert_eq!(
            legacy_loader(&legacy, "1.12.2").await,
            Some((ModLoader::Forge, "14.23.5.2860".to_string()))
        );
    }

    #[test]
    fn logos_keep_only_known_image_types() {
        assert_eq!(logo_image(&[0, 137, 80, 78]), Some(&[137, 80, 78][..]));
        assert_eq!(logo_image(&[3, 1]), Some(&[1][..]));
        assert_eq!(logo_image(&[9, 1, 2]), None);
        assert_eq!(logo_image(&[0]), None);
        assert_eq!(logo_image(&[]), None);
    }
}
