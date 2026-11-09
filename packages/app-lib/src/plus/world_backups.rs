//! Scheduled backups of an instance's worlds, kept in Pawrinth's data folder beside the
//! instances and apart from content rollback. Each world has its own folder of zips, so
//! a backup always belongs to one instance and one world.
//!
//! - **Schedules** are checked every few minutes and when Pawrinth starts, so a backup
//!   missed while Pawrinth was closed happens soon after it opens. A world is only
//!   backed up again once it has changed, and never while Minecraft has it open.
//! - **Retention** removes the oldest automatic backups beyond the chosen count. Backups
//!   made by hand are never removed automatically.
//! - **Restoring** needs the instance to be closed. The current world is backed up first,
//!   the backup is unpacked beside it, and only then do the two swap places.

use super::settings::{PlusSettings, WorldBackupSchedule};
use crate::ErrorKind;
use crate::State;
use anyhow::bail;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

const BACKUPS_FOLDER: &str = "world-backups";
const SAVES_FOLDER: &str = "saves";
const LOCK_FILE: &str = "session.lock";
const TIME_FORMAT: &str = "%Y-%m-%d_%H-%M-%S";
const MANUAL_SUFFIX: &str = "_manual";
const BEFORE_RESTORE_SUFFIX: &str = "_before-restore";
/// Backups made just before a restore are kept this many times per world.
const KEEP_BEFORE_RESTORE: usize = 5;
const CHECK_EVERY: Duration = Duration::from_secs(5 * 60);
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    Automatic,
    Manual,
    BeforeRestore,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorldBackup {
    pub file_name: String,
    pub created: DateTime<Utc>,
    pub size: u64,
    pub kind: BackupKind,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorldBackups {
    /// The world's folder name in `saves`.
    pub world: String,
    /// Whether the world is still in the instance.
    pub exists: bool,
    /// Newest first.
    pub backups: Vec<WorldBackup>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InstanceBackups {
    pub schedule: WorldBackupSchedule,
    pub worlds: Vec<WorldBackups>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RestoreReport {
    /// The backup of the world as it was before the restore, if it existed.
    pub saved_current_as: Option<String>,
}

/// A world or backup name from outside, which has to be a single plain folder or file
/// name.
fn plain_name(name: &str) -> crate::Result<&str> {
    let mut components = Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None)
            if !name.contains(['/', '\\']) && !name.starts_with('.') =>
        {
            Ok(name)
        }
        _ => bail!(ErrorKind::InputError(format!("{name} isn't a valid name"))),
    }
}

fn instance_folder(instance_id: &str) -> String {
    instance_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn backups_root(state: &State) -> PathBuf {
    state.directories.config_dir.join(BACKUPS_FOLDER)
}

fn world_backup_dir(state: &State, instance_id: &str, world: &str) -> PathBuf {
    backups_root(state)
        .join(instance_folder(instance_id))
        .join(world)
}

fn backup_file_name(created: DateTime<Local>, kind: BackupKind) -> String {
    let suffix = match kind {
        BackupKind::Automatic => "",
        BackupKind::Manual => MANUAL_SUFFIX,
        BackupKind::BeforeRestore => BEFORE_RESTORE_SUFFIX,
    };
    format!("{}{suffix}.zip", created.format(TIME_FORMAT))
}

fn parse_backup_name(name: &str) -> Option<(DateTime<Utc>, BackupKind)> {
    let stem = name.strip_suffix(".zip")?;
    let (time, kind) = if let Some(time) = stem.strip_suffix(MANUAL_SUFFIX) {
        (time, BackupKind::Manual)
    } else if let Some(time) = stem.strip_suffix(BEFORE_RESTORE_SUFFIX) {
        (time, BackupKind::BeforeRestore)
    } else {
        (stem, BackupKind::Automatic)
    };
    let time = time.split(" (").next()?;
    let local = NaiveDateTime::parse_from_str(time, TIME_FORMAT).ok()?;
    let created = Local.from_local_datetime(&local).earliest()?;
    Some((created.with_timezone(&Utc), kind))
}

fn list_backups(dir: &Path) -> Vec<WorldBackup> {
    let mut backups: Vec<WorldBackup> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let (created, kind) = parse_backup_name(&file_name)?;
            let size = entry.metadata().ok()?.len();
            Some(WorldBackup {
                file_name,
                created,
                size,
                kind,
            })
        })
        .collect();
    backups.sort_by(|a, b| b.created.cmp(&a.created));
    backups
}

fn is_world(dir: &Path) -> bool {
    dir.join("level.dat").is_file()
}

fn instance_worlds(instance_path: &Path) -> Vec<String> {
    let mut worlds: Vec<String> =
        fs::read_dir(instance_path.join(SAVES_FOLDER))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| is_world(&entry.path()))
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| !name.starts_with('.'))
            .collect();
    worlds.sort_by_key(|name| name.to_lowercase());
    worlds
}

/// Holds Minecraft's world lock while a backup or restore reads the world. Fails while
/// Minecraft has the world open.
fn lock_world(world_dir: &Path) -> crate::Result<fs::File> {
    let lock = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(world_dir.join(LOCK_FILE))?;
    match lock.try_lock() {
        Ok(()) => Ok(lock),
        Err(fs::TryLockError::WouldBlock) => bail!(ErrorKind::InputError(
            "Minecraft has this world open, so close it first".to_string()
        )),
        Err(fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

fn newest_change(dir: &Path) -> Option<SystemTime> {
    let mut newest: Option<SystemTime> = None;
    let mut pending = vec![dir.to_path_buf()];
    while let Some(folder) = pending.pop() {
        for entry in fs::read_dir(&folder).ok()?.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() && entry.file_name() != LOCK_FILE {
                let modified = entry.metadata().ok()?.modified().ok()?;
                newest =
                    Some(newest.map_or(modified, |time| time.max(modified)));
            }
        }
    }
    newest
}

/// Zips one world, every dimension and file in it, into a new file in `dir`.
fn write_backup(
    world_dir: &Path,
    world: &str,
    dir: &Path,
    kind: BackupKind,
) -> crate::Result<WorldBackup> {
    let _lock = lock_world(world_dir)?;
    fs::create_dir_all(dir)?;
    let created = Local::now();
    let mut file_name = backup_file_name(created, kind);
    let mut count = 1;
    while dir.join(&file_name).exists() {
        count += 1;
        file_name = backup_file_name(created, kind)
            .replace(".zip", &format!(" ({count}).zip"));
    }
    let mut temporary = tempfile::NamedTempFile::new_in(dir)?;
    {
        let mut writer = zip::ZipWriter::new(temporary.as_file_mut());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        let mut pending = vec![world_dir.to_path_buf()];
        let mut buffer = vec![0; 256 * 1024];
        while let Some(folder) = pending.pop() {
            for entry in fs::read_dir(&folder)? {
                let entry = entry?;
                let file_type = entry.file_type()?;
                if file_type.is_dir() {
                    pending.push(entry.path());
                    continue;
                }
                if !file_type.is_file() || entry.file_name() == LOCK_FILE {
                    continue;
                }
                let path = entry.path();
                let relative = path
                    .strip_prefix(world_dir)?
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                writer.start_file(format!("{world}/{relative}"), options)?;
                let mut source = fs::File::open(&path)?;
                loop {
                    let read = source.read(&mut buffer)?;
                    if read == 0 {
                        break;
                    }
                    writer.write_all(&buffer[..read])?;
                }
            }
        }
        writer.finish()?;
    }
    let size = temporary.as_file().metadata()?.len();
    temporary
        .persist_noclobber(dir.join(&file_name))
        .map_err(|error| error.error)?;
    Ok(WorldBackup {
        file_name,
        created: created.with_timezone(&Utc),
        size,
        kind,
    })
}

/// Removes the oldest automatic backups beyond `keep`, and the oldest backups made
/// before restores beyond a few. Backups made by hand stay.
fn prune(dir: &Path, keep: usize) {
    let backups = list_backups(dir);
    for (kind, limit) in [
        (BackupKind::Automatic, keep),
        (BackupKind::BeforeRestore, KEEP_BEFORE_RESTORE),
    ] {
        for old in backups
            .iter()
            .filter(|backup| backup.kind == kind)
            .skip(limit)
        {
            if let Err(error) = fs::remove_file(dir.join(&old.file_name)) {
                tracing::warn!("Couldn't remove an old world backup: {error}");
            }
        }
    }
}

async fn schedule_of(instance_id: &str, state: &State) -> WorldBackupSchedule {
    PlusSettings::read(&state.pool)
        .await
        .ok()
        .and_then(|settings| {
            settings.world_backups.instances.get(instance_id).cloned()
        })
        .unwrap_or_default()
}

/// The instance's backup schedule and every world's backups, including worlds that have
/// since been deleted.
pub async fn list(instance_id: &str) -> crate::Result<InstanceBackups> {
    let state = State::get().await?;
    let instance_path =
        crate::api::instance::get_full_path(instance_id).await?;
    let root = backups_root(&state).join(instance_folder(instance_id));
    let present = instance_worlds(&instance_path);
    let schedule = schedule_of(instance_id, &state).await;
    let worlds = tokio::task::spawn_blocking(move || {
        let mut names = present.clone();
        for entry in fs::read_dir(&root).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_dir() && !names.contains(&name) {
                names.push(name);
            }
        }
        names
            .into_iter()
            .map(|world| WorldBackups {
                exists: present.contains(&world),
                backups: list_backups(&root.join(&world)),
                world,
            })
            .filter(|world| world.exists || !world.backups.is_empty())
            .collect::<Vec<_>>()
    })
    .await?;
    Ok(InstanceBackups { schedule, worlds })
}

pub async fn set_schedule(
    instance_id: &str,
    schedule: WorldBackupSchedule,
) -> crate::Result<WorldBackupSchedule> {
    let state = State::get().await?;
    if crate::api::instance::get(instance_id).await?.is_none() {
        bail!(ErrorKind::InputError("unknown instance".to_string()));
    }
    let schedule = schedule.normalized();
    let settings = PlusSettings::merge_patch(
        &serde_json::json!({ "world_backups": { "instances": {
            instance_id: schedule
        } } }),
        &state.pool,
    )
    .await?;
    Ok(settings
        .world_backups
        .instances
        .get(instance_id)
        .cloned()
        .unwrap_or(schedule))
}

pub async fn back_up_now(
    instance_id: &str,
    world: &str,
) -> crate::Result<WorldBackup> {
    let world = plain_name(world)?.to_string();
    let state = State::get().await?;
    let instance_path =
        crate::api::instance::get_full_path(instance_id).await?;
    let world_dir = instance_path.join(SAVES_FOLDER).join(&world);
    if !is_world(&world_dir) {
        bail!(ErrorKind::InputError(format!("{world} isn't a world")));
    }
    let dir = world_backup_dir(&state, instance_id, &world);
    tokio::task::spawn_blocking(move || {
        write_backup(&world_dir, &world, &dir, BackupKind::Manual)
    })
    .await?
}

pub async fn delete(
    instance_id: &str,
    world: &str,
    file_name: &str,
) -> crate::Result<()> {
    let world = plain_name(world)?;
    let file_name = plain_name(file_name)?;
    if parse_backup_name(file_name).is_none() {
        bail!(ErrorKind::InputError(
            "that isn't a world backup".to_string()
        ));
    }
    let state = State::get().await?;
    let path = world_backup_dir(&state, instance_id, world).join(file_name);
    tokio::fs::remove_file(&path).await?;
    Ok(())
}

/// Unpacks a backup's world folder into `target`, checking every path stays inside it.
fn unpack(backup: &Path, world: &str, target: &Path) -> crate::Result<()> {
    let mut archive =
        zip::ZipArchive::new(std::io::BufReader::new(fs::File::open(backup)?))?;
    let prefix = format!("{world}/");
    let mut found_level = false;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name()?.replace('\\', "/");
        let Some(relative) = name.strip_prefix(&prefix) else {
            bail!(ErrorKind::InputError(
                "this backup holds a different world".to_string()
            ));
        };
        let Some(safe) = entry.enclosed_name() else {
            bail!(ErrorKind::InputError(
                "this backup contains an unsafe path".to_string()
            ));
        };
        if !safe.starts_with(world) {
            bail!(ErrorKind::InputError(
                "this backup contains an unsafe path".to_string()
            ));
        }
        found_level |= relative == "level.dat";
        let destination = target.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::File::create(&destination)?;
        std::io::copy(&mut entry, &mut output)?;
    }
    if !found_level {
        bail!(ErrorKind::InputError(
            "this backup doesn't contain a world".to_string()
        ));
    }
    Ok(())
}

fn unique_beside(folder: &Path, label: &str) -> PathBuf {
    let parent = folder.parent().unwrap_or(folder);
    let base = folder
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut count = 0;
    loop {
        let candidate =
            parent.join(format!(".pawrinth-{label}-{base}-{count}"));
        if !candidate.exists() {
            return candidate;
        }
        count += 1;
    }
}

/// Puts a backup back as the world. The current world is backed up first, and the
/// restore only replaces it once the backup has been fully unpacked.
fn restore_blocking(
    world_dir: &Path,
    world: &str,
    backup: &Path,
    backup_dir: &Path,
) -> crate::Result<RestoreReport> {
    let saved_current_as = if is_world(world_dir) {
        Some(
            write_backup(
                world_dir,
                world,
                backup_dir,
                BackupKind::BeforeRestore,
            )?
            .file_name,
        )
    } else {
        None
    };
    let staging = unique_beside(world_dir, "restoring");
    if let Err(error) = unpack(backup, world, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let replaced = if world_dir.exists() {
        drop(lock_world(world_dir)?);
        let replaced = unique_beside(world_dir, "replaced");
        fs::rename(world_dir, &replaced)?;
        Some(replaced)
    } else {
        None
    };
    if let Err(error) = fs::rename(&staging, world_dir) {
        if let Some(replaced) = &replaced {
            let _ = fs::rename(replaced, world_dir);
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(error.into());
    }
    if let Some(replaced) = replaced
        && let Err(error) = fs::remove_dir_all(&replaced)
    {
        tracing::warn!("Couldn't tidy up the replaced world: {error}");
    }
    prune(backup_dir, usize::MAX);
    Ok(RestoreReport { saved_current_as })
}

pub async fn restore(
    instance_id: &str,
    world: &str,
    file_name: &str,
) -> crate::Result<RestoreReport> {
    let world = plain_name(world)?.to_string();
    let file_name = plain_name(file_name)?.to_string();
    let state = State::get().await?;
    if crate::state::instance_has_running_process(instance_id, &state).await? {
        bail!(ErrorKind::InputError(
            "close Minecraft before restoring a world".to_string()
        ));
    }
    let instance_path =
        crate::api::instance::get_full_path(instance_id).await?;
    let world_dir = instance_path.join(SAVES_FOLDER).join(&world);
    let backup_dir = world_backup_dir(&state, instance_id, &world);
    let backup = backup_dir.join(&file_name);
    if parse_backup_name(&file_name).is_none() || !backup.is_file() {
        bail!(ErrorKind::InputError(
            "that backup doesn't exist".to_string()
        ));
    }
    tokio::fs::create_dir_all(instance_path.join(SAVES_FOLDER)).await?;
    tokio::task::spawn_blocking(move || {
        restore_blocking(&world_dir, &world, &backup, &backup_dir)
    })
    .await?
}

/// Backs up every world of every instance whose schedule says it's due.
async fn run_due(state: &State) -> crate::Result<()> {
    let settings = PlusSettings::read(&state.pool).await?;
    for (instance_id, schedule) in settings.world_backups.instances {
        if !schedule.enabled {
            continue;
        }
        let Ok(instance_path) =
            crate::api::instance::get_full_path(&instance_id).await
        else {
            continue;
        };
        let root = backups_root(state).join(instance_folder(&instance_id));
        let interval =
            chrono::Duration::hours(i64::from(schedule.interval_hours));
        let keep = schedule.keep as usize;
        let result = tokio::task::spawn_blocking(move || {
            for world in instance_worlds(&instance_path) {
                let world_dir = instance_path.join(SAVES_FOLDER).join(&world);
                let dir = root.join(&world);
                let last = list_backups(&dir)
                    .into_iter()
                    .find(|backup| backup.kind == BackupKind::Automatic);
                if let Some(last) = &last {
                    if Utc::now() - last.created < interval {
                        continue;
                    }
                    let changed =
                        newest_change(&world_dir).is_some_and(|time| {
                            DateTime::<Utc>::from(time) > last.created
                        });
                    if !changed {
                        continue;
                    }
                }
                match write_backup(
                    &world_dir,
                    &world,
                    &dir,
                    BackupKind::Automatic,
                ) {
                    Ok(_) => prune(&dir, keep),
                    Err(error) => {
                        tracing::info!(
                            "Skipped backing up {world} for now: {error}"
                        );
                    }
                }
            }
        })
        .await;
        if let Err(error) = result {
            tracing::warn!("World backups stopped early: {error}");
        }
    }
    Ok(())
}

/// Checks the schedules shortly after start and then every few minutes.
pub(crate) fn start(state: &'static State) {
    tokio::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_AFTER).await;
        loop {
            if let Err(error) = run_due(state).await {
                tracing::warn!("Couldn't check world backups: {error}");
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_world(root: &Path, name: &str) -> PathBuf {
        let world = root.join(SAVES_FOLDER).join(name);
        fs::create_dir_all(world.join("region")).unwrap();
        fs::create_dir_all(world.join("DIM-1").join("region")).unwrap();
        fs::create_dir_all(world.join("DIM1").join("region")).unwrap();
        fs::write(world.join("level.dat"), b"level").unwrap();
        fs::write(world.join("region").join("r.0.0.mca"), b"overworld")
            .unwrap();
        fs::write(
            world.join("DIM-1").join("region").join("r.0.0.mca"),
            b"nether",
        )
        .unwrap();
        fs::write(world.join("DIM1").join("region").join("r.0.0.mca"), b"end")
            .unwrap();
        world
    }

    #[test]
    fn names_round_trip() {
        let created = Local::now();
        for kind in [
            BackupKind::Automatic,
            BackupKind::Manual,
            BackupKind::BeforeRestore,
        ] {
            let name = backup_file_name(created, kind);
            let (parsed, parsed_kind) = parse_backup_name(&name).unwrap();
            assert_eq!(parsed_kind, kind);
            assert_eq!(
                parsed.timestamp(),
                created.with_timezone(&Utc).timestamp()
            );
        }
        assert!(parse_backup_name("notes.txt").is_none());
        assert!(parse_backup_name("2024-01-01_10-00-00 (2).zip").is_some());
    }

    #[test]
    fn only_plain_names_are_accepted() {
        assert!(plain_name("New World").is_ok());
        assert!(plain_name("../outside").is_err());
        assert!(plain_name("a/b").is_err());
        assert!(plain_name("a\\b").is_err());
        assert!(plain_name(".hidden").is_err());
        assert!(plain_name("").is_err());
    }

    #[test]
    fn every_dimension_is_backed_up_and_restored() {
        let root = tempfile::tempdir().unwrap();
        let world = make_world(root.path(), "My World");
        let dir = root.path().join("backups");
        let backup =
            write_backup(&world, "My World", &dir, BackupKind::Manual).unwrap();
        assert!(backup.size > 0);

        fs::write(
            world.join("DIM-1").join("region").join("r.0.0.mca"),
            b"griefed",
        )
        .unwrap();
        fs::write(world.join("new.dat"), b"later").unwrap();

        let report = restore_blocking(
            &world,
            "My World",
            &dir.join(&backup.file_name),
            &dir,
        )
        .unwrap();
        assert_eq!(
            fs::read(world.join("DIM-1").join("region").join("r.0.0.mca"))
                .unwrap(),
            b"nether"
        );
        assert_eq!(
            fs::read(world.join("DIM1").join("region").join("r.0.0.mca"))
                .unwrap(),
            b"end"
        );
        assert!(!world.join("new.dat").exists());

        let safety = report.saved_current_as.unwrap();
        let safety_path = dir.join(&safety);
        let undo =
            restore_blocking(&world, "My World", &safety_path, &dir).unwrap();
        assert!(undo.saved_current_as.is_some());
        assert_eq!(
            fs::read(world.join("DIM-1").join("region").join("r.0.0.mca"))
                .unwrap(),
            b"griefed"
        );
        assert!(world.join("new.dat").exists());
        let leftovers: Vec<_> = fs::read_dir(world.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(leftovers, ["My World"]);
    }

    #[test]
    fn a_broken_backup_leaves_the_world_as_it_was() {
        let root = tempfile::tempdir().unwrap();
        let world = make_world(root.path(), "Keep");
        let dir = root.path().join("backups");
        fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("2024-01-01_10-00-00.zip");
        {
            let mut writer =
                zip::ZipWriter::new(fs::File::create(&bad).unwrap());
            writer
                .start_file(
                    "Other/level.dat",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer.write_all(b"other").unwrap();
            writer.finish().unwrap();
        }
        assert!(restore_blocking(&world, "Keep", &bad, &dir).is_err());
        assert_eq!(fs::read(world.join("level.dat")).unwrap(), b"level");
        assert_eq!(
            fs::read_dir(world.parent().unwrap()).unwrap().count(),
            1,
            "no half-restored folder is left behind"
        );
    }

    #[test]
    fn unsafe_paths_in_a_backup_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let bad = root.path().join("bad.zip");
        {
            let mut writer =
                zip::ZipWriter::new(fs::File::create(&bad).unwrap());
            for name in ["W/level.dat", "W/../../escape.txt"] {
                writer
                    .start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(b"x").unwrap();
            }
            writer.finish().unwrap();
        }
        let target = root.path().join("target");
        assert!(unpack(&bad, "W", &target).is_err());
        assert!(!root.path().join("escape.txt").exists());
    }

    #[test]
    fn old_automatic_backups_are_removed_but_manual_ones_stay() {
        let dir = tempfile::tempdir().unwrap();
        for (name, kind) in [
            ("2024-01-01_10-00-00.zip", "auto"),
            ("2024-01-02_10-00-00.zip", "auto"),
            ("2024-01-03_10-00-00.zip", "auto"),
            ("2024-01-01_09-00-00_manual.zip", "manual"),
        ] {
            fs::write(dir.path().join(name), kind).unwrap();
        }
        prune(dir.path(), 2);
        let mut left: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "2024-01-01_09-00-00_manual.zip",
                "2024-01-02_10-00-00.zip",
                "2024-01-03_10-00-00.zip"
            ]
        );
    }

    #[test]
    fn worlds_are_folders_with_level_data() {
        let root = tempfile::tempdir().unwrap();
        make_world(root.path(), "B World");
        make_world(root.path(), "a world");
        fs::create_dir_all(root.path().join(SAVES_FOLDER).join("not a world"))
            .unwrap();
        assert_eq!(instance_worlds(root.path()), ["a world", "B World"]);
    }

    #[test]
    fn a_world_open_in_minecraft_is_not_backed_up() {
        let root = tempfile::tempdir().unwrap();
        let world = make_world(root.path(), "Open");
        let held = lock_world(&world).unwrap();
        let other = fs::OpenOptions::new()
            .write(true)
            .open(world.join(LOCK_FILE))
            .unwrap();
        assert!(matches!(
            other.try_lock(),
            Err(fs::TryLockError::WouldBlock)
        ));
        drop(other);
        assert!(
            write_backup(
                &world,
                "Open",
                &root.path().join("b"),
                BackupKind::Manual
            )
            .is_err()
        );
        drop(held);
    }
}
