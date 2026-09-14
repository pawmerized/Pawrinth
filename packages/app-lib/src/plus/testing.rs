use sqlx::SqlitePool;
use tempfile::TempDir;

/// A fully migrated app database in a temporary folder.
pub(crate) struct TestDb {
    pub pool: SqlitePool,
    _dir: TempDir,
}

impl TestDb {
    pub async fn new() -> Self {
        let dir = tempfile::tempdir().expect("temporary folder");
        let pool =
            crate::state::db::open_migrated_app_db(&dir.path().join("app.db"))
                .await
                .expect("migrated database");
        Self { pool, _dir: dir }
    }
}
