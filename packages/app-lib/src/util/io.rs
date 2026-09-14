// IO error
// A wrapper around the tokio IO functions that adds the path to the error message, instead of the uninformative std::io::Error.

use anyhow::{Context, Result};
use std::{
    io::{BufRead, BufReader, ErrorKind, Write},
    path::Path,
    time::Duration,
};
use tempfile::{NamedTempFile, TempPath};
use tokio::task::spawn_blocking;

#[derive(Debug, thiserror::Error)]
pub enum IOError {
    #[error("{source}, path: {path}")]
    IOPathError {
        #[source]
        source: std::io::Error,
        path: String,
    },
    #[error(transparent)]
    IOError(#[from] std::io::Error),
}

impl IOError {
    pub fn from(source: std::io::Error) -> Self {
        Self::IOError(source)
    }
    pub fn with_path(
        source: std::io::Error,
        path: impl AsRef<std::path::Path>,
    ) -> Self {
        let path = path.as_ref();

        Self::IOPathError {
            source,
            path: path.to_string_lossy().to_string(),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        match self {
            IOError::IOPathError { source, .. } => source.kind(),
            IOError::IOError(source) => source.kind(),
        }
    }
}

pub fn canonicalize(
    path: impl AsRef<std::path::Path>,
) -> Result<std::path::PathBuf, IOError> {
    let path = path.as_ref();
    dunce::canonicalize(path).map_err(|e| IOError::IOPathError {
        source: e,
        path: path.to_string_lossy().to_string(),
    })
}

pub async fn read_dir(
    path: impl AsRef<std::path::Path>,
) -> Result<tokio::fs::ReadDir, IOError> {
    let path = path.as_ref();
    tokio::fs::read_dir(path)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })
}

pub async fn create_dir(
    path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
    let path = path.as_ref();
    tokio::fs::create_dir(path)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })
}

pub async fn create_dir_all(
    path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
    let path = path.as_ref();
    tokio::fs::create_dir_all(path)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })
}

pub async fn remove_dir_all(
    path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
    let path = path.as_ref();
    tokio::fs::remove_dir_all(path)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })
}

/// Reads a text file to a string, automatically detecting its encoding and
/// substituting any invalid characters with the Unicode replacement character.
///
/// This function is best suited for reading Minecraft instance files, whose
/// encoding may vary depending on the platform, launchers, client versions
/// (older Minecraft versions tended to rely on the system's default codepage
/// more on Windows platforms), and mods used, while not being highly sensitive
/// to occasional occurrences of mojibake or character replacements.
pub async fn read_any_encoding_to_string(
    path: impl AsRef<std::path::Path>,
) -> Result<(String, &'static encoding_rs::Encoding), IOError> {
    let path = path.as_ref();
    let file_bytes =
        tokio::fs::read(path)
            .await
            .map_err(|e| IOError::IOPathError {
                source: e,
                path: path.to_string_lossy().to_string(),
            })?;

    let file_encoding = {
        let mut encoding_detector = chardetng::EncodingDetector::new(
            chardetng::Iso2022JpDetection::Allow,
        );
        encoding_detector.feed(&file_bytes, true);
        encoding_detector.guess(None, chardetng::Utf8Detection::Allow)
    };

    let (file_string, actual_file_encoding, _) =
        file_encoding.decode(&file_bytes);
    Ok((file_string.to_string(), actual_file_encoding))
}

pub async fn read(
    path: impl AsRef<std::path::Path>,
) -> Result<Vec<u8>, IOError> {
    let path = path.as_ref();
    tokio::fs::read(path)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })
}

pub async fn write(
    path: impl AsRef<std::path::Path>,
    data: impl AsRef<[u8]>,
) -> Result<(), IOError> {
    let path = path.as_ref().to_owned();
    let data = data.as_ref().to_owned();
    spawn_blocking(move || {
        let cloned_path = path.clone();
        sync_write(data, path).map_err(|e| IOError::IOPathError {
            source: e,
            path: cloned_path.to_string_lossy().to_string(),
        })
    })
    .await
    .map_err(|_| std::io::Error::other("background task failed"))??;

    Ok(())
}

fn sync_write(
    data: impl AsRef<[u8]>,
    path: impl AsRef<Path>,
) -> Result<(), std::io::Error> {
    let mut tempfile =
        NamedTempFile::new_in(path.as_ref().parent().ok_or_else(|| {
            std::io::Error::other(
                "could not get parent directory for temporary file",
            )
        })?)?;
    tempfile.write_all(data.as_ref())?;
    persist_replacing(tempfile.into_temp_path(), path.as_ref())
}

const REPLACE_ATTEMPTS: u32 = 6;

/// Moves a finished temporary file over `path`. Windows refuses with "access denied" while
/// anything else has the destination open, which is normal when several installs download
/// the same shared file or an antivirus is scanning it. This retries briefly, and a
/// destination that already holds the same bytes counts as done.
pub(crate) fn persist_replacing(
    mut temporary: TempPath,
    path: &Path,
) -> std::io::Result<()> {
    let mut attempt = 1;
    loop {
        let error = match temporary.persist(path) {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };
        if error.error.kind() != ErrorKind::PermissionDenied {
            return Err(error.error);
        }
        temporary = error.path;
        if same_contents(&temporary, path).unwrap_or(false) {
            return Ok(());
        }
        if attempt == REPLACE_ATTEMPTS {
            return Err(error.error);
        }
        std::thread::sleep(Duration::from_millis(25 << attempt));
        attempt += 1;
    }
}

fn same_contents(left: &Path, right: &Path) -> std::io::Result<bool> {
    let left = std::fs::File::open(left)?;
    let right = std::fs::File::open(right)?;
    if left.metadata()?.len() != right.metadata()?.len() {
        return Ok(false);
    }
    let mut left = BufReader::new(left);
    let mut right = BufReader::new(right);
    loop {
        let left_chunk = left.fill_buf()?;
        let right_chunk = right.fill_buf()?;
        let shared = left_chunk.len().min(right_chunk.len());
        if shared == 0 {
            return Ok(left_chunk.len() == right_chunk.len());
        }
        if left_chunk[..shared] != right_chunk[..shared] {
            return Ok(false);
        }
        left.consume(shared);
        right.consume(shared);
    }
}

pub fn is_same_disk(old_dir: &Path, new_dir: &Path) -> Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        // we need to use `symlink_metadata` instead of `metadata`, because
        // if this file is a symlink, we need to query the symlink file itself,
        // rather than the target.
        // downloaded JREs use symlinks to point to certain stuff like LICENSE
        // files.
        // this fixes moving JRE dirs.

        let old_meta = std::fs::symlink_metadata(old_dir)
            .with_context(|| format!("getting meta of old dir {old_dir:?}"))?;
        let new_meta = std::fs::symlink_metadata(new_dir)
            .with_context(|| format!("getting meta of new dir {new_dir:?}"))?;

        Ok(old_meta.dev() == new_meta.dev())
    }

    #[cfg(windows)]
    {
        let old_dir = canonicalize(old_dir)
            .with_context(|| format!("canonicalizing {old_dir:?}"))?;
        let new_dir = canonicalize(new_dir)
            .with_context(|| format!("canonicalizing {new_dir:?}"))?;

        let old_component = old_dir.components().next();
        let new_component = new_dir.components().next();

        match (old_component, new_component) {
            (
                Some(std::path::Component::Prefix(old)),
                Some(std::path::Component::Prefix(new)),
            ) => Ok(old.as_os_str() == new.as_os_str()),
            _ => Ok(false),
        }
    }
}

pub async fn rename_or_move(
    from: impl AsRef<std::path::Path>,
    to: impl AsRef<std::path::Path>,
) -> Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();

    let to_parent = to
        .parent()
        .with_context(|| format!("getting parent of `to` dir {to:?}"))?;
    let same_disk = is_same_disk(from, to_parent).with_context(|| {
        format!("checking if `to_parent` ({to_parent:?}) and `from` ({from:?}) are on the same disk")
    })?;

    if same_disk {
        tokio::fs::rename(from, to)
            .await
            .map_err(|e| IOError::IOPathError {
                source: e,
                path: from.to_string_lossy().to_string(),
            })
            .with_context(|| format!("moving {from:?} to {to:?} on same disk"))
    } else {
        move_recursive(from, to).await.with_context(|| {
            format!("moving {from:?} to {to:?} on different disks")
        })
    }
}

#[async_recursion::async_recursion]
async fn move_recursive(from: &Path, to: &Path) -> Result<()> {
    if from.is_file() {
        copy(from, to)
            .await
            .with_context(|| format!("copying {from:?} to {to:?}"))?;
        remove_file(from).await.with_context(|| {
            format!("removing {from:?} after copying to {to:?}")
        })?;
        return Ok(());
    }

    create_dir(to)
        .await
        .with_context(|| format!("creating dir for {to:?}"))?;

    let mut dir = read_dir(from)
        .await
        .with_context(|| format!("reading dir {from:?}"))?;
    while let Some(entry) = dir
        .next_entry()
        .await
        .with_context(|| format!("reading dir entry in {from:?}"))?
    {
        let new_path = to.join(entry.file_name());
        move_recursive(&entry.path(), &new_path)
            .await
            .with_context(|| {
                format!("moving {:?} to {new_path:?}", entry.path())
            })?;
    }

    Ok(())
}

pub async fn copy(
    from: impl AsRef<std::path::Path>,
    to: impl AsRef<std::path::Path>,
) -> Result<u64, IOError> {
    let from: &Path = from.as_ref();
    let to = to.as_ref();
    tokio::fs::copy(from, to)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: from.to_string_lossy().to_string(),
        })
}

pub async fn remove_file(
    path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
    let path = path.as_ref();
    tokio::fs::remove_file(path)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })
}

pub async fn remove_dir(
    path: impl AsRef<std::path::Path>,
) -> Result<(), IOError> {
    let path = path.as_ref();
    tokio::fs::remove_dir(path)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })
}

pub async fn metadata(
    path: impl AsRef<std::path::Path>,
) -> Result<std::fs::Metadata, IOError> {
    let path = path.as_ref();
    tokio::fs::metadata(path)
        .await
        .map_err(|e| IOError::IOPathError {
            source: e,
            path: path.to_string_lossy().to_string(),
        })
}

/// Gets a resource file from the executable. Returns `theseus::Result<(TempDir, PathBuf)>`.
#[macro_export]
macro_rules! get_resource_file {
    (directory: $relative_dir:expr, file: $file_name:expr) => {
        'get_resource_file: {
            let dir = match tempfile::tempdir() {
                Ok(dir) => dir,
                Err(e) => {
                    break 'get_resource_file $crate::Result::Err(
                        $crate::util::io::IOError::from(e).into(),
                    );
                }
            };
            let path = dir.path().join($file_name);
            if let Err(e) = $crate::util::io::write(
                &path,
                include_bytes!(concat!($relative_dir, "/", $file_name)),
            )
            .await
            {
                break 'get_resource_file $crate::Result::Err(e.into());
            }
            let path = match $crate::util::io::canonicalize(path) {
                Ok(path) => path,
                Err(e) => {
                    break 'get_resource_file $crate::Result::Err(e.into());
                }
            };
            $crate::Result::Ok((dir, path))
        }
    };

    ($relative_dir:literal / $file_name:literal) => {
        get_resource_file!(directory: $relative_dir, file: $file_name)
    };

    (env $dir_env_name:literal / $file_name:literal) => {
        get_resource_file!(directory: env!($dir_env_name), file: $file_name)
    };
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = UNITS[0];
    for next_unit in UNITS.iter().skip(1) {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next_unit;
    }

    if unit == "B" {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {unit}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_with(dir: &Path, contents: &[u8]) -> TempPath {
        let mut file = NamedTempFile::new_in(dir).unwrap();
        file.write_all(contents).unwrap();
        file.into_temp_path()
    }

    #[test]
    fn replaces_a_file_nothing_is_using() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared");
        std::fs::write(&path, b"old").unwrap();

        persist_replacing(temporary_with(dir.path(), b"new"), &path).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }

    #[test]
    fn many_installs_writing_one_file_all_succeed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared");

        std::thread::scope(|scope| {
            for _ in 0..16 {
                scope.spawn(|| {
                    for _ in 0..25 {
                        sync_write(b"same contents", &path).unwrap();
                    }
                });
            }
        });

        assert_eq!(std::fs::read(&path).unwrap(), b"same contents");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(windows)]
    mod while_the_destination_is_open {
        use super::*;
        use std::os::windows::fs::OpenOptionsExt;

        const FILE_SHARE_READ: u32 = 1;

        fn open_blocking_replacement(path: &Path) -> std::fs::File {
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(path)
                .unwrap()
        }

        #[test]
        fn the_same_bytes_count_as_done() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("shared");
            std::fs::write(&path, b"same").unwrap();
            let _open = open_blocking_replacement(&path);

            persist_replacing(temporary_with(dir.path(), b"same"), &path)
                .unwrap();

            assert_eq!(std::fs::read(&path).unwrap(), b"same");
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        }

        #[test]
        fn different_bytes_go_in_once_it_closes() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("shared");
            std::fs::write(&path, b"old").unwrap();
            let open = open_blocking_replacement(&path);

            std::thread::scope(|scope| {
                scope.spawn(move || {
                    std::thread::sleep(Duration::from_millis(120));
                    drop(open);
                });
                persist_replacing(temporary_with(dir.path(), b"new"), &path)
                    .unwrap();
            });

            assert_eq!(std::fs::read(&path).unwrap(), b"new");
        }

        #[test]
        fn different_bytes_fail_if_it_stays_open() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("shared");
            std::fs::write(&path, b"old").unwrap();
            let _open = open_blocking_replacement(&path);

            let error =
                persist_replacing(temporary_with(dir.path(), b"new"), &path)
                    .unwrap_err();

            assert_eq!(error.kind(), ErrorKind::PermissionDenied);
            assert_eq!(std::fs::read(&path).unwrap(), b"old");
        }
    }
}
