//! Whether a newer version of an instance's modpack is out, for packs from Modrinth,
//! CurseForge and Technic, so the library can say so without each instance being
//! opened. Nothing is downloaded or changed here.

use super::{curseforge, curseforge_pack, technic};
use crate::State;
use crate::state::InstanceLink;
use futures::StreamExt;
use serde::Serialize;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

const KEPT_FOR: Duration = Duration::from_secs(15 * 60);
const CONCURRENCY: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PackProvider {
    Modrinth,
    Curseforge,
    Technic,
}

#[derive(Clone, Debug, Serialize)]
pub struct PackUpdate {
    pub instance_id: String,
    pub provider: PackProvider,
    /// The newer version's name, as its site shows it.
    pub latest: String,
    /// The newer version's id on Modrinth or CurseForge. Technic has none.
    pub latest_id: Option<String>,
}

enum Installed {
    Modrinth,
    Curseforge { project_id: u64, file_id: u64 },
    Technic { version: String },
}

static KNOWN: LazyLock<Mutex<Option<(Instant, Vec<PackUpdate>)>>> =
    LazyLock::new(|| Mutex::new(None));

/// Makes the next check ask again, after a pack was installed or updated.
pub(super) fn forget() {
    if let Ok(mut known) = KNOWN.lock() {
        *known = None;
    }
}

/// CurseForge file ids only ever grow, so a higher id is a newer file.
fn is_newer_file(newest: u64, installed: u64) -> bool {
    newest > installed
}

async fn check_one(
    instance_id: String,
    installed: Installed,
) -> Option<PackUpdate> {
    let state = State::get().await.ok()?;
    match installed {
        Installed::Modrinth => {
            let info = crate::state::get_linked_modpack_info(
                &instance_id,
                None,
                None,
                &state,
            )
            .await
            .ok()??;
            let version = info.update_version.filter(|_| info.has_update)?;
            Some(PackUpdate {
                instance_id,
                provider: PackProvider::Modrinth,
                latest: version.version_number,
                latest_id: Some(version.id),
            })
        }
        Installed::Curseforge {
            project_id,
            file_id,
        } => {
            let project =
                curseforge::fetch_project(project_id, &state).await.ok()?;
            let game_version = crate::api::instance::get(&instance_id)
                .await
                .ok()??
                .applied_content_set
                .game_version;
            let newest = curseforge_pack::newest_pack_file(
                &project,
                Some(&game_version),
                &state,
            )
            .await
            .ok()?;
            is_newer_file(newest.id, file_id).then(|| PackUpdate {
                instance_id,
                provider: PackProvider::Curseforge,
                latest: newest.display_name,
                latest_id: Some(newest.id.to_string()),
            })
        }
        Installed::Technic { version } => {
            let latest = technic::latest_version(instance_id.clone()).await?;
            (latest != version).then(|| PackUpdate {
                instance_id,
                provider: PackProvider::Technic,
                latest,
                latest_id: None,
            })
        }
    }
}

/// Every instance whose modpack has a newer version. Sites that can't be reached are
/// left out rather than failing the rest, and answers are reused for a while unless
/// `force` asks again.
pub async fn check(force: bool) -> crate::Result<Vec<PackUpdate>> {
    if !force
        && let Ok(known) = KNOWN.lock()
        && let Some((checked, updates)) = known.as_ref()
        && checked.elapsed() < KEPT_FOR
    {
        return Ok(updates.clone());
    }
    let state = State::get().await?;
    let instances = crate::state::list_instances(&state.pool).await?;
    let mut from_curseforge =
        curseforge_pack::installed_packs().await.unwrap_or_default();
    // Asking how CurseForge is reached checks the key with CurseForge, so it's only asked
    // when there's a CurseForge pack to check.
    if !from_curseforge.is_empty()
        && curseforge::data_mode().await != curseforge::DataMode::Live
    {
        from_curseforge.clear();
    }
    let from_technic = technic::installed_packs().await.unwrap_or_default();

    let mut work = Vec::new();
    for metadata in instances {
        let id = metadata.instance.id;
        match metadata.link {
            InstanceLink::ModrinthModpack { .. } => {
                work.push((id, Installed::Modrinth));
            }
            InstanceLink::ImportedModpack { .. } => {
                if let Some(pack) =
                    from_curseforge.iter().find(|pack| pack.instance_id == id)
                {
                    let installed = Installed::Curseforge {
                        project_id: pack.project_id,
                        file_id: pack.file_id,
                    };
                    work.push((id, installed));
                } else if let Some(pack) =
                    from_technic.iter().find(|pack| pack.instance_id == id)
                {
                    let installed = Installed::Technic {
                        version: pack.version.clone(),
                    };
                    work.push((id, installed));
                }
            }
            _ => {}
        }
    }
    let updates: Vec<PackUpdate> = futures::stream::iter(work)
        .map(|(id, installed)| check_one(id, installed))
        .buffer_unordered(CONCURRENCY)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .flatten()
        .collect();
    if let Ok(mut known) = KNOWN.lock() {
        *known = Some((Instant::now(), updates.clone()));
    }
    Ok(updates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_higher_file_id_counts_as_newer() {
        assert!(is_newer_file(5001, 5000));
        assert!(!is_newer_file(5000, 5000));
        assert!(!is_newer_file(4999, 5000));
    }
}
