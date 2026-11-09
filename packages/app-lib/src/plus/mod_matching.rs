//! Recognises mods that weren't installed through Pawrinth: jars from Technic, the FTB
//! App, another launcher or a folder someone copied in.
//!
//! - **Modrinth.** A jar whose exact bytes are on Modrinth is already recognised by its
//!   SHA-1 when content is listed, so it's never linked to CurseForge as well.
//! - **CurseForge.** Other jars are looked up by CurseForge's fingerprint. An exact match
//!   whose loader agrees with the jar's own metadata is linked to that CurseForge file,
//!   and CurseForge updates manage it from then on.
//! - **Duplicates** are found by the mod ids declared inside each jar and by project,
//!   never by file name.
//! - **The same mod on Modrinth.** A CurseForge file can be switched to Modrinth only
//!   when Modrinth has the same version for this Minecraft version and loader, the two
//!   projects link the same source code, and the Modrinth jar declares the same mod.
//!
//! Nothing is removed or replaced without someone choosing to, and a snapshot is taken
//! first so it can be undone from the instance's history.

use super::curseforge::{self as cf, DataMode};
use super::operations;
use crate::ErrorKind;
use crate::state::content_store::content_file_path;
use crate::state::instances::adapters::sqlite::{content_rows, instance_rows};
use crate::state::{CachedEntry, InstanceFile, ModLoader, State};
use anyhow::bail;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// Nested jars are only read this deep.
const MAX_NESTING: u8 = 1;
/// A metadata file larger than this isn't one.
const MAX_METADATA_BYTES: u64 = 4 * 1024 * 1024;
/// Ids every loader provides itself.
const BUILT_IN_IDS: [&str; 10] = [
    "minecraft",
    "java",
    "fabricloader",
    "fabric-loader",
    "quilt_loader",
    "quilt_base",
    "forge",
    "neoforge",
    "fml",
    "javafml",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JarLoader {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

/// What a mod jar says about itself.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct JarMetadata {
    pub loaders: Vec<JarLoader>,
    /// The mods the jar declares, its own first.
    pub mod_ids: Vec<String>,
    /// Mods bundled inside it, and ids it provides for others.
    pub bundled_ids: Vec<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    /// Minecraft versions it supports, as written. Several mean any of them.
    pub minecraft: Vec<String>,
    /// How `minecraft` is written.
    pub minecraft_style: Option<RangeStyle>,
    /// Mods it requires, other than Minecraft and its loader.
    pub requires: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RangeStyle {
    /// Fabric and Quilt predicates such as `>=1.20 <1.21`, `~1.20.1` or `1.20.x`.
    Semver,
    /// Maven ranges such as `[1.20.1,1.21)`, used by Forge and NeoForge.
    Maven,
    /// One exact version such as `1.12.2`, as old `mcmod.info` files give it.
    Exact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    Compatible,
    Incompatible,
    Unknown,
}

fn push_unique(list: &mut Vec<String>, value: &str) {
    let value = value.trim().to_ascii_lowercase();
    if !value.is_empty() && !list.contains(&value) {
        list.push(value);
    }
}

fn read_entry_text<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Option<String> {
    let entry = archive.by_name(name).ok()?;
    if entry.size() > MAX_METADATA_BYTES {
        return None;
    }
    let mut text = String::new();
    entry
        .take(MAX_METADATA_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    Some(text.trim_start_matches('\u{feff}').to_string())
}

fn read_entry_bytes<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Option<Vec<u8>> {
    let entry = archive.by_name(name).ok()?;
    if entry.size() > limit {
        return None;
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.take(limit).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn json_strings(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::String(text) => vec![text.clone()],
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

fn is_built_in(id: &str) -> bool {
    BUILT_IN_IDS.contains(&id)
}

fn nested_ids<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    paths: &[String],
    depth: u8,
) -> Vec<String> {
    if depth >= MAX_NESTING {
        return Vec::new();
    }
    let mut ids = Vec::new();
    for path in paths {
        let Some(bytes) = read_entry_bytes(archive, path, 64 * 1024 * 1024)
        else {
            continue;
        };
        let Ok(mut nested) = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        else {
            continue;
        };
        let metadata = metadata_from_archive(&mut nested, depth + 1);
        for id in metadata.mod_ids.iter().chain(&metadata.bundled_ids) {
            push_unique(&mut ids, id);
        }
    }
    ids
}

fn read_fabric<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    metadata: &mut JarMetadata,
    depth: u8,
) {
    let Some(value) = read_entry_text(archive, "fabric.mod.json")
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    else {
        return;
    };
    let Some(id) = value.get("id").and_then(serde_json::Value::as_str) else {
        return;
    };
    metadata.loaders.push(JarLoader::Fabric);
    push_unique(&mut metadata.mod_ids, id);
    for provided in value.get("provides").map(json_strings).unwrap_or_default()
    {
        push_unique(&mut metadata.bundled_ids, &provided);
    }
    metadata.name = metadata.name.take().or_else(|| {
        value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    });
    metadata.version = metadata.version.take().or_else(|| {
        value
            .get("version")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    });
    if let Some(depends) =
        value.get("depends").and_then(serde_json::Value::as_object)
    {
        for (dependency, versions) in depends {
            let dependency = dependency.to_ascii_lowercase();
            if dependency == "minecraft" {
                metadata.minecraft = json_strings(versions);
                metadata.minecraft_style = Some(RangeStyle::Semver);
            } else if !is_built_in(&dependency) {
                push_unique(&mut metadata.requires, &dependency);
            }
        }
    }
    let jars: Vec<String> = value
        .get("jars")
        .and_then(serde_json::Value::as_array)
        .map(|jars| {
            jars.iter()
                .filter_map(|jar| {
                    jar.get("file")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    for id in nested_ids(archive, &jars, depth) {
        push_unique(&mut metadata.bundled_ids, &id);
    }
}

fn read_quilt<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    metadata: &mut JarMetadata,
    depth: u8,
) {
    let Some(loader) = read_entry_text(archive, "quilt.mod.json")
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| value.get("quilt_loader").cloned())
    else {
        return;
    };
    let Some(id) = loader.get("id").and_then(serde_json::Value::as_str) else {
        return;
    };
    metadata.loaders.push(JarLoader::Quilt);
    push_unique(&mut metadata.mod_ids, id);
    metadata.name = metadata.name.take().or_else(|| {
        loader
            .pointer("/metadata/name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    });
    metadata.version = metadata.version.take().or_else(|| {
        loader
            .get("version")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    });
    for dependency in loader
        .get("depends")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let (id, versions) = match dependency {
            serde_json::Value::String(id) => (id.clone(), Vec::new()),
            serde_json::Value::Object(object) => (
                object
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                object.get("versions").map(json_strings).unwrap_or_default(),
            ),
            _ => continue,
        };
        let id = id.to_ascii_lowercase();
        if id == "minecraft" {
            metadata.minecraft = versions;
            metadata.minecraft_style = Some(RangeStyle::Semver);
        } else if !is_built_in(&id) {
            push_unique(&mut metadata.requires, &id);
        }
    }
    let jars: Vec<String> =
        loader.get("jars").map(json_strings).unwrap_or_default();
    for id in nested_ids(archive, &jars, depth) {
        push_unique(&mut metadata.bundled_ids, &id);
    }
}

fn manifest_version<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Option<String> {
    read_entry_text(archive, "META-INF/MANIFEST.MF")?
        .lines()
        .find_map(|line| {
            line.strip_prefix("Implementation-Version:")
                .map(|version| version.trim().to_string())
        })
        .filter(|version| !version.is_empty())
}

fn read_mods_toml<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    file: &str,
    loader: JarLoader,
    metadata: &mut JarMetadata,
    depth: u8,
) {
    let Some(value) = read_entry_text(archive, file)
        .and_then(|text| text.parse::<toml::Table>().ok())
    else {
        return;
    };
    let mods = value
        .get("mods")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let ids: Vec<String> = mods
        .iter()
        .filter_map(|entry| {
            entry
                .get("modId")
                .and_then(toml::Value::as_str)
                .map(str::to_string)
        })
        .collect();
    if ids.is_empty() {
        return;
    }
    if !metadata.loaders.contains(&loader) {
        metadata.loaders.push(loader);
    }
    for id in &ids {
        push_unique(&mut metadata.mod_ids, id);
    }
    let first = &mods[0];
    metadata.name = metadata.name.take().or_else(|| {
        first
            .get("displayName")
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    });
    if metadata.version.is_none() {
        let version = first
            .get("version")
            .and_then(toml::Value::as_str)
            .map(str::to_string);
        metadata.version = match version {
            Some(version) if version.contains("${") => {
                manifest_version(archive)
            }
            other => other,
        };
    }
    if let Some(dependencies) =
        value.get("dependencies").and_then(toml::Value::as_table)
    {
        for entries in dependencies.values() {
            for dependency in entries.as_array().into_iter().flatten() {
                let Some(id) = dependency
                    .get("modId")
                    .and_then(toml::Value::as_str)
                    .map(str::to_ascii_lowercase)
                else {
                    continue;
                };
                let required = dependency
                    .get("mandatory")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false)
                    || dependency
                        .get("type")
                        .and_then(toml::Value::as_str)
                        .is_some_and(|kind| {
                            kind.eq_ignore_ascii_case("required")
                        });
                if id == "minecraft" {
                    if let Some(range) = dependency
                        .get("versionRange")
                        .and_then(toml::Value::as_str)
                        && metadata.minecraft.is_empty()
                    {
                        metadata.minecraft = vec![range.to_string()];
                        metadata.minecraft_style = Some(RangeStyle::Maven);
                    }
                } else if required
                    && !is_built_in(&id)
                    && !ids.iter().any(|own| own.eq_ignore_ascii_case(&id))
                {
                    push_unique(&mut metadata.requires, &id);
                }
            }
        }
    }
    let jars: Vec<String> =
        read_entry_text(archive, "META-INF/jarjar/metadata.json")
            .and_then(|text| {
                serde_json::from_str::<serde_json::Value>(&text).ok()
            })
            .and_then(|value| value.get("jars").cloned())
            .and_then(|jars| jars.as_array().cloned())
            .map(|jars| {
                jars.iter()
                    .filter_map(|jar| {
                        jar.get("path")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default();
    for id in nested_ids(archive, &jars, depth) {
        push_unique(&mut metadata.bundled_ids, &id);
    }
}

fn read_mcmod_info<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    metadata: &mut JarMetadata,
) {
    let Some(value) = read_entry_text(archive, "mcmod.info")
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    else {
        return;
    };
    let mods = match &value {
        serde_json::Value::Array(mods) => mods.clone(),
        serde_json::Value::Object(object) => object
            .get("modList")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let mut found = false;
    for entry in &mods {
        let Some(id) = entry.get("modid").and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        found = true;
        push_unique(&mut metadata.mod_ids, id);
        if metadata.name.is_none() {
            metadata.name = entry
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
        }
        if metadata.version.is_none() {
            metadata.version = entry
                .get("version")
                .and_then(serde_json::Value::as_str)
                .filter(|version| !version.contains("${"))
                .map(str::to_string);
        }
        if metadata.minecraft.is_empty()
            && let Some(minecraft) =
                entry.get("mcversion").and_then(serde_json::Value::as_str)
        {
            metadata.minecraft = vec![minecraft.to_string()];
            metadata.minecraft_style = Some(RangeStyle::Exact);
        }
    }
    if found && !metadata.loaders.contains(&JarLoader::Forge) {
        metadata.loaders.push(JarLoader::Forge);
    }
}

fn metadata_from_archive<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    depth: u8,
) -> JarMetadata {
    let mut metadata = JarMetadata::default();
    read_fabric(archive, &mut metadata, depth);
    read_quilt(archive, &mut metadata, depth);
    read_mods_toml(
        archive,
        "META-INF/neoforge.mods.toml",
        JarLoader::NeoForge,
        &mut metadata,
        depth,
    );
    read_mods_toml(
        archive,
        "META-INF/mods.toml",
        JarLoader::Forge,
        &mut metadata,
        depth,
    );
    if metadata.mod_ids.is_empty() {
        read_mcmod_info(archive, &mut metadata);
    }
    metadata
}

/// Reads a jar's own metadata. `None` for anything that isn't a mod jar.
pub fn read_metadata(path: &Path) -> Option<JarMetadata> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive =
        zip::ZipArchive::new(std::io::BufReader::new(file)).ok()?;
    let metadata = metadata_from_archive(&mut archive, 0);
    (!metadata.mod_ids.is_empty()).then_some(metadata)
}

fn version_parts(version: &str) -> Option<Vec<u64>> {
    let version = version.trim();
    if version.is_empty() {
        return None;
    }
    version
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect()
}

fn compare_parts(a: &[u64], b: &[u64]) -> Ordering {
    let length = a.len().max(b.len());
    for index in 0..length {
        let left = a.get(index).copied().unwrap_or(0);
        let right = b.get(index).copied().unwrap_or(0);
        match left.cmp(&right) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

/// One Fabric or Quilt comparator such as `>=1.20`, `~1.20.1`, `1.20.x` or `1.20.1`.
fn semver_term(term: &str, game: &[u64]) -> Option<bool> {
    let term = term.trim().trim_end_matches('-');
    if term.is_empty() || term == "*" {
        return Some(true);
    }
    let (operator, rest) = [">=", "<=", ">", "<", "=", "~", "^"]
        .iter()
        .find_map(|operator| {
            term.strip_prefix(operator).map(|rest| (*operator, rest))
        })
        .unwrap_or(("", term));
    let rest = rest.trim();
    if rest
        .split('.')
        .any(|part| part == "x" || part == "X" || part == "*")
    {
        let prefix: Option<Vec<u64>> = rest
            .split('.')
            .take_while(|part| !matches!(*part, "x" | "X" | "*"))
            .map(|part| part.parse().ok())
            .collect();
        let prefix = prefix?;
        return Some(
            matches!(operator, "" | "=")
                && game.len() >= prefix.len()
                && game[..prefix.len()] == prefix[..],
        );
    }
    let version = version_parts(rest)?;
    let order = compare_parts(game, &version);
    Some(match operator {
        ">=" => order != Ordering::Less,
        ">" => order == Ordering::Greater,
        "<=" => order != Ordering::Greater,
        "<" => order == Ordering::Less,
        "~" => {
            let mut upper = vec![
                version.first().copied().unwrap_or(0),
                version.get(1).copied().unwrap_or(0) + 1,
            ];
            if version.len() < 2 {
                upper = vec![version.first().copied().unwrap_or(0) + 1];
            }
            order != Ordering::Less
                && compare_parts(game, &upper) == Ordering::Less
        }
        "^" => {
            let upper = vec![version.first().copied().unwrap_or(0) + 1];
            order != Ordering::Less
                && compare_parts(game, &upper) == Ordering::Less
        }
        _ => order == Ordering::Equal,
    })
}

fn semver_matches(predicate: &str, game: &[u64]) -> Option<bool> {
    for term in predicate.split_whitespace() {
        if !semver_term(term, game)? {
            return Some(false);
        }
    }
    Some(true)
}

/// Maven ranges such as `[1.20.1,1.21)`, `[1.12.2]` or `[1.19,1.20),[1.20.1,)`. A bare
/// version is a soft requirement, which any version satisfies.
fn maven_matches(spec: &str, game: &[u64]) -> Option<bool> {
    let spec = spec.trim();
    if spec.is_empty() || spec == "*" {
        return Some(true);
    }
    if spec.contains("${") {
        return None;
    }
    if !spec.starts_with('[') && !spec.starts_with('(') {
        return version_parts(spec).map(|_| true);
    }
    let mut ranges = Vec::new();
    let mut start = None;
    for (index, character) in spec.char_indices() {
        match character {
            '[' | '(' => start = Some(index),
            ']' | ')' => {
                if let Some(begin) = start.take() {
                    ranges.push(&spec[begin..=index]);
                }
            }
            _ => {}
        }
    }
    if ranges.is_empty() {
        return None;
    }
    let mut any = false;
    for range in ranges {
        let inclusive_low = range.starts_with('[');
        let inclusive_high = range.ends_with(']');
        let inner = &range[1..range.len() - 1];
        let matched = match inner.split_once(',') {
            None => {
                compare_parts(game, &version_parts(inner)?) == Ordering::Equal
            }
            Some((low, high)) => {
                let low_ok = if low.trim().is_empty() {
                    true
                } else {
                    let order = compare_parts(game, &version_parts(low)?);
                    order == Ordering::Greater
                        || (inclusive_low && order == Ordering::Equal)
                };
                let high_ok = if high.trim().is_empty() {
                    true
                } else {
                    let order = compare_parts(game, &version_parts(high)?);
                    order == Ordering::Less
                        || (inclusive_high && order == Ordering::Equal)
                };
                low_ok && high_ok
            }
        };
        any |= matched;
    }
    Some(any)
}

/// An exact version, or a Maven range when it's written as one.
fn exact_matches(spec: &str, game: &[u64]) -> Option<bool> {
    let spec = spec.trim();
    if spec.starts_with('[') || spec.starts_with('(') {
        return maven_matches(spec, game);
    }
    if spec.contains("${") {
        return None;
    }
    Some(compare_parts(game, &version_parts(spec)?) == Ordering::Equal)
}

pub fn minecraft_compatibility(
    jar: &JarMetadata,
    game_version: &str,
) -> Compatibility {
    if jar.minecraft.is_empty() {
        return Compatibility::Compatible;
    }
    let Some(game) = version_parts(game_version) else {
        return Compatibility::Unknown;
    };
    let mut unknown = false;
    for spec in &jar.minecraft {
        let result = match jar.minecraft_style {
            Some(RangeStyle::Maven) => maven_matches(spec, &game),
            Some(RangeStyle::Exact) => exact_matches(spec, &game),
            _ => semver_matches(spec, &game),
        };
        match result {
            Some(true) => return Compatibility::Compatible,
            Some(false) => {}
            None => unknown = true,
        }
    }
    if unknown {
        Compatibility::Unknown
    } else {
        Compatibility::Incompatible
    }
}

pub fn loader_compatibility(
    jar: &JarMetadata,
    loader: ModLoader,
    game_version: &str,
) -> Compatibility {
    let has = |kind| jar.loaders.contains(&kind);
    match loader {
        ModLoader::Fabric if has(JarLoader::Fabric) => {
            Compatibility::Compatible
        }
        ModLoader::Quilt if has(JarLoader::Quilt) || has(JarLoader::Fabric) => {
            Compatibility::Compatible
        }
        ModLoader::Forge if has(JarLoader::Forge) => Compatibility::Compatible,
        ModLoader::NeoForge if has(JarLoader::NeoForge) => {
            Compatibility::Compatible
        }
        ModLoader::NeoForge if has(JarLoader::Forge) => {
            match version_parts(game_version) {
                Some(game)
                    if compare_parts(&game, &[1, 20, 1]) == Ordering::Equal =>
                {
                    Compatibility::Compatible
                }
                _ => Compatibility::Unknown,
            }
        }
        _ => Compatibility::Incompatible,
    }
}

pub fn compatibility(
    jar: &JarMetadata,
    loader: ModLoader,
    game_version: &str,
) -> Compatibility {
    match (
        loader_compatibility(jar, loader, game_version),
        minecraft_compatibility(jar, game_version),
    ) {
        (Compatibility::Incompatible, _) | (_, Compatibility::Incompatible) => {
            Compatibility::Incompatible
        }
        (Compatibility::Compatible, Compatibility::Compatible) => {
            Compatibility::Compatible
        }
        _ => Compatibility::Unknown,
    }
}

/// Compares mod versions such as `1.2.10` and `1.2.9`, numbers numerically.
pub fn compare_mod_versions(a: &str, b: &str) -> Ordering {
    let parts = |version: &str| -> Vec<Result<u64, String>> {
        version
            .trim()
            .trim_start_matches(['v', 'V'])
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|part| !part.is_empty())
            .map(|part| part.parse::<u64>().map_err(|_| part.to_lowercase()))
            .collect()
    };
    let (left, right) = (parts(a), parts(b));
    for (left, right) in left.iter().zip(right.iter()) {
        let order = match (left, right) {
            (Ok(left), Ok(right)) => left.cmp(right),
            (Ok(_), Err(_)) => Ordering::Greater,
            (Err(_), Ok(_)) => Ordering::Less,
            (Err(left), Err(right)) => left.cmp(right),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    left.len().cmp(&right.len())
}

fn same_version(a: &str, b: &str) -> bool {
    let clean = |version: &str| {
        version
            .trim()
            .trim_start_matches(['v', 'V'])
            .to_ascii_lowercase()
    };
    !a.trim().is_empty() && clean(a) == clean(b)
}

/// `https://github.com/Owner/Repo/` and `http://github.com/owner/repo.git` are the same.
fn normalized_source(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    let host = parsed.host_str()?.trim_start_matches("www.").to_lowercase();
    let path = parsed
        .path()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_lowercase();
    let mut segments = path.split('/').filter(|part| !part.is_empty());
    let owner = segments.next()?;
    let repository = segments.next();
    Some(match repository {
        Some(repository) => format!("{host}/{owner}/{repository}"),
        None => format!("{host}/{owner}"),
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum ModSource {
    Modrinth {
        project_id: String,
        version_id: String,
    },
    Curseforge {
        project_id: u64,
        file_id: u64,
    },
    Unknown,
}

impl ModSource {
    fn rank(&self) -> u8 {
        match self {
            ModSource::Modrinth { .. } => 2,
            ModSource::Curseforge { .. } => 1,
            ModSource::Unknown => 0,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ModFile {
    pub relative_path: String,
    pub file_name: String,
    pub enabled: bool,
    pub source: ModSource,
    /// The project's name on Modrinth or CurseForge, or the jar's own.
    pub name: String,
    pub version: Option<String>,
    pub mod_ids: Vec<String>,
    pub compatibility: Compatibility,
    #[serde(skip)]
    metadata: Option<JarMetadata>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DuplicateGroup {
    pub name: String,
    pub files: Vec<ModFile>,
    /// The copy to keep: compatible, newest, and managed by Modrinth or CurseForge.
    pub keep: String,
    /// Only when the copy to keep is known to work with this instance.
    pub can_remove: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModrinthEquivalent {
    pub relative_path: String,
    pub file_name: String,
    pub name: String,
    pub version: String,
    pub project_id: String,
    pub version_id: String,
    pub modrinth_file_name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModReport {
    pub game_version: String,
    pub loader: ModLoader,
    pub files: Vec<ModFile>,
    /// File names linked to CurseForge just now.
    pub newly_linked: Vec<String>,
    pub duplicates: Vec<DuplicateGroup>,
    pub modrinth_equivalents: Vec<ModrinthEquivalent>,
    /// CurseForge could be asked about unrecognised files.
    pub curseforge_checked: bool,
}

/// Fingerprints CurseForge didn't know, so they're only asked about once a session.
static UNMATCHED_FINGERPRINTS: LazyLock<Mutex<HashSet<u32>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

fn curseforge_project_of(project_ref: &str) -> Option<u64> {
    project_ref.strip_prefix("curseforge:")?.parse().ok()
}

fn is_mod_jar(file: &InstanceFile) -> bool {
    let path = file.relative_path.to_ascii_lowercase();
    path.starts_with("mods/")
        && !path["mods/".len()..].contains('/')
        && (path.ends_with(".jar") || path.ends_with(".jar.disabled"))
}

struct Scanned {
    files: Vec<ModFile>,
    game_version: String,
    loader: ModLoader,
}

async fn instance_target(
    instance_id: &str,
    state: &State,
) -> crate::Result<(crate::state::Instance, String, ModLoader)> {
    let instance = instance_rows::get_instance_by_id(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            ErrorKind::InputError(format!("unknown instance {instance_id}"))
        })?;
    let set_id = instance.applied_content_set_id.clone().ok_or_else(|| {
        ErrorKind::InputError("this instance has no content yet".to_string())
    })?;
    let set = content_rows::get_content_set(&set_id, &state.pool)
        .await?
        .ok_or_else(|| {
            ErrorKind::InputError(
                "this instance has no content yet".to_string(),
            )
        })?;
    Ok((instance, set.game_version, set.loader))
}

async fn jar_path(
    instance: &crate::state::Instance,
    file: &InstanceFile,
    state: &State,
) -> crate::Result<PathBuf> {
    state
        .content_store
        .instance_path(&instance.path, &content_file_path(file))
        .await
}

/// Lists the instance's mod jars with what's known about each, and links unknown jars
/// to CurseForge when `link_curseforge` is set and there's a confident match.
async fn scan(
    instance_id: &str,
    link_curseforge: bool,
    state: &State,
) -> crate::Result<(Scanned, Vec<String>, bool)> {
    let (instance, game_version, loader) =
        instance_target(instance_id, state).await?;
    let instance_files: Vec<InstanceFile> =
        content_rows::get_instance_files(instance_id, &state.pool)
            .await?
            .into_iter()
            .filter(|file| !file.missing && is_mod_jar(file))
            .collect();

    let hashes: Vec<&str> = instance_files
        .iter()
        .map(|file| file.sha1.as_str())
        .collect();
    let modrinth: HashMap<String, (String, String)> =
        CachedEntry::get_file_many(
            &hashes,
            None,
            &state.pool,
            &state.api_semaphore,
        )
        .await?
        .into_iter()
        .map(|file| (file.hash, (file.project_id, file.version_id)))
        .collect();
    let origins = cf::origins(instance_id, state).await?;

    let mut paths = Vec::with_capacity(instance_files.len());
    for file in &instance_files {
        paths.push(jar_path(&instance, file, state).await?);
    }
    let read_paths = paths.clone();
    let metadata: Vec<Option<JarMetadata>> =
        tokio::task::spawn_blocking(move || {
            read_paths.iter().map(|path| read_metadata(path)).collect()
        })
        .await?;

    let mut sources: Vec<ModSource> = instance_files
        .iter()
        .map(|file| {
            if let Some((project_id, version_id)) = modrinth.get(&file.sha1) {
                return ModSource::Modrinth {
                    project_id: project_id.clone(),
                    version_id: version_id.clone(),
                };
            }
            match origins.get(&file.relative_path) {
                Some((project_ref, file_id)) => {
                    match (
                        curseforge_project_of(project_ref),
                        file_id.parse::<u64>().ok(),
                    ) {
                        (Some(project_id), Some(file_id)) => {
                            ModSource::Curseforge {
                                project_id,
                                file_id,
                            }
                        }
                        _ => ModSource::Unknown,
                    }
                }
                None => ModSource::Unknown,
            }
        })
        .collect();

    let curseforge_available = cf::data_mode().await == DataMode::Live;
    let mut newly_linked = Vec::new();
    if link_curseforge && curseforge_available {
        newly_linked = link_to_curseforge(
            instance_id,
            &instance_files,
            &paths,
            &metadata,
            &mut sources,
            state,
        )
        .await?;
    }

    let mut names: HashMap<String, String> = HashMap::new();
    let modrinth_ids: Vec<&str> = sources
        .iter()
        .filter_map(|source| match source {
            ModSource::Modrinth { project_id, .. } => Some(project_id.as_str()),
            _ => None,
        })
        .collect();
    if !modrinth_ids.is_empty() {
        for project in CachedEntry::get_project_many(
            &modrinth_ids,
            None,
            &state.pool,
            &state.api_semaphore,
        )
        .await
        .unwrap_or_default()
        {
            names.insert(project.id.clone(), project.title.clone());
        }
    }
    let curseforge_ids: Vec<u64> = sources
        .iter()
        .filter_map(|source| match source {
            ModSource::Curseforge { project_id, .. } => Some(*project_id),
            _ => None,
        })
        .collect();
    if curseforge_available && !curseforge_ids.is_empty() {
        for project in cf::fetch_projects_by_id(&curseforge_ids, state)
            .await
            .unwrap_or_default()
        {
            names.insert(format!("curseforge:{}", project.id), project.name);
        }
    }

    let files = instance_files
        .into_iter()
        .zip(sources)
        .zip(metadata)
        .map(|((file, source), metadata)| {
            let project_name = match &source {
                ModSource::Modrinth { project_id, .. } => {
                    names.get(project_id).cloned()
                }
                ModSource::Curseforge { project_id, .. } => {
                    names.get(&format!("curseforge:{project_id}")).cloned()
                }
                ModSource::Unknown => None,
            };
            let name = project_name
                .or_else(|| metadata.as_ref().and_then(|jar| jar.name.clone()))
                .unwrap_or_else(|| file.file_name.clone());
            let compatibility =
                metadata.as_ref().map_or(Compatibility::Unknown, |jar| {
                    compatibility(jar, loader, &game_version)
                });
            ModFile {
                relative_path: file.relative_path,
                file_name: file.file_name,
                enabled: file.enabled,
                source,
                name,
                version: metadata.as_ref().and_then(|jar| jar.version.clone()),
                mod_ids: metadata
                    .as_ref()
                    .map(|jar| jar.mod_ids.clone())
                    .unwrap_or_default(),
                compatibility,
                metadata,
            }
        })
        .collect();
    Ok((
        Scanned {
            files,
            game_version,
            loader,
        },
        newly_linked,
        curseforge_available,
    ))
}

/// CurseForge tags files with the loaders they're for.
fn curseforge_loaders(tags: &[String]) -> Vec<JarLoader> {
    tags.iter()
        .filter_map(|tag| match tag.to_ascii_lowercase().as_str() {
            "fabric" => Some(JarLoader::Fabric),
            "quilt" => Some(JarLoader::Quilt),
            "forge" => Some(JarLoader::Forge),
            "neoforge" => Some(JarLoader::NeoForge),
            _ => None,
        })
        .collect()
}

/// An exact fingerprint match only links a jar when the loaders CurseForge lists for
/// the file agree with the jar's own metadata.
fn confident_curseforge_match(
    metadata: Option<&JarMetadata>,
    curseforge_tags: &[String],
) -> bool {
    let listed = curseforge_loaders(curseforge_tags);
    match metadata {
        Some(jar) => {
            listed.is_empty()
                || listed.iter().any(|loader| jar.loaders.contains(loader))
        }
        None => false,
    }
}

async fn link_to_curseforge(
    instance_id: &str,
    files: &[InstanceFile],
    paths: &[PathBuf],
    metadata: &[Option<JarMetadata>],
    sources: &mut [ModSource],
    state: &State,
) -> crate::Result<Vec<String>> {
    let skipped: HashSet<u32> = UNMATCHED_FINGERPRINTS
        .lock()
        .map(|set| set.clone())
        .unwrap_or_default();
    let mut candidates = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        if *source != ModSource::Unknown || metadata[index].is_none() {
            continue;
        }
        let path = paths[index].clone();
        let fingerprint = tokio::task::spawn_blocking(move || {
            std::fs::read(&path)
                .ok()
                .map(|bytes| super::curseforge_export::fingerprint(&bytes))
        })
        .await?;
        if let Some(fingerprint) = fingerprint
            && !skipped.contains(&fingerprint)
        {
            candidates.push((index, fingerprint));
        }
    }
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let fingerprints: Vec<u32> = candidates
        .iter()
        .map(|(_, fingerprint)| *fingerprint)
        .collect();
    let matches = cf::fetch_fingerprint_matches(&fingerprints, state).await?;
    if let Ok(mut set) = UNMATCHED_FINGERPRINTS.lock() {
        for fingerprint in &fingerprints {
            if !matches.contains_key(fingerprint) {
                set.insert(*fingerprint);
            }
        }
    }
    let file_ids: Vec<u64> =
        matches.values().map(|(_, file_id)| *file_id).collect();
    let curseforge_files: HashMap<u64, cf::ProjectFile> =
        cf::fetch_files_by_id(&file_ids, state)
            .await?
            .into_iter()
            .map(|file| (file.id, file))
            .collect();

    let mut linked = Vec::new();
    for (index, fingerprint) in candidates {
        let Some((project_id, file_id)) = matches.get(&fingerprint).copied()
        else {
            continue;
        };
        let Some(curseforge_file) = curseforge_files.get(&file_id) else {
            continue;
        };
        if curseforge_file.mod_id != project_id
            || !confident_curseforge_match(
                metadata[index].as_ref(),
                &curseforge_file.game_versions,
            )
        {
            continue;
        }
        cf::record_origin(
            instance_id,
            &files[index].relative_path,
            &cf::curseforge_ref(project_id),
            &file_id.to_string(),
            state,
        )
        .await?;
        sources[index] = ModSource::Curseforge {
            project_id,
            file_id,
        };
        linked.push(files[index].file_name.clone());
    }
    Ok(linked)
}

/// The ids two jars must share to be the same mod, plus their project on Modrinth or
/// CurseForge.
fn identity_keys(file: &ModFile) -> Vec<String> {
    let mut keys: Vec<String> = file
        .mod_ids
        .iter()
        .filter(|id| !is_built_in(id))
        .map(|id| format!("mod:{id}"))
        .collect();
    match &file.source {
        ModSource::Modrinth { project_id, .. } => {
            keys.push(format!("modrinth:{project_id}"));
        }
        ModSource::Curseforge { project_id, .. } => {
            keys.push(format!("curseforge:{project_id}"));
        }
        ModSource::Unknown => {}
    }
    keys
}

fn keep_order(a: &ModFile, b: &ModFile) -> Ordering {
    let compatibility_rank = |file: &ModFile| match file.compatibility {
        Compatibility::Compatible => 2,
        Compatibility::Unknown => 1,
        Compatibility::Incompatible => 0,
    };
    compatibility_rank(b)
        .cmp(&compatibility_rank(a))
        .then_with(|| match (&a.version, &b.version) {
            (Some(left), Some(right)) => compare_mod_versions(right, left),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        })
        .then_with(|| b.source.rank().cmp(&a.source.rank()))
        .then_with(|| a.relative_path.cmp(&b.relative_path))
}

/// Enabled jars that are the same mod. Files only count as the same mod when they
/// declare the same mod id or belong to the same project, never because their names
/// look alike.
pub fn find_duplicates(files: &[ModFile]) -> Vec<DuplicateGroup> {
    let enabled: Vec<&ModFile> =
        files.iter().filter(|file| file.enabled).collect();
    let mut parent: Vec<usize> = (0..enabled.len()).collect();
    fn root(parent: &mut [usize], mut index: usize) -> usize {
        while parent[index] != index {
            parent[index] = parent[parent[index]];
            index = parent[index];
        }
        index
    }
    let mut owner: HashMap<String, usize> = HashMap::new();
    for (index, file) in enabled.iter().enumerate() {
        for key in identity_keys(file) {
            match owner.get(&key) {
                Some(&other) => {
                    let (a, b) =
                        (root(&mut parent, index), root(&mut parent, other));
                    if a != b {
                        parent[a] = b;
                    }
                }
                None => {
                    owner.insert(key, index);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<ModFile>> = BTreeMap::new();
    for index in 0..enabled.len() {
        let group = root(&mut parent, index);
        groups
            .entry(group)
            .or_default()
            .push(enabled[index].clone());
    }
    groups
        .into_values()
        .filter(|files| files.len() > 1)
        .map(|mut files| {
            files.sort_by(keep_order);
            let keep = &files[0];
            DuplicateGroup {
                name: keep.name.clone(),
                keep: keep.relative_path.clone(),
                can_remove: keep.compatibility == Compatibility::Compatible,
                files,
            }
        })
        .collect()
}

fn modrinth_loader_names(loader: ModLoader) -> &'static [&'static str] {
    match loader {
        ModLoader::Fabric => &["fabric"],
        ModLoader::Quilt => &["quilt", "fabric"],
        ModLoader::Forge => &["forge"],
        ModLoader::NeoForge => &["neoforge"],
        ModLoader::Vanilla => &[],
    }
}

/// The same version of a CurseForge file's mod on Modrinth, for this Minecraft version
/// and loader. Both projects have to link the same source code.
async fn modrinth_equivalents(
    scanned: &Scanned,
    state: &State,
) -> crate::Result<Vec<ModrinthEquivalent>> {
    let candidates: Vec<(&ModFile, u64)> = scanned
        .files
        .iter()
        .filter_map(|file| match &file.source {
            ModSource::Curseforge { project_id, .. }
                if file.version.is_some() =>
            {
                Some((file, *project_id))
            }
            _ => None,
        })
        .collect();
    if candidates.is_empty() || cf::data_mode().await != DataMode::Live {
        return Ok(Vec::new());
    }
    let ids: Vec<u64> = candidates.iter().map(|(_, id)| *id).collect();
    let projects: HashMap<u64, cf::Project> =
        cf::fetch_projects_by_id(&ids, state)
            .await?
            .into_iter()
            .map(|project| (project.id, project))
            .collect();
    let loaders = modrinth_loader_names(scanned.loader);
    let mut found = Vec::new();
    for (file, project_id) in candidates {
        let Some(curseforge) = projects.get(&project_id) else {
            continue;
        };
        let Some(source) = curseforge
            .links
            .source_url
            .as_deref()
            .and_then(normalized_source)
        else {
            continue;
        };
        let Ok(Some(project)) = CachedEntry::get_project(
            &curseforge.slug,
            None,
            &state.pool,
            &state.api_semaphore,
        )
        .await
        else {
            continue;
        };
        if project.source_url.as_deref().and_then(normalized_source)
            != Some(source)
        {
            continue;
        }
        let Ok(Some(versions)) = CachedEntry::get_project_versions(
            &project.id,
            None,
            &state.pool,
            &state.api_semaphore,
        )
        .await
        else {
            continue;
        };
        let installed_version = file.version.clone().unwrap_or_default();
        let Some(version) = versions.into_iter().find(|version| {
            version.game_versions.contains(&scanned.game_version)
                && version
                    .loaders
                    .iter()
                    .any(|loader| loaders.contains(&loader.as_str()))
                && same_version(&version.version_number, &installed_version)
        }) else {
            continue;
        };
        let Some(primary) = version
            .files
            .iter()
            .find(|file| file.primary)
            .or(version.files.first())
        else {
            continue;
        };
        found.push(ModrinthEquivalent {
            relative_path: file.relative_path.clone(),
            file_name: file.file_name.clone(),
            name: project.title.clone(),
            version: version.version_number.clone(),
            project_id: project.id.clone(),
            version_id: version.id.clone(),
            modrinth_file_name: primary.filename.clone(),
        });
    }
    Ok(found)
}

/// Checks an instance's mods: links what CurseForge recognises, and lists duplicates
/// and CurseForge files that are on Modrinth at the same version.
pub async fn check(instance_id: &str) -> crate::Result<ModReport> {
    let state = State::get().await?;
    let (scanned, newly_linked, curseforge_checked) =
        scan(instance_id, true, &state).await?;
    let duplicates = find_duplicates(&scanned.files);
    let modrinth_equivalents = modrinth_equivalents(&scanned, &state)
        .await
        .unwrap_or_default();
    Ok(ModReport {
        game_version: scanned.game_version,
        loader: scanned.loader,
        files: scanned.files,
        newly_linked,
        duplicates,
        modrinth_equivalents,
        curseforge_checked,
    })
}

/// Links an instance's unknown mods to CurseForge after an import or install, without
/// changing any file.
pub(crate) fn link_in_background(instance_id: String) {
    tokio::spawn(async move {
        let Ok(state) = State::get().await else {
            return;
        };
        if let Err(error) = scan(&instance_id, true, &state).await {
            tracing::warn!("Couldn't check the new instance's mods: {error}");
        }
    });
}

/// Removes one copy of a duplicated mod, keeping another that works with the instance.
/// A snapshot is taken first, and its id is returned so the removal can be undone.
pub async fn remove_duplicate(
    instance_id: &str,
    remove_path: &str,
    keep_path: &str,
) -> crate::Result<String> {
    let state = State::get().await?;
    let (scanned, _, _) = scan(instance_id, false, &state).await?;
    let groups = find_duplicates(&scanned.files);
    let Some(group) = groups.iter().find(|group| {
        group
            .files
            .iter()
            .any(|file| file.relative_path == remove_path)
            && group
                .files
                .iter()
                .any(|file| file.relative_path == keep_path)
    }) else {
        bail!(ErrorKind::InputError(
            "those files aren't copies of the same mod any more".to_string()
        ));
    };
    if remove_path == keep_path {
        bail!(ErrorKind::InputError(
            "the copy to keep can't also be removed".to_string()
        ));
    }
    let keep = group
        .files
        .iter()
        .find(|file| file.relative_path == keep_path)
        .ok_or_else(|| ErrorKind::InputError("unknown file".to_string()))?;
    if keep.compatibility != Compatibility::Compatible || !keep.enabled {
        bail!(ErrorKind::InputError(format!(
            "{} isn't known to work with this instance, so the other copy stays",
            keep.file_name
        )));
    }
    let snapshot = operations::create_snapshot(
        instance_id,
        &format!("Before removing a duplicate of {}", group.name),
    )
    .await?;
    crate::api::instance::remove_project(instance_id, remove_path).await?;
    cf::forget_origin(instance_id, remove_path, &state).await?;
    Ok(snapshot)
}

fn provided_ids(files: &[ModFile], except: &str) -> HashSet<String> {
    let mut ids: HashSet<String> =
        BUILT_IN_IDS.iter().map(|id| id.to_string()).collect();
    for file in files
        .iter()
        .filter(|file| file.enabled && file.relative_path != except)
    {
        if let Some(jar) = &file.metadata {
            ids.extend(jar.mod_ids.iter().cloned());
            ids.extend(jar.bundled_ids.iter().cloned());
        }
    }
    ids
}

#[derive(Clone, Debug, Serialize)]
pub struct SwitchReport {
    pub snapshot_id: String,
    pub new_path: String,
}

/// Replaces a CurseForge file with the same version from Modrinth, after checking the
/// Modrinth jar is the same mod, works with this instance and needs nothing missing.
pub async fn switch_to_modrinth(
    instance_id: &str,
    relative_path: &str,
    version_id: &str,
) -> crate::Result<SwitchReport> {
    let state = State::get().await?;
    let (scanned, _, _) = scan(instance_id, false, &state).await?;
    let file = scanned
        .files
        .iter()
        .find(|file| file.relative_path == relative_path)
        .ok_or_else(|| {
            ErrorKind::InputError("that mod isn't in this instance".to_string())
        })?;
    let current = file.metadata.clone().ok_or_else(|| {
        ErrorKind::InputError(
            "Pawrinth can't read that mod's details".to_string(),
        )
    })?;
    if !matches!(file.source, ModSource::Curseforge { .. }) {
        bail!(ErrorKind::InputError(
            "only mods from CurseForge are switched to Modrinth".to_string()
        ));
    }
    let version = CachedEntry::get_version(
        version_id,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        ErrorKind::InputError("Modrinth couldn't find that version".to_string())
    })?;
    let loaders = modrinth_loader_names(scanned.loader);
    if !version.game_versions.contains(&scanned.game_version)
        || !version
            .loaders
            .iter()
            .any(|loader| loaders.contains(&loader.as_str()))
    {
        bail!(ErrorKind::InputError(
            "that Modrinth version isn't for this Minecraft version and loader"
                .to_string()
        ));
    }
    let primary = version
        .files
        .iter()
        .find(|file| file.primary)
        .or(version.files.first())
        .ok_or_else(|| {
            ErrorKind::InputError("that version has no file".to_string())
        })?;
    let staging = state.directories.caches_dir();
    crate::util::io::create_dir_all(&staging).await?;
    let downloaded = crate::util::fetch::fetch_file(
        &primary.url,
        primary.hashes.get("sha1").map(String::as_str),
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
        None,
        &staging,
    )
    .await?;
    let downloaded_path = downloaded.path().to_path_buf();
    let replacement =
        tokio::task::spawn_blocking(move || read_metadata(&downloaded_path))
            .await?
            .ok_or_else(|| {
                ErrorKind::InputError(
                    "the Modrinth file isn't a mod Pawrinth can read"
                        .to_string(),
                )
            })?;
    drop(downloaded);
    if !replacement
        .mod_ids
        .iter()
        .any(|id| current.mod_ids.contains(id))
    {
        bail!(ErrorKind::InputError(format!(
            "the Modrinth file doesn't declare the same mod as {}, so it stays as it is",
            file.file_name
        )));
    }
    if compatibility(&replacement, scanned.loader, &scanned.game_version)
        == Compatibility::Incompatible
    {
        bail!(ErrorKind::InputError(
            "the Modrinth file doesn't work with this instance".to_string()
        ));
    }
    let provided = provided_ids(&scanned.files, relative_path);
    let missing: Vec<&String> = replacement
        .requires
        .iter()
        .filter(|id| !current.requires.contains(id) && !provided.contains(*id))
        .collect();
    if !missing.is_empty() {
        bail!(ErrorKind::InputError(format!(
            "the Modrinth file also needs {}, which isn't installed",
            missing
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    let snapshot = operations::create_snapshot(
        instance_id,
        &format!("Before switching {} to Modrinth", file.name),
    )
    .await?;
    crate::api::instance::remove_project(instance_id, relative_path).await?;
    cf::forget_origin(instance_id, relative_path, &state).await?;
    match crate::api::instance::add_project_from_version(
        instance_id,
        version_id,
        crate::util::fetch::DownloadReason::Standalone,
        None,
    )
    .await
    {
        Ok(new_path) => Ok(SwitchReport {
            snapshot_id: snapshot,
            new_path,
        }),
        Err(error) => {
            let items = operations::preview_restore(&snapshot).await?;
            let ordinals: Vec<i64> =
                items.iter().map(|item| item.ordinal).collect();
            operations::restore(&snapshot, &ordinals, &[]).await?;
            Err(ErrorKind::InputError(format!(
                "the Modrinth version couldn't be installed, so the CurseForge file was put back: {error}"
            ))
            .into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn jar(
        files: &[(&str, &str)],
    ) -> zip::ZipArchive<std::io::Cursor<Vec<u8>>> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, text) in files {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(text.as_bytes()).unwrap();
        }
        let bytes = writer.finish().unwrap().into_inner();
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap()
    }

    #[test]
    fn fabric_metadata_is_read() {
        let mut archive = jar(&[(
            "fabric.mod.json",
            r#"{ "id": "sodium", "name": "Sodium", "version": "0.5.3",
                "depends": { "minecraft": ">=1.20 <1.21", "fabricloader": ">=0.14", "fabric-api": "*" },
                "provides": ["indium-compat"] }"#,
        )]);
        let metadata = metadata_from_archive(&mut archive, 0);
        assert_eq!(metadata.loaders, [JarLoader::Fabric]);
        assert_eq!(metadata.mod_ids, ["sodium"]);
        assert_eq!(metadata.bundled_ids, ["indium-compat"]);
        assert_eq!(metadata.version.as_deref(), Some("0.5.3"));
        assert_eq!(metadata.requires, ["fabric-api"]);
        assert_eq!(
            minecraft_compatibility(&metadata, "1.20.1"),
            Compatibility::Compatible
        );
        assert_eq!(
            minecraft_compatibility(&metadata, "1.21"),
            Compatibility::Incompatible
        );
    }

    #[test]
    fn forge_metadata_is_read_with_its_manifest_version() {
        let mut archive = jar(&[
            (
                "META-INF/mods.toml",
                r#"modLoader="javafml"
loaderVersion="[47,)"
[[mods]]
modId="jei"
version="${file.jarVersion}"
displayName="Just Enough Items"
[[dependencies.jei]]
modId="forge"
mandatory=true
versionRange="[47,)"
[[dependencies.jei]]
modId="minecraft"
mandatory=true
versionRange="[1.20.1,1.20.2)"
[[dependencies.jei]]
modId="architectury"
type="required"
versionRange="*"
"#,
            ),
            (
                "META-INF/MANIFEST.MF",
                "Manifest-Version: 1.0\r\nImplementation-Version: 15.2.0.27\r\n",
            ),
        ]);
        let metadata = metadata_from_archive(&mut archive, 0);
        assert_eq!(metadata.loaders, [JarLoader::Forge]);
        assert_eq!(metadata.mod_ids, ["jei"]);
        assert_eq!(metadata.version.as_deref(), Some("15.2.0.27"));
        assert_eq!(metadata.requires, ["architectury"]);
        assert_eq!(
            compatibility(&metadata, ModLoader::Forge, "1.20.1"),
            Compatibility::Compatible
        );
        assert_eq!(
            compatibility(&metadata, ModLoader::Forge, "1.20.4"),
            Compatibility::Incompatible
        );
        assert_eq!(
            compatibility(&metadata, ModLoader::Fabric, "1.20.1"),
            Compatibility::Incompatible
        );
        assert_eq!(
            compatibility(&metadata, ModLoader::NeoForge, "1.20.1"),
            Compatibility::Compatible
        );
    }

    #[test]
    fn old_forge_mods_use_mcmod_info() {
        let mut archive = jar(&[(
            "mcmod.info",
            r#"[{ "modid": "ironchest", "name": "Iron Chests", "version": "7.0.72", "mcversion": "1.12.2" }]"#,
        )]);
        let metadata = metadata_from_archive(&mut archive, 0);
        assert_eq!(metadata.loaders, [JarLoader::Forge]);
        assert_eq!(metadata.mod_ids, ["ironchest"]);
        assert_eq!(
            compatibility(&metadata, ModLoader::Forge, "1.12.2"),
            Compatibility::Compatible
        );
        assert_eq!(
            compatibility(&metadata, ModLoader::Forge, "1.7.10"),
            Compatibility::Incompatible
        );
    }

    #[test]
    fn jars_without_mod_metadata_are_not_mods() {
        let mut archive = jar(&[("com/example/Library.class", "x")]);
        assert!(metadata_from_archive(&mut archive, 0).mod_ids.is_empty());
    }

    #[test]
    fn quilt_instances_run_fabric_mods() {
        let metadata = JarMetadata {
            loaders: vec![JarLoader::Fabric],
            mod_ids: vec!["a".to_string()],
            ..JarMetadata::default()
        };
        assert_eq!(
            loader_compatibility(&metadata, ModLoader::Quilt, "1.20.1"),
            Compatibility::Compatible
        );
        assert_eq!(
            loader_compatibility(&metadata, ModLoader::Forge, "1.20.1"),
            Compatibility::Incompatible
        );
    }

    #[test]
    fn fabric_predicates() {
        let game = version_parts("1.20.1").unwrap();
        for (predicate, expected) in [
            ("*", true),
            ("1.20.1", true),
            ("1.20", false),
            ("~1.20", true),
            ("~1.20.1", true),
            ("~1.19", false),
            ("^1.19", true),
            ("1.20.x", true),
            ("1.19.x", false),
            (">=1.20- <1.21-", true),
            (">1.20.1", false),
            ("<=1.20.1", true),
        ] {
            assert_eq!(
                semver_matches(predicate, &game),
                Some(expected),
                "{predicate}"
            );
        }
        assert_eq!(semver_matches("23w31a", &game), None);
    }

    #[test]
    fn maven_ranges() {
        let game = version_parts("1.20.1").unwrap();
        for (spec, expected) in [
            ("[1.20.1,1.21)", true),
            ("[1.20,)", true),
            ("(1.20.1,)", false),
            ("[1.20.1]", true),
            ("[1.19,1.20),[1.20.1,1.20.2)", true),
            ("[1.21,)", false),
            ("1.20.1", true),
        ] {
            assert_eq!(maven_matches(spec, &game), Some(expected), "{spec}");
        }
        assert_eq!(maven_matches("${mcversion}", &game), None);
    }

    #[test]
    fn exact_versions() {
        let game = version_parts("1.12.2").unwrap();
        assert_eq!(exact_matches("1.12.2", &game), Some(true));
        assert_eq!(exact_matches("1.12", &game), Some(false));
        assert_eq!(exact_matches("[1.12,1.13)", &game), Some(true));
        assert_eq!(exact_matches("${mcversion}", &game), None);
    }

    #[test]
    fn mod_versions_compare_numerically() {
        assert_eq!(compare_mod_versions("1.2.10", "1.2.9"), Ordering::Greater);
        assert_eq!(
            compare_mod_versions("v0.29.3+1.16", "0.25.4+1.16"),
            Ordering::Greater
        );
        assert_eq!(compare_mod_versions("1.0", "1.0.1"), Ordering::Less);
        assert!(same_version("v15.2.0.27", "15.2.0.27"));
        assert!(!same_version("", ""));
    }

    #[test]
    fn source_links_are_compared_loosely() {
        assert_eq!(
            normalized_source("https://github.com/CaffeineMC/sodium-fabric/"),
            normalized_source(
                "http://www.github.com/caffeinemc/Sodium-Fabric.git"
            )
        );
        assert_ne!(
            normalized_source("https://github.com/a/one"),
            normalized_source("https://github.com/a/two")
        );
    }

    fn file(
        path: &str,
        ids: &[&str],
        version: &str,
        source: ModSource,
        compatibility: Compatibility,
    ) -> ModFile {
        ModFile {
            relative_path: path.to_string(),
            file_name: path.trim_start_matches("mods/").to_string(),
            enabled: true,
            source,
            name: ids.first().copied().unwrap_or("mod").to_string(),
            version: Some(version.to_string()),
            mod_ids: ids.iter().map(|id| id.to_string()).collect(),
            compatibility,
            metadata: None,
        }
    }

    #[test]
    fn copies_of_one_mod_from_both_sites_are_duplicates() {
        let files = vec![
            file(
                "mods/fabric-api-0.25.4.jar",
                &["fabric-api"],
                "0.25.4+1.16",
                ModSource::Unknown,
                Compatibility::Compatible,
            ),
            file(
                "mods/fabric-api-0.29.3.jar",
                &["fabric-api"],
                "0.29.3+1.16",
                ModSource::Curseforge {
                    project_id: 306612,
                    file_id: 3159126,
                },
                Compatibility::Compatible,
            ),
            file(
                "mods/sodium.jar",
                &["sodium"],
                "0.1.0",
                ModSource::Unknown,
                Compatibility::Compatible,
            ),
        ];
        let groups = find_duplicates(&files);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].keep, "mods/fabric-api-0.29.3.jar");
        assert!(groups[0].can_remove);
    }

    #[test]
    fn similar_names_alone_are_not_duplicates() {
        let files = vec![
            file(
                "mods/jei-1.jar",
                &["jei"],
                "1",
                ModSource::Unknown,
                Compatibility::Compatible,
            ),
            file(
                "mods/jei-addon.jar",
                &["jeiaddon"],
                "1",
                ModSource::Unknown,
                Compatibility::Compatible,
            ),
        ];
        assert!(find_duplicates(&files).is_empty());
    }

    #[test]
    fn the_same_project_is_a_duplicate_even_without_metadata() {
        let source = ModSource::Modrinth {
            project_id: "P7dR8mSH".to_string(),
            version_id: "a".to_string(),
        };
        let files = vec![
            file(
                "mods/a.jar",
                &[],
                "1",
                source.clone(),
                Compatibility::Compatible,
            ),
            file(
                "mods/b.jar",
                &[],
                "2",
                ModSource::Modrinth {
                    project_id: "P7dR8mSH".to_string(),
                    version_id: "b".to_string(),
                },
                Compatibility::Compatible,
            ),
        ];
        assert_eq!(find_duplicates(&files).len(), 1);
    }

    #[test]
    fn nothing_is_removed_when_no_copy_is_known_to_work() {
        let files = vec![
            file(
                "mods/a-1.jar",
                &["a"],
                "1",
                ModSource::Unknown,
                Compatibility::Unknown,
            ),
            file(
                "mods/a-2.jar",
                &["a"],
                "2",
                ModSource::Unknown,
                Compatibility::Incompatible,
            ),
        ];
        let groups = find_duplicates(&files);
        assert_eq!(groups[0].keep, "mods/a-1.jar");
        assert!(!groups[0].can_remove);
    }

    #[test]
    fn disabled_copies_are_not_duplicates() {
        let mut disabled = file(
            "mods/a-1.jar",
            &["a"],
            "1",
            ModSource::Unknown,
            Compatibility::Compatible,
        );
        disabled.enabled = false;
        let files = vec![
            disabled,
            file(
                "mods/a-2.jar",
                &["a"],
                "2",
                ModSource::Unknown,
                Compatibility::Compatible,
            ),
        ];
        assert!(find_duplicates(&files).is_empty());
    }

    #[test]
    fn curseforge_matches_need_the_jar_to_agree_on_the_loader() {
        let fabric = JarMetadata {
            loaders: vec![JarLoader::Fabric],
            mod_ids: vec!["a".to_string()],
            ..JarMetadata::default()
        };
        assert!(confident_curseforge_match(
            Some(&fabric),
            &["1.20.1".to_string(), "Fabric".to_string()]
        ));
        assert!(!confident_curseforge_match(
            Some(&fabric),
            &["1.20.1".to_string(), "Forge".to_string()]
        ));
        assert!(confident_curseforge_match(
            Some(&fabric),
            &["1.12.2".to_string()]
        ));
        assert!(!confident_curseforge_match(None, &["Fabric".to_string()]));
    }
}
