//! What an instance has installed, in terms both Modrinth and CurseForge can be checked
//! against, so a mod installed from one site shows as installed on the other. Files are
//! matched to CurseForge projects by fingerprint, the way the CurseForge app does it,
//! and every file also gives names: its mod ids and display name, and its project's
//! slug and title on either site. Those catch the same mod uploaded as a different file.

use super::curseforge::{self as cf, DataMode};
use super::curseforge_export::fingerprint;
use crate::state::content_store::is_managed_content_path;
use crate::state::instances::adapters::sqlite::{content_rows, instance_rows};
use crate::state::{CachedEntry, InstanceFile, State};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::sync::{LazyLock, Mutex, PoisonError};

/// Shorter names are too likely to belong to a different mod.
const MIN_NAME_LENGTH: usize = 3;

#[derive(Clone, Debug, Default, Serialize)]
pub struct InstalledIdentity {
    pub curseforge_ids: Vec<u64>,
    /// Lowercase letters and digits only, as `name_key` makes them.
    pub names: Vec<String>,
}

#[derive(Clone, Default)]
struct KnownFile {
    names: Vec<String>,
    /// Only worked out while CurseForge can be asked about it.
    fingerprint: Option<u32>,
    /// `None` until CurseForge was asked; `Some(None)` when it didn't know the file.
    curseforge_id: Option<Option<u64>>,
}

static FILES: LazyLock<Mutex<HashMap<String, KnownFile>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static CURSEFORGE_NAMES: LazyLock<Mutex<HashMap<u64, Vec<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// "Just Enough Items (JEI)" and "just-enough-items-jei" both become
/// "justenoughitemsjei".
pub fn name_key(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|char| char.to_ascii_lowercase())
        .collect()
}

fn push_name(names: &mut BTreeSet<String>, text: &str) {
    let key = name_key(text);
    if key.len() >= MIN_NAME_LENGTH {
        names.insert(key);
    }
}

/// The names come from the jar's metadata. The fingerprint reads the whole file, so it's
/// only worked out when asked for.
fn read_file(
    path: &std::path::Path,
    with_fingerprint: bool,
) -> (Vec<String>, Option<u32>) {
    let mut names = BTreeSet::new();
    if let Some(metadata) = super::mod_matching::read_metadata(path) {
        for id in &metadata.mod_ids {
            push_name(&mut names, id);
        }
        if let Some(name) = &metadata.name {
            push_name(&mut names, name);
        }
    }
    let fingerprint = with_fingerprint
        .then(|| std::fs::read(path).ok())
        .flatten()
        .map(|bytes| fingerprint(&bytes));
    (names.into_iter().collect(), fingerprint)
}

async fn curseforge_names(
    ids: &[u64],
    state: &State,
) -> HashMap<u64, Vec<String>> {
    let missing: Vec<u64> = {
        let known = CURSEFORGE_NAMES
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        ids.iter()
            .copied()
            .filter(|id| !known.contains_key(id))
            .collect()
    };
    if !missing.is_empty()
        && let Ok(projects) = cf::fetch_projects_by_id(&missing, state).await
    {
        let mut known = CURSEFORGE_NAMES
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for project in projects {
            let mut names = BTreeSet::new();
            push_name(&mut names, &project.slug);
            push_name(&mut names, &project.name);
            known.insert(project.id, names.into_iter().collect());
        }
    }
    let known = CURSEFORGE_NAMES
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    ids.iter()
        .filter_map(|id| Some((*id, known.get(id)?.clone())))
        .collect()
}

/// What this instance has installed. Files Pawrinth hasn't seen before are read once
/// and remembered by their hash; a site that can't be reached only leaves out what it
/// would have added.
pub async fn of_instance(
    instance_id: &str,
    state: &State,
) -> crate::Result<InstalledIdentity> {
    let Some(instance) =
        instance_rows::get_instance_by_id(instance_id, &state.pool).await?
    else {
        return Ok(InstalledIdentity::default());
    };
    let files: Vec<InstanceFile> =
        content_rows::get_instance_files(instance_id, &state.pool)
            .await?
            .into_iter()
            .filter(|file| {
                !file.missing
                    && is_managed_content_path(
                        file.relative_path.trim_end_matches(".disabled"),
                    )
            })
            .collect();

    let mut curseforge_ids = BTreeSet::new();
    let origins = cf::origins(instance_id, state).await?;
    let mut has_origin = std::collections::HashSet::new();
    for file in &files {
        let logical = file.relative_path.trim_end_matches(".disabled");
        if let Some(id) = origins
            .get(logical)
            .and_then(|(project, _)| project.strip_prefix("curseforge:"))
            .and_then(|id| id.parse::<u64>().ok())
        {
            curseforge_ids.insert(id);
            has_origin.insert(file.sha1.clone());
        }
    }

    let live = cf::data_mode().await == DataMode::Live;
    let wants_fingerprint =
        |file: &InstanceFile| live && !has_origin.contains(&file.sha1);
    let unread: Vec<(&InstanceFile, bool)> = {
        let known = FILES.lock().unwrap_or_else(PoisonError::into_inner);
        files
            .iter()
            .filter_map(|file| {
                let print = wants_fingerprint(file);
                match known.get(&file.sha1) {
                    None => Some((file, print)),
                    Some(entry) => (print
                        && entry.fingerprint.is_none()
                        && entry.curseforge_id.is_none())
                    .then_some((file, true)),
                }
            })
            .collect()
    };
    if !unread.is_empty() {
        let mut paths = Vec::with_capacity(unread.len());
        for (file, print) in &unread {
            paths.push((
                file.sha1.clone(),
                *print,
                state
                    .content_store
                    .instance_path(
                        &instance.path,
                        &crate::state::content_store::content_file_path(file),
                    )
                    .await?,
            ));
        }
        let read = tokio::task::spawn_blocking(move || {
            paths
                .into_iter()
                .map(|(sha1, print, path)| (sha1, read_file(&path, print)))
                .collect::<Vec<_>>()
        })
        .await?;
        let mut known = FILES.lock().unwrap_or_else(PoisonError::into_inner);
        for (sha1, (names, fingerprint)) in read {
            let entry = known.entry(sha1).or_default();
            entry.names = names;
            entry.fingerprint = fingerprint.or(entry.fingerprint);
        }
    }

    let unasked: Vec<(String, u32)> = if live {
        let known = FILES.lock().unwrap_or_else(PoisonError::into_inner);
        files
            .iter()
            .filter(|file| !has_origin.contains(&file.sha1))
            .filter_map(|file| {
                let entry = known.get(&file.sha1)?;
                entry
                    .curseforge_id
                    .is_none()
                    .then_some((file.sha1.clone(), entry.fingerprint?))
            })
            .collect()
    } else {
        Vec::new()
    };
    if !unasked.is_empty() {
        let wanted: Vec<u32> =
            unasked.iter().map(|(_, print)| *print).collect();
        if let Ok(found) = cf::fetch_fingerprint_matches(&wanted, state).await {
            let mut known =
                FILES.lock().unwrap_or_else(PoisonError::into_inner);
            for (sha1, print) in &unasked {
                if let Some(file) = known.get_mut(sha1) {
                    file.curseforge_id =
                        Some(found.get(print).map(|(project, _)| *project));
                }
            }
        }
    }

    let mut names = BTreeSet::new();
    {
        let known = FILES.lock().unwrap_or_else(PoisonError::into_inner);
        for file in &files {
            if let Some(file) = known.get(&file.sha1) {
                names.extend(file.names.iter().cloned());
                if let Some(Some(id)) = file.curseforge_id {
                    curseforge_ids.insert(id);
                }
            }
        }
    }

    let hashes: Vec<&str> =
        files.iter().map(|file| file.sha1.as_str()).collect();
    if let Ok(found) = CachedEntry::get_file_many(
        &hashes,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await
    {
        let ids: Vec<String> =
            found.into_iter().map(|file| file.project_id).collect();
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        if let Ok(projects) = CachedEntry::get_project_many(
            &ids,
            None,
            &state.pool,
            &state.api_semaphore,
        )
        .await
        {
            for project in projects {
                if let Some(slug) = &project.slug {
                    push_name(&mut names, slug);
                }
                push_name(&mut names, &project.title);
            }
        }
    }

    let ids: Vec<u64> = curseforge_ids.iter().copied().collect();
    if live {
        for project_names in curseforge_names(&ids, state).await.into_values() {
            names.extend(project_names);
        }
    }

    Ok(InstalledIdentity {
        curseforge_ids: ids,
        names: names.into_iter().collect(),
    })
}

/// Whether a project with this slug and name is already installed, by its names.
pub fn has_name(identity: &InstalledIdentity, slug: &str, name: &str) -> bool {
    [slug, name].iter().any(|text| {
        let key = name_key(text);
        key.len() >= MIN_NAME_LENGTH
            && identity.names.binary_search(&key).is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_however_a_site_writes_them() {
        assert_eq!(name_key("Just Enough Items (JEI)"), "justenoughitemsjei");
        assert_eq!(name_key("fabric-api"), "fabricapi");
        let identity = InstalledIdentity {
            curseforge_ids: Vec::new(),
            names: vec!["fabricapi".to_string(), "jei".to_string()],
        };
        assert!(has_name(&identity, "fabric-api", "Fabric API"));
        assert!(has_name(&identity, "jei", "Just Enough Items (JEI)"));
        assert!(!has_name(&identity, "jeiaddon", "JEI Addon"));
        assert!(!has_name(&identity, "", "Fa"));
    }
}
