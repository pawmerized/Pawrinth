//! Settings from the official Modrinth App. Its database is only read from a copy (see
//! `api::pack::import::modrinth_app`), and nothing is changed until someone picks which
//! settings to bring over. Its data folder, telemetry and ad choices are never copied:
//! Pawrinth keeps its own data apart, and keeps the privacy choices made in Pawrinth.

use crate::State;
use crate::api::pack::import::modrinth_app;
use crate::state::{DefaultPage, Settings, Theme, WindowSize};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::{Connection, Row};
use std::path::Path;

/// Every setting that can be brought over, in the order they're shown.
pub const KEYS: [&str; 19] = [
    "theme",
    "locale",
    "default_page",
    "collapsed_navigation",
    "toggle_sidebar",
    "advanced_rendering",
    "native_decorations",
    "discord_rpc",
    "hide_on_process_start",
    "max_concurrent_downloads",
    "max_concurrent_writes",
    "memory",
    "force_fullscreen",
    "game_resolution",
    "extra_launch_args",
    "custom_env_vars",
    "hook_pre_launch",
    "hook_wrapper",
    "hook_post_exit",
];

#[derive(Clone, Debug, Serialize)]
pub struct SettingChange {
    pub key: String,
    pub current: Value,
    pub incoming: Value,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SettingsPreview {
    /// Whether the Modrinth App is on this computer.
    pub found: bool,
    /// Settings whose value there differs from Pawrinth's.
    pub changes: Vec<SettingChange>,
    /// How many settings already match.
    pub matching: usize,
}

fn integer(row: &sqlx::sqlite::SqliteRow, column: &str) -> Option<i64> {
    row.try_get::<Option<i64>, _>(column).ok().flatten()
}

fn flag(row: &sqlx::sqlite::SqliteRow, column: &str) -> Option<Value> {
    integer(row, column).map(|value| json!(value == 1))
}

fn text(row: &sqlx::sqlite::SqliteRow, column: &str) -> Option<String> {
    row.try_get::<Option<String>, _>(column).ok().flatten()
}

/// The official app's settings, in the same shape Pawrinth shows its own. Columns an
/// older or newer version of that app doesn't have are left out.
async fn read(data_folder: &Path) -> crate::Result<Vec<(String, Value)>> {
    let (_copy, database) = modrinth_app::copy_database(data_folder).await?;
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&database)
            .read_only(true),
    )
    .await?;
    let Some(row) = sqlx::query("SELECT * FROM settings LIMIT 1")
        .fetch_optional(&mut connection)
        .await?
    else {
        return Ok(Vec::new());
    };
    let lists = sqlx::query(
        "SELECT json(extra_launch_args) AS args, json(custom_env_vars) AS env FROM settings LIMIT 1",
    )
    .fetch_optional(&mut connection)
    .await
    .ok()
    .flatten();
    let list = |column: &str| {
        lists
            .as_ref()
            .and_then(|lists| text(lists, column))
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    };

    let mut values = Vec::new();
    let mut put = |key: &str, value: Option<Value>| {
        if let Some(value) = value {
            values.push((key.to_string(), value));
        }
    };
    put(
        "theme",
        text(&row, "theme")
            .map(|theme| json!(Theme::from_string(&theme).as_str())),
    );
    put("locale", text(&row, "locale").map(Value::from));
    put(
        "default_page",
        text(&row, "default_page")
            .map(|page| json!(DefaultPage::from_string(&page).as_str())),
    );
    put("collapsed_navigation", flag(&row, "collapsed_navigation"));
    put("toggle_sidebar", flag(&row, "toggle_sidebar"));
    put("advanced_rendering", flag(&row, "advanced_rendering"));
    put("native_decorations", flag(&row, "native_decorations"));
    put("discord_rpc", flag(&row, "discord_rpc"));
    put("hide_on_process_start", flag(&row, "hide_on_process_start"));
    put(
        "max_concurrent_downloads",
        integer(&row, "max_concurrent_downloads")
            .filter(|count| (1..=64).contains(count))
            .map(Value::from),
    );
    put(
        "max_concurrent_writes",
        integer(&row, "max_concurrent_writes")
            .filter(|count| (1..=64).contains(count))
            .map(Value::from),
    );
    put(
        "memory",
        integer(&row, "mc_memory_max")
            .filter(|memory| *memory >= 512)
            .map(Value::from),
    );
    put("force_fullscreen", flag(&row, "mc_force_fullscreen"));
    if let (Some(width), Some(height)) = (
        integer(&row, "mc_game_resolution_x"),
        integer(&row, "mc_game_resolution_y"),
    ) && let (Ok(width), Ok(height)) =
        (u16::try_from(width), u16::try_from(height))
    {
        put("game_resolution", Some(json!([width, height])));
    }
    put(
        "extra_launch_args",
        list("args").filter(|args| {
            args.as_array()
                .is_some_and(|args| args.iter().all(Value::is_string))
        }),
    );
    put(
        "custom_env_vars",
        list("env").filter(|vars| {
            serde_json::from_value::<Vec<(String, String)>>(vars.clone())
                .is_ok()
        }),
    );
    put(
        "hook_pre_launch",
        Some(json!(text(&row, "hook_pre_launch").unwrap_or_default())),
    );
    put(
        "hook_wrapper",
        Some(json!(text(&row, "hook_wrapper").unwrap_or_default())),
    );
    put(
        "hook_post_exit",
        Some(json!(text(&row, "hook_post_exit").unwrap_or_default())),
    );
    Ok(values)
}

fn current(settings: &Settings, key: &str) -> Value {
    match key {
        "theme" => json!(settings.theme.as_str()),
        "locale" => json!(settings.locale),
        "default_page" => json!(settings.default_page.as_str()),
        "collapsed_navigation" => json!(settings.collapsed_navigation),
        "toggle_sidebar" => json!(settings.toggle_sidebar),
        "advanced_rendering" => json!(settings.advanced_rendering),
        "native_decorations" => json!(settings.native_decorations),
        "discord_rpc" => json!(settings.discord_rpc),
        "hide_on_process_start" => json!(settings.hide_on_process_start),
        "max_concurrent_downloads" => json!(settings.max_concurrent_downloads),
        "max_concurrent_writes" => json!(settings.max_concurrent_writes),
        "memory" => json!(settings.memory.maximum),
        "force_fullscreen" => json!(settings.force_fullscreen),
        "game_resolution" => {
            json!([settings.game_resolution.0, settings.game_resolution.1])
        }
        "extra_launch_args" => json!(settings.extra_launch_args),
        "custom_env_vars" => json!(settings.custom_env_vars),
        "hook_pre_launch" => {
            json!(settings.hooks.pre_launch.clone().unwrap_or_default())
        }
        "hook_wrapper" => {
            json!(settings.hooks.wrapper.clone().unwrap_or_default())
        }
        "hook_post_exit" => {
            json!(settings.hooks.post_exit.clone().unwrap_or_default())
        }
        _ => Value::Null,
    }
}

fn hook(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
}

/// Sets one setting from the official app's value. Values were checked when read.
fn assign(settings: &mut Settings, key: &str, value: &Value) {
    let as_bool = value.as_bool();
    let as_count = value.as_u64().and_then(|count| usize::try_from(count).ok());
    match key {
        "theme" => {
            if let Some(theme) = value.as_str() {
                settings.theme = Theme::from_string(theme);
            }
        }
        "locale" => {
            if let Some(locale) = value.as_str() {
                settings.locale = locale.to_string();
            }
        }
        "default_page" => {
            if let Some(page) = value.as_str() {
                settings.default_page = DefaultPage::from_string(page);
            }
        }
        "collapsed_navigation" => {
            settings.collapsed_navigation =
                as_bool.unwrap_or(settings.collapsed_navigation);
        }
        "toggle_sidebar" => {
            settings.toggle_sidebar =
                as_bool.unwrap_or(settings.toggle_sidebar);
        }
        "advanced_rendering" => {
            settings.advanced_rendering =
                as_bool.unwrap_or(settings.advanced_rendering);
        }
        "native_decorations" => {
            settings.native_decorations =
                as_bool.unwrap_or(settings.native_decorations);
        }
        "discord_rpc" => {
            settings.discord_rpc = as_bool.unwrap_or(settings.discord_rpc);
        }
        "hide_on_process_start" => {
            settings.hide_on_process_start =
                as_bool.unwrap_or(settings.hide_on_process_start);
        }
        "max_concurrent_downloads" => {
            settings.max_concurrent_downloads =
                as_count.unwrap_or(settings.max_concurrent_downloads);
        }
        "max_concurrent_writes" => {
            settings.max_concurrent_writes =
                as_count.unwrap_or(settings.max_concurrent_writes);
        }
        "memory" => {
            if let Some(memory) =
                value.as_u64().and_then(|memory| u32::try_from(memory).ok())
            {
                settings.memory.maximum = memory;
            }
        }
        "force_fullscreen" => {
            settings.force_fullscreen =
                as_bool.unwrap_or(settings.force_fullscreen);
        }
        "game_resolution" => {
            if let Ok((width, height)) =
                serde_json::from_value::<(u16, u16)>(value.clone())
            {
                settings.game_resolution = WindowSize(width, height);
            }
        }
        "extra_launch_args" => {
            if let Ok(args) = serde_json::from_value(value.clone()) {
                settings.extra_launch_args = args;
            }
        }
        "custom_env_vars" => {
            if let Ok(vars) = serde_json::from_value(value.clone()) {
                settings.custom_env_vars = vars;
            }
        }
        "hook_pre_launch" => settings.hooks.pre_launch = hook(value),
        "hook_wrapper" => settings.hooks.wrapper = hook(value),
        "hook_post_exit" => settings.hooks.post_exit = hook(value),
        _ => {}
    }
}

fn changes(
    current: impl Fn(&str) -> Value,
    incoming: &[(String, Value)],
) -> (Vec<SettingChange>, usize) {
    let mut changed = Vec::new();
    let mut matching = 0;
    for key in KEYS {
        let Some((_, value)) = incoming.iter().find(|(name, _)| name == key)
        else {
            continue;
        };
        let current = current(key);
        if &current == value {
            matching += 1;
        } else {
            changed.push(SettingChange {
                key: key.to_string(),
                current,
                incoming: value.clone(),
            });
        }
    }
    (changed, matching)
}

/// What importing would change, without changing anything.
pub async fn preview() -> crate::Result<SettingsPreview> {
    let Some(folder) = modrinth_app::default_path() else {
        return Ok(SettingsPreview::default());
    };
    let state = State::get().await?;
    let settings = Settings::get(&state.pool).await?;
    let incoming = read(&folder).await?;
    let (changes, matching) = changes(|key| current(&settings, key), &incoming);
    Ok(SettingsPreview {
        found: true,
        changes,
        matching,
    })
}

/// Brings over only the chosen settings, reading the official app again so what's
/// imported is what it has now. Returns how many were changed.
pub async fn apply(keys: Vec<String>) -> crate::Result<usize> {
    let Some(folder) = modrinth_app::default_path() else {
        return Ok(0);
    };
    let state = State::get().await?;
    let mut settings = Settings::get(&state.pool).await?;
    let incoming = read(&folder).await?;
    let (changes, _) = changes(|key| current(&settings, key), &incoming);
    let mut applied = 0;
    for change in changes.iter().filter(|change| keys.contains(&change.key)) {
        assign(&mut settings, &change.key, &change.incoming);
        applied += 1;
    }
    if applied > 0 {
        crate::api::settings::set(settings).await?;
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_differences_are_offered_and_nothing_else_is_read() {
        let mine = |key: &str| match key {
            "discord_rpc" => json!(true),
            "memory" => json!(4096),
            "hook_wrapper" => json!(""),
            _ => Value::Null,
        };
        let incoming = vec![
            ("discord_rpc".to_string(), json!(true)),
            ("memory".to_string(), json!(6144)),
            ("hook_wrapper".to_string(), json!("")),
            ("custom_dir".to_string(), json!("C:/elsewhere")),
            ("telemetry".to_string(), json!(true)),
        ];
        let (changed, matching) = changes(mine, &incoming);
        assert_eq!(matching, 2);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].key, "memory");
        assert_eq!(changed[0].current, json!(4096));
    }

    #[test]
    fn every_setting_reads_back_what_was_brought_over() {
        let mut settings: Settings = serde_json::from_value(json!({
            "max_concurrent_downloads": 10,
            "max_concurrent_writes": 10,
            "theme": "dark",
            "locale": "en-US",
            "default_page": "Home",
            "collapsed_navigation": false,
            "hide_nametag_skins_page": false,
            "advanced_rendering": false,
            "native_decorations": false,
            "toggle_sidebar": false,
            "sync_theme_across_devices": false,
            "sync_behavior_across_devices": false,
            "telemetry": false,
            "discord_rpc": false,
            "personalized_ads": false,
            "extra_launch_args": [],
            "custom_env_vars": [],
            "memory": { "maximum": 2048 },
            "force_fullscreen": false,
            "game_resolution": [854, 480],
            "hide_on_process_start": false,
            "hooks": { "pre_launch": "", "wrapper": "", "post_exit": "" },
            "migrated": true,
            "developer_mode": false,
            "feature_flags": {},
            "version": 1
        }))
        .unwrap();
        for key in KEYS {
            let value = match key {
                "theme" => json!("light"),
                "locale" => json!("de-DE"),
                "default_page" => json!("library"),
                "max_concurrent_downloads" => json!(7),
                "max_concurrent_writes" => json!(3),
                "memory" => json!(6144),
                "game_resolution" => json!([1280, 720]),
                "extra_launch_args" => json!(["-Dfoo=1"]),
                "custom_env_vars" => json!([["NAME", "value"]]),
                "hook_pre_launch" | "hook_wrapper" | "hook_post_exit" => {
                    json!("prime-run")
                }
                _ => json!(true),
            };
            assert_ne!(
                current(&settings, key),
                value,
                "{key} starts different"
            );
            assign(&mut settings, key, &value);
            assert_eq!(current(&settings, key), value, "{key}");
        }
    }

    #[test]
    fn an_empty_hook_stays_off() {
        assert_eq!(hook(&json!("  ")), None);
        assert_eq!(hook(&json!("prime-run")), Some("prime-run".to_string()));
    }
}
