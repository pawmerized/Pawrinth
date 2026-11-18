//! Pawrinth additions to the launcher core.

pub mod backgrounds;
pub mod copy_content;
pub mod curseforge;
pub mod curseforge_export;
pub mod curseforge_key;
pub mod curseforge_pack;
#[cfg(debug_assertions)]
mod curseforge_samples;
pub mod diagnostics;
pub mod discord;
pub(crate) mod fault;
pub(crate) mod guard;
pub mod horizon;
pub mod horizon_content;
pub mod installed_identity;
pub mod manifest;
pub mod memory;
pub mod mod_library;
pub mod mod_matching;
pub mod modrinth_app_settings;
pub mod news;
pub mod operations;
pub mod pack_updates;
pub mod server_pack;
pub mod settings;
pub mod setup;
pub mod skin_gallery;
pub mod skin_lookup;
pub mod technic;
pub mod theme_files;
pub mod world_backups;

#[cfg(test)]
mod rollback_tests;
#[cfg(test)]
pub(crate) mod testing;

/// Playtime and server-play analytics are Modrinth's own product data, so a fork
/// doesn't submit them.
pub(crate) const REPORT_USAGE_TO_MODRINTH: bool = false;
