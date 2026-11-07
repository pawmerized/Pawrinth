use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    State,
    install::{InstallPhaseDetails, InstallProgressReporter},
    prelude::ModLoader,
    state::{AppliedContentSetPatch, EditInstance, InstanceInstallStage},
    util::{fetch::fetch, io},
};

use super::{finish_import, recache_icon};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftInstance {
    pub name: Option<String>,
    pub base_mod_loader: Option<MinecraftInstanceModLoader>,
    pub profile_image_path: Option<PathBuf>,
    pub installed_modpack: Option<InstalledModpack>,
    pub game_version: String, // Minecraft game version. Non-prioritized, use this if Vanilla
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftInstanceModLoader {
    pub name: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModpack {
    pub thumbnail_url: Option<String>,
}

// Check if folder has a minecraftinstance.json that parses
pub async fn is_valid_curseforge(instance_folder: PathBuf) -> bool {
    let minecraft_instance = serde_json::from_str::<MinecraftInstance>(
        &io::read_any_encoding_to_string(
            &instance_folder.join("minecraftinstance.json"),
        )
        .await
        .unwrap_or(("".into(), encoding_rs::UTF_8))
        .0,
    );
    minecraft_instance.is_ok()
}

pub async fn import_curseforge(
    curseforge_instance_folder: PathBuf, // instance's folder
    instance_id: &str,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
) -> crate::Result<()> {
    // Load minecraftinstance.json
    let minecraft_instance = serde_json::from_str::<MinecraftInstance>(
        &io::read_any_encoding_to_string(
            &curseforge_instance_folder.join("minecraftinstance.json"),
        )
        .await
        .unwrap_or(("".into(), encoding_rs::UTF_8))
        .0,
    )?;
    let override_title = minecraft_instance.name;
    let backup_name = format!(
        "Curseforge-{}",
        curseforge_instance_folder
            .file_name()
            .map_or("Unknown".to_string(), |a| a.to_string_lossy().to_string())
    );

    let state = State::get().await?;
    // Recache Curseforge Icon if it exists
    let mut icon = None;

    if let Some(icon_path) = minecraft_instance.profile_image_path.clone() {
        icon = recache_icon(icon_path).await?;
    } else if let Some(InstalledModpack {
        thumbnail_url: Some(thumbnail_url),
    }) = minecraft_instance.installed_modpack.clone()
    {
        let icon_bytes = fetch(
            &thumbnail_url,
            None,
            None,
            None,
            &state.fetch_semaphore,
            &state.pool,
        )
        .await?;
        icon =
            Some(crate::api::instance::cache_icon(icon_bytes, &state).await?);
    }

    // base mod loader is always None for vanilla
    if let Some(instance_mod_loader) = minecraft_instance.base_mod_loader {
        let game_version = minecraft_instance.game_version;

        let (mod_loader, loader_version) =
            parse_loader_name(&instance_mod_loader.name);

        let mod_loader = mod_loader.unwrap_or(ModLoader::Vanilla);

        let loader_version = if mod_loader != ModLoader::Vanilla {
            crate::launcher::get_loader_version_from_profile(
                &game_version,
                mod_loader,
                loader_version.as_deref(),
                &state.directories,
            )
            .await?
        } else {
            None
        };

        crate::api::instance::edit(
            instance_id,
            EditInstance {
                install_stage: Some(InstanceInstallStage::PackInstalling),
                name: Some(
                    override_title
                        .clone()
                        .unwrap_or_else(|| backup_name.to_string()),
                ),
                icon_path: Some(
                    icon.clone().map(|x| x.to_string_lossy().to_string()),
                ),
                content_set_patch: Some(AppliedContentSetPatch {
                    source_kind: None,
                    game_version: Some(game_version.clone()),
                    protocol_version: Some(None),
                    loader: Some(mod_loader),
                    loader_version: Some(loader_version.clone().map(|x| x.id)),
                }),
                ..EditInstance::default()
            },
        )
        .await?;
    } else {
        crate::api::instance::edit(
            instance_id,
            EditInstance {
                name: Some(
                    override_title
                        .clone()
                        .unwrap_or_else(|| backup_name.to_string()),
                ),
                icon_path: Some(
                    icon.clone().map(|x| x.to_string_lossy().to_string()),
                ),
                content_set_patch: Some(AppliedContentSetPatch {
                    source_kind: None,
                    game_version: Some(minecraft_instance.game_version.clone()),
                    protocol_version: Some(None),
                    loader: Some(ModLoader::Vanilla),
                    loader_version: Some(None),
                }),
                ..EditInstance::default()
            },
        )
        .await?;
    }

    // Copy in contained folders as overrides
    finish_import(
        instance_id,
        curseforge_instance_folder,
        &state,
        reporter,
        details,
    )
    .await?;

    Ok(())
}

/// Reads names such as `forge-47.4.10`, `fabric-0.19.5-1.21.1`, `quilt-0.26.4-1.21.1`
/// and `neoforge-21.1.90`. Unknown names import as vanilla.
fn parse_loader_name(name: &str) -> (Option<ModLoader>, Option<String>) {
    match name.split('-').collect::<Vec<&str>>()[..] {
        ["forge", version] => {
            (Some(ModLoader::Forge), Some(version.to_string()))
        }
        ["fabric", version, _game_version] => {
            (Some(ModLoader::Fabric), Some(version.to_string()))
        }
        ["quilt", version, _game_version] => {
            (Some(ModLoader::Quilt), Some(version.to_string()))
        }
        ["neoforge", version] | ["neoforge", _, version] => {
            (Some(ModLoader::NeoForge), Some(version.to_string()))
        }
        _ => (None, None),
    }
}

#[cfg(test)]
mod loader_name_tests {
    use super::*;

    #[test]
    fn reads_each_loader() {
        assert_eq!(
            parse_loader_name("forge-47.4.10"),
            (Some(ModLoader::Forge), Some("47.4.10".to_string()))
        );
        assert_eq!(
            parse_loader_name("fabric-0.19.5-1.21.1"),
            (Some(ModLoader::Fabric), Some("0.19.5".to_string()))
        );
        assert_eq!(
            parse_loader_name("quilt-0.26.4-1.21.1"),
            (Some(ModLoader::Quilt), Some("0.26.4".to_string()))
        );
        assert_eq!(
            parse_loader_name("neoforge-21.1.90"),
            (Some(ModLoader::NeoForge), Some("21.1.90".to_string()))
        );
    }

    #[test]
    fn unknown_names_fall_back_to_vanilla() {
        assert_eq!(parse_loader_name("mystery"), (None, None));
    }
}
