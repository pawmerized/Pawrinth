//! A record of an instance's managed content, and pure comparisons between two
//! records. Rollback, snapshots and instance comparison all build on this.

use crate::state::instances::ContentSourceKind;
use crate::state::{ModLoader, ProjectType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Where a file came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentProvider {
    Modrinth,
    Local,
    #[serde(rename = "curseforge")]
    CurseForge,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub file_id: String,
    /// Canonical path, without any `.disabled` suffix.
    pub relative_path: String,
    pub file_name: String,
    pub enabled: bool,
    pub sha512: String,
    pub size: u64,
    pub provider: ContentProvider,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub project_type: ProjectType,
    pub source_kind: ContentSourceKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemChange {
    Updated,
    Added,
    Removed,
    Toggled,
}

impl ItemChange {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Updated => "updated",
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Toggled => "toggled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "updated" => Self::Updated,
            "added" => Self::Added,
            "removed" => Self::Removed,
            "toggled" => Self::Toggled,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ItemDiff {
    pub change: ItemChange,
    pub before: Option<ManifestEntry>,
    pub after: Option<ManifestEntry>,
}

/// Pairs entries by file id, then path, then project, so a renamed or re-created
/// file of the same project reads as an update rather than a removal and addition.
pub fn diff_manifests(
    before: &[ManifestEntry],
    after: &[ManifestEntry],
) -> Vec<ItemDiff> {
    let mut remaining: Vec<&ManifestEntry> = after.iter().collect();
    let mut pairs: Vec<(&ManifestEntry, Option<&ManifestEntry>)> =
        before.iter().map(|entry| (entry, None)).collect();

    let matchers: [fn(&ManifestEntry, &ManifestEntry) -> bool; 3] = [
        |a, b| a.file_id == b.file_id,
        |a, b| a.relative_path == b.relative_path,
        |a, b| a.project_id.is_some() && a.project_id == b.project_id,
    ];
    for matcher in matchers {
        for (entry, partner) in &mut pairs {
            if partner.is_some() {
                continue;
            }
            if let Some(index) =
                remaining.iter().position(|other| matcher(entry, other))
            {
                *partner = Some(remaining.remove(index));
            }
        }
    }

    let mut items = Vec::new();
    for (entry, partner) in pairs {
        match partner {
            None => items.push(ItemDiff {
                change: ItemChange::Removed,
                before: Some(entry.clone()),
                after: None,
            }),
            Some(other) => {
                let content_changed = entry.sha512 != other.sha512
                    || entry.relative_path != other.relative_path
                    || entry.version_id != other.version_id;
                let change = if content_changed {
                    ItemChange::Updated
                } else if entry.enabled != other.enabled {
                    ItemChange::Toggled
                } else {
                    continue;
                };
                items.push(ItemDiff {
                    change,
                    before: Some(entry.clone()),
                    after: Some(other.clone()),
                });
            }
        }
    }
    items.extend(remaining.into_iter().map(|entry| ItemDiff {
        change: ItemChange::Added,
        before: None,
        after: Some(entry.clone()),
    }));
    items
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreStatus {
    /// The file is still what this operation left behind.
    Restorable,
    /// The file already matches the earlier state.
    AlreadyRestored,
    /// The user or something else changed the file after the operation.
    ChangedSince,
    /// The file was removed after the operation.
    RemovedSince,
    /// A different file now occupies the path that would be restored.
    PathOccupied,
    /// The preserved copy is missing or failed its integrity check.
    BackupDamaged,
}

impl RestoreStatus {
    /// Conflicts stay unselected unless the user chooses to overwrite them.
    pub fn is_conflict(self) -> bool {
        matches!(
            self,
            Self::ChangedSince | Self::RemovedSince | Self::PathOccupied
        )
    }
}

fn find<'a>(
    current: &'a [ManifestEntry],
    path: &str,
) -> Option<&'a ManifestEntry> {
    current.iter().find(|entry| entry.relative_path == path)
}

pub fn classify_restore(
    item: &ItemDiff,
    current: &[ManifestEntry],
    backup_verified: bool,
) -> RestoreStatus {
    let after_current = item
        .after
        .as_ref()
        .and_then(|after| find(current, &after.relative_path));
    let before_current = item
        .before
        .as_ref()
        .and_then(|before| find(current, &before.relative_path));

    match (&item.before, &item.after) {
        (Some(before), after) => {
            if before_current.is_some_and(|entry| entry.sha512 == before.sha512)
            {
                return RestoreStatus::AlreadyRestored;
            }
            if !backup_verified {
                return RestoreStatus::BackupDamaged;
            }
            match after {
                Some(after) => match after_current {
                    Some(entry) if entry.sha512 == after.sha512 => {
                        if before.relative_path != after.relative_path
                            && before_current.is_some()
                        {
                            RestoreStatus::PathOccupied
                        } else {
                            RestoreStatus::Restorable
                        }
                    }
                    Some(_) => RestoreStatus::ChangedSince,
                    None => RestoreStatus::RemovedSince,
                },
                None if before_current.is_some() => RestoreStatus::PathOccupied,
                None => RestoreStatus::Restorable,
            }
        }
        (None, Some(after)) => match after_current {
            None => RestoreStatus::AlreadyRestored,
            Some(entry) if entry.sha512 == after.sha512 => {
                RestoreStatus::Restorable
            }
            Some(_) => RestoreStatus::ChangedSince,
        },
        (None, None) => RestoreStatus::AlreadyRestored,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VersionedManifest {
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManifestComparison {
    pub same: Vec<ManifestEntry>,
    pub only_left: Vec<ManifestEntry>,
    pub only_right: Vec<ManifestEntry>,
    pub different_versions: Vec<VersionDifference>,
    pub different_sources: Vec<VersionDifference>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VersionDifference {
    pub left: ManifestEntry,
    pub right: ManifestEntry,
}

fn identity(entry: &ManifestEntry) -> String {
    match &entry.project_id {
        Some(project_id) => format!("project:{project_id}"),
        None => format!("file:{}", entry.relative_path),
    }
}

/// Compares two manifests by project, or by path for files with no known project.
pub fn compare_manifests(
    left: &[ManifestEntry],
    right: &[ManifestEntry],
) -> ManifestComparison {
    let mut right_by_identity: HashMap<String, &ManifestEntry> =
        right.iter().map(|entry| (identity(entry), entry)).collect();
    let mut comparison = ManifestComparison::default();
    for entry in left {
        match right_by_identity.remove(&identity(entry)) {
            None => comparison.only_left.push(entry.clone()),
            Some(other) if other.sha512 == entry.sha512 => {
                comparison.same.push(entry.clone());
            }
            Some(other) => {
                let difference = VersionDifference {
                    left: entry.clone(),
                    right: other.clone(),
                };
                if other.provider == entry.provider {
                    comparison.different_versions.push(difference);
                } else {
                    comparison.different_sources.push(difference);
                }
            }
        }
    }
    comparison.only_right = right
        .iter()
        .filter(|entry| right_by_identity.contains_key(&identity(entry)))
        .cloned()
        .collect();
    comparison
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn entry(
        id: &str,
        path: &str,
        sha: &str,
        project: Option<&str>,
    ) -> ManifestEntry {
        ManifestEntry {
            file_id: id.to_string(),
            relative_path: path.to_string(),
            file_name: path.rsplit('/').next().unwrap().to_string(),
            enabled: true,
            sha512: sha.to_string(),
            size: 10,
            provider: if project.is_some() {
                ContentProvider::Modrinth
            } else {
                ContentProvider::Local
            },
            project_id: project.map(str::to_string),
            version_id: project.map(|_| format!("version-of-{sha}")),
            project_type: ProjectType::Mod,
            source_kind: ContentSourceKind::Local,
        }
    }

    #[test]
    fn unchanged_manifests_have_no_items() {
        let manifest = [entry("1", "mods/a.jar", "a", Some("A"))];
        assert!(diff_manifests(&manifest, &manifest).is_empty());
    }

    #[test]
    fn rename_with_same_file_id_is_one_update() {
        let before = [entry("1", "mods/a-1.jar", "a1", Some("A"))];
        let after = [entry("1", "mods/a-2.jar", "a2", Some("A"))];
        let items = diff_manifests(&before, &after);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].change, ItemChange::Updated);
    }

    #[test]
    fn recreated_file_of_same_project_is_one_update() {
        let before = [entry("1", "mods/a-1.jar", "a1", Some("A"))];
        let after = [entry("2", "mods/a-2.jar", "a2", Some("A"))];
        let items = diff_manifests(&before, &after);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].change, ItemChange::Updated);
    }

    #[test]
    fn unrelated_projects_stay_separate() {
        let before = [entry("1", "mods/a.jar", "a", Some("A"))];
        let after = [entry("2", "mods/b.jar", "b", Some("B"))];
        let changes: Vec<_> = diff_manifests(&before, &after)
            .iter()
            .map(|item| item.change)
            .collect();
        assert_eq!(changes, [ItemChange::Removed, ItemChange::Added]);
    }

    #[test]
    fn unrelated_entries_are_left_out() {
        let kept = entry("9", "mods/keep.jar", "k", Some("K"));
        let before = [kept.clone(), entry("1", "mods/a.jar", "a1", Some("A"))];
        let after = [kept, entry("1", "mods/a.jar", "a2", Some("A"))];
        let items = diff_manifests(&before, &after);
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].before.as_ref().unwrap().project_id.as_deref(),
            Some("A")
        );
    }

    #[test]
    fn enable_state_alone_is_a_toggle() {
        let before = [entry("1", "mods/a.jar", "a", Some("A"))];
        let mut changed = before[0].clone();
        changed.enabled = false;
        let items = diff_manifests(&before, &[changed]);
        assert_eq!(items[0].change, ItemChange::Toggled);
    }

    fn update_item() -> ItemDiff {
        ItemDiff {
            change: ItemChange::Updated,
            before: Some(entry("1", "mods/a-1.jar", "a1", Some("A"))),
            after: Some(entry("1", "mods/a-2.jar", "a2", Some("A"))),
        }
    }

    #[test]
    fn classifies_each_restore_state() {
        let item = update_item();
        let after = entry("1", "mods/a-2.jar", "a2", Some("A"));
        let before = entry("1", "mods/a-1.jar", "a1", Some("A"));
        let edited = entry("1", "mods/a-2.jar", "other", Some("A"));

        assert_eq!(
            classify_restore(&item, &[after.clone()], true),
            RestoreStatus::Restorable
        );
        assert_eq!(
            classify_restore(&item, &[before.clone()], true),
            RestoreStatus::AlreadyRestored
        );
        assert_eq!(
            classify_restore(&item, &[edited], true),
            RestoreStatus::ChangedSince
        );
        assert_eq!(
            classify_restore(&item, &[], true),
            RestoreStatus::RemovedSince
        );
        assert_eq!(
            classify_restore(&item, &[after], false),
            RestoreStatus::BackupDamaged
        );
    }

    #[test]
    fn already_restored_wins_over_a_damaged_backup() {
        let before = entry("1", "mods/a-1.jar", "a1", Some("A"));
        assert_eq!(
            classify_restore(&update_item(), &[before], false),
            RestoreStatus::AlreadyRestored
        );
    }

    #[test]
    fn added_dependency_restores_by_removing_it() {
        let added = entry("2", "mods/dep.jar", "d", Some("D"));
        let item = ItemDiff {
            change: ItemChange::Added,
            before: None,
            after: Some(added.clone()),
        };
        assert_eq!(
            classify_restore(&item, &[added], true),
            RestoreStatus::Restorable
        );
        assert_eq!(
            classify_restore(&item, &[], true),
            RestoreStatus::AlreadyRestored
        );
    }

    #[test]
    fn removed_file_restores_unless_its_path_is_taken() {
        let removed = entry("3", "mods/old.jar", "o", Some("O"));
        let item = ItemDiff {
            change: ItemChange::Removed,
            before: Some(removed),
            after: None,
        };
        assert_eq!(
            classify_restore(&item, &[], true),
            RestoreStatus::Restorable
        );
        let other = entry("4", "mods/old.jar", "x", None);
        assert_eq!(
            classify_restore(&item, &[other], true),
            RestoreStatus::PathOccupied
        );
    }

    #[test]
    fn compare_groups_by_project() {
        let left = [
            entry("1", "mods/shared.jar", "s", Some("S")),
            entry("2", "mods/a-1.jar", "a1", Some("A")),
            entry("3", "mods/only-left.jar", "l", Some("L")),
        ];
        let right = [
            entry("7", "mods/shared.jar", "s", Some("S")),
            entry("8", "mods/a-2.jar", "a2", Some("A")),
            entry("9", "mods/only-right.jar", "r", None),
        ];
        let comparison = compare_manifests(&left, &right);
        assert_eq!(comparison.same.len(), 1);
        assert_eq!(comparison.different_versions.len(), 1);
        assert_eq!(comparison.only_left[0].relative_path, "mods/only-left.jar");
        assert_eq!(
            comparison.only_right[0].relative_path,
            "mods/only-right.jar"
        );
    }
}
