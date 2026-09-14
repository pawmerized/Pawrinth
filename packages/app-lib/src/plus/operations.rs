//! Update history, rollback and snapshots.
//!
//! The content store keeps files by hash, so an operation pins the old blobs and
//! records what changed. Restoring goes through the same journaled install and
//! remove paths as any other content change, which makes each file crash safe.
//! A restore is first recorded as its own operation, so it can be reversed if it
//! fails partway or the app stops.

use super::manifest::{
    ContentProvider, ItemChange, ItemDiff, ManifestEntry, RestoreStatus,
    VersionedManifest, classify_restore, diff_manifests,
};
use super::settings::PlusSettings;
use crate::ErrorKind;
use crate::state::content_store::{
    content_file_path, file_path_on_disk, hash_file, instance_storage,
    is_managed_content_path,
};
use crate::state::instances::adapters::sqlite::{content_rows, instance_rows};
use crate::state::instances::commands::{
    ContentOrigin, InstallContent, install_stored_file, remove_project,
    rename_project_companion_file,
};
use crate::state::{InstanceInstallStage, ProjectType, State};
use anyhow::bail;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Retention owner kind. Separate from upstream's `rollback` kind, whose owners
/// are released when their backup folder no longer exists.
const RETAIN_KIND: &str = "plus-op";
const SAFETY_MARGIN_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    UpdateAll,
    UpdateOne,
    Rollback,
    Snapshot,
}

impl OperationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::UpdateAll => "update_all",
            Self::UpdateOne => "update_one",
            Self::Rollback => "rollback",
            Self::Snapshot => "snapshot",
        }
    }

    fn parse(value: &str) -> crate::Result<Self> {
        Ok(match value {
            "update_all" => Self::UpdateAll,
            "update_one" => Self::UpdateOne,
            "rollback" => Self::Rollback,
            "snapshot" => Self::Snapshot,
            other => bail!(ErrorKind::ContentStoreError(format!(
                "unknown operation kind {other}"
            ))),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Applying,
    Applied,
    Partial,
    Reverted,
    Captured,
    NeedsAttention,
}

impl OperationState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Applying => "applying",
            Self::Applied => "applied",
            Self::Partial => "partial",
            Self::Reverted => "reverted",
            Self::Captured => "captured",
            Self::NeedsAttention => "needs_attention",
        }
    }

    fn parse(value: &str) -> crate::Result<Self> {
        Ok(match value {
            "applying" => Self::Applying,
            "applied" => Self::Applied,
            "partial" => Self::Partial,
            "reverted" => Self::Reverted,
            "captured" => Self::Captured,
            "needs_attention" => Self::NeedsAttention,
            other => bail!(ErrorKind::ContentStoreError(format!(
                "unknown operation state {other}"
            ))),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub instance_id: String,
    pub kind: OperationKind,
    pub state: OperationState,
    pub parent_id: Option<String>,
    pub label: Option<String>,
    pub game_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
    pub items: Vec<OperationItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationItem {
    pub ordinal: i64,
    #[serde(flatten)]
    pub diff: ItemDiff,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RestorePreviewItem {
    pub ordinal: i64,
    pub status: RestoreStatus,
    #[serde(flatten)]
    pub diff: ItemDiff,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RestoreReport {
    pub restored: Vec<String>,
    pub skipped: Vec<String>,
    pub rollback_operation_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageUsage {
    pub used_bytes: u64,
    pub limit_bytes: u64,
    pub operations: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdatePreflight {
    pub required_bytes: u64,
    pub available_bytes: u64,
    pub enough_space: bool,
    pub rollback_enabled: bool,
    pub over_budget: bool,
    pub usage: StorageUsage,
}

struct InstanceSnapshot {
    instance: crate::state::Instance,
    manifest: VersionedManifest,
}

/// Reads an instance's managed content. With `pin_owner`, files not yet in the
/// content store are copied in and every blob is pinned for that owner.
async fn capture(
    instance_id: &str,
    pin_owner: Option<&str>,
    state: &State,
) -> crate::Result<InstanceSnapshot> {
    let _content_lock = state.instance_locks.lock_content(instance_id).await;
    let _store_lock = state.content_store.files_lock.lock().await;

    let instance = instance_rows::get_instance_by_id(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            ErrorKind::InputError(format!("unknown instance {instance_id}"))
        })?;
    let content_set_id =
        instance.applied_content_set_id.clone().ok_or_else(|| {
            ErrorKind::InputError(format!(
                "instance {instance_id} has no applied content set"
            ))
        })?;
    let content_set =
        content_rows::get_content_set(&content_set_id, &state.pool)
            .await?
            .ok_or_else(|| {
                ErrorKind::InputError(format!(
                    "instance {instance_id} has no applied content set"
                ))
            })?;

    let files =
        content_rows::get_instance_files(instance_id, &state.pool).await?;
    let entries =
        content_rows::get_content_entries(&content_set_id, &state.pool).await?;
    let bindings: HashMap<String, String> =
        instance_storage(&state.pool, instance_id)
            .await?
            .into_iter()
            .map(|binding| (binding.file_id, binding.blob_sha512))
            .collect();
    let origins = super::curseforge::origins(instance_id, state).await?;
    let entries_by_file: HashMap<&str, &crate::state::ContentEntry> = entries
        .iter()
        .filter_map(|entry| Some((entry.file_id.as_deref()?, entry)))
        .collect();

    let mut manifest_entries = Vec::new();
    let mut pins = Vec::new();
    for file in files {
        if file.missing || !is_managed_content_path(&file.relative_path) {
            continue;
        }
        let path = state
            .content_store
            .instance_path(&instance.path, &content_file_path(&file))
            .await?;
        let disk_exists = tokio::fs::try_exists(&path).await?;
        let sha512 = match bindings.get(&file.id) {
            Some(sha512) if !disk_exists => sha512.clone(),
            Some(sha512)
                if state
                    .content_store
                    .instance_file_matches(&path, sha512)
                    .await? =>
            {
                sha512.clone()
            }
            _ if !disk_exists => continue,
            _ if pin_owner.is_some() => {
                state.content_store.store_file(&path).await?.metadata.sha512
            }
            _ => hash_file(&path).await?.sha512,
        };
        pins.push(sha512.clone());

        let entry = entries_by_file.get(file.id.as_str());
        let modrinth_project = entry.and_then(|entry| entry.project_id.clone());
        let origin = modrinth_project
            .is_none()
            .then(|| origins.get(&file.relative_path))
            .flatten();
        let Some(project_type) =
            ProjectType::get_from_parent_folder(&file.relative_path)
        else {
            continue;
        };
        manifest_entries.push(ManifestEntry {
            file_id: file.id.clone(),
            relative_path: file.relative_path.clone(),
            file_name: file.file_name.clone(),
            enabled: file.enabled,
            sha512,
            size: file.size,
            provider: if modrinth_project.is_some() {
                ContentProvider::Modrinth
            } else if origin.is_some() {
                ContentProvider::CurseForge
            } else {
                ContentProvider::Local
            },
            project_id: modrinth_project
                .or_else(|| origin.map(|(project, _)| project.clone())),
            version_id: entry
                .and_then(|entry| entry.version_id.clone())
                .or_else(|| origin.map(|(_, file)| file.clone())),
            project_type,
            source_kind: entry.map_or(
                crate::state::instances::ContentSourceKind::Local,
                |entry| entry.source_kind,
            ),
        });
    }

    if let Some(owner) = pin_owner {
        state
            .content_store
            .retain(RETAIN_KIND, owner, &pins)
            .await?;
    }

    Ok(InstanceSnapshot {
        manifest: VersionedManifest {
            game_version: content_set.game_version.clone(),
            loader: content_set.loader,
            loader_version: content_set.loader_version.clone(),
            entries: manifest_entries,
        },
        instance,
    })
}

async fn insert_operation(
    id: &str,
    instance_id: &str,
    kind: OperationKind,
    state_value: OperationState,
    parent_id: Option<&str>,
    label: Option<&str>,
    scope: Option<&str>,
    manifest: &VersionedManifest,
    pool: &sqlx::SqlitePool,
) -> crate::Result<()> {
    let kind = kind.as_str();
    let state_value = state_value.as_str();
    let loader = manifest.loader.as_str();
    let manifest_json = serde_json::to_string(manifest)?;
    let now = Utc::now().timestamp();
    sqlx::query!(
        "
        INSERT INTO plus_content_operations
            (id, instance_id, kind, state, parent_id, label, game_version,
             loader, loader_version, manifest, scope, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ",
        id,
        instance_id,
        kind,
        state_value,
        parent_id,
        label,
        manifest.game_version,
        loader,
        manifest.loader_version,
        manifest_json,
        scope,
        now,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn set_operation_state(
    id: &str,
    value: OperationState,
    error: Option<&str>,
    pool: &sqlx::SqlitePool,
) -> crate::Result<()> {
    let value = value.as_str();
    let now = Utc::now().timestamp();
    sqlx::query!(
        "
        UPDATE plus_content_operations
        SET state = $2, last_error = $3, finished_at = $4
        WHERE id = $1
        ",
        id,
        value,
        error,
        now,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn write_items(
    id: &str,
    items: &[ItemDiff],
    pool: &sqlx::SqlitePool,
) -> crate::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query!(
        "DELETE FROM plus_content_operation_items WHERE operation_id = $1",
        id
    )
    .execute(&mut *tx)
    .await?;
    for (ordinal, item) in items.iter().enumerate() {
        let ordinal = ordinal as i64;
        let change = item.change.as_str();
        let before = item
            .before
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let after =
            item.after.as_ref().map(serde_json::to_string).transpose()?;
        sqlx::query!(
            "
            INSERT INTO plus_content_operation_items
                (operation_id, ordinal, change, before_entry, after_entry)
            VALUES ($1, $2, $3, $4, $5)
            ",
            id,
            ordinal,
            change,
            before,
            after,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn read_items(
    id: &str,
    pool: &sqlx::SqlitePool,
) -> crate::Result<Vec<OperationItem>> {
    let rows = sqlx::query!(
        r#"
        SELECT ordinal AS "ordinal!: i64", change, before_entry, after_entry
        FROM plus_content_operation_items
        WHERE operation_id = $1
        ORDER BY ordinal
        "#,
        id
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(OperationItem {
                ordinal: row.ordinal,
                diff: ItemDiff {
                    change: ItemChange::parse(&row.change).ok_or_else(
                        || {
                            ErrorKind::ContentStoreError(format!(
                                "unknown change {}",
                                row.change
                            ))
                        },
                    )?,
                    before: row
                        .before_entry
                        .as_deref()
                        .map(serde_json::from_str)
                        .transpose()?,
                    after: row
                        .after_entry
                        .as_deref()
                        .map(serde_json::from_str)
                        .transpose()?,
                },
            })
        })
        .collect()
}

async fn read_manifest(
    id: &str,
    pool: &sqlx::SqlitePool,
) -> crate::Result<VersionedManifest> {
    let raw = sqlx::query_scalar!(
        "SELECT manifest FROM plus_content_operations WHERE id = $1",
        id
    )
    .fetch_one(pool)
    .await?;
    Ok(serde_json::from_str(&raw)?)
}

async fn rollback_enabled(state: &State) -> crate::Result<bool> {
    Ok(PlusSettings::read(&state.pool).await?.rollback.enabled)
}

/// Records the instance before an update. Runs before any file is replaced; an
/// error here stops the update with nothing changed.
pub(crate) async fn begin_update(
    operation_id: &str,
    instance_id: &str,
    kind: OperationKind,
    state: &State,
) -> crate::Result<bool> {
    if !rollback_enabled(state).await? {
        return Ok(false);
    }
    let preflight = preflight_with_state(instance_id, state).await?;
    if !preflight.enough_space {
        bail!(ErrorKind::InputError(format!(
            "not enough disk space to update with recovery data: {} bytes needed, {} bytes free",
            preflight.required_bytes, preflight.available_bytes
        )));
    }
    let result: crate::Result<()> = async {
        let snapshot = capture(instance_id, Some(operation_id), state).await?;
        insert_operation(
            operation_id,
            instance_id,
            kind,
            OperationState::Applying,
            None,
            None,
            None,
            &snapshot.manifest,
            &state.pool,
        )
        .await
    }
    .await;
    if let Err(error) = result {
        let _ = state.content_store.release(RETAIN_KIND, operation_id).await;
        return Err(error);
    }
    Ok(true)
}

/// Records what the update changed and keeps only the blobs needed to undo it.
pub(crate) async fn finish_update(
    operation_id: &str,
    instance_id: &str,
    succeeded: bool,
    state: &State,
) -> crate::Result<()> {
    let before = read_manifest(operation_id, &state.pool).await?;
    let current = capture(instance_id, None, state).await?;
    let items = diff_manifests(&before.entries, &current.manifest.entries);
    let pins: Vec<String> = items
        .iter()
        .filter_map(|item| item.before.as_ref().map(|b| b.sha512.clone()))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    write_items(operation_id, &items, &state.pool).await?;
    if items.is_empty() {
        set_operation_state(
            operation_id,
            OperationState::Reverted,
            None,
            &state.pool,
        )
        .await?;
        state
            .content_store
            .release(RETAIN_KIND, operation_id)
            .await?;
    } else {
        state
            .content_store
            .replace_retained(RETAIN_KIND, operation_id, &pins)
            .await?;
        set_operation_state(
            operation_id,
            if succeeded {
                OperationState::Applied
            } else {
                OperationState::Partial
            },
            None,
            &state.pool,
        )
        .await?;
    }
    prune(instance_id, state).await
}

/// Called after a failed job has run its own cleanup. If that cleanup failed the
/// recovery data stays pinned, because it may be the only copy left.
pub(crate) async fn settle_failed_job(
    operation_id: &str,
    instance_id: Option<&str>,
    cleanup_succeeded: bool,
    state: &State,
) -> crate::Result<()> {
    let exists = sqlx::query_scalar!(
        "SELECT id FROM plus_content_operations WHERE id = $1 AND state = 'applying'",
        operation_id
    )
    .fetch_optional(&state.pool)
    .await?;
    let (Some(_), Some(instance_id)) = (exists, instance_id) else {
        return Ok(());
    };
    if cleanup_succeeded {
        finish_update(operation_id, instance_id, false, state).await
    } else {
        set_operation_state(
            operation_id,
            OperationState::NeedsAttention,
            Some("the update failed and its automatic cleanup did not finish"),
            &state.pool,
        )
        .await
    }
}

/// Settles operations that a crash left unfinished. Runs after upstream's job
/// recovery, so interrupted file changes and jobs have already been rolled back.
pub(crate) async fn reconcile_operations(state: &State) -> crate::Result<()> {
    let orphaned_instances = sqlx::query_scalar!(
        "
        SELECT DISTINCT id FROM plus_content_operations
        WHERE instance_id NOT IN (SELECT id FROM instances)
        "
    )
    .fetch_all(&state.pool)
    .await?;
    for id in orphaned_instances {
        remove_operation(&id, state).await?;
    }

    let orphaned_pins = sqlx::query_scalar!(
        "
        SELECT DISTINCT owner_id FROM store_retained_refs
        WHERE owner_kind = 'plus-op'
          AND owner_id NOT IN (SELECT id FROM plus_content_operations)
        "
    )
    .fetch_all(&state.pool)
    .await?;
    for owner in orphaned_pins {
        state.content_store.release(RETAIN_KIND, &owner).await?;
    }

    let unfinished = sqlx::query!(
        "
        SELECT id, instance_id, kind, parent_id, scope
        FROM plus_content_operations
        WHERE state = 'applying'
        "
    )
    .fetch_all(&state.pool)
    .await?;
    for row in unfinished {
        let id = row.id;
        let result = match OperationKind::parse(&row.kind)? {
            OperationKind::Rollback => {
                let scope = row
                    .scope
                    .as_deref()
                    .map(serde_json::from_str::<Vec<String>>)
                    .transpose()?
                    .unwrap_or_default();
                reverse_rollback(&id, &row.instance_id, &scope, state).await
            }
            _ => {
                let job_succeeded = sqlx::query_scalar!(
                    "SELECT status FROM install_jobs WHERE id = $1",
                    id
                )
                .fetch_optional(&state.pool)
                .await?
                .is_some_and(|status| status == "succeeded");
                finish_update(&id, &row.instance_id, job_succeeded, state).await
            }
        };
        if let Err(error) = result {
            tracing::error!(
                operation_id = %id,
                "Could not settle an interrupted operation: {error}"
            );
            set_operation_state(
                &id,
                OperationState::NeedsAttention,
                Some(&error.to_string()),
                &state.pool,
            )
            .await?;
        }
    }
    Ok(())
}

async fn ensure_can_change(
    instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    if crate::state::instance_has_running_process(instance_id, state).await? {
        bail!(ErrorKind::InputError(
            "stop this instance before restoring its content".to_string(),
        ));
    }
    let instance = instance_rows::get_instance_by_id(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            ErrorKind::InputError(format!("unknown instance {instance_id}"))
        })?;
    if instance.install_stage != InstanceInstallStage::Installed {
        bail!(ErrorKind::InputError(
            "wait for this instance's current install to finish".to_string(),
        ));
    }
    Ok(())
}

/// Full hash check of a preserved file, reading the bytes rather than the catalog.
async fn backup_is_intact(sha512: &str, state: &State) -> crate::Result<bool> {
    let Some(handle) = state.content_store.lookup(Some(sha512), None).await?
    else {
        return Ok(false);
    };
    match hash_file(&handle.path).await {
        Ok(hashes) => Ok(hashes.sha512 == sha512),
        Err(_) => Ok(false),
    }
}

async fn preview_items(
    items: &[OperationItem],
    instance_id: &str,
    state: &State,
) -> crate::Result<Vec<RestorePreviewItem>> {
    let current = capture(instance_id, None, state).await?;
    let mut verified: HashMap<String, bool> = HashMap::new();
    let mut preview = Vec::with_capacity(items.len());
    for item in items {
        let backup_verified = match &item.diff.before {
            Some(before) => match verified.get(&before.sha512) {
                Some(result) => *result,
                None => {
                    let result =
                        backup_is_intact(&before.sha512, state).await?;
                    verified.insert(before.sha512.clone(), result);
                    result
                }
            },
            None => true,
        };
        preview.push(RestorePreviewItem {
            ordinal: item.ordinal,
            status: classify_restore(
                &item.diff,
                &current.manifest.entries,
                backup_verified,
            ),
            diff: item.diff.clone(),
        });
    }
    Ok(preview)
}

pub async fn preview_restore(
    operation_id: &str,
) -> crate::Result<Vec<RestorePreviewItem>> {
    let state = State::get().await?;
    let operation = get_operation(operation_id, &state).await?;
    let items = restore_items_for(&operation, &state).await?;
    preview_items(&items, &operation.instance_id, &state).await
}

/// A snapshot's items are worked out against the instance as it is now, because
/// its target stays fixed while the instance moves on.
async fn restore_items_for(
    operation: &Operation,
    state: &State,
) -> crate::Result<Vec<OperationItem>> {
    if operation.kind != OperationKind::Snapshot {
        return Ok(operation.items.clone());
    }
    let target = read_manifest(&operation.id, &state.pool).await?;
    let current = capture(&operation.instance_id, None, state).await?;
    Ok(diff_manifests(&target.entries, &current.manifest.entries)
        .into_iter()
        .enumerate()
        .map(|(ordinal, diff)| OperationItem {
            ordinal: ordinal as i64,
            diff,
        })
        .collect())
}

fn modrinth_origin(entry: &ManifestEntry) -> Option<ContentOrigin<'_>> {
    if entry.provider != ContentProvider::Modrinth {
        return None;
    }
    entry
        .project_id
        .as_deref()
        .zip(entry.version_id.as_deref())
        .map(|(project_id, version_id)| ContentOrigin {
            project_id,
            version_id,
        })
}

/// Puts back the record of which CurseForge file this is, so a restored file still
/// gets update checks.
async fn restore_origin(
    instance_id: &str,
    entry: &ManifestEntry,
    state: &State,
) -> crate::Result<()> {
    if entry.provider != ContentProvider::CurseForge {
        return super::curseforge::forget_origin(
            instance_id,
            &entry.relative_path,
            state,
        )
        .await;
    }
    if let (Some(project), Some(file)) = (&entry.project_id, &entry.version_id)
    {
        super::curseforge::record_origin(
            instance_id,
            &entry.relative_path,
            project,
            file,
            state,
        )
        .await?;
    }
    Ok(())
}

async fn apply_item(
    instance_id: &str,
    item: &ItemDiff,
    current: &[ManifestEntry],
    state: &State,
) -> crate::Result<()> {
    match &item.before {
        None => {
            if let Some(after) = &item.after
                && current
                    .iter()
                    .any(|entry| entry.relative_path == after.relative_path)
            {
                remove_project(instance_id, &after.relative_path, state)
                    .await?;
                super::curseforge::forget_origin(
                    instance_id,
                    &after.relative_path,
                    state,
                )
                .await?;
            }
        }
        Some(before) => {
            let handle = state
                .content_store
                .lookup(Some(&before.sha512), None)
                .await?
                .ok_or_else(|| {
                    ErrorKind::ContentStoreError(format!(
                        "the preserved copy of {} is missing",
                        before.file_name
                    ))
                })?;
            let previous_path = item
                .after
                .as_ref()
                .map(|after| after.relative_path.as_str())
                .filter(|path| {
                    current.iter().any(|entry| entry.relative_path == *path)
                })
                .or_else(|| {
                    current
                        .iter()
                        .find(|entry| {
                            entry.relative_path == before.relative_path
                        })
                        .map(|entry| entry.relative_path.as_str())
                });
            install_stored_file(
                instance_id,
                InstallContent {
                    requested_path: &before.relative_path,
                    stored_file: &handle,
                    project_type: before.project_type,
                    source_kind: before.source_kind,
                    origin: modrinth_origin(before),
                    enabled_override: Some(before.enabled),
                    previous_path,
                },
                state,
            )
            .await?;
            if let Some(previous_path) = previous_path
                && previous_path != before.relative_path
            {
                super::curseforge::forget_origin(
                    instance_id,
                    previous_path,
                    state,
                )
                .await?;
            }
            restore_origin(instance_id, before, state).await?;
            if let Some(previous_path) = previous_path
                && previous_path != before.relative_path
            {
                rename_project_companion_file(
                    instance_id,
                    previous_path,
                    &before.relative_path,
                    state,
                )
                .await?;
            }
        }
    }
    Ok(())
}

/// Reads the bytes back: the restored file must match the preserved hash.
async fn verify_item(
    instance: &crate::state::Instance,
    item: &ItemDiff,
    state: &State,
) -> crate::Result<()> {
    let current =
        content_rows::get_instance_files(&instance.id, &state.pool).await?;
    if let Some(before) = &item.before {
        let file = current
            .iter()
            .find(|file| file.relative_path == before.relative_path)
            .ok_or_else(|| {
                ErrorKind::ContentStoreError(format!(
                    "{} isn't registered after restoring it",
                    before.file_name
                ))
            })?;
        let path = state
            .content_store
            .instance_path(
                &instance.path,
                &file_path_on_disk(&before.relative_path, file.enabled),
            )
            .await?;
        if hash_file(&path).await?.sha512 != before.sha512 {
            bail!(ErrorKind::ContentStoreError(format!(
                "{} doesn't match its preserved copy after restoring it",
                before.file_name
            )));
        }
    } else if let Some(after) = &item.after
        && current
            .iter()
            .any(|file| file.relative_path == after.relative_path)
    {
        bail!(ErrorKind::ContentStoreError(format!(
            "{} is still present after removing it",
            after.file_name
        )));
    }
    Ok(())
}

/// Returns the instance to the state recorded before a restore started.
async fn reverse_rollback(
    rollback_id: &str,
    instance_id: &str,
    scope: &[String],
    state: &State,
) -> crate::Result<()> {
    let target = read_manifest(rollback_id, &state.pool).await?;
    let current = capture(instance_id, None, state).await?;
    let in_scope = |entry: &ManifestEntry| {
        scope.iter().any(|path| path == &entry.relative_path)
    };
    let target_entries: Vec<_> = target
        .entries
        .iter()
        .filter(|e| in_scope(e))
        .cloned()
        .collect();
    let current_entries: Vec<_> = current
        .manifest
        .entries
        .iter()
        .filter(|e| in_scope(e))
        .cloned()
        .collect();
    for item in diff_manifests(&target_entries, &current_entries) {
        apply_item(instance_id, &item, &current.manifest.entries, state)
            .await?;
    }
    set_operation_state(
        rollback_id,
        OperationState::Reverted,
        None,
        &state.pool,
    )
    .await?;
    state.content_store.release(RETAIN_KIND, rollback_id).await
}

/// Restores the chosen items. Conflicts are only overwritten when listed in
/// `overwrite_conflicts`. Every preserved file is checked before anything changes.
pub async fn restore(
    operation_id: &str,
    ordinals: &[i64],
    overwrite_conflicts: &[i64],
) -> crate::Result<RestoreReport> {
    let state = State::get().await?;
    let operation = get_operation(operation_id, &state).await?;
    let instance_id = operation.instance_id.clone();
    ensure_can_change(&instance_id, &state).await?;

    let items = restore_items_for(&operation, &state).await?;
    let preview = preview_items(&items, &instance_id, &state).await?;
    let mut report = RestoreReport::default();
    let mut chosen = Vec::new();
    for item in preview {
        let name = item
            .diff
            .before
            .as_ref()
            .or(item.diff.after.as_ref())
            .map_or_else(String::new, |entry| entry.file_name.clone());
        if !ordinals.contains(&item.ordinal) {
            continue;
        }
        match item.status {
            RestoreStatus::Restorable => chosen.push(item),
            RestoreStatus::BackupDamaged => {
                bail!(ErrorKind::ContentStoreError(format!(
                    "the preserved copy of {name} is damaged, so nothing was restored"
                )));
            }
            status if status.is_conflict() => {
                if overwrite_conflicts.contains(&item.ordinal) {
                    chosen.push(item);
                } else {
                    report.skipped.push(name);
                }
            }
            _ => report.skipped.push(name),
        }
    }
    if chosen.is_empty() {
        return Ok(report);
    }

    let scope: Vec<String> = chosen
        .iter()
        .flat_map(|item| [&item.diff.before, &item.diff.after])
        .flatten()
        .map(|entry| entry.relative_path.clone())
        .collect();
    let rollback_id = Uuid::new_v4().to_string();
    let snapshot = capture(&instance_id, Some(&rollback_id), &state).await?;
    let scope_json = serde_json::to_string(&scope)?;
    insert_operation(
        &rollback_id,
        &instance_id,
        OperationKind::Rollback,
        OperationState::Applying,
        Some(operation_id),
        None,
        Some(&scope_json),
        &snapshot.manifest,
        &state.pool,
    )
    .await?;

    let apply: crate::Result<()> = async {
        let mut current = snapshot.manifest.entries.clone();
        for item in &chosen {
            apply_item(&instance_id, &item.diff, &current, &state).await?;
            verify_item(&snapshot.instance, &item.diff, &state).await?;
            super::fault::hit("restore-after-item");
            super::fault::fail_point("restore-after-item")?;
            current =
                capture(&instance_id, None, &state).await?.manifest.entries;
        }
        Ok(())
    }
    .await;

    if let Err(error) = apply {
        let message = error.to_string();
        match reverse_rollback(&rollback_id, &instance_id, &scope, &state).await
        {
            Ok(()) => {
                sqlx::query!(
                    "UPDATE plus_content_operations SET last_error = $2 WHERE id = $1",
                    operation_id,
                    message,
                )
                .execute(&state.pool)
                .await?;
            }
            Err(reverse_error) => {
                set_operation_state(
                    &rollback_id,
                    OperationState::NeedsAttention,
                    Some(&format!(
                        "{message}; returning to the earlier state also failed: {reverse_error}"
                    )),
                    &state.pool,
                )
                .await?;
            }
        }
        return Err(error);
    }

    let current = capture(&instance_id, None, &state).await?;
    let in_scope =
        |entry: &&ManifestEntry| scope.contains(&entry.relative_path);
    let before: Vec<_> = snapshot
        .manifest
        .entries
        .iter()
        .filter(in_scope)
        .cloned()
        .collect();
    let after: Vec<_> = current
        .manifest
        .entries
        .iter()
        .filter(in_scope)
        .cloned()
        .collect();
    let rollback_items = diff_manifests(&before, &after);
    write_items(&rollback_id, &rollback_items, &state.pool).await?;
    let pins: Vec<String> = rollback_items
        .iter()
        .filter_map(|item| item.before.as_ref().map(|b| b.sha512.clone()))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    state
        .content_store
        .replace_retained(RETAIN_KIND, &rollback_id, &pins)
        .await?;
    set_operation_state(
        &rollback_id,
        OperationState::Applied,
        None,
        &state.pool,
    )
    .await?;

    report.restored = chosen
        .iter()
        .filter_map(|item| {
            item.diff
                .before
                .as_ref()
                .or(item.diff.after.as_ref())
                .map(|entry| entry.file_name.clone())
        })
        .collect();
    report.rollback_operation_id = Some(rollback_id);
    prune(&instance_id, &state).await?;
    crate::event::emit::emit_instance(
        &instance_id,
        crate::event::InstancePayloadType::Edited,
    )
    .await?;
    Ok(report)
}

async fn get_operation(id: &str, state: &State) -> crate::Result<Operation> {
    let row = sqlx::query!(
        r#"
        SELECT id, instance_id, kind, state AS "op_state", parent_id, label,
            game_version, loader, loader_version, last_error,
            created_at AS "created_at!: i64", finished_at
        FROM plus_content_operations
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ErrorKind::InputError("unknown operation".to_string()))?;
    let items = read_items(&row.id, &state.pool).await?;
    Ok(Operation {
        id: row.id,
        instance_id: row.instance_id,
        kind: OperationKind::parse(&row.kind)?,
        state: OperationState::parse(&row.op_state)?,
        parent_id: row.parent_id,
        label: row.label,
        game_version: row.game_version,
        loader: row.loader,
        loader_version: row.loader_version,
        last_error: row.last_error,
        created_at: row.created_at,
        finished_at: row.finished_at,
        items,
    })
}

/// Newest first. Operations that changed nothing are left out.
pub async fn list_operations(
    instance_id: &str,
) -> crate::Result<Vec<Operation>> {
    let state = State::get().await?;
    let ids = sqlx::query_scalar!(
        "
        SELECT id FROM plus_content_operations
        WHERE instance_id = $1 AND state != 'reverted'
        ORDER BY created_at DESC, rowid DESC
        ",
        instance_id
    )
    .fetch_all(&state.pool)
    .await?;
    let mut operations = Vec::with_capacity(ids.len());
    for id in ids {
        operations.push(get_operation(&id, &state).await?);
    }
    Ok(operations)
}

async fn remove_operation(id: &str, state: &State) -> crate::Result<()> {
    sqlx::query!("DELETE FROM plus_content_operations WHERE id = $1", id)
        .execute(&state.pool)
        .await?;
    state.content_store.release(RETAIN_KIND, id).await
}

pub async fn delete_operation(id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    remove_operation(id, &state).await
}

/// Automatic cleanup is off by default. When on, it keeps the newest operation of
/// every instance, so the latest recovery copy is never removed behind the user.
async fn prune(instance_id: &str, state: &State) -> crate::Result<()> {
    let reverted = sqlx::query_scalar!(
        "
        SELECT id FROM plus_content_operations
        WHERE instance_id = $1 AND state = 'reverted'
        ",
        instance_id
    )
    .fetch_all(&state.pool)
    .await?;
    for id in reverted {
        remove_operation(&id, state).await?;
    }

    let settings = PlusSettings::read(&state.pool).await?.rollback;
    if !settings.auto_cleanup {
        return Ok(());
    }
    let cutoff = (settings.retention_days > 0).then(|| {
        Utc::now().timestamp() - i64::from(settings.retention_days) * 86_400
    });
    let rows = sqlx::query!(
        r#"
        SELECT id, created_at AS "created_at!: i64"
        FROM plus_content_operations
        WHERE instance_id = $1
          AND kind != 'snapshot'
          AND state IN ('applied', 'partial')
        ORDER BY created_at DESC, rowid DESC
        "#,
        instance_id
    )
    .fetch_all(&state.pool)
    .await?;
    for (index, row) in rows.iter().enumerate().skip(1) {
        let too_many = index >= settings.max_operations_per_instance as usize;
        let too_old = cutoff.is_some_and(|cutoff| row.created_at < cutoff);
        if too_many || too_old {
            remove_operation(&row.id, state).await?;
        }
    }
    Ok(())
}

async fn storage_usage_with_state(
    state: &State,
) -> crate::Result<StorageUsage> {
    let settings = PlusSettings::read(&state.pool).await?.rollback;
    let used = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(SUM(size), 0) AS "used!: i64" FROM store_blobs
        WHERE sha512 IN (
            SELECT blob_sha512 FROM store_retained_refs WHERE owner_kind = 'plus-op'
        )
        "#
    )
    .fetch_one(&state.pool)
    .await?;
    let operations = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!: i64" FROM plus_content_operations
        WHERE state != 'reverted'
        "#
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(StorageUsage {
        used_bytes: used as u64,
        limit_bytes: settings.max_storage_bytes,
        operations: operations as u64,
    })
}

pub async fn storage_usage() -> crate::Result<StorageUsage> {
    let state = State::get().await?;
    storage_usage_with_state(&state).await
}

async fn preflight_with_state(
    instance_id: &str,
    state: &State,
) -> crate::Result<UpdatePreflight> {
    let files =
        content_rows::get_instance_files(instance_id, &state.pool).await?;
    let installed: u64 = files
        .iter()
        .filter(|file| is_managed_content_path(&file.relative_path))
        .map(|file| file.size)
        .sum();
    // Updated files are roughly the size of what they replace, and each one is
    // written twice: to the shared store and into the instance.
    let required_bytes = installed * 2 + SAFETY_MARGIN_BYTES;
    let available_bytes =
        fs4::available_space(state.directories.config_dir.as_path())?;
    let usage = storage_usage_with_state(state).await?;
    Ok(UpdatePreflight {
        required_bytes,
        available_bytes,
        enough_space: available_bytes >= required_bytes,
        rollback_enabled: rollback_enabled(state).await?,
        over_budget: usage.used_bytes > usage.limit_bytes,
        usage,
    })
}

pub async fn preflight_update(
    instance_id: &str,
) -> crate::Result<UpdatePreflight> {
    let state = State::get().await?;
    preflight_with_state(instance_id, &state).await
}

/// Names a point-in-time copy of an instance's content. Worlds aren't included.
pub async fn create_snapshot(
    instance_id: &str,
    label: &str,
) -> crate::Result<String> {
    let state = State::get().await?;
    let id = Uuid::new_v4().to_string();
    let result: crate::Result<()> = async {
        let snapshot = capture(instance_id, Some(&id), &state).await?;
        insert_operation(
            &id,
            instance_id,
            OperationKind::Snapshot,
            OperationState::Captured,
            None,
            Some(label),
            None,
            &snapshot.manifest,
            &state.pool,
        )
        .await
    }
    .await;
    if let Err(error) = result {
        let _ = state.content_store.release(RETAIN_KIND, &id).await;
        return Err(error);
    }
    Ok(id)
}

pub async fn instance_manifest(
    instance_id: &str,
) -> crate::Result<VersionedManifest> {
    let state = State::get().await?;
    Ok(capture(instance_id, None, &state).await?.manifest)
}

pub async fn snapshot_manifest(id: &str) -> crate::Result<VersionedManifest> {
    let state = State::get().await?;
    read_manifest(id, &state.pool).await
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SyncReport {
    pub copied: Vec<String>,
    pub skipped: Vec<String>,
}

/// Copies chosen files from one instance into another with the same game version
/// and loader. The source instance is never changed.
pub async fn sync_from_instance(
    source_id: &str,
    target_id: &str,
    paths: &[String],
) -> crate::Result<SyncReport> {
    let state = State::get().await?;
    ensure_can_change(target_id, &state).await?;
    let source = capture(source_id, None, &state).await?;
    let target = capture(target_id, None, &state).await?;
    if source.manifest.game_version != target.manifest.game_version
        || source.manifest.loader != target.manifest.loader
    {
        bail!(ErrorKind::InputError(
            "these instances use a different game version or loader, so content can't be synchronized"
                .to_string(),
        ));
    }
    let mut report = SyncReport::default();
    for path in paths {
        let Some(entry) = source
            .manifest
            .entries
            .iter()
            .find(|entry| &entry.relative_path == path)
        else {
            report.skipped.push(path.clone());
            continue;
        };
        if target
            .manifest
            .entries
            .iter()
            .any(|existing| existing.relative_path == entry.relative_path)
        {
            report.skipped.push(entry.file_name.clone());
            continue;
        }
        let source_path = state
            .content_store
            .instance_path(
                &source.instance.path,
                &file_path_on_disk(&entry.relative_path, entry.enabled),
            )
            .await?;
        let handle = state.content_store.store_file(&source_path).await?;
        install_stored_file(
            target_id,
            InstallContent {
                requested_path: &entry.relative_path,
                stored_file: &handle,
                project_type: entry.project_type,
                source_kind: crate::state::instances::ContentSourceKind::Local,
                origin: modrinth_origin(entry),
                enabled_override: Some(entry.enabled),
                previous_path: None,
            },
            &state,
        )
        .await?;
        restore_origin(target_id, entry, &state).await?;
        report.copied.push(entry.file_name.clone());
    }
    crate::event::emit::emit_instance(
        target_id,
        crate::event::InstancePayloadType::Edited,
    )
    .await?;
    Ok(report)
}
