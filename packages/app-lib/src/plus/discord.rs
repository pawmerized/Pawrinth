//! What Pawrinth tells Discord. Presence runs under Pawrinth's own Discord application
//! unless someone chooses their own, and never under Modrinth's. An application ID isn't
//! a secret, because Discord shows it to anyone it's shared with.

use super::settings::{DiscordSettings, PlusSettings, PrivacySettings};
use crate::State;
use crate::state::ModLoader;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const BUILT_IN_APPLICATION_ID: &str = "1566677456579076156";
const BUILT_IN_LOGO_ASSET: &str = "image-1793598490229";
const CUSTOM_LOGO_ASSET: &str = "pawrinth";
const LOGO_TEXT: &str = "Pawrinth";
const DEFAULT_IDLE_TEXT: &str = "Browsing Pawrinth";
const HIDDEN_PLAYING_TEXT: &str = "Playing Minecraft";
const INSTALL_BUTTON_LABEL: &str = "Install Pawrinth";
/// The installer from the newest release on Pawrinth's GitHub. Releases publish it
/// under this fixed name as well as with their version number.
pub(crate) const INSTALLER_URL: &str = "https://github.com/pawmerized/Pawrinth/releases/latest/download/Pawrinth-Setup.exe";
const COMMUNITY_BUTTON_LABEL: &str = "Join the Discord";
/// Pawrinth's community server.
pub(crate) const COMMUNITY_INVITE_URL: &str = "https://discord.gg/HDVG4TmuB7";
const SAMPLE_INSTANCE: &str = "My Instance";
const SAMPLE_GAME_VERSION: &str = "1.21.1";
/// Discord refuses a whole activity if any text is outside these limits.
const MIN_TEXT_CHARS: usize = 2;
const MAX_TEXT_CHARS: usize = 128;
const MAX_BUTTON_LABEL_CHARS: usize = 32;
const MAX_BUTTON_URL_CHARS: usize = 512;
const MAX_IMAGE_CHARS: usize = 256;
const MIN_APPLICATION_ID_LENGTH: usize = 17;
const MAX_APPLICATION_ID_LENGTH: usize = 20;
const API_BASE: &str = "https://discord.com/api/v10/oauth2/applications";
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);
/// Discord allows five updates every twenty seconds, so this stays well inside that.
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayingInstance {
    pub name: String,
    pub game_version: Option<String>,
    pub loader: Option<ModLoader>,
    pub started_at: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PresenceButton {
    pub label: String,
    pub url: String,
}

/// The small picture on the logo. `image` names an art asset on the Discord application.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PresenceBadge {
    pub image: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PresenceContent {
    pub details: String,
    pub state: Option<String>,
    pub started_at: Option<i64>,
    pub logo_image: String,
    pub logo_text: String,
    pub badge: Option<PresenceBadge>,
    pub buttons: Vec<PresenceButton>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationSource {
    BuiltIn,
    Custom,
}

/// Why the last attempt to reach Discord failed, in terms a person can act on.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PresenceProblem {
    NotRunning,
    /// Windows won't let a normal app reach a Discord running as administrator.
    Blocked,
    ApplicationRefused,
    ActivityRefused {
        message: String,
    },
    NoReply,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscordStatus {
    pub enabled: bool,
    pub application: ApplicationSource,
    pub connected: bool,
    pub problem: Option<PresenceProblem>,
    pub install_button: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiscordPreview {
    pub idle: PresenceContent,
    pub playing: PresenceContent,
}

/// `None` means Discord's website couldn't be asked, not that the answer was no.
#[derive(Clone, Debug, Serialize)]
pub struct DiscordCheck {
    pub status: DiscordStatus,
    pub application_known: Option<bool>,
    pub application_name: Option<String>,
    pub logo_image: String,
    pub logo_uploaded: Option<bool>,
}

pub fn is_application_id(id: &str) -> bool {
    (MIN_APPLICATION_ID_LENGTH..=MAX_APPLICATION_ID_LENGTH).contains(&id.len())
        && id.bytes().all(|byte| byte.is_ascii_digit())
}

/// A valid ID only counts in custom mode, so going back to the default is one switch.
pub fn application_id(
    settings: &DiscordSettings,
) -> (ApplicationSource, String) {
    let chosen = settings
        .application_id
        .as_deref()
        .map(str::trim)
        .filter(|id| settings.use_custom && is_application_id(id));
    match chosen {
        Some(id) => (ApplicationSource::Custom, id.to_string()),
        None => (
            ApplicationSource::BuiltIn,
            BUILT_IN_APPLICATION_ID.to_string(),
        ),
    }
}

fn is_web_image(image: &str) -> bool {
    url::Url::parse(image).is_ok_and(|parsed| parsed.scheme() == "https")
}

fn is_asset_name(image: &str) -> bool {
    !image.is_empty()
        && image
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
}

fn logo_image(settings: &DiscordSettings) -> String {
    let chosen =
        settings
            .logo_image
            .as_deref()
            .map(str::trim)
            .filter(|image| {
                settings.use_custom
                    && image.chars().count() <= MAX_IMAGE_CHARS
                    && (is_web_image(image) || is_asset_name(image))
            });
    match chosen {
        Some(image) if is_web_image(image) => image.to_string(),
        Some(name) => name.to_lowercase(),
        None if application_id(settings).0 == ApplicationSource::Custom => {
            CUSTOM_LOGO_ASSET.to_string()
        }
        None => BUILT_IN_LOGO_ASSET.to_string(),
    }
}

fn fitted(text: &str, max_chars: usize) -> Option<String> {
    let text = text.trim();
    (text.chars().count() >= MIN_TEXT_CHARS)
        .then(|| text.chars().take(max_chars).collect())
}

fn custom_text(text: Option<&str>) -> Option<String> {
    fitted(text?, MAX_TEXT_CHARS)
}

fn loader_name(loader: ModLoader) -> Option<&'static str> {
    match loader {
        ModLoader::Vanilla => None,
        ModLoader::Forge => Some("Forge"),
        ModLoader::Fabric => Some("Fabric"),
        ModLoader::Quilt => Some("Quilt"),
        ModLoader::NeoForge => Some("NeoForge"),
    }
}

fn loader_badge(loader: ModLoader) -> Option<PresenceBadge> {
    let name = loader_name(loader)?;
    Some(PresenceBadge {
        image: name.to_lowercase(),
        text: name.to_string(),
    })
}

fn game_line(instance: &PlayingInstance, with_loader: bool) -> Option<String> {
    let version = instance.game_version.as_deref()?;
    let loader = instance
        .loader
        .and_then(loader_name)
        .filter(|_| with_loader);
    Some(match loader {
        Some(loader) => format!("Minecraft {version} · {loader}"),
        None => format!("Minecraft {version}"),
    })
}

fn filled_template(template: &str, instance: &PlayingInstance) -> String {
    template
        .replace("{instance}", &instance.name)
        .replace("{version}", instance.game_version.as_deref().unwrap_or(""))
        .replace(
            "{loader}",
            instance.loader.and_then(loader_name).unwrap_or(""),
        )
}

fn button_from(label: &str, url: &str) -> Option<PresenceButton> {
    let label = fitted(label, MAX_BUTTON_LABEL_CHARS)?;
    let url = url.trim();
    let parsed = url::Url::parse(url).ok()?;
    (parsed.scheme() == "https" && url.len() <= MAX_BUTTON_URL_CHARS).then(
        || PresenceButton {
            label,
            url: url.to_string(),
        },
    )
}

fn install_button(download_url: Option<&str>) -> Option<PresenceButton> {
    button_from(INSTALL_BUTTON_LABEL, download_url?)
}

fn custom_buttons(settings: &DiscordSettings) -> Vec<PresenceButton> {
    [
        (&settings.button_label, &settings.button_url),
        (&settings.second_button_label, &settings.second_button_url),
    ]
    .into_iter()
    .filter_map(|(label, url)| button_from(label.as_deref()?, url.as_deref()?))
    .collect()
}

fn presence_buttons(settings: &DiscordSettings) -> Vec<PresenceButton> {
    if settings.use_custom {
        custom_buttons(settings)
    } else {
        install_button(Some(INSTALLER_URL))
            .into_iter()
            .chain(button_from(COMMUNITY_BUTTON_LABEL, COMMUNITY_INVITE_URL))
            .collect()
    }
}

/// A line that asks for something hidden falls back to the default, so a saved template
/// can't put the instance back on Discord.
fn custom_playing_line(
    template: Option<&str>,
    instance: &PlayingInstance,
    show_instance: bool,
    show_version: bool,
) -> Option<String> {
    let template = template.filter(|template| {
        (show_instance || !template.contains("{instance}"))
            && (show_version
                || !(template.contains("{version}")
                    || template.contains("{loader}")))
    })?;
    custom_text(Some(&filled_template(template, instance)))
}

/// Streamer mode and the display settings keep the instance's name and game off Discord,
/// where anyone looking at the profile would see them.
pub fn presence_content(
    settings: &DiscordSettings,
    privacy: &PrivacySettings,
    playing: Option<&PlayingInstance>,
) -> PresenceContent {
    let custom = settings.use_custom;
    let buttons = presence_buttons(settings);
    let logo_image = logo_image(settings);
    let logo_text = custom
        .then(|| custom_text(settings.logo_text.as_deref()))
        .flatten()
        .unwrap_or_else(|| LOGO_TEXT.to_string());
    let Some(instance) = playing else {
        return PresenceContent {
            details: custom
                .then(|| custom_text(settings.idle_text.as_deref()))
                .flatten()
                .unwrap_or_else(|| DEFAULT_IDLE_TEXT.to_string()),
            state: custom
                .then(|| custom_text(settings.idle_state_text.as_deref()))
                .flatten(),
            started_at: None,
            logo_image,
            logo_text,
            badge: None,
            buttons,
        };
    };
    let started_at = instance.started_at.filter(|_| settings.show_elapsed);
    let show_instance = settings.show_instance && !privacy.streamer_mode;
    let show_version = settings.show_version && !privacy.streamer_mode;
    if !show_instance && !show_version {
        return PresenceContent {
            details: HIDDEN_PLAYING_TEXT.to_string(),
            state: None,
            started_at,
            logo_image,
            logo_text,
            badge: None,
            buttons,
        };
    }
    let badge = instance
        .loader
        .and_then(loader_badge)
        .filter(|_| custom && settings.show_loader_badge && show_version);
    let custom_line = |template: &Option<String>| {
        custom
            .then(|| {
                custom_playing_line(
                    template.as_deref(),
                    instance,
                    show_instance,
                    show_version,
                )
            })
            .flatten()
    };
    let default_details = if show_instance {
        fitted(&format!("Playing {}", instance.name), MAX_TEXT_CHARS)
    } else {
        game_line(instance, badge.is_none())
            .map(|line| format!("Playing {line}"))
    };
    let default_state = game_line(instance, badge.is_none())
        .filter(|_| show_instance && show_version);
    PresenceContent {
        details: custom_line(&settings.playing_text)
            .or(default_details)
            .unwrap_or_else(|| HIDDEN_PLAYING_TEXT.to_string()),
        state: custom_line(&settings.playing_state_text).or(default_state),
        started_at,
        logo_image,
        logo_text,
        badge,
        buttons,
    }
}

pub async fn status() -> crate::Result<DiscordStatus> {
    let state = State::get().await?;
    let settings = crate::state::Settings::get(&state.pool).await?;
    let plus = PlusSettings::read(&state.pool).await?;
    let guard = &state.presence.discord_rpc;
    Ok(DiscordStatus {
        enabled: settings.discord_rpc,
        application: application_id(&plus.discord).0,
        connected: guard.is_connected(),
        problem: guard.problem(),
        install_button: install_button(Some(INSTALLER_URL)).is_some(),
    })
}

/// Goes through the same steps as the real presence, so what's previewed is what's sent.
pub async fn preview() -> crate::Result<DiscordPreview> {
    let state = State::get().await?;
    let plus = PlusSettings::read(&state.pool).await?;
    let sample = PlayingInstance {
        name: SAMPLE_INSTANCE.to_string(),
        game_version: Some(SAMPLE_GAME_VERSION.to_string()),
        loader: Some(ModLoader::Fabric),
        started_at: Some(chrono::Utc::now().timestamp()),
    };
    Ok(DiscordPreview {
        idle: presence_content(&plus.discord, &plus.privacy, None),
        playing: presence_content(&plus.discord, &plus.privacy, Some(&sample)),
    })
}

/// Sends whatever should be showing now, after a setting changed.
pub async fn refresh() -> crate::Result<()> {
    let state = State::get().await?;
    state
        .presence
        .discord_rpc
        .clear_to_default(&state.pool, &state.process_manager)
        .await
}

/// For when something else shouldn't wait on Discord, which can take a while to answer.
pub fn refresh_in_background() {
    tokio::spawn(async {
        if let Err(error) = refresh().await {
            tracing::warn!("Couldn't update Discord: {error}");
        }
    });
}

/// Launching a game shouldn't wait on Discord either.
pub fn show_playing_in_background(instance: PlayingInstance) {
    tokio::spawn(async move {
        let result = async {
            let state = State::get().await?;
            state
                .presence
                .discord_rpc
                .show_playing(instance, &state.pool)
                .await
        };
        if let Err(error) = result.await {
            tracing::warn!("Couldn't update Discord: {error}");
        }
    });
}

#[derive(Deserialize)]
struct ApplicationInfo {
    name: Option<String>,
}

#[derive(Deserialize)]
struct ArtAsset {
    name: String,
}

enum Lookup<T> {
    Found(T),
    Unknown,
    Unavailable,
}

async fn lookup<T: serde::de::DeserializeOwned>(path: &str) -> Lookup<T> {
    let response = crate::util::fetch::REQWEST_CLIENT
        .get(format!("{API_BASE}/{path}"))
        .timeout(LOOKUP_TIMEOUT)
        .send()
        .await;
    let Ok(response) = response else {
        return Lookup::Unavailable;
    };
    let status = response.status().as_u16();
    if status == 400 || status == 404 {
        return Lookup::Unknown;
    }
    if !response.status().is_success() {
        return Lookup::Unavailable;
    }
    match response.json().await {
        Ok(value) => Lookup::Found(value),
        Err(_) => Lookup::Unavailable,
    }
}

/// Whether Discord's website knows this application, or `None` if it couldn't be asked.
pub(crate) async fn application_known(id: &str) -> Option<bool> {
    match lookup::<ApplicationInfo>(&format!("{id}/rpc")).await {
        Lookup::Found(_) => Some(true),
        Lookup::Unknown => Some(false),
        Lookup::Unavailable => None,
    }
}

/// Tries Discord right away, then asks its website about the application and its pictures,
/// which are public. A picture given as a web address can't be looked up.
pub async fn check() -> crate::Result<DiscordCheck> {
    refresh().await?;
    let state = State::get().await?;
    let plus = PlusSettings::read(&state.pool).await?;
    let (_, id) = application_id(&plus.discord);
    let logo_image = logo_image(&plus.discord);
    let (application_known, application_name) =
        match lookup::<ApplicationInfo>(&format!("{id}/rpc")).await {
            Lookup::Found(info) => (Some(true), info.name),
            Lookup::Unknown => (Some(false), None),
            Lookup::Unavailable => (None, None),
        };
    let logo_uploaded = if is_web_image(&logo_image) {
        None
    } else {
        match lookup::<Vec<ArtAsset>>(&format!("{id}/assets")).await {
            Lookup::Found(assets) => {
                Some(assets.iter().any(|asset| asset.name == logo_image))
            }
            Lookup::Unknown => Some(false),
            Lookup::Unavailable => None,
        }
    };
    Ok(DiscordCheck {
        status: status().await?,
        application_known,
        application_name,
        logo_image,
        logo_uploaded,
    })
}

/// Picks Discord up when it starts after Pawrinth, or comes back after a restart.
pub fn start_keep_alive() {
    tokio::spawn(async {
        let mut ticks = tokio::time::interval(KEEP_ALIVE_INTERVAL);
        ticks.tick().await;
        loop {
            ticks.tick().await;
            let Ok(state) = State::get().await else {
                continue;
            };
            if let Err(error) = state
                .presence
                .discord_rpc
                .keep_alive(&state.pool, &state.process_manager)
                .await
            {
                tracing::debug!("Couldn't update Discord: {error}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plus::testing::TestDb;
    use crate::state::DiscordGuard;
    use serde_json::{Value, json};

    const ID: &str = "1234567890123456789";

    fn defaults() -> DiscordSettings {
        DiscordSettings::default()
    }

    fn customized() -> DiscordSettings {
        DiscordSettings {
            use_custom: true,
            ..DiscordSettings::default()
        }
    }

    fn privacy() -> PrivacySettings {
        PrivacySettings::default()
    }

    fn streamer() -> PrivacySettings {
        PrivacySettings {
            streamer_mode: true,
            ..PrivacySettings::default()
        }
    }

    fn instance() -> PlayingInstance {
        PlayingInstance {
            name: "Skyblock".to_string(),
            game_version: Some("1.20.1".to_string()),
            loader: Some(ModLoader::Fabric),
            started_at: Some(1_000),
        }
    }

    #[test]
    fn real_application_ids_are_recognized() {
        assert!(is_application_id(ID));
        assert!(is_application_id(BUILT_IN_APPLICATION_ID));
        for bad in ["", "123", "not-a-number-12345678", &"1".repeat(21)] {
            assert!(!is_application_id(bad), "{bad}");
        }
    }

    #[test]
    fn the_default_needs_no_setup_and_never_uses_modrinths() {
        let (source, id) = application_id(&defaults());
        assert_eq!(source, ApplicationSource::BuiltIn);
        assert_eq!(id, BUILT_IN_APPLICATION_ID);
        assert_ne!(id, "1123683254248148992");
    }

    #[test]
    fn a_saved_application_only_counts_in_custom_mode() {
        let mut saved = customized();
        saved.application_id = Some(format!(" {ID} "));
        assert_eq!(
            application_id(&saved),
            (ApplicationSource::Custom, ID.to_string())
        );

        saved.use_custom = false;
        assert_eq!(application_id(&saved).0, ApplicationSource::BuiltIn);
    }

    #[test]
    fn a_bad_application_goes_back_to_the_built_in_one() {
        let mut saved = customized();
        saved.application_id = Some("nonsense".to_string());
        assert_eq!(
            application_id(&saved),
            (
                ApplicationSource::BuiltIn,
                BUILT_IN_APPLICATION_ID.to_string()
            )
        );
    }

    #[test]
    fn idle_has_friendly_defaults() {
        let content = presence_content(&defaults(), &privacy(), None);
        assert_eq!(content.details, "Browsing Pawrinth");
        assert_eq!(content.state, None);
        assert_eq!(content.started_at, None);
        assert_eq!(content.logo_image, BUILT_IN_LOGO_ASSET);
        assert_eq!(content.logo_text, "Pawrinth");
    }

    #[test]
    fn custom_text_is_ignored_in_default_mode() {
        let mut saved = defaults();
        saved.idle_text = Some("Picking a modpack".to_string());
        saved.playing_text = Some("In {instance}".to_string());
        saved.logo_text = Some("Something else".to_string());
        saved.button_label = Some("Open".to_string());
        saved.button_url = Some("https://example.com".to_string());
        let idle = presence_content(&saved, &privacy(), None);
        assert_eq!(idle.details, "Browsing Pawrinth");
        assert_eq!(idle.logo_text, "Pawrinth");
        assert!(!idle.buttons.iter().any(|button| button.label == "Open"));
        assert_eq!(
            presence_content(&saved, &privacy(), Some(&instance())).details,
            "Playing Skyblock"
        );
    }

    #[test]
    fn idle_text_can_be_custom() {
        let mut custom = customized();
        custom.idle_text = Some("  Picking a modpack  ".to_string());
        assert_eq!(
            presence_content(&custom, &privacy(), None).details,
            "Picking a modpack"
        );
    }

    #[test]
    fn text_discord_would_refuse_falls_back() {
        let mut custom = customized();
        custom.idle_text = Some("x".to_string());
        assert_eq!(
            presence_content(&custom, &privacy(), None).details,
            "Browsing Pawrinth"
        );

        custom.idle_text = Some("y".repeat(500));
        assert_eq!(
            presence_content(&custom, &privacy(), None)
                .details
                .chars()
                .count(),
            MAX_TEXT_CHARS
        );
    }

    #[test]
    fn playing_shows_the_instance_and_game() {
        let content =
            presence_content(&defaults(), &privacy(), Some(&instance()));
        assert_eq!(content.details, "Playing Skyblock");
        assert_eq!(content.state.as_deref(), Some("Minecraft 1.20.1 · Fabric"));
        assert_eq!(content.started_at, Some(1_000));
    }

    #[test]
    fn the_version_can_be_hidden_on_its_own() {
        let mut settings = defaults();
        settings.show_version = false;
        let content =
            presence_content(&settings, &privacy(), Some(&instance()));
        assert_eq!(content.details, "Playing Skyblock");
        assert_eq!(content.state, None);
    }

    #[test]
    fn the_instance_can_be_hidden_on_its_own() {
        let mut settings = defaults();
        settings.show_instance = false;
        let content =
            presence_content(&settings, &privacy(), Some(&instance()));
        assert_eq!(content.details, "Playing Minecraft 1.20.1 · Fabric");
        assert_eq!(content.state, None);
        assert!(!format!("{content:?}").contains("Skyblock"));
    }

    #[test]
    fn hiding_both_only_says_playing_minecraft() {
        let mut settings = defaults();
        settings.show_instance = false;
        settings.show_version = false;
        let content =
            presence_content(&settings, &privacy(), Some(&instance()));
        assert_eq!(content.details, "Playing Minecraft");
        assert_eq!(content.state, None);
    }

    #[test]
    fn a_template_cannot_bring_back_what_is_hidden() {
        let mut custom = customized();
        custom.playing_text = Some("In {instance}".to_string());
        custom.playing_state_text = Some("On {version}".to_string());

        custom.show_instance = false;
        assert_eq!(
            presence_content(&custom, &privacy(), Some(&instance())).details,
            "Playing Minecraft 1.20.1 · Fabric"
        );

        custom.show_instance = true;
        custom.show_version = false;
        let content = presence_content(&custom, &privacy(), Some(&instance()));
        assert_eq!(content.details, "In Skyblock");
        assert_eq!(content.state, None);
    }

    #[test]
    fn vanilla_has_no_loader_to_show() {
        let mut vanilla = instance();
        vanilla.loader = Some(ModLoader::Vanilla);
        let content = presence_content(&defaults(), &privacy(), Some(&vanilla));
        assert_eq!(content.state.as_deref(), Some("Minecraft 1.20.1"));
    }

    #[test]
    fn an_unknown_game_shows_no_game_line() {
        let mut unknown = instance();
        unknown.game_version = None;
        let content = presence_content(&defaults(), &privacy(), Some(&unknown));
        assert_eq!(content.state, None);
    }

    #[test]
    fn playing_text_fills_in_its_tokens() {
        let mut custom = customized();
        custom.playing_text =
            Some("In {instance} on {version} with {loader}".to_string());
        assert_eq!(
            presence_content(&custom, &privacy(), Some(&instance())).details,
            "In Skyblock on 1.20.1 with Fabric"
        );
    }

    #[test]
    fn streamer_mode_keeps_the_instance_off_discord() {
        let mut custom = customized();
        custom.playing_text = Some("In {instance}".to_string());
        custom.playing_state_text = Some("On {version}".to_string());
        custom.show_loader_badge = true;
        let content = presence_content(&custom, &streamer(), Some(&instance()));
        assert_eq!(content.details, "Playing Minecraft");
        assert_eq!(content.state, None);
        assert_eq!(content.badge, None);
        assert!(!format!("{content:?}").contains("Skyblock"));
    }

    #[test]
    fn elapsed_time_can_be_turned_off() {
        let mut settings = defaults();
        settings.show_elapsed = false;
        let content =
            presence_content(&settings, &privacy(), Some(&instance()));
        assert_eq!(content.started_at, None);
    }

    #[test]
    fn the_second_line_can_be_custom() {
        let mut custom = customized();
        custom.idle_state_text = Some("Choosing mods".to_string());
        custom.playing_state_text =
            Some("On {version} with {loader}".to_string());
        assert_eq!(
            presence_content(&custom, &privacy(), None).state.as_deref(),
            Some("Choosing mods")
        );
        assert_eq!(
            presence_content(&custom, &privacy(), Some(&instance()))
                .state
                .as_deref(),
            Some("On 1.20.1 with Fabric")
        );
    }

    #[test]
    fn a_second_line_discord_would_refuse_falls_back() {
        let mut custom = customized();
        custom.idle_state_text = Some("x".to_string());
        custom.playing_state_text = Some("{loader}".to_string());
        assert_eq!(presence_content(&custom, &privacy(), None).state, None);

        let mut vanilla = instance();
        vanilla.loader = Some(ModLoader::Vanilla);
        assert_eq!(
            presence_content(&custom, &privacy(), Some(&vanilla))
                .state
                .as_deref(),
            Some("Minecraft 1.20.1")
        );
    }

    #[test]
    fn the_logo_text_can_be_custom() {
        let mut custom = customized();
        assert_eq!(
            presence_content(&custom, &privacy(), None).logo_text,
            "Pawrinth"
        );
        custom.logo_text = Some("Pawrinth Launcher".to_string());
        assert_eq!(
            presence_content(&custom, &privacy(), Some(&instance())).logo_text,
            "Pawrinth Launcher"
        );
    }

    #[test]
    fn the_logo_picture_follows_the_application() {
        assert_eq!(logo_image(&defaults()), BUILT_IN_LOGO_ASSET);

        let mut custom = customized();
        assert_eq!(logo_image(&custom), BUILT_IN_LOGO_ASSET);

        custom.application_id = Some(ID.to_string());
        assert_eq!(logo_image(&custom), CUSTOM_LOGO_ASSET);

        custom.logo_image = Some(" My_Logo ".to_string());
        assert_eq!(logo_image(&custom), "my_logo");

        custom.logo_image = Some("https://example.com/a.png".to_string());
        assert_eq!(logo_image(&custom), "https://example.com/a.png");
    }

    #[test]
    fn a_picture_discord_cannot_use_is_ignored() {
        let mut custom = customized();
        for bad in ["has spaces", "http://example.com/a.png", "", "a/b"] {
            custom.logo_image = Some(bad.to_string());
            assert_eq!(logo_image(&custom), BUILT_IN_LOGO_ASSET, "{bad}");
        }
        custom.logo_image = Some("x".repeat(MAX_IMAGE_CHARS + 1));
        assert_eq!(logo_image(&custom), BUILT_IN_LOGO_ASSET);
    }

    #[test]
    fn the_loader_badge_is_off_until_its_pictures_are_uploaded() {
        let content =
            presence_content(&customized(), &privacy(), Some(&instance()));
        assert_eq!(content.badge, None);
    }

    #[test]
    fn the_loader_badge_is_custom_only() {
        let mut settings = defaults();
        settings.show_loader_badge = true;
        let content =
            presence_content(&settings, &privacy(), Some(&instance()));
        assert_eq!(content.badge, None);
    }

    #[test]
    fn the_loader_badge_moves_the_loader_off_the_second_line() {
        let mut custom = customized();
        custom.show_loader_badge = true;
        let content = presence_content(&custom, &privacy(), Some(&instance()));
        assert_eq!(
            content.badge,
            Some(PresenceBadge {
                image: "fabric".to_string(),
                text: "Fabric".to_string(),
            })
        );
        assert_eq!(content.state.as_deref(), Some("Minecraft 1.20.1"));
    }

    #[test]
    fn every_loader_badge_names_a_picture_that_ships() {
        for (loader, image) in [
            (ModLoader::Fabric, "fabric"),
            (ModLoader::Forge, "forge"),
            (ModLoader::NeoForge, "neoforge"),
            (ModLoader::Quilt, "quilt"),
        ] {
            assert_eq!(loader_badge(loader).unwrap().image, image);
        }
        assert_eq!(loader_badge(ModLoader::Vanilla), None);
    }

    #[test]
    fn the_loader_badge_never_shows_when_browsing_or_hidden() {
        let mut custom = customized();
        custom.show_loader_badge = true;
        assert_eq!(presence_content(&custom, &privacy(), None).badge, None);

        custom.show_version = false;
        assert_eq!(
            presence_content(&custom, &privacy(), Some(&instance())).badge,
            None
        );
    }

    #[test]
    fn the_built_in_status_offers_the_installer_and_the_community() {
        let buttons = presence_buttons(&DiscordSettings::default());
        assert_eq!(
            buttons,
            vec![
                PresenceButton {
                    label: "Install Pawrinth".to_string(),
                    url: INSTALLER_URL.to_string(),
                },
                PresenceButton {
                    label: "Join the Discord".to_string(),
                    url: COMMUNITY_INVITE_URL.to_string(),
                },
            ]
        );
        assert!(
            INSTALLER_URL
                .starts_with("https://github.com/pawmerized/Pawrinth/")
        );
        assert!(COMMUNITY_INVITE_URL.starts_with("https://discord.gg/"));
    }

    #[test]
    fn the_install_button_needs_a_safe_address() {
        assert_eq!(install_button(None), None);
        assert_eq!(install_button(Some("")), None);
        assert_eq!(install_button(Some("http://example.com")), None);
        assert_eq!(install_button(Some("not a url")), None);
        assert_eq!(
            install_button(Some("https://example.com/download")),
            Some(PresenceButton {
                label: "Install Pawrinth".to_string(),
                url: "https://example.com/download".to_string(),
            })
        );
    }

    #[test]
    fn buttons_need_a_label_and_a_safe_address() {
        let with = |label: &str, url: &str| {
            let mut custom = customized();
            custom.button_label = Some(label.to_string());
            custom.button_url = Some(url.to_string());
            presence_content(&custom, &privacy(), None).buttons
        };
        assert_eq!(
            with("Get Pawrinth", "https://example.com/a"),
            vec![PresenceButton {
                label: "Get Pawrinth".to_string(),
                url: "https://example.com/a".to_string(),
            }]
        );
        assert!(with("x", "https://example.com").is_empty());
        assert!(with("Open", "http://example.com").is_empty());
        assert!(with("Open", "javascript:alert(1)").is_empty());
        assert!(with("Open", "not a url").is_empty());
        let long_label = with(&"z".repeat(50), "https://example.com");
        assert_eq!(long_label[0].label.chars().count(), MAX_BUTTON_LABEL_CHARS);
    }

    #[test]
    fn two_buttons_can_be_custom() {
        let mut custom = customized();
        custom.button_label = Some("First".to_string());
        custom.button_url = Some("https://example.com/1".to_string());
        custom.second_button_label = Some("Second".to_string());
        custom.second_button_url = Some("https://example.com/2".to_string());
        let labels: Vec<String> = presence_content(&custom, &privacy(), None)
            .buttons
            .into_iter()
            .map(|button| button.label)
            .collect();
        assert_eq!(labels, ["First", "Second"]);
    }

    async fn captured_after(
        patch: Value,
        action: impl AsyncFnOnce(&DiscordGuard, &sqlx::SqlitePool),
    ) -> Vec<Value> {
        let folder = tempfile::tempdir().unwrap();
        let capture = folder.path().join("discord.jsonl");
        // SAFETY: nextest runs each test in its own process, before any other thread reads it.
        unsafe {
            std::env::set_var("PAWRINTH_DISCORD_CAPTURE", &capture);
        }
        let db = TestDb::new().await;
        PlusSettings::merge_patch(&patch, &db.pool).await.unwrap();
        let guard = DiscordGuard::init().unwrap();
        action(&guard, &db.pool).await;
        std::fs::read_to_string(&capture)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn the_default_goes_out_with_no_setup() {
        let captured = captured_after(json!({}), async |guard, pool| {
            guard.show_playing(instance(), pool).await.unwrap();
        })
        .await;
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0]["application_id"], BUILT_IN_APPLICATION_ID);
        let activity = &captured[0]["activity"];
        assert_eq!(activity["details"], "Playing Skyblock");
        assert_eq!(activity["state"], "Minecraft 1.20.1 · Fabric");
        assert_eq!(activity["timestamps"]["start"], 1_000);
        assert_eq!(activity["assets"]["large_image"], BUILT_IN_LOGO_ASSET);
        assert_eq!(activity["assets"]["large_text"], "Pawrinth");
        assert!(activity["assets"].get("small_image").is_none());
    }

    #[tokio::test]
    async fn custom_settings_go_out_under_the_chosen_application() {
        let captured = captured_after(
            json!({
                "discord": {
                    "use_custom": true,
                    "application_id": ID,
                    "show_loader_badge": true,
                    "logo_image": "my_logo",
                },
            }),
            async |guard, pool| {
                guard.show_playing(instance(), pool).await.unwrap();
            },
        )
        .await;
        assert_eq!(captured[0]["application_id"], ID);
        let assets = &captured[0]["activity"]["assets"];
        assert_eq!(assets["large_image"], "my_logo");
        assert_eq!(assets["small_image"], "fabric");
        assert_eq!(assets["small_text"], "Fabric");
    }

    #[tokio::test]
    async fn nothing_pretends_to_be_a_party_or_an_end_time() {
        let captured = captured_after(json!({}), async |guard, pool| {
            guard.show_playing(instance(), pool).await.unwrap();
        })
        .await;
        let activity = &captured[0]["activity"];
        assert!(activity.get("party").is_none());
        assert!(activity.get("secrets").is_none());
        assert!(activity["timestamps"].get("end").is_none());
    }

    #[tokio::test]
    async fn streamer_mode_applies_to_what_is_really_sent() {
        let captured = captured_after(
            json!({ "privacy": { "streamer_mode": true } }),
            async |guard, pool| {
                guard.show_playing(instance(), pool).await.unwrap();
            },
        )
        .await;
        assert_eq!(captured[0]["activity"]["details"], "Playing Minecraft");
        assert!(!captured[0].to_string().contains("Skyblock"));
    }

    #[tokio::test]
    async fn turning_it_off_clears_the_status() {
        let folder = tempfile::tempdir().unwrap();
        let capture = folder.path().join("discord.jsonl");
        // SAFETY: nextest runs each test in its own process, before any other thread reads it.
        unsafe {
            std::env::set_var("PAWRINTH_DISCORD_CAPTURE", &capture);
        }
        let db = TestDb::new().await;
        sqlx::query("UPDATE settings SET discord_rpc = 0")
            .execute(&db.pool)
            .await
            .unwrap();
        let guard = DiscordGuard::init().unwrap();
        guard.show_playing(instance(), &db.pool).await.unwrap();
        let captured = std::fs::read_to_string(&capture).unwrap_or_default();
        assert!(!captured.contains("activity"));
        assert!(!guard.is_connected());
    }
}
