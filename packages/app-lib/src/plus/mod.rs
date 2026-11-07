//! Pawrinth additions to the launcher core.

pub mod backgrounds;
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
pub mod manifest;
pub mod news;
pub mod operations;
pub mod settings;
pub mod setup;
pub mod skin_gallery;
pub mod skin_lookup;
pub mod theme_files;

#[cfg(test)]
mod rollback_tests;
#[cfg(test)]
pub(crate) mod testing;

/// Playtime and server-play analytics are Modrinth's own product data, so a fork
/// doesn't submit them.
pub(crate) const REPORT_USAGE_TO_MODRINTH: bool = false;
