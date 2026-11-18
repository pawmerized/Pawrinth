//! Reading and writing theme files people choose with the system's file dialogs.
//! Colors are validated by the theme engine; this only keeps the file operations safe.

use crate::ErrorKind;
use anyhow::bail;
use std::path::Path;

const MAX_THEME_FILE_BYTES: u64 = 256 * 1024;
const THEME_FORMAT: &str = "pawrinth-theme";

fn has_json_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
}

fn is_theme(contents: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(contents).is_ok_and(|parsed| {
        parsed.get("format").and_then(serde_json::Value::as_str)
            == Some(THEME_FORMAT)
    })
}

/// Refuses to write anything that isn't a theme, so this can't overwrite arbitrary files
/// with arbitrary content.
pub(crate) fn validate_for_saving(
    path: &Path,
    contents: &str,
) -> Result<(), String> {
    if !has_json_extension(path) {
        return Err("theme files have to end in .json".to_string());
    }
    if contents.len() as u64 > MAX_THEME_FILE_BYTES {
        return Err("that theme is too large to save".to_string());
    }
    if !is_theme(contents) {
        return Err("that isn't a valid theme".to_string());
    }
    Ok(())
}

/// Only replaces a file that's already a theme, so a save can't overwrite another
/// program's settings even when it's given their path.
pub async fn save(path: &Path, contents: &str) -> crate::Result<()> {
    validate_for_saving(path, contents).map_err(ErrorKind::InputError)?;
    if tokio::fs::try_exists(path).await? && read(path).await.is_err() {
        bail!(ErrorKind::InputError(
            "that file isn't a theme, so it wasn't replaced".to_string()
        ));
    }
    crate::util::io::write(path, contents.as_bytes()).await?;
    Ok(())
}

/// Only hands back theme files, so it can't be used to read anything else.
pub async fn read(path: &Path) -> crate::Result<String> {
    let not_a_theme =
        || ErrorKind::InputError("that file isn't a theme".to_string());
    if !has_json_extension(path) {
        bail!(not_a_theme());
    }
    let size = tokio::fs::metadata(path).await?.len();
    if size > MAX_THEME_FILE_BYTES {
        bail!(ErrorKind::InputError(
            "that file is too large to be a theme".to_string()
        ));
    }
    let contents = String::from_utf8(tokio::fs::read(path).await?)
        .map_err(|_| not_a_theme())?;
    if !is_theme(&contents) {
        bail!(not_a_theme());
    }
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    const THEME: &str = r#"{ "format": "pawrinth-theme", "version": 2 }"#;

    #[test]
    fn only_theme_contents_can_be_saved() {
        let path = Path::new("my.json");
        assert!(validate_for_saving(path, THEME).is_ok());
        assert!(validate_for_saving(path, "not json").is_err());
        assert!(validate_for_saving(path, "{}").is_err());
        assert!(
            validate_for_saving(path, r#"{ "format": "something-else" }"#)
                .is_err()
        );
    }

    #[test]
    fn only_json_files_can_be_written() {
        assert!(validate_for_saving(Path::new("a.JSON"), THEME).is_ok());
        assert!(validate_for_saving(Path::new("a.exe"), THEME).is_err());
        assert!(validate_for_saving(Path::new("a"), THEME).is_err());
    }

    #[test]
    fn huge_contents_are_refused() {
        let contents = format!(
            r#"{{ "format": "pawrinth-theme", "pad": "{}" }}"#,
            "x".repeat(MAX_THEME_FILE_BYTES as usize)
        );
        assert!(validate_for_saving(Path::new("a.json"), &contents).is_err());
    }

    #[tokio::test]
    async fn a_saved_theme_reads_back_the_same() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("theme.json");
        save(&path, THEME).await.unwrap();
        assert_eq!(read(&path).await.unwrap(), THEME);
    }

    #[tokio::test]
    async fn only_theme_files_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join("id_ed25519");
        tokio::fs::write(&secret, "private key").await.unwrap();
        assert!(read(&secret).await.is_err());
        let settings = dir.path().join("settings.json");
        tokio::fs::write(&settings, r#"{ "editor": true }"#)
            .await
            .unwrap();
        assert!(read(&settings).await.is_err());
    }

    #[tokio::test]
    async fn another_programs_json_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let settings = dir.path().join("settings.json");
        tokio::fs::write(&settings, r#"{ "editor": true }"#)
            .await
            .unwrap();
        assert!(save(&settings, THEME).await.is_err());
        assert_eq!(
            tokio::fs::read_to_string(&settings).await.unwrap(),
            r#"{ "editor": true }"#
        );
        let theme = dir.path().join("old-theme.json");
        save(&theme, THEME).await.unwrap();
        save(&theme, THEME).await.unwrap();
    }

    #[tokio::test]
    async fn a_binary_file_is_refused_when_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("binary.json");
        tokio::fs::write(&path, [0xff, 0xfe, 0x00]).await.unwrap();
        assert!(read(&path).await.is_err());
    }

    #[tokio::test]
    async fn an_oversized_file_is_refused_when_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.json");
        tokio::fs::write(&path, vec![b' '; MAX_THEME_FILE_BYTES as usize + 1])
            .await
            .unwrap();
        assert!(read(&path).await.is_err());
    }
}
