//! Copying mods, resource packs, shaders and data packs from one instance to another.
//! Content from Modrinth or CurseForge is installed again in the other instance, so
//! it gets a version made for that instance's Minecraft version and loader, with its
//! dependencies. Files from elsewhere are copied as they are only when they fit
//! without a different version. Nothing is copied before it's planned and shown.

use super::curseforge::{self as cf, DataMode};
use super::installed_identity::{self, has_name};
use crate::ErrorKind;
use crate::state::instances::adapters::sqlite::content_rows;
use crate::state::instances::commands::{
    InstanceInstallProjectRequest, install_resolved_content_plan,
    resolve_install_plan,
};
use crate::state::{CachedEntry, ProjectType, State};
use anyhow::bail;
use futures::StreamExt;
use modrinth_content_management::{ContentType, ResolutionPreferences};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

const PLAN_CONCURRENCY: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyStatus {
    /// The same version works in the other instance.
    SameVersion,
    /// Another version of it is made for the other instance.
    OtherVersion,
    /// A file from elsewhere that's copied as it is.
    CopyFile,
    AlreadyInstalled,
    /// No version of it is made for the other instance.
    NoVersion,
    /// It came from CurseForge, which isn't set up.
    CurseforgeUnavailable,
    /// A world can't go to an older Minecraft version without risking damage.
    OlderGame,
    /// A world is copied only while its own instance isn't running, so the copy is whole.
    GameRunning,
}

#[derive(Clone, Debug, Serialize)]
pub struct CopyItem {
    pub path: String,
    pub name: String,
    pub status: CopyStatus,
    /// Turned off in the instance it's copied from, so the copy is turned off too.
    #[serde(skip)]
    pub disabled: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CopyReport {
    pub copied: Vec<String>,
    pub failed: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
enum Step {
    Modrinth {
        project_id: String,
        version_id: Option<String>,
        content_type: ContentType,
    },
    Curseforge {
        project_id: u64,
        file_id: u64,
    },
    File {
        project_type: ProjectType,
    },
    World {
        folder: String,
    },
    Nothing,
}

/// The folder of a world, from `saves/<folder>`.
fn world_folder(path: &str) -> Option<&str> {
    super::world_backups::plain_name(
        path.strip_prefix("saves/")?.trim_end_matches('/'),
    )
    .ok()
}

/// A name for the copied world that the other instance doesn't have yet.
fn free_world_name(saves: &std::path::Path, folder: &str) -> String {
    if !saves.join(folder).exists() {
        return folder.to_string();
    }
    (2..)
        .map(|number| format!("{folder} ({number})"))
        .find(|name| !saves.join(name).exists())
        .unwrap_or_else(|| folder.to_string())
}

/// Whether Minecraft `target` came out before `source`, by Modrinth's version list.
/// Versions it doesn't list count as not older.
async fn is_older(source: &str, target: &str, state: &State) -> bool {
    if source == target {
        return false;
    }
    let Ok(Some(versions)) =
        CachedEntry::get_game_versions(None, &state.pool, &state.api_semaphore)
            .await
    else {
        return false;
    };
    let date = |name: &str| {
        versions
            .iter()
            .find(|version| version.version == name)
            .map(|version| version.date.clone())
    };
    matches!(
        (date(source), date(target)),
        (Some(source), Some(target)) if target < source
    )
}

fn kind_of(path: &str) -> Option<(ProjectType, ContentType)> {
    let folder = path.split('/').next()?;
    Some(match folder {
        "mods" => (ProjectType::Mod, ContentType::Mod),
        "resourcepacks" => {
            (ProjectType::ResourcePack, ContentType::ResourcePack)
        }
        "shaderpacks" => (ProjectType::ShaderPack, ContentType::Shader),
        "datapacks" => (ProjectType::DataPack, ContentType::DataPack),
        _ => return None,
    })
}

/// A mod made for one loader version can't be assumed to work with another, but
/// resource packs, shaders and data packs usually load anywhere.
fn file_fits(
    project_type: ProjectType,
    same_game_version: bool,
    same_loader: bool,
) -> bool {
    project_type != ProjectType::Mod || (same_game_version && same_loader)
}

/// What both instances have, read once before each file is planned.
struct Known {
    target_id: String,
    target_version: String,
    target_loader: crate::state::ModLoader,
    same_game_version: bool,
    same_loader: bool,
    modrinth: HashMap<String, (String, String)>,
    versions: HashMap<String, crate::state::Version>,
    projects: HashMap<String, crate::state::Project>,
    origins: HashMap<String, (String, String)>,
    curseforge_live: bool,
    identity: installed_identity::InstalledIdentity,
    in_target: Vec<String>,
}

async fn plan_items(
    source_id: &str,
    target_id: &str,
    paths: &[String],
    state: &Arc<State>,
) -> crate::Result<Vec<(CopyItem, Step)>> {
    if source_id == target_id {
        bail!(ErrorKind::InputError(
            "choose a different instance to copy to".to_string()
        ));
    }
    let (Some(source), Some(target)) = (
        crate::api::instance::get(source_id).await?,
        crate::api::instance::get(target_id).await?,
    ) else {
        bail!(ErrorKind::InputError("unknown instance".to_string()));
    };
    let target_version = target.applied_content_set.game_version.clone();
    let target_loader = target.applied_content_set.loader;

    let files: HashMap<String, crate::state::InstanceFile> =
        content_rows::get_instance_files(source_id, &state.pool)
            .await?
            .into_iter()
            .map(|file| {
                (
                    file.relative_path
                        .trim_end_matches(".disabled")
                        .to_string(),
                    file,
                )
            })
            .collect();
    let wanted: Vec<&crate::state::InstanceFile> = paths
        .iter()
        .filter_map(|path| files.get(path.trim_end_matches(".disabled")))
        .collect();

    let hashes: Vec<&str> =
        wanted.iter().map(|file| file.sha1.as_str()).collect();
    let modrinth: HashMap<String, (String, String)> =
        CachedEntry::get_file_many(
            &hashes,
            None,
            &state.pool,
            &state.api_semaphore,
        )
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|file| (file.hash, (file.project_id, file.version_id)))
        .collect();
    let version_ids: Vec<&str> = modrinth
        .values()
        .map(|(_, version)| version.as_str())
        .collect();
    let versions: HashMap<String, crate::state::Version> =
        CachedEntry::get_version_many(
            &version_ids,
            None,
            &state.pool,
            &state.api_semaphore,
        )
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|version| (version.id.clone(), version))
        .collect();
    let project_ids: Vec<&str> = modrinth
        .values()
        .map(|(project, _)| project.as_str())
        .collect();
    let projects: HashMap<String, crate::state::Project> =
        CachedEntry::get_project_many(
            &project_ids,
            None,
            &state.pool,
            &state.api_semaphore,
        )
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|project| (project.id.clone(), project))
        .collect();

    let mut planned = Vec::new();
    let worlds: Vec<(&String, &str)> = paths
        .iter()
        .filter_map(|path| Some((path, world_folder(path)?)))
        .collect();
    if !worlds.is_empty() {
        let source_saves = crate::api::instance::get_full_path(source_id)
            .await?
            .join("saves");
        let older = is_older(
            &source.applied_content_set.game_version,
            &target_version,
            state,
        )
        .await;
        let running =
            crate::state::instance_has_running_process(source_id, state)
                .await?;
        for (path, folder) in worlds {
            if !source_saves.join(folder).is_dir() {
                continue;
            }
            let status = if older {
                CopyStatus::OlderGame
            } else if running {
                CopyStatus::GameRunning
            } else {
                CopyStatus::CopyFile
            };
            let step = if status == CopyStatus::CopyFile {
                Step::World {
                    folder: folder.to_string(),
                }
            } else {
                Step::Nothing
            };
            let item = CopyItem {
                path: path.clone(),
                name: folder.to_string(),
                status,
                disabled: false,
            };
            planned.push((item, step));
        }
    }

    let known = Known {
        target_id: target_id.to_string(),
        same_game_version: source.applied_content_set.game_version
            == target_version,
        same_loader: source.applied_content_set.loader == target_loader,
        target_version,
        target_loader,
        modrinth,
        versions,
        projects,
        origins: cf::origins(source_id, state).await.unwrap_or_default(),
        curseforge_live: cf::data_mode().await == DataMode::Live,
        identity: installed_identity::of_instance(target_id, state).await?,
        in_target: crate::state::get_installed_project_ids_for_instance(
            target_id, None, state,
        )
        .await?,
    };
    let known = Arc::new(known);
    let planned_files: Vec<Option<(CopyItem, Step)>> =
        futures::stream::iter(wanted.into_iter().cloned())
            .map(|file| plan_file(file, known.clone(), state.clone()))
            .buffered(PLAN_CONCURRENCY)
            .collect()
            .await;
    planned.extend(planned_files.into_iter().flatten());
    Ok(planned)
}

/// Owned arguments, because Tauri commands need futures that can move between threads.
async fn plan_file(
    file: crate::state::InstanceFile,
    known: Arc<Known>,
    state: Arc<State>,
) -> Option<(CopyItem, Step)> {
    let logical = file.relative_path.trim_end_matches(".disabled");
    let (project_type, content_type) = kind_of(logical)?;
    let item = |name: String, status: CopyStatus| CopyItem {
        path: file.relative_path.clone(),
        name,
        status,
        disabled: !file.enabled,
    };
    let as_file = |unavailable: CopyStatus| {
        if file_fits(project_type, known.same_game_version, known.same_loader) {
            (
                item(file.file_name.clone(), CopyStatus::CopyFile),
                Step::File { project_type },
            )
        } else {
            (item(file.file_name.clone(), unavailable), Step::Nothing)
        }
    };

    if let Some((project_id, version_id)) = known.modrinth.get(&file.sha1) {
        let project = known.projects.get(project_id);
        let name = project
            .map(|project| project.title.clone())
            .unwrap_or_else(|| file.file_name.clone());
        let slug = project
            .and_then(|project| project.slug.clone())
            .unwrap_or_default();
        if known.in_target.contains(project_id)
            || has_name(&known.identity, &slug, &name)
        {
            return Some((
                item(name, CopyStatus::AlreadyInstalled),
                Step::Nothing,
            ));
        }
        let fits = known.versions.get(version_id).is_some_and(|version| {
            version.game_versions.contains(&known.target_version)
                && (project_type != ProjectType::Mod
                    || version
                        .loaders
                        .iter()
                        .any(|loader| loader == known.target_loader.as_str()))
        });
        let (status, version) = if fits {
            (CopyStatus::SameVersion, Some(version_id.clone()))
        } else if resolve_install_plan(
            &known.target_id,
            InstanceInstallProjectRequest {
                project_id: project_id.clone(),
                version_id: None,
                content_type,
                selected: ResolutionPreferences::default(),
            },
            &state,
        )
        .await
        .is_ok()
        {
            (CopyStatus::OtherVersion, None)
        } else {
            (CopyStatus::NoVersion, None)
        };
        let step = if status == CopyStatus::NoVersion {
            Step::Nothing
        } else {
            Step::Modrinth {
                project_id: project_id.clone(),
                version_id: version,
                content_type,
            }
        };
        return Some((item(name, status), step));
    }

    let Some((project_id, file_id)) =
        known.origins.get(logical).and_then(|(project, file)| {
            Some((
                project.strip_prefix("curseforge:")?.parse::<u64>().ok()?,
                file.parse::<u64>().ok()?,
            ))
        })
    else {
        return Some(as_file(CopyStatus::NoVersion));
    };
    if known.identity.curseforge_ids.contains(&project_id) {
        return Some((
            item(file.file_name.clone(), CopyStatus::AlreadyInstalled),
            Step::Nothing,
        ));
    }
    if !known.curseforge_live {
        return Some(as_file(CopyStatus::CurseforgeUnavailable));
    }
    let name = cf::project(project_id)
        .await
        .map(|project| project.name)
        .unwrap_or_else(|_| file.file_name.clone());
    let options =
        cf::install_options(&known.target_id, project_id, false, None)
            .await
            .ok();
    let same = options.as_ref().is_some_and(|options| {
        options.files.iter().any(|option| option.file.id == file_id)
    });
    let other = options.and_then(|options| options.recommended_file_id);
    Some(match (same, other) {
        (true, _) => (
            item(name, CopyStatus::SameVersion),
            Step::Curseforge {
                project_id,
                file_id,
            },
        ),
        (false, Some(other)) => (
            item(name, CopyStatus::OtherVersion),
            Step::Curseforge {
                project_id,
                file_id: other,
            },
        ),
        (false, None) => (item(name, CopyStatus::NoVersion), Step::Nothing),
    })
}

/// What copying these files to another instance would do. Nothing is changed.
pub async fn plan(
    source_id: &str,
    target_id: &str,
    paths: Vec<String>,
) -> crate::Result<Vec<CopyItem>> {
    let state = State::get().await?;
    Ok(plan_items(source_id, target_id, &paths, &state)
        .await?
        .into_iter()
        .map(|(item, _)| item)
        .collect())
}

/// Copies what the plan says can be copied, one at a time. Disabled content stays
/// disabled in the other instance.
pub async fn copy(
    source_id: &str,
    target_id: &str,
    paths: Vec<String>,
) -> crate::Result<CopyReport> {
    let state = State::get().await?;
    let planned = plan_items(source_id, target_id, &paths, &state).await?;
    let source_dir = crate::api::instance::get_full_path(source_id).await?;
    let mut report = CopyReport::default();
    for (item, step) in planned {
        let copied: crate::Result<Option<String>> = match step {
            Step::Nothing => continue,
            Step::Modrinth {
                project_id,
                version_id,
                content_type,
            } => match resolve_install_plan(
                target_id,
                InstanceInstallProjectRequest {
                    project_id,
                    version_id,
                    content_type,
                    selected: ResolutionPreferences::default(),
                },
                &state,
            )
            .await
            {
                Ok(plan) => {
                    install_resolved_content_plan(target_id, &plan, &state)
                        .await
                        .map(|paths| paths.into_iter().next())
                }
                Err(error) => Err(error),
            },
            Step::Curseforge {
                project_id,
                file_id,
            } => {
                cf::install(target_id, project_id, file_id, false, false, None)
                    .await
                    .map(|report| Some(report.path))
            }
            Step::World { folder } => {
                let target_saves =
                    crate::api::instance::get_full_path(target_id)
                        .await?
                        .join("saves");
                let name = free_world_name(&target_saves, &folder);
                let from = source_dir.join("saves").join(&folder);
                let to = target_saves.join(&name);
                tokio::task::spawn_blocking(move || {
                    super::server_pack::copy_folder(&from, &to)
                })
                .await?
                .map(|()| None)
                .map_err(crate::Error::from)
            }
            Step::File { project_type } => {
                crate::api::instance::add_project_from_path(
                    target_id,
                    &source_dir.join(
                        crate::state::content_store::file_path_on_disk(
                            &item.path,
                            !item.disabled,
                        ),
                    ),
                    Some(project_type),
                )
                .await
                .map(Some)
            }
        };
        let result = match copied {
            Ok(Some(path)) if item.disabled => {
                crate::api::instance::toggle_disable_project(
                    target_id,
                    path.trim_end_matches(".disabled"),
                    Some(false),
                )
                .await
                .map(|_| ())
            }
            other => other.map(|_| ()),
        };
        match result {
            Ok(()) => report.copied.push(item.name),
            Err(error) => report.failed.push((item.name, error.to_string())),
        }
    }
    if !report.copied.is_empty() {
        let _ = crate::event::emit::emit_instance(
            target_id,
            crate::event::InstancePayloadType::Edited,
        )
        .await;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ModLoader;

    #[test]
    fn only_mods_need_a_matching_version_to_be_copied_as_files() {
        assert!(file_fits(ProjectType::ResourcePack, false, false));
        assert!(file_fits(ProjectType::ShaderPack, false, true));
        assert!(file_fits(ProjectType::Mod, true, true));
        assert!(!file_fits(ProjectType::Mod, true, false));
        assert!(!file_fits(ProjectType::Mod, false, true));
    }

    #[test]
    fn content_folders_decide_the_kind() {
        assert_eq!(
            kind_of("mods/a.jar").map(|kind| kind.0),
            Some(ProjectType::Mod)
        );
        assert_eq!(
            kind_of("shaderpacks/b.zip").map(|kind| kind.0),
            Some(ProjectType::ShaderPack)
        );
        assert!(kind_of("config/c.toml").is_none());
    }

    #[test]
    fn worlds_are_named_by_one_folder_and_never_overwritten() {
        assert_eq!(world_folder("saves/My World"), Some("My World"));
        assert_eq!(world_folder("saves/../x"), None);
        assert_eq!(world_folder("mods/a.jar"), None);
        let saves = tempfile::tempdir().unwrap();
        assert_eq!(free_world_name(saves.path(), "World"), "World");
        std::fs::create_dir(saves.path().join("World")).unwrap();
        std::fs::create_dir(saves.path().join("World (2)")).unwrap();
        assert_eq!(free_world_name(saves.path(), "World"), "World (3)");
    }

    #[test]
    fn loader_names_match_modrinth_loaders() {
        assert_eq!(ModLoader::NeoForge.as_str(), "neoforge");
    }
}
