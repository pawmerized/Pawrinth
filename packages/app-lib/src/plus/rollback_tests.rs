//! End-to-end rollback tests against a real launcher state in a temporary folder.
//! nextest runs each test in its own process, which fits the global `State`.

use super::manifest::RestoreStatus;
use super::operations::{self, OperationKind, OperationState};
use crate::state::instances::ContentSourceKind;
use crate::state::instances::commands::{
    ContentOrigin, CreateInstance, InstallContent, add_project_from_path,
    create_instance, install_stored_file, set_instance_install_stage,
    toggle_disable_project,
};
use crate::state::{
    InstanceInstallStage, InstanceLink, ModLoader, ProjectType, State,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct Harness {
    state: Arc<State>,
    instance_id: String,
    instance_dir: PathBuf,
    source_dir: PathBuf,
    /// Held so the folder lasts the whole test; crash tests reuse a parent's.
    _root: Option<tempfile::TempDir>,
}

/// Points the launcher at `root`. Must run before the first `State::init` in the
/// process, and each test owns its process under nextest.
async fn init_state(root: &Path) -> Arc<State> {
    assert!(
        !root.to_string_lossy().contains("ModrinthApp"),
        "tests must never use the official data folder"
    );
    crate::install_crypto_provider();
    // SAFETY: set before any other thread in this test process reads it.
    unsafe {
        std::env::set_var("THESEUS_CONFIG_DIR", root);
    }
    State::init("com.pawmerized.pawrinth.tests".to_string())
        .await
        .expect("launcher state");
    State::get().await.expect("launcher state")
}

impl Harness {
    async fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary folder");
        let mut harness = Self::in_folder(root.path()).await;
        harness._root = Some(root);
        harness
    }

    async fn in_folder(root: &Path) -> Self {
        let state = init_state(root).await;
        let source_dir = root.join("sources");
        std::fs::create_dir_all(&source_dir).unwrap();
        let instance = create_instance(
            CreateInstance {
                name: "rollback test".to_string(),
                path: None,
                game_version: "1.21.1".to_string(),
                loader: ModLoader::Vanilla,
                loader_version: None,
                icon_path: None,
                icon_config: None,
                link: InstanceLink::Unmanaged,
            },
            &state,
        )
        .await
        .expect("instance");
        set_instance_install_stage(
            &instance.id,
            InstanceInstallStage::Installed,
            &state.pool,
        )
        .await
        .expect("install stage");
        let instance_dir =
            state.directories.instances_dir().join(&instance.path);
        Self {
            state,
            instance_id: instance.id,
            instance_dir,
            source_dir,
            _root: None,
        }
    }

    fn write_source(&self, file_name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.source_dir.join(file_name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    async fn install(&self, file_name: &str, bytes: &[u8]) -> String {
        let source = self.write_source(file_name, bytes);
        add_project_from_path(
            &self.instance_id,
            &source,
            Some(ProjectType::Mod),
            &self.state,
        )
        .await
        .expect("install")
    }

    /// Replaces a file the way Update All does.
    async fn replace(
        &self,
        previous: &str,
        file_name: &str,
        bytes: &[u8],
    ) -> String {
        let source = self.write_source(file_name, bytes);
        let handle = self
            .state
            .content_store
            .store_file(&source)
            .await
            .expect("store new version");
        let requested = format!("mods/{file_name}");
        install_stored_file(
            &self.instance_id,
            InstallContent {
                requested_path: &requested,
                stored_file: &handle,
                project_type: ProjectType::Mod,
                source_kind: ContentSourceKind::Local,
                origin: None::<ContentOrigin<'_>>,
                enabled_override: None,
                previous_path: Some(previous),
            },
            &self.state,
        )
        .await
        .expect("replace")
    }

    async fn begin(&self) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        assert!(
            operations::begin_update(
                &id,
                &self.instance_id,
                OperationKind::UpdateAll,
                &self.state,
            )
            .await
            .expect("begin")
        );
        id
    }

    async fn finish(&self, id: &str, succeeded: bool) {
        operations::finish_update(
            id,
            &self.instance_id,
            succeeded,
            &self.state,
        )
        .await
        .expect("finish");
    }

    /// Every file in `mods/` with its bytes.
    fn mods(&self) -> BTreeMap<String, Vec<u8>> {
        let mods = self.instance_dir.join("mods");
        let Ok(entries) = std::fs::read_dir(&mods) else {
            return BTreeMap::new();
        };
        entries
            .map(|entry| {
                let entry = entry.unwrap();
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    std::fs::read(entry.path()).unwrap(),
                )
            })
            .collect()
    }

    async fn restore_all(
        &self,
        id: &str,
    ) -> crate::Result<operations::RestoreReport> {
        let ordinals: Vec<i64> = operations::preview_restore(id)
            .await?
            .iter()
            .map(|item| item.ordinal)
            .collect();
        operations::restore(id, &ordinals, &[]).await
    }

    async fn operation_state(&self, id: &str) -> OperationState {
        operations::list_operations(&self.instance_id)
            .await
            .unwrap()
            .into_iter()
            .find(|operation| operation.id == id)
            .map(|operation| operation.state)
            .unwrap_or(OperationState::Reverted)
    }
}

fn jar(label: &str) -> Vec<u8> {
    format!("not a real jar, just distinct bytes: {label}").into_bytes()
}

#[tokio::test]
async fn full_rollback_restores_exact_bytes_and_leaves_other_files() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    harness.install("b-1.jar", &jar("b1")).await;
    harness.install("unrelated.jar", &jar("unrelated")).await;
    let before = harness.mods();

    let id = harness.begin().await;
    harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
    harness.replace("mods/b-1.jar", "b-2.jar", &jar("b2")).await;
    harness.finish(&id, true).await;
    assert_ne!(harness.mods(), before);
    assert_eq!(harness.operation_state(&id).await, OperationState::Applied);

    let report = harness.restore_all(&id).await.unwrap();
    assert_eq!(report.restored.len(), 2);
    assert!(report.skipped.is_empty());
    assert_eq!(harness.mods(), before, "restored bytes must match exactly");
}

#[tokio::test]
async fn world_and_config_files_are_not_touched() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    let world = harness.instance_dir.join("saves/my world/level.dat");
    let config = harness.instance_dir.join("config/a.toml");
    std::fs::create_dir_all(world.parent().unwrap()).unwrap();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&world, b"precious world").unwrap();
    std::fs::write(&config, b"setting = 1").unwrap();

    let id = harness.begin().await;
    harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
    harness.finish(&id, true).await;
    std::fs::write(&world, b"played since the update").unwrap();
    std::fs::write(&config, b"setting = 2").unwrap();

    harness.restore_all(&id).await.unwrap();
    assert_eq!(std::fs::read(&world).unwrap(), b"played since the update");
    assert_eq!(std::fs::read(&config).unwrap(), b"setting = 2");
}

#[tokio::test]
async fn selective_rollback_restores_only_the_chosen_item() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    harness.install("b-1.jar", &jar("b1")).await;

    let id = harness.begin().await;
    harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
    harness.replace("mods/b-1.jar", "b-2.jar", &jar("b2")).await;
    harness.finish(&id, true).await;

    let preview = operations::preview_restore(&id).await.unwrap();
    let target = preview
        .iter()
        .find(|item| item.diff.before.as_ref().unwrap().file_name == "a-1.jar")
        .unwrap();
    operations::restore(&id, &[target.ordinal], &[])
        .await
        .unwrap();

    let mods = harness.mods();
    assert!(mods.contains_key("a-1.jar"));
    assert!(mods.contains_key("b-2.jar"));
    assert!(!mods.contains_key("a-2.jar"));
}

#[tokio::test]
async fn repeating_a_rollback_changes_nothing() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    let id = harness.begin().await;
    harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
    harness.finish(&id, true).await;

    harness.restore_all(&id).await.unwrap();
    let after_first = harness.mods();
    let second = harness.restore_all(&id).await.unwrap();
    assert!(second.restored.is_empty());
    assert_eq!(harness.mods(), after_first);
}

#[tokio::test]
async fn a_rollback_can_itself_be_undone() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    let id = harness.begin().await;
    harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
    harness.finish(&id, true).await;
    let updated = harness.mods();

    let report = harness.restore_all(&id).await.unwrap();
    let rollback_id = report.rollback_operation_id.unwrap();
    harness.restore_all(&rollback_id).await.unwrap();
    assert_eq!(harness.mods(), updated);
}

#[tokio::test]
async fn a_file_changed_since_the_update_is_not_overwritten() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    let id = harness.begin().await;
    harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
    harness.finish(&id, true).await;
    harness.replace("mods/a-2.jar", "a-3.jar", &jar("a3")).await;

    let preview = operations::preview_restore(&id).await.unwrap();
    assert_eq!(preview[0].status, RestoreStatus::RemovedSince);

    let report = harness.restore_all(&id).await.unwrap();
    assert!(report.restored.is_empty());
    assert_eq!(report.skipped.len(), 1);
    assert!(harness.mods().contains_key("a-3.jar"));
}

#[tokio::test]
async fn a_damaged_backup_stops_the_restore_before_any_change() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    harness.install("b-1.jar", &jar("b1")).await;
    let id = harness.begin().await;
    harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
    harness.replace("mods/b-1.jar", "b-2.jar", &jar("b2")).await;
    harness.finish(&id, true).await;
    let updated = harness.mods();

    let preview = operations::preview_restore(&id).await.unwrap();
    let damaged = preview
        .iter()
        .find(|item| item.diff.before.as_ref().unwrap().file_name == "b-1.jar")
        .unwrap();
    let blob = harness
        .state
        .content_store
        .lookup(Some(&damaged.diff.before.as_ref().unwrap().sha512), None)
        .await
        .unwrap()
        .unwrap();
    let blob_path = blob.path.clone();
    drop(blob);
    let mut permissions = std::fs::metadata(&blob_path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&blob_path, permissions).unwrap();
    std::fs::write(&blob_path, b"corrupted").unwrap();

    let preview = operations::preview_restore(&id).await.unwrap();
    assert!(
        preview
            .iter()
            .any(|item| item.status == RestoreStatus::BackupDamaged)
    );
    assert!(harness.restore_all(&id).await.is_err());
    assert_eq!(harness.mods(), updated, "nothing may change");
}

#[tokio::test]
async fn pinned_backups_survive_a_store_purge() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    let before = harness.mods();
    let id = harness.begin().await;
    harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
    harness.finish(&id, true).await;

    harness
        .state
        .content_store
        .cleanup(&harness.state, true)
        .await
        .unwrap();
    harness.restore_all(&id).await.unwrap();
    assert_eq!(harness.mods(), before);
}

#[tokio::test]
async fn deleting_an_operation_releases_its_backups() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    let id = harness.begin().await;
    harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
    harness.finish(&id, true).await;
    assert!(operations::storage_usage().await.unwrap().used_bytes > 0);

    operations::delete_operation(&id).await.unwrap();
    assert_eq!(operations::storage_usage().await.unwrap().used_bytes, 0);
    assert!(
        operations::list_operations(&harness.instance_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn an_update_that_changed_nothing_leaves_no_history() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    let id = harness.begin().await;
    harness.finish(&id, false).await;
    assert!(
        operations::list_operations(&harness.instance_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_partial_update_is_not_reported_as_applied() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    harness.install("b-1.jar", &jar("b1")).await;
    let before = harness.mods();
    let id = harness.begin().await;
    harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
    harness.finish(&id, false).await;
    assert_eq!(harness.operation_state(&id).await, OperationState::Partial);

    harness.restore_all(&id).await.unwrap();
    assert_eq!(harness.mods(), before);
}

#[tokio::test]
async fn disabled_state_set_after_the_update_is_kept() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    let id = harness.begin().await;
    harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
    harness.finish(&id, true).await;
    toggle_disable_project(
        &harness.instance_id,
        "mods/a-2.jar",
        Some(false),
        &harness.state,
    )
    .await
    .unwrap();

    harness.restore_all(&id).await.unwrap();
    assert_eq!(harness.mods().get("a-1.jar.disabled"), Some(&jar("a1")));
}

#[tokio::test]
async fn curseforge_origin_follows_update_and_rollback() {
    use super::curseforge::{forget_origin, record_origin};
    use super::manifest::ContentProvider;

    let harness = Harness::new().await;
    harness.install("cf-1.jar", &jar("cf1")).await;
    record_origin(
        &harness.instance_id,
        "mods/cf-1.jar",
        "curseforge:123",
        "1001",
        &harness.state,
    )
    .await
    .unwrap();
    let manifest = operations::instance_manifest(&harness.instance_id)
        .await
        .unwrap();
    assert_eq!(manifest.entries[0].provider, ContentProvider::CurseForge);
    assert_eq!(manifest.entries[0].version_id.as_deref(), Some("1001"));
    let before = harness.mods();

    let id = harness.begin().await;
    harness
        .replace("mods/cf-1.jar", "cf-2.jar", &jar("cf2"))
        .await;
    forget_origin(&harness.instance_id, "mods/cf-1.jar", &harness.state)
        .await
        .unwrap();
    record_origin(
        &harness.instance_id,
        "mods/cf-2.jar",
        "curseforge:123",
        "1002",
        &harness.state,
    )
    .await
    .unwrap();
    harness.finish(&id, true).await;

    let history = operations::list_operations(&harness.instance_id)
        .await
        .unwrap();
    let item = &history[0].items[0].diff;
    assert_eq!(
        item.before.as_ref().unwrap().version_id.as_deref(),
        Some("1001")
    );
    assert_eq!(
        item.after.as_ref().unwrap().version_id.as_deref(),
        Some("1002")
    );

    harness.restore_all(&id).await.unwrap();
    assert_eq!(harness.mods(), before);
    let restored = operations::instance_manifest(&harness.instance_id)
        .await
        .unwrap();
    assert_eq!(restored.entries[0].provider, ContentProvider::CurseForge);
    assert_eq!(restored.entries[0].version_id.as_deref(), Some("1001"));
}

#[tokio::test]
async fn snapshot_restores_after_later_changes() {
    let harness = Harness::new().await;
    harness.install("a-1.jar", &jar("a1")).await;
    harness.install("b-1.jar", &jar("b1")).await;
    let before = harness.mods();
    let snapshot =
        operations::create_snapshot(&harness.instance_id, "before experiments")
            .await
            .unwrap();

    harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
    harness.install("new.jar", &jar("new")).await;
    crate::state::instances::commands::remove_project(
        &harness.instance_id,
        "mods/b-1.jar",
        &harness.state,
    )
    .await
    .unwrap();

    harness.restore_all(&snapshot).await.unwrap();
    assert_eq!(harness.mods(), before);
}

#[cfg(feature = "plus-fault-injection")]
mod faults {
    use super::*;

    #[tokio::test]
    async fn a_failure_partway_returns_to_the_pre_rollback_state() {
        let harness = Harness::new().await;
        harness.install("a-1.jar", &jar("a1")).await;
        harness.install("b-1.jar", &jar("b1")).await;
        let id = harness.begin().await;
        harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
        harness.replace("mods/b-1.jar", "b-2.jar", &jar("b2")).await;
        harness.finish(&id, true).await;
        let updated = harness.mods();

        // SAFETY: only this test's process reads the variable.
        unsafe { std::env::set_var("PLUS_FAIL_AT", "restore-after-item") };
        let result = harness.restore_all(&id).await;
        unsafe { std::env::remove_var("PLUS_FAIL_AT") };

        assert!(result.is_err());
        assert_eq!(harness.mods(), updated);

        harness.restore_all(&id).await.unwrap();
        assert!(harness.mods().contains_key("a-1.jar"));
        assert!(harness.mods().contains_key("b-1.jar"));
    }

    const CHILD_ROOT: &str = "PLUS_TEST_CHILD_ROOT";

    /// Runs this same test in a child process that aborts during its restore, then
    /// starts up again here on the same folder and checks recovery.
    #[tokio::test]
    async fn a_crash_during_rollback_recovers_on_next_start() {
        if let Ok(root) = std::env::var(CHILD_ROOT) {
            let harness = Harness::in_folder(Path::new(&root)).await;
            harness.install("a-1.jar", &jar("a1")).await;
            harness.install("b-1.jar", &jar("b1")).await;
            let id = harness.begin().await;
            harness.replace("mods/a-1.jar", "a-2.jar", &jar("a2")).await;
            harness.replace("mods/b-1.jar", "b-2.jar", &jar("b2")).await;
            harness.finish(&id, true).await;
            std::fs::write(
                Path::new(&root).join("expected-instance"),
                harness.instance_dir.to_string_lossy().as_bytes(),
            )
            .unwrap();
            // SAFETY: only this process reads the variable.
            unsafe { std::env::set_var("PLUS_CRASH_AT", "restore-after-item") };
            let _ = harness.restore_all(&id).await;
            panic!("the child should have aborted during its restore");
        }

        let root = tempfile::tempdir().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "plus::rollback_tests::faults::a_crash_during_rollback_recovers_on_next_start",
                "--nocapture",
            ])
            .env(CHILD_ROOT, root.path())
            .status()
            .unwrap();
        assert!(!status.success(), "the child must not finish normally");

        let state = init_state(root.path()).await;
        let instance_dir = PathBuf::from(
            std::fs::read_to_string(root.path().join("expected-instance"))
                .unwrap(),
        );
        let mut names: Vec<String> =
            std::fs::read_dir(instance_dir.join("mods"))
                .unwrap()
                .map(|entry| {
                    entry.unwrap().file_name().to_string_lossy().into_owned()
                })
                .collect();
        names.sort();
        assert_eq!(names, ["a-2.jar", "b-2.jar"]);

        let unsettled = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM plus_content_operations WHERE state = 'applying'"
        )
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(unsettled, 0);
    }
}
