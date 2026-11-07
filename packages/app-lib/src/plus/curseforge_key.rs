//! Where the CurseForge API key comes from. A key someone enters is kept in the
//! operating system's credential store and nowhere else: not in the database, not in
//! settings and not in logs. No command ever returns it.
//!
//! CurseForge's terms make a key non-transferable, so builds don't ship one. A project
//! key would be another `KeySource` here, once CurseForge allows that in writing.

use super::curseforge::{MINECRAFT_GAME_ID, api_base, http_client};
use crate::{ErrorKind, State};
use reqwest::header::HeaderValue;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::Mutex;

const CREDENTIAL_ACCOUNT: &str = "curseforge-api-key";
const MAX_KEY_LENGTH: usize = 256;
const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
const MIN_LENGTH_FOR_HINT: usize = 12;
const HINT_CHARACTERS: usize = 4;
const USUAL_KEY_START: &str = "$2a$";
#[cfg(debug_assertions)]
const DEVELOPMENT_KEY_ENV: &str = "CURSEFORGE_API_KEY";

/// Prints as redacted in logs and debug output.
#[derive(Clone, Debug)]
pub struct ApiKey(SecretString);

impl ApiKey {
    /// Trims what gets picked up when copying a key, and refuses anything else that
    /// couldn't be one.
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        let valid = !trimmed.is_empty()
            && trimmed.len() <= MAX_KEY_LENGTH
            && trimmed.bytes().all(|byte| byte.is_ascii_graphic());
        valid.then(|| Self(SecretString::from(trimmed.to_string())))
    }

    /// Only for sending the key to CurseForge or removing it from text.
    pub(crate) fn expose(&self) -> &str {
        self.0.expose_secret()
    }

    /// Marked sensitive, so HTTP logging leaves it out.
    pub(super) fn header_value(&self) -> crate::Result<HeaderValue> {
        let mut value = HeaderValue::from_str(self.expose()).map_err(|_| {
            ErrorKind::InputError(
                "that API key can't be sent to CurseForge".to_string(),
            )
        })?;
        value.set_sensitive(true);
        Ok(value)
    }

    /// The last few characters, so people can tell their keys apart. Short keys show
    /// nothing, because a few characters would be too much of them.
    pub(super) fn hint(&self) -> String {
        let key = self.expose();
        let shown = if key.len() >= MIN_LENGTH_FOR_HINT {
            &key[key.len() - HINT_CHARACTERS..]
        } else {
            ""
        };
        format!("••••••••{shown}")
    }

    fn has_usual_start(&self) -> bool {
        self.expose().starts_with(USUAL_KEY_START)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeySource {
    User,
    /// An environment variable, which only development builds read.
    Development,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyCheck {
    Valid,
    Rejected,
    RateLimited,
    ServiceError,
    Unreachable,
    Malformed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyStatus {
    pub source: Option<KeySource>,
    pub hint: Option<String>,
    /// `None` when there is no key, or CurseForge couldn't give an answer.
    pub check: Option<KeyCheck>,
    pub store_available: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyOutcome {
    pub check: KeyCheck,
    pub saved: bool,
    /// Keys normally have the same start, so a rejected key without it was probably
    /// cut off while being copied.
    pub unusual_format: bool,
}

/// CurseForge's answer about the current key. An answer that couldn't be had isn't
/// kept, so a network problem can't turn into a lasting verdict.
static VALIDITY: Mutex<Option<bool>> = Mutex::const_new(None);

fn store_error(action: &str) -> crate::Error {
    ErrorKind::InputError(format!(
        "your system's credential store couldn't {action} the key"
    ))
    .into()
}

fn credential(app_identifier: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(app_identifier, CREDENTIAL_ACCOUNT)
}

fn read_credential(app_identifier: &str) -> keyring::Result<Option<String>> {
    match credential(app_identifier)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error),
    }
}

fn write_credential(app_identifier: &str, key: &str) -> keyring::Result<()> {
    credential(app_identifier)?.set_password(key)
}

fn delete_credential(app_identifier: &str) -> keyring::Result<()> {
    match credential(app_identifier)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error),
    }
}

async fn app_identifier() -> crate::Result<String> {
    Ok(State::get().await?.directories.app_identifier.clone())
}

async fn read_stored() -> crate::Result<Option<ApiKey>> {
    let identifier = app_identifier().await?;
    let stored =
        tokio::task::spawn_blocking(move || read_credential(&identifier))
            .await
            .map_err(|_| store_error("read"))?
            .map_err(|_| store_error("read"))?;
    Ok(stored.and_then(|value| ApiKey::parse(&value)))
}

#[cfg(debug_assertions)]
fn development_key() -> Option<ApiKey> {
    std::env::var(DEVELOPMENT_KEY_ENV)
        .ok()
        .and_then(|value| ApiKey::parse(&value))
}

#[cfg(not(debug_assertions))]
fn development_key() -> Option<ApiKey> {
    None
}

/// A saved key always wins, so a stray environment variable can't replace it.
fn choose_key(
    stored: Option<ApiKey>,
    development: Option<ApiKey>,
) -> Option<(ApiKey, KeySource)> {
    stored
        .map(|key| (key, KeySource::User))
        .or_else(|| development.map(|key| (key, KeySource::Development)))
}

pub(crate) async fn resolve() -> Option<(ApiKey, KeySource)> {
    let stored = read_stored().await.unwrap_or_else(|_| {
        tracing::warn!(
            "Couldn't read the CurseForge key from the credential store"
        );
        None
    });
    choose_key(stored, development_key())
}

pub(super) fn classify_status(status: u16) -> KeyCheck {
    match status {
        200..=299 => KeyCheck::Valid,
        401 | 403 => KeyCheck::Rejected,
        429 => KeyCheck::RateLimited,
        _ => KeyCheck::ServiceError,
    }
}

/// Uses an endpoint that really checks the key, because some are open to anyone.
async fn check_with_curseforge(key: &ApiKey) -> KeyCheck {
    let Ok(header) = key.header_value() else {
        return KeyCheck::Malformed;
    };
    let response = http_client()
        .get(format!("{}/games/{MINECRAFT_GAME_ID}", api_base()))
        .header("x-api-key", header)
        .timeout(CHECK_TIMEOUT)
        .send()
        .await;
    match response {
        Ok(response) => classify_status(response.status().as_u16()),
        // The error's text can include request details, so it isn't kept.
        Err(_) => KeyCheck::Unreachable,
    }
}

fn as_validity(check: KeyCheck) -> Option<bool> {
    match check {
        KeyCheck::Valid => Some(true),
        KeyCheck::Rejected => Some(false),
        _ => None,
    }
}

/// Asks CurseForge the first time, then remembers until the key changes.
pub(super) async fn validity(key: &ApiKey) -> Option<bool> {
    let mut known = VALIDITY.lock().await;
    if known.is_none() {
        *known = as_validity(check_with_curseforge(key).await);
    }
    *known
}

/// CurseForge turned the key down partway through, for example because it was revoked.
pub(super) async fn mark_rejected() {
    *VALIDITY.lock().await = Some(false);
}

fn outcome(check: KeyCheck, saved: bool, key: Option<&ApiKey>) -> KeyOutcome {
    KeyOutcome {
        check,
        saved,
        unusual_format: key.is_some_and(|key| !key.has_usual_start()),
    }
}

pub async fn status() -> KeyStatus {
    let stored = read_stored().await;
    let store_available = stored.is_ok();
    let resolved = choose_key(stored.ok().flatten(), development_key());
    let Some((key, source)) = resolved else {
        return KeyStatus {
            source: None,
            hint: None,
            check: None,
            store_available,
        };
    };
    let check = validity(&key).await.map(|valid| {
        if valid {
            KeyCheck::Valid
        } else {
            KeyCheck::Rejected
        }
    });
    KeyStatus {
        source: Some(source),
        hint: Some(key.hint()),
        check,
        store_available,
    }
}

/// A key CurseForge definitely refuses isn't saved. One that can't be checked right
/// now is, so being offline doesn't lose it.
pub async fn save(raw: &str) -> crate::Result<KeyOutcome> {
    let Some(key) = ApiKey::parse(raw) else {
        return Ok(outcome(KeyCheck::Malformed, false, None));
    };
    let check = check_with_curseforge(&key).await;
    if check == KeyCheck::Rejected {
        return Ok(outcome(check, false, Some(&key)));
    }
    let identifier = app_identifier().await?;
    let stored = key.clone();
    tokio::task::spawn_blocking(move || {
        write_credential(&identifier, stored.expose())
    })
    .await
    .map_err(|_| store_error("save"))?
    .map_err(|_| store_error("save"))?;
    *VALIDITY.lock().await = as_validity(check);
    Ok(outcome(check, true, Some(&key)))
}

/// Tests a key that was typed but not saved, or the current key when none is given.
pub async fn test(raw: Option<&str>) -> crate::Result<KeyOutcome> {
    if let Some(raw) = raw {
        let Some(key) = ApiKey::parse(raw) else {
            return Ok(outcome(KeyCheck::Malformed, false, None));
        };
        let check = check_with_curseforge(&key).await;
        return Ok(outcome(check, false, Some(&key)));
    }
    let Some((key, _)) = resolve().await else {
        return Err(ErrorKind::InputError(
            "there's no API key to test".to_string(),
        )
        .into());
    };
    let check = check_with_curseforge(&key).await;
    *VALIDITY.lock().await = as_validity(check);
    Ok(outcome(check, false, Some(&key)))
}

pub async fn remove() -> crate::Result<()> {
    let identifier = app_identifier().await?;
    tokio::task::spawn_blocking(move || delete_credential(&identifier))
        .await
        .map_err(|_| store_error("remove"))?
        .map_err(|_| store_error("remove"))?;
    *VALIDITY.lock().await = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = "$2a$10$exampleExampleExampleExampleExample";

    fn key() -> ApiKey {
        ApiKey::parse(EXAMPLE).unwrap()
    }

    #[test]
    fn parsing_trims_copied_whitespace() {
        let parsed = ApiKey::parse(&format!("  {EXAMPLE}\n")).unwrap();
        assert_eq!(parsed.expose(), EXAMPLE);
    }

    #[test]
    fn parsing_refuses_what_cannot_be_a_key() {
        assert!(ApiKey::parse("").is_none());
        assert!(ApiKey::parse("   ").is_none());
        assert!(ApiKey::parse("two words").is_none());
        assert!(ApiKey::parse("line\nbreak").is_none());
        assert!(ApiKey::parse("naïve").is_none());
        assert!(ApiKey::parse(&"x".repeat(MAX_KEY_LENGTH + 1)).is_none());
        assert!(ApiKey::parse(&"x".repeat(MAX_KEY_LENGTH)).is_some());
    }

    #[test]
    fn debug_output_never_contains_the_key() {
        let printed = format!("{:?}", key());
        assert!(!printed.contains(EXAMPLE));
        assert!(!printed.contains("example"));
    }

    #[test]
    fn the_hint_shows_only_the_end_of_a_long_key() {
        let hint = key().hint();
        assert!(hint.ends_with("mple"));
        assert!(!hint.contains(&EXAMPLE[..8]));
        assert_eq!(hint.chars().filter(|c| c.is_ascii()).count(), 4);
    }

    #[test]
    fn the_hint_shows_nothing_of_a_short_key() {
        let hint = ApiKey::parse("short").unwrap().hint();
        assert!(hint.chars().all(|c| !c.is_ascii()));
    }

    #[test]
    fn the_header_is_marked_sensitive() {
        let header = key().header_value().unwrap();
        assert!(header.is_sensitive());
        assert!(!format!("{header:?}").contains("example"));
    }

    #[test]
    fn statuses_mean_the_right_things() {
        assert_eq!(classify_status(200), KeyCheck::Valid);
        assert_eq!(classify_status(401), KeyCheck::Rejected);
        assert_eq!(classify_status(403), KeyCheck::Rejected);
        assert_eq!(classify_status(429), KeyCheck::RateLimited);
        assert_eq!(classify_status(500), KeyCheck::ServiceError);
        assert_eq!(classify_status(503), KeyCheck::ServiceError);
    }

    #[test]
    fn only_a_definite_answer_is_remembered() {
        assert_eq!(as_validity(KeyCheck::Valid), Some(true));
        assert_eq!(as_validity(KeyCheck::Rejected), Some(false));
        assert_eq!(as_validity(KeyCheck::Unreachable), None);
        assert_eq!(as_validity(KeyCheck::RateLimited), None);
        assert_eq!(as_validity(KeyCheck::ServiceError), None);
    }

    #[test]
    fn a_saved_key_wins_over_the_development_key() {
        let saved = ApiKey::parse("saved").unwrap();
        let development = ApiKey::parse("development").unwrap();
        let (chosen, source) =
            choose_key(Some(saved), Some(development.clone())).unwrap();
        assert_eq!(chosen.expose(), "saved");
        assert_eq!(source, KeySource::User);

        let (chosen, source) = choose_key(None, Some(development)).unwrap();
        assert_eq!(chosen.expose(), "development");
        assert_eq!(source, KeySource::Development);

        assert!(choose_key(None, None).is_none());
    }

    #[test]
    fn a_cut_off_key_is_flagged_only_when_its_start_is_missing() {
        assert!(
            !outcome(KeyCheck::Rejected, false, Some(&key())).unusual_format
        );
        let cut_off = ApiKey::parse("exampleExampleExample").unwrap();
        assert!(
            outcome(KeyCheck::Rejected, false, Some(&cut_off)).unusual_format
        );
    }

    /// Uses real launcher state and the real Windows Credential Manager.
    #[cfg(windows)]
    #[tokio::test]
    async fn support_reports_never_contain_the_saved_key() {
        let root = tempfile::tempdir().unwrap();
        crate::install_crypto_provider();
        // SAFETY: set before any other thread in this test process reads it.
        unsafe {
            std::env::set_var("THESEUS_CONFIG_DIR", root.path());
        }
        let identifier =
            format!("com.pawmerized.pawrinth.tests-{}", std::process::id());
        State::init(identifier.clone()).await.unwrap();
        let state = State::get().await.unwrap();
        write_credential(&identifier, EXAMPLE).unwrap();

        let report = crate::install::diagnostics::censor_support_text(
            format!("the request failed using {EXAMPLE} just now"),
            &state.pool,
        )
        .await;
        delete_credential(&identifier).unwrap();

        let report = report.unwrap();
        assert!(!report.contains(EXAMPLE));
        assert!(report.contains("{CURSEFORGE_API_KEY}"));
    }

    /// Goes through the real Windows Credential Manager, under a name of its own.
    #[cfg(windows)]
    #[test]
    fn the_windows_credential_store_keeps_replaces_and_removes_a_key() {
        let identifier = format!("pawrinth-test-{}", std::process::id());
        assert_eq!(read_credential(&identifier).unwrap(), None);

        write_credential(&identifier, "first").unwrap();
        assert_eq!(read_credential(&identifier).unwrap().unwrap(), "first");

        write_credential(&identifier, "second").unwrap();
        assert_eq!(read_credential(&identifier).unwrap().unwrap(), "second");

        delete_credential(&identifier).unwrap();
        assert_eq!(read_credential(&identifier).unwrap(), None);

        delete_credential(&identifier).unwrap();
    }
}
