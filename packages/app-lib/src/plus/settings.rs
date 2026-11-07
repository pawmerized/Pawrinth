//! Pawrinth settings, kept in one JSON column of the existing `settings`
//! row so upstream's per-setting query text stays untouched.

use crate::State;
use serde::{Deserialize, Serialize};

pub const UI_SCALE_MIN: f32 = 0.8;
pub const UI_SCALE_MAX: f32 = 1.5;
const GIB: u64 = 1024 * 1024 * 1024;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct PlusSettings {
    pub appearance: AppearanceSettings,
    pub interface: InterfaceSettings,
    pub rollback: RollbackSettings,
    pub diagnostics: DiagnosticsSettings,
    pub onboarding: OnboardingSettings,
    pub discovery: DiscoverySettings,
    pub privacy: PrivacySettings,
    pub skins: SkinSettings,
    pub discord: DiscordSettings,
    /// Fields from a newer build are preserved rather than dropped.
    #[serde(flatten)]
    pub unknown: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct AppearanceSettings {
    pub active_theme_id: Option<String>,
    /// Theme documents are validated by the theme engine before they apply.
    pub themes: Vec<serde_json::Value>,
    /// Validated by the frontend, which is the only thing that renders it.
    pub background: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct InterfaceSettings {
    pub ui_scale: f32,
    pub reduce_motion: bool,
    pub density: Density,
    pub remember_last_page: bool,
}

impl Default for InterfaceSettings {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            reduce_motion: false,
            density: Density::Comfortable,
            remember_last_page: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    Comfortable,
    Compact,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct RollbackSettings {
    pub enabled: bool,
    pub max_operations_per_instance: u32,
    pub max_storage_bytes: u64,
    /// `0` keeps operations until the user removes them.
    pub retention_days: u32,
    pub auto_cleanup: bool,
}

impl Default for RollbackSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_operations_per_instance: 20,
            max_storage_bytes: 5 * GIB,
            retention_days: 0,
            auto_cleanup: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct DiagnosticsSettings {
    /// Upstream posts console output to mclo.gs automatically; here it's opt-in.
    pub send_logs_to_mclogs: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct OnboardingSettings {
    pub wizard_completed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct DiscoverySettings {
    pub remember_search_history: bool,
    pub search_history: Vec<String>,
    pub recently_viewed: Vec<serde_json::Value>,
}

/// What this PC shows. None of this changes an account, a real username, or anything
/// the game or a server sees.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct PrivacySettings {
    pub streamer_mode: bool,
    pub blur_usernames: bool,
    pub display_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct SkinSettings {
    /// Validated by the frontend, which is the only thing that reads them.
    pub favorites: Vec<serde_json::Value>,
}

/// What Discord shows while Pawrinth runs. Whether it shows anything at all is upstream's
/// `discord_rpc` setting.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct DiscordSettings {
    pub use_custom: bool,
    pub application_id: Option<String>,
    pub show_instance: bool,
    pub show_version: bool,
    pub show_elapsed: bool,
    pub show_loader_badge: bool,
    pub idle_text: Option<String>,
    pub idle_state_text: Option<String>,
    pub playing_text: Option<String>,
    pub playing_state_text: Option<String>,
    pub logo_image: Option<String>,
    pub logo_text: Option<String>,
    pub button_label: Option<String>,
    pub button_url: Option<String>,
    pub second_button_label: Option<String>,
    pub second_button_url: Option<String>,
}

impl Default for DiscordSettings {
    fn default() -> Self {
        Self {
            use_custom: false,
            application_id: None,
            show_instance: true,
            show_version: true,
            show_elapsed: true,
            show_loader_badge: false,
            idle_text: None,
            idle_state_text: None,
            playing_text: None,
            playing_state_text: None,
            logo_image: None,
            logo_text: None,
            button_label: None,
            button_url: None,
            second_button_label: None,
            second_button_url: None,
        }
    }
}

impl Default for PlusSettings {
    fn default() -> Self {
        Self {
            appearance: AppearanceSettings::default(),
            interface: InterfaceSettings::default(),
            rollback: RollbackSettings::default(),
            diagnostics: DiagnosticsSettings::default(),
            onboarding: OnboardingSettings::default(),
            discovery: DiscoverySettings::default(),
            privacy: PrivacySettings::default(),
            skins: SkinSettings::default(),
            discord: DiscordSettings::default(),
            unknown: serde_json::Map::new(),
        }
    }
}

impl PlusSettings {
    fn normalized(mut self) -> Self {
        let scale = self.interface.ui_scale;
        self.interface.ui_scale = if scale.is_finite() {
            scale.clamp(UI_SCALE_MIN, UI_SCALE_MAX)
        } else {
            1.0
        };
        self.rollback.max_operations_per_instance =
            self.rollback.max_operations_per_instance.max(1);
        self
    }

    pub(crate) async fn read(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> crate::Result<Self> {
        let raw = sqlx::query_scalar!("SELECT plus FROM settings")
            .fetch_one(exec)
            .await?;
        Ok(serde_json::from_str::<Self>(&raw)
            .unwrap_or_default()
            .normalized())
    }

    /// Applies an RFC 7396 merge patch in one statement, so concurrent patches
    /// to different sections can't overwrite each other.
    pub(crate) async fn merge_patch(
        patch: &serde_json::Value,
        pool: &sqlx::SqlitePool,
    ) -> crate::Result<Self> {
        let patch = serde_json::to_string(patch)?;
        sqlx::query!("UPDATE settings SET plus = json_patch(plus, $1)", patch)
            .execute(pool)
            .await?;
        Self::read(pool).await
    }
}

fn with_current_background_path(
    mut settings: PlusSettings,
    state: &State,
) -> PlusSettings {
    if let Some(background) = settings.appearance.background.as_mut()
        && let Some(stored) =
            background.get("path").and_then(serde_json::Value::as_str)
        && let Some(current) = super::backgrounds::current_path(
            stored,
            &state.directories.caches_dir(),
        )
    {
        background["path"] = current.to_string_lossy().into();
    }
    settings
}

pub async fn get() -> crate::Result<PlusSettings> {
    let state = State::get().await?;
    let settings = PlusSettings::read(&state.pool).await?;
    Ok(with_current_background_path(settings, &state))
}

pub async fn patch(patch: serde_json::Value) -> crate::Result<PlusSettings> {
    let state = State::get().await?;
    let settings = PlusSettings::merge_patch(&patch, &state.pool).await?;
    if patch.get("discord").is_some() || patch.get("privacy").is_some() {
        super::discord::refresh_in_background();
    }
    Ok(with_current_background_path(settings, &state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plus::testing::TestDb;
    use serde_json::json;

    #[test]
    fn empty_object_gives_defaults() {
        let settings: PlusSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, PlusSettings::default());
    }

    #[test]
    fn out_of_range_scale_is_clamped() {
        let mut settings = PlusSettings::default();
        settings.interface.ui_scale = 9.0;
        assert_eq!(settings.normalized().interface.ui_scale, UI_SCALE_MAX);
    }

    #[tokio::test]
    async fn patches_persist_and_merge() {
        let db = TestDb::new().await;

        let first = PlusSettings::merge_patch(
            &json!({ "interface": { "reduce_motion": true } }),
            &db.pool,
        )
        .await
        .unwrap();
        assert!(first.interface.reduce_motion);

        let second = PlusSettings::merge_patch(
            &json!({ "rollback": { "retention_days": 30 } }),
            &db.pool,
        )
        .await
        .unwrap();
        assert!(second.interface.reduce_motion);
        assert_eq!(second.rollback.retention_days, 30);

        let reread = PlusSettings::read(&db.pool).await.unwrap();
        assert_eq!(reread, second);
    }

    #[tokio::test]
    async fn unknown_fields_survive_a_round_trip() {
        let db = TestDb::new().await;
        let settings = PlusSettings::merge_patch(
            &json!({ "from_a_newer_build": { "kept": true } }),
            &db.pool,
        )
        .await
        .unwrap();
        assert_eq!(
            settings.unknown.get("from_a_newer_build"),
            Some(&json!({ "kept": true }))
        );
    }
}
