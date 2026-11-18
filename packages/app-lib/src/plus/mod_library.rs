//! A personal list of mods that go into every new instance. Each mod is installed only
//! when a version of it fits the instance's Minecraft version and mod loader, with its
//! required dependencies, and never when the instance already has the same mod from
//! either site. Anything left out is reported with the reason.

use super::curseforge::{self as cf, DataMode};
use super::installed_identity::{self, has_name};
use crate::State;
use crate::state::ModLoader;
use crate::state::instances::commands::{
    InstanceInstallProjectRequest, install_resolved_content_plan,
    resolve_install_plan,
};
use modrinth_content_management::{ContentType, ResolutionPreferences};
use serde::{Deserialize, Serialize};

pub const MODRINTH: &str = "modrinth";
pub const CURSEFORGE: &str = "curseforge";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct LibraryMod {
    /// `modrinth` or `curseforge`.
    pub source: String,
    pub project_id: String,
    pub name: String,
    pub slug: Option<String>,
    pub icon_url: Option<String>,
    pub auto_install: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct ModLibrarySettings {
    /// Whether new instances get the library's mods at all.
    pub enabled: bool,
    pub mods: Vec<LibraryMod>,
}

impl Default for ModLibrarySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            mods: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    AlreadyInstalled,
    NoModLoader,
    NoMatchingVersion,
    CurseforgeUnavailable,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkippedMod {
    pub name: String,
    pub reason: SkipReason,
    /// The error, when installing failed.
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct LibraryReport {
    pub installed: Vec<String>,
    pub skipped: Vec<SkippedMod>,
}

impl LibraryReport {
    fn skip(&mut self, name: &str, reason: SkipReason, detail: Option<String>) {
        self.skipped.push(SkippedMod {
            name: name.to_string(),
            reason,
            detail,
        });
    }
}

/// Modrinth's mods first, so a CurseForge copy of the same mod is then seen as
/// already installed instead of being added twice.
fn install_order(mods: &[LibraryMod]) -> Vec<&LibraryMod> {
    let mut wanted: Vec<&LibraryMod> =
        mods.iter().filter(|item| item.auto_install).collect();
    wanted.sort_by_key(|item| item.source != MODRINTH);
    wanted
}

async fn install_modrinth(
    instance_id: &str,
    item: &LibraryMod,
    state: &State,
) -> Result<(), (SkipReason, Option<String>)> {
    let plan = resolve_install_plan(
        instance_id,
        InstanceInstallProjectRequest {
            project_id: item.project_id.clone(),
            version_id: None,
            content_type: ContentType::Mod,
            selected: ResolutionPreferences::default(),
        },
        state,
    )
    .await
    .map_err(|error| {
        let text = error.to_string();
        if text.contains("no compatible version") {
            (SkipReason::NoMatchingVersion, None)
        } else {
            (SkipReason::Failed, Some(text))
        }
    })?;
    install_resolved_content_plan(instance_id, &plan, state)
        .await
        .map(|_| ())
        .map_err(|error| (SkipReason::Failed, Some(error.to_string())))
}

async fn install_curseforge(
    instance_id: &str,
    item: &LibraryMod,
) -> Result<(), (SkipReason, Option<String>)> {
    if cf::data_mode().await != DataMode::Live {
        return Err((SkipReason::CurseforgeUnavailable, None));
    }
    let failed =
        |error: crate::Error| (SkipReason::Failed, Some(error.to_string()));
    let project_id = item
        .project_id
        .parse::<u64>()
        .map_err(|_| (SkipReason::Failed, None))?;
    let options = cf::install_options(instance_id, project_id, false, None)
        .await
        .map_err(failed)?;
    let Some(file_id) = options.recommended_file_id else {
        return Err((SkipReason::NoMatchingVersion, None));
    };
    cf::install(instance_id, project_id, file_id, false, false, None)
        .await
        .map(|_| ())
        .map_err(failed)
}

/// Installs the library's mods into an instance, one at a time so each one sees what
/// the ones before it added.
pub async fn install_into(instance_id: &str) -> crate::Result<LibraryReport> {
    let state = State::get().await?;
    let library = super::settings::get().await?.mod_library;
    let mut report = LibraryReport::default();
    let wanted = install_order(&library.mods);
    if !library.enabled || wanted.is_empty() {
        return Ok(report);
    }
    let Some(metadata) = crate::api::instance::get(instance_id).await? else {
        return Ok(report);
    };
    if metadata.applied_content_set.loader == ModLoader::Vanilla {
        for item in wanted {
            report.skip(&item.name, SkipReason::NoModLoader, None);
        }
        return Ok(report);
    }

    for item in wanted {
        let identity =
            installed_identity::of_instance(instance_id, &state).await?;
        let from_modrinth =
            crate::state::get_installed_project_ids_for_instance(
                instance_id,
                None,
                &state,
            )
            .await?;
        let already = has_name(
            &identity,
            item.slug.as_deref().unwrap_or_default(),
            &item.name,
        ) || match item.source.as_str() {
            MODRINTH => from_modrinth.contains(&item.project_id),
            _ => item
                .project_id
                .parse::<u64>()
                .is_ok_and(|id| identity.curseforge_ids.contains(&id)),
        };
        if already {
            report.skip(&item.name, SkipReason::AlreadyInstalled, None);
            continue;
        }
        let result = match item.source.as_str() {
            MODRINTH => install_modrinth(instance_id, item, &state).await,
            CURSEFORGE => install_curseforge(instance_id, item).await,
            _ => Err((SkipReason::Failed, None)),
        };
        match result {
            Ok(()) => report.installed.push(item.name.clone()),
            Err((reason, detail)) => report.skip(&item.name, reason, detail),
        }
    }
    if !report.installed.is_empty() {
        let _ = crate::event::emit::emit_instance(
            instance_id,
            crate::event::InstancePayloadType::Edited,
        )
        .await;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library_mod(source: &str, name: &str, auto_install: bool) -> LibraryMod {
        LibraryMod {
            source: source.to_string(),
            project_id: name.to_lowercase(),
            name: name.to_string(),
            auto_install,
            ..LibraryMod::default()
        }
    }

    #[test]
    fn modrinth_mods_go_first_and_paused_ones_are_left_out() {
        let mods = vec![
            library_mod(CURSEFORGE, "JEI", true),
            library_mod(MODRINTH, "Sodium", true),
            library_mod(MODRINTH, "Lithium", false),
            library_mod(MODRINTH, "Mod Menu", true),
        ];
        let names: Vec<&str> = install_order(&mods)
            .iter()
            .map(|item| item.name.as_str())
            .collect();
        assert_eq!(names, ["Sodium", "Mod Menu", "JEI"]);
    }
}
