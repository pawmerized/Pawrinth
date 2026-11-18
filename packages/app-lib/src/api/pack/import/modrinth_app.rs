//! Instances from the official Modrinth App. Pawrinth never opens that app's data
//! folder with its own database code, because Pawrinth's migrations would stop the
//! official app from starting (see `plus::guard`). Instead its database files are
//! copied to a temporary folder and only the copy is read. Instance folders are
//! copied, never moved, so the Modrinth App keeps everything it had.

use std::path::{Path, PathBuf};

use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::{Connection, Row};

use crate::{
    ErrorKind, State,
    install::{InstallPhaseDetails, InstallProgressReporter},
    prelude::ModLoader,
    state::{AppliedContentSetPatch, EditInstance, InstanceInstallStage},
};

use super::{finish_import, ftb_app::has_mods, recache_icon};

const DATABASE: &str = "app.db";
const PROFILES: &str = "profiles";

/// A Modrinth App instance, as the import list shows it.
#[derive(Clone, Debug, Serialize)]
pub struct ModrinthAppInstance {
    pub folder: String,
    pub name: String,
    pub game_version: String,
    /// `None` when the Modrinth App uses a loader Pawrinth doesn't know.
    pub loader: Option<ModLoader>,
    #[serde(skip)]
    loader_version: Option<String>,
    #[serde(skip)]
    icon_path: Option<PathBuf>,
}

/// The official app's data folder, when it's on this computer.
pub fn default_path() -> Option<PathBuf> {
    let folder = dirs::data_dir()?.join("ModrinthApp");
    folder.join(DATABASE).is_file().then_some(folder)
}

/// Its database names each instance's folder, which must be one plain folder name.
fn plain_folder(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':'])
}

fn known_loader(name: &str) -> Option<ModLoader> {
    match name {
        "vanilla" => Some(ModLoader::Vanilla),
        "forge" => Some(ModLoader::Forge),
        "fabric" => Some(ModLoader::Fabric),
        "quilt" => Some(ModLoader::Quilt),
        "neoforge" => Some(ModLoader::NeoForge),
        _ => None,
    }
}

/// Copies the database and its write-ahead log, so the copy has everything the app
/// saved even while it's open, and the original is only ever read.
pub(crate) async fn copy_database(
    data_folder: &Path,
) -> crate::Result<(tempfile::TempDir, PathBuf)> {
    let copy = tempfile::tempdir()?;
    for suffix in ["", "-wal", "-shm"] {
        let name = format!("{DATABASE}{suffix}");
        let source = data_folder.join(&name);
        if tokio::fs::try_exists(&source).await.unwrap_or(false) {
            tokio::fs::copy(&source, copy.path().join(&name)).await?;
        }
    }
    let database = copy.path().join(DATABASE);
    Ok((copy, database))
}

/// Every instance the Modrinth App lists whose folder is still there, and the folder
/// that holds them.
async fn read_instances(
    data_folder: &Path,
) -> crate::Result<(PathBuf, Vec<ModrinthAppInstance>)> {
    if !data_folder.join(DATABASE).is_file() {
        return Err(ErrorKind::InputError(
            "this folder isn't the Modrinth App's".to_string(),
        )
        .into());
    }
    let (_copy, database) = copy_database(data_folder).await?;
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(&database),
    )
    .await?;
    let unreadable = |_| {
        ErrorKind::InputError(
            "this Modrinth App version's instances can't be read".to_string(),
        )
    };
    let custom_folder: Option<String> =
        sqlx::query("SELECT custom_dir FROM settings LIMIT 1")
            .fetch_optional(&mut connection)
            .await
            .ok()
            .flatten()
            .and_then(|row| {
                row.try_get::<Option<String>, _>("custom_dir")
                    .ok()
                    .flatten()
            });
    let rows = sqlx::query(
        "SELECT i.path, i.name, i.icon_path, s.game_version, s.loader, s.loader_version
         FROM instances i
         LEFT JOIN instance_content_sets s ON s.id = i.applied_content_set_id",
    )
    .fetch_all(&mut connection)
    .await
    .map_err(unreadable)?;
    connection.close().await.ok();

    let base = custom_folder
        .map(PathBuf::from)
        .filter(|folder| folder.join(PROFILES).is_dir())
        .unwrap_or_else(|| data_folder.to_path_buf());
    let profiles = base.join(PROFILES);
    let mut instances = Vec::new();
    for row in rows {
        let folder: String = row.try_get("path").map_err(unreadable)?;
        let game_version: Option<String> =
            row.try_get("game_version").map_err(unreadable)?;
        let (Some(game_version), true) = (game_version, plain_folder(&folder))
        else {
            continue;
        };
        if !profiles.join(&folder).is_dir() {
            continue;
        }
        let name: Option<String> = row.try_get("name").map_err(unreadable)?;
        let loader: Option<String> =
            row.try_get("loader").map_err(unreadable)?;
        instances.push(ModrinthAppInstance {
            name: name
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| folder.clone()),
            folder,
            game_version,
            loader: known_loader(loader.as_deref().unwrap_or("vanilla")),
            loader_version: row
                .try_get("loader_version")
                .map_err(unreadable)?,
            icon_path: row
                .try_get::<Option<String>, _>("icon_path")
                .map_err(unreadable)?
                .map(PathBuf::from),
        });
    }
    instances.sort_by_key(|instance| instance.name.to_lowercase());
    Ok((profiles, instances))
}

pub async fn list_instances(
    data_folder: &Path,
) -> crate::Result<Vec<ModrinthAppInstance>> {
    Ok(read_instances(data_folder).await?.1)
}

/// The name the instance has in the Modrinth App.
pub(crate) async fn instance_name(
    data_folder: &Path,
    folder: &str,
) -> Option<String> {
    let (_, instances) = read_instances(data_folder).await.ok()?;
    instances
        .into_iter()
        .find(|instance| instance.folder == folder)
        .map(|instance| instance.name)
}

pub async fn import_modrinth_app(
    data_folder: PathBuf,
    folder: String,
    instance_id: &str,
    reporter: InstallProgressReporter,
    details: InstallPhaseDetails,
) -> crate::Result<()> {
    let (profiles, instances) = read_instances(&data_folder).await?;
    let instance = instances
        .into_iter()
        .find(|instance| instance.folder == folder)
        .ok_or_else(|| {
            ErrorKind::InputError(
                "the Modrinth App doesn't have that instance any more"
                    .to_string(),
            )
        })?;
    let source = profiles.join(&instance.folder);
    let state = State::get().await?;
    let loader = match instance.loader {
        Some(loader) => loader,
        None if has_mods(&source).await => {
            return Err(ErrorKind::InputError(format!(
                "Pawrinth doesn't support the mod loader {} uses",
                instance.name
            ))
            .into());
        }
        None => ModLoader::Vanilla,
    };
    let loader_version = if loader == ModLoader::Vanilla {
        None
    } else {
        crate::launcher::get_loader_version_from_profile(
            &instance.game_version,
            loader,
            instance.loader_version.as_deref(),
            &state.directories,
        )
        .await?
        .map(|version| version.id)
    };
    let icon = match instance.icon_path {
        Some(path) => recache_icon(path).await.unwrap_or(None),
        None => None,
    };

    crate::api::instance::edit(
        instance_id,
        EditInstance {
            install_stage: (loader != ModLoader::Vanilla)
                .then_some(InstanceInstallStage::PackInstalling),
            name: Some(instance.name),
            icon_path: Some(
                icon.map(|path| path.to_string_lossy().to_string()),
            ),
            content_set_patch: Some(AppliedContentSetPatch {
                source_kind: None,
                game_version: Some(instance.game_version),
                protocol_version: Some(None),
                loader: Some(loader),
                loader_version: Some(loader_version),
            }),
            ..EditInstance::default()
        },
    )
    .await?;

    finish_import(instance_id, source, &state, reporter, details).await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fake_modrinth_app(folder: &Path) {
        tokio::fs::create_dir_all(folder.join(PROFILES).join("Fabric Pack"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(folder.join(PROFILES).join("Odd Loader"))
            .await
            .unwrap();
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(folder.join(DATABASE))
                .create_if_missing(true),
        )
        .await
        .unwrap();
        for statement in [
            "CREATE TABLE settings (id INTEGER, custom_dir TEXT)",
            "INSERT INTO settings VALUES (0, NULL)",
            "CREATE TABLE instances (id TEXT, path TEXT, applied_content_set_id INTEGER, name TEXT, icon_path TEXT)",
            "CREATE TABLE instance_content_sets (id INTEGER, game_version TEXT, loader TEXT, loader_version TEXT)",
            "INSERT INTO instances VALUES ('a', 'Fabric Pack', 1, 'My Fabric Pack', NULL)",
            "INSERT INTO instances VALUES ('b', 'Odd Loader', 2, 'Odd', NULL)",
            "INSERT INTO instances VALUES ('c', '..', 1, 'Escape', NULL)",
            "INSERT INTO instances VALUES ('d', 'Deleted', 1, 'Gone', NULL)",
            "INSERT INTO instance_content_sets VALUES (1, '1.21.1', 'fabric', '0.16.5')",
            "INSERT INTO instance_content_sets VALUES (2, '1.21.1', 'something-new', NULL)",
        ] {
            sqlx::query(statement)
                .execute(&mut connection)
                .await
                .unwrap();
        }
        connection.close().await.unwrap();
    }

    #[tokio::test]
    async fn lists_instances_from_a_copy_and_skips_unsafe_or_missing_folders() {
        let data = tempfile::tempdir().unwrap();
        fake_modrinth_app(data.path()).await;
        let before = tokio::fs::read(data.path().join(DATABASE)).await.unwrap();

        let instances = list_instances(data.path()).await.unwrap();

        let names: Vec<_> = instances
            .iter()
            .map(|instance| instance.name.as_str())
            .collect();
        assert_eq!(names, ["My Fabric Pack", "Odd"]);
        assert_eq!(instances[0].folder, "Fabric Pack");
        assert_eq!(instances[0].loader, Some(ModLoader::Fabric));
        assert_eq!(instances[0].loader_version.as_deref(), Some("0.16.5"));
        assert_eq!(instances[1].loader, None);
        assert_eq!(
            tokio::fs::read(data.path().join(DATABASE)).await.unwrap(),
            before
        );
    }

    #[tokio::test]
    async fn a_folder_without_the_database_is_refused() {
        let data = tempfile::tempdir().unwrap();
        assert!(list_instances(data.path()).await.is_err());
    }

    #[test]
    fn only_plain_folder_names_are_used() {
        assert!(plain_folder("Fabric 1.21.1"));
        assert!(!plain_folder(".."));
        assert!(!plain_folder("a/b"));
        assert!(!plain_folder("a\\b"));
        assert!(!plain_folder("C:"));
        assert!(!plain_folder(""));
    }
}
