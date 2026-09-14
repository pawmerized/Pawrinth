use crate::ErrorKind;
use std::path::{Component, Path};

/// Data folder names used by official Modrinth App builds.
const OFFICIAL_DATA_DIR_NAMES: [&str; 2] =
    ["ModrinthApp", "com.modrinth.theseus"];

/// Refuses to open a data folder that belongs to an official Modrinth App
/// install. Fork migrations in a shared `app.db` make the official app fail on
/// its next launch, so this runs before the database is touched.
pub(crate) fn ensure_isolated_data_dir(path: &Path) -> crate::Result<()> {
    if is_official_data_dir(path) {
        return Err(ErrorKind::FSError(format!(
            "refusing to use {}: it belongs to the official Modrinth App",
            path.display()
        ))
        .into());
    }
    Ok(())
}

fn is_official_data_dir(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => {
            let name = name.to_string_lossy();
            OFFICIAL_DATA_DIR_NAMES
                .iter()
                .any(|official| name.eq_ignore_ascii_case(official))
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_official_folders_in_any_case() {
        assert!(is_official_data_dir(Path::new(
            "C:/Users/a/AppData/Roaming/ModrinthApp"
        )));
        assert!(is_official_data_dir(Path::new(
            "C:/Users/a/AppData/Roaming/modrinthapp/profiles"
        )));
        assert!(is_official_data_dir(Path::new(
            "/home/a/.config/com.modrinth.theseus"
        )));
    }

    #[test]
    fn accepts_fork_folders() {
        assert!(!is_official_data_dir(Path::new(
            "C:/Users/a/AppData/Roaming/com.pawmerized.pawrinth"
        )));
        assert!(!is_official_data_dir(Path::new("D:/Games/Pawrinth")));
    }
}
