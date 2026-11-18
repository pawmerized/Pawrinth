//! Horizon's own resource packs and shader packs, from the registry Horizon's launcher
//! uses (<https://horizon-rust.github.io/horizon-mods/index.json>). Only versions made
//! for the instance's Minecraft version are offered, revoked files never are, and every
//! download is checked against the registry's size and SHA-256 before it's installed.
//! Files go where Horizon reads them: `resourcepacks/` and `shaderpacks/` in the game
//! folder, which is the instance folder.

use crate::ErrorKind;
use crate::state::ProjectType;
use crate::util::fetch::REQWEST_CLIENT;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

const INDEX_URL: &str =
    "https://horizon-rust.github.io/horizon-mods/index.json";
const PAGE_URL: &str = "https://playhorizon.dev/mods/";
const KEPT_FOR: Duration = Duration::from_secs(10 * 60);
const MAX_INDEX_BYTES: usize = 16 << 20;
const MAX_FILE_BYTES: u64 = 512 << 20;
/// The index format this reads. A newer one is refused rather than misread.
const INDEX_SCHEMA: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackKind {
    ResourcePack,
    Shader,
}

impl PackKind {
    fn registry_kind(self) -> &'static str {
        match self {
            Self::ResourcePack => "resourcepack",
            Self::Shader => "shader",
        }
    }

    fn folder(self) -> &'static str {
        match self {
            Self::ResourcePack => "resourcepacks",
            Self::Shader => "shaderpacks",
        }
    }

    fn project_type(self) -> ProjectType {
        match self {
            Self::ResourcePack => ProjectType::ResourcePack,
            Self::Shader => ProjectType::ShaderPack,
        }
    }

    fn accepts(self, file_name: &str) -> bool {
        let lower = file_name.to_ascii_lowercase();
        match self {
            Self::ResourcePack => lower.ends_with(".zip"),
            Self::Shader => {
                lower.ends_with(".hshader") || lower.ends_with(".zip")
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct Index {
    schema: u32,
    #[serde(default)]
    projects: Vec<IndexProject>,
    #[serde(default)]
    revoked: Vec<Revoked>,
}

#[derive(Clone, Debug, Deserialize)]
struct IndexProject {
    id: String,
    kind: String,
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    authors: Vec<String>,
    license: Option<String>,
    icon: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    screenshots: Vec<String>,
    #[serde(default)]
    versions: Vec<IndexVersion>,
    #[serde(default)]
    downloads: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct IndexVersion {
    version: String,
    game: Option<String>,
    /// When the version came out, as an RFC 3339 date.
    #[serde(default)]
    published: Option<String>,
    file: IndexFile,
}

#[derive(Clone, Debug, Deserialize)]
struct IndexFile {
    name: String,
    url: String,
    size: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
struct Revoked {
    id: String,
    /// Only this file is revoked; without it, the whole project is.
    sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HorizonPack {
    pub id: String,
    pub kind: PackKind,
    pub name: String,
    pub summary: String,
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub screenshots: Vec<String>,
    pub downloads: u64,
    pub version: String,
    pub size: u64,
    pub page_url: String,
    /// The file of this pack the instance has, when it has one.
    pub installed: Option<String>,
    /// Whether the installed file is an older version than `version`.
    pub update_available: bool,
    /// When `version` came out, and when the pack's first version did.
    pub updated: Option<String>,
    pub created: Option<String>,
}

static INDEX: LazyLock<Mutex<Option<(Instant, Arc<Index>)>>> =
    LazyLock::new(|| Mutex::new(None));

/// Ids and file names come from the registry, so they're checked before they name a file.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
        && name != ".."
}

async fn index() -> crate::Result<Arc<Index>> {
    if let Ok(known) = INDEX.lock()
        && let Some((fetched, index)) = known.as_ref()
        && fetched.elapsed() < KEPT_FOR
    {
        return Ok(index.clone());
    }
    let response = REQWEST_CLIENT
        .get(INDEX_URL)
        .send()
        .await?
        .error_for_status()?;
    let bytes = response.bytes().await?;
    if bytes.len() > MAX_INDEX_BYTES {
        bail!(ErrorKind::InputError(
            "Horizon's registry is larger than expected".to_string()
        ));
    }
    let index: Index = serde_json::from_slice(&bytes)?;
    if index.schema != INDEX_SCHEMA {
        bail!(ErrorKind::InputError(
            "Horizon's registry has changed format; update Pawrinth to browse it"
                .to_string()
        ));
    }
    let index = Arc::new(index);
    if let Ok(mut known) = INDEX.lock() {
        *known = Some((Instant::now(), index.clone()));
    }
    Ok(index)
}

fn is_revoked(index: &Index, id: &str, sha256: &str) -> bool {
    index.revoked.iter().any(|revoked| {
        revoked.id == id
            && revoked
                .sha256
                .as_deref()
                .is_none_or(|revoked| revoked.eq_ignore_ascii_case(sha256))
    })
}

/// The newest version made for this Minecraft version that isn't revoked. The registry
/// lists versions newest first.
fn chosen_version<'a>(
    index: &Index,
    project: &'a IndexProject,
    kind: PackKind,
    game_version: &str,
) -> Option<&'a IndexVersion> {
    project.versions.iter().find(|version| {
        version.game.as_deref() == Some(game_version)
            && plain_name(&version.file.name)
            && kind.accepts(&version.file.name)
            && !is_revoked(index, &project.id, &version.file.sha256)
    })
}

/// Files in the folder that belong to a project: only those named exactly like one of its
/// versions in the registry, so a file someone named after the pack is never taken for it.
fn installed_files(
    folder: &Path,
    project: &IndexProject,
    kind: PackKind,
) -> Vec<String> {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            let enabled = name.trim_end_matches(".disabled");
            kind.accepts(enabled)
                && project.versions.iter().any(|version| {
                    version.file.name.eq_ignore_ascii_case(enabled)
                })
        })
        .collect()
}

async fn instance_folder(
    instance_id: &str,
) -> crate::Result<(std::path::PathBuf, String)> {
    let Some(metadata) = crate::api::instance::get(instance_id).await? else {
        bail!(ErrorKind::InputError("unknown instance".to_string()));
    };
    Ok((
        crate::api::instance::get_full_path(instance_id).await?,
        metadata.applied_content_set.game_version,
    ))
}

/// Horizon's packs of one kind that run on this instance's Minecraft version, most
/// downloaded first, with what the instance already has.
pub async fn list(
    instance_id: &str,
    kind: PackKind,
) -> crate::Result<Vec<HorizonPack>> {
    let (instance_dir, game_version) = instance_folder(instance_id).await?;
    let index = index().await?;
    let folder = instance_dir.join(kind.folder());
    let mut packs: Vec<HorizonPack> = index
        .projects
        .iter()
        .filter(|project| {
            project.kind == kind.registry_kind() && plain_name(&project.id)
        })
        .filter_map(|project| {
            let version = chosen_version(&index, project, kind, &game_version)?;
            let installed =
                installed_files(&folder, project, kind).into_iter().next();
            let update_available = installed.as_deref().is_some_and(|name| {
                name.trim_end_matches(".disabled") != version.file.name
            });
            Some(HorizonPack {
                id: project.id.clone(),
                kind,
                name: project.name.clone(),
                summary: project.summary.clone(),
                authors: project.authors.clone(),
                license: project.license.clone(),
                icon: project.icon.clone(),
                categories: project.categories.clone(),
                screenshots: project.screenshots.clone(),
                downloads: project.downloads,
                version: version.version.clone(),
                updated: version.published.clone(),
                created: project
                    .versions
                    .iter()
                    .filter_map(|version| version.published.clone())
                    .min(),
                size: version.file.size,
                page_url: format!("{PAGE_URL}{}", project.id),
                installed,
                update_available,
            })
        })
        .collect();
    packs.sort_by(|a, b| b.downloads.cmp(&a.downloads));
    Ok(packs)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Downloads a pack's version for this instance, checks it, and installs it in place of
/// any older version of the same pack, which is removed only once the new one is in.
/// Returns the installed file's name.
pub async fn install(
    instance_id: &str,
    kind: PackKind,
    id: &str,
) -> crate::Result<String> {
    let (instance_dir, game_version) = instance_folder(instance_id).await?;
    let index = index().await?;
    let Some(project) = index.projects.iter().find(|project| {
        project.id == id && project.kind == kind.registry_kind()
    }) else {
        bail!(ErrorKind::InputError(
            "Horizon's registry doesn't list that pack".to_string()
        ));
    };
    let Some(version) = chosen_version(&index, project, kind, &game_version)
    else {
        bail!(ErrorKind::InputError(format!(
            "{} has no version for Minecraft {game_version}",
            project.name
        )));
    };
    if version.file.size > MAX_FILE_BYTES {
        bail!(ErrorKind::InputError(format!(
            "{} is larger than Pawrinth downloads",
            project.name
        )));
    }
    let bytes = REQWEST_CLIENT
        .get(&version.file.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if bytes.len() as u64 != version.file.size
        || !sha256_hex(&bytes).eq_ignore_ascii_case(&version.file.sha256)
    {
        bail!(ErrorKind::InputError(format!(
            "the download of {} didn't match Horizon's registry, so it wasn't installed",
            project.name
        )));
    }

    let folder = instance_dir.join(kind.folder());
    let older = installed_files(&folder, project, kind);
    let staging = tempfile::tempdir()?;
    let downloaded = staging.path().join(&version.file.name);
    tokio::fs::write(&downloaded, &bytes).await?;
    let added = crate::api::instance::add_project_from_path(
        instance_id,
        &downloaded,
        Some(kind.project_type()),
    )
    .await?;
    for name in older {
        let path = format!("{}/{name}", kind.folder());
        if path.trim_end_matches(".disabled")
            != added.trim_end_matches(".disabled")
        {
            let _ =
                crate::api::instance::remove_project(instance_id, &path).await;
        }
    }
    Ok(version.file.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str, versions: &[(&str, &str, &str)]) -> IndexProject {
        IndexProject {
            id: id.to_string(),
            kind: "shader".to_string(),
            name: id.to_string(),
            summary: String::new(),
            authors: Vec::new(),
            license: None,
            icon: None,
            categories: Vec::new(),
            screenshots: Vec::new(),
            versions: versions
                .iter()
                .map(|(game, name, sha256)| IndexVersion {
                    version: name.to_string(),
                    game: Some(game.to_string()),
                    published: None,
                    file: IndexFile {
                        name: name.to_string(),
                        url: format!("https://example.invalid/{name}"),
                        size: 1,
                        sha256: sha256.to_string(),
                    },
                })
                .collect(),
            downloads: 0,
        }
    }

    #[test]
    fn only_a_version_for_this_game_that_isnt_revoked_is_offered() {
        let aurora = project(
            "aurora",
            &[
                ("26.3", "aurora-2.hshader", "bad"),
                ("26.3", "aurora-1.hshader", "good"),
                ("26.2", "aurora-0.hshader", "old"),
            ],
        );
        let index = Index {
            schema: 1,
            projects: vec![aurora.clone()],
            revoked: vec![Revoked {
                id: "aurora".to_string(),
                sha256: Some("BAD".to_string()),
            }],
        };
        let chosen = chosen_version(&index, &aurora, PackKind::Shader, "26.3");
        assert_eq!(
            chosen.map(|v| v.file.name.as_str()),
            Some("aurora-1.hshader")
        );
        assert!(
            chosen_version(&index, &aurora, PackKind::Shader, "1.21.1")
                .is_none()
        );

        let whole = Index {
            revoked: vec![Revoked {
                id: "aurora".to_string(),
                sha256: None,
            }],
            ..index
        };
        assert!(
            chosen_version(&whole, &aurora, PackKind::Shader, "26.3").is_none()
        );
    }

    #[test]
    fn registry_names_never_leave_the_folder() {
        assert!(plain_name("aurora-5.9.3.hshader"));
        assert!(!plain_name("../aurora.hshader"));
        assert!(!plain_name("dir/aurora.hshader"));
        assert!(!plain_name(".hidden"));
        assert!(!PackKind::ResourcePack.accepts("pack.hshader"));
        assert!(PackKind::Shader.accepts("aurora.HSHADER"));
    }

    #[test]
    fn installed_files_are_found_by_the_registrys_names() {
        let folder = tempfile::tempdir().unwrap();
        for name in [
            "aurora-1.hshader",
            "aurora-2.hshader.disabled",
            "aurora-my-edit.hshader",
            "auroras-1.hshader",
            "aurora-1.txt",
        ] {
            std::fs::write(folder.path().join(name), b"x").unwrap();
        }
        let aurora = project(
            "aurora",
            &[
                ("26.3", "aurora-2.hshader", "b"),
                ("26.3", "aurora-1.hshader", "a"),
                ("26.3", "aurora-1.txt", "c"),
            ],
        );
        let mut found =
            installed_files(folder.path(), &aurora, PackKind::Shader);
        found.sort();
        assert_eq!(found, ["aurora-1.hshader", "aurora-2.hshader.disabled"]);
        assert_eq!(sha256_hex(b"abc").len(), 64);
    }
}
