//! Read-only checks for the first-launch wizard. Nothing here changes any file.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MinecraftFolder {
    pub path: PathBuf,
    pub saves: usize,
    pub versions: usize,
}

fn default_minecraft_folder() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        return Some(dirs::data_dir()?.join("minecraft"));
    }
    if cfg!(target_os = "windows") {
        return Some(dirs::data_dir()?.join(".minecraft"));
    }
    Some(dirs::home_dir()?.join(".minecraft"))
}

async fn count_folders(path: &Path) -> usize {
    let Ok(mut entries) = tokio::fs::read_dir(path).await else {
        return 0;
    };
    let mut count = 0;
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry.file_type().await.is_ok_and(|kind| kind.is_dir()) {
            count += 1;
        }
    }
    count
}

async fn inspect(path: PathBuf) -> Option<MinecraftFolder> {
    if !tokio::fs::metadata(&path).await.ok()?.is_dir() {
        return None;
    }
    Some(MinecraftFolder {
        saves: count_folders(&path.join("saves")).await,
        versions: count_folders(&path.join("versions")).await,
        path,
    })
}

/// Looks for the official launcher's folder, so the wizard can say what it found.
pub async fn detect_minecraft_folder() -> Option<MinecraftFolder> {
    inspect(default_minecraft_folder()?).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_folder_is_not_reported() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(inspect(dir.path().join("nope")).await, None);
    }

    #[tokio::test]
    async fn a_file_is_not_a_minecraft_folder() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".minecraft");
        std::fs::write(&file, b"").unwrap();
        assert_eq!(inspect(file).await, None);
    }

    #[tokio::test]
    async fn counts_only_folders_and_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join(".minecraft");
        std::fs::create_dir_all(root.join("saves/world one")).unwrap();
        std::fs::create_dir_all(root.join("saves/world two")).unwrap();
        std::fs::create_dir_all(root.join("versions/1.21.1")).unwrap();
        std::fs::write(root.join("saves/not-a-world.txt"), b"x").unwrap();

        let found = inspect(root.clone()).await.unwrap();
        assert_eq!(found.saves, 2);
        assert_eq!(found.versions, 1);
        assert!(root.join("saves/not-a-world.txt").exists());
    }
}
