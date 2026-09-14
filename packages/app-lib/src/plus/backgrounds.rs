//! Custom background images. Files are checked and copied into the app's cache
//! folder, which the webview is allowed to read, so the original can move or go.

use crate::{ErrorKind, State};
use anyhow::bail;
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 64 * 1024 * 1024;
const FOLDER: &str = "plus-backgrounds";

fn format_of(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => Some("png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("jpg"),
        [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..] => Some("gif"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("webp"),
        _ => None,
    }
}

/// Decides from the file's bytes, because a renamed file can't be trusted.
pub(crate) fn validated_format(
    bytes: &[u8],
    size: u64,
) -> Result<&'static str, String> {
    if size > MAX_BYTES {
        return Err("that image is larger than the 64 MB limit".to_string());
    }
    format_of(bytes).ok_or_else(|| {
        "that file isn't a PNG, JPEG, WebP or GIF image".to_string()
    })
}

/// Returns the copied file's path.
pub async fn import(source: &Path) -> crate::Result<PathBuf> {
    let state = State::get().await?;
    let size = tokio::fs::metadata(source).await?.len();
    if size > MAX_BYTES {
        bail!(ErrorKind::InputError(
            "that image is larger than the 64 MB limit".to_string()
        ));
    }
    let bytes = tokio::fs::read(source).await?;
    let extension =
        validated_format(&bytes, size).map_err(ErrorKind::InputError)?;
    let folder = state.directories.caches_dir().join(FOLDER);
    crate::util::io::create_dir_all(&folder).await?;
    let hash = sha1_smol::Sha1::from(&bytes).digest().to_string();
    let target = folder.join(format!("{hash}.{extension}"));
    crate::util::io::write(&target, &bytes).await?;
    Ok(target)
}

/// Settings keep the full path, which goes stale when the app folder is moved. The
/// file name is what identifies an image, so this finds it in the current folder.
pub(crate) fn current_path(stored: &str, caches_dir: &Path) -> Option<PathBuf> {
    let current = caches_dir.join(FOLDER).join(Path::new(stored).file_name()?);
    current.is_file().then_some(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_an_image_after_the_app_folder_moves() {
        let caches = tempfile::tempdir().unwrap();
        let folder = caches.path().join(FOLDER);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("abc.png"), b"image").unwrap();
        let old_path = Path::new("old-folder").join(FOLDER).join("abc.png");

        assert_eq!(
            current_path(old_path.to_str().unwrap(), caches.path()),
            Some(folder.join("abc.png"))
        );
    }

    #[test]
    fn leaves_a_missing_image_alone() {
        let caches = tempfile::tempdir().unwrap();
        assert_eq!(current_path("gone.png", caches.path()), None);
        assert_eq!(current_path("", caches.path()), None);
    }

    #[test]
    fn recognizes_formats_from_their_bytes() {
        assert_eq!(validated_format(b"\x89PNG\r\n\x1a\nrest", 10), Ok("png"));
        assert_eq!(validated_format(b"\xFF\xD8\xFFrest", 10), Ok("jpg"));
        assert_eq!(validated_format(b"GIF89a-rest", 10), Ok("gif"));
        assert_eq!(validated_format(b"GIF87a-rest", 10), Ok("gif"));
        assert_eq!(validated_format(b"RIFF1234WEBPrest", 10), Ok("webp"));
    }

    #[test]
    fn rejects_other_files_and_huge_ones() {
        assert!(validated_format(b"<svg onload=alert(1)>", 10).is_err());
        assert!(validated_format(b"MZ-an-executable", 10).is_err());
        assert!(validated_format(b"", 0).is_err());
        assert!(validated_format(b"GIF89a", MAX_BYTES + 1).is_err());
    }
}
