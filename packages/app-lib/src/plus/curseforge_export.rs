//! Exports an instance as a CurseForge modpack: the `.zip` with a `manifest.json`
//! that the CurseForge app imports. Files installed from CurseForge are listed by
//! project and file. With a key, other mods and packs are matched against
//! CurseForge by fingerprint, the way the CurseForge app recognises them. Whatever
//! isn't on CurseForge goes inside the zip, as a Modrinth export does with files
//! that aren't on Modrinth.

use super::curseforge::{self as api, DataMode};
use crate::ErrorKind;
use crate::api::instance::{
    ExportSelection, check_export_destination, collect_export_files,
    logical_content_path,
};
use crate::event::LoadingBarType;
use crate::event::emit::{emit_loading, init_loading};
use crate::state::content_store::{ReadableContent, is_managed_content_path};
use crate::state::{ModLoader, State};
use anyhow::Context;
use path_util::SafeRelativeUtf8UnixPathBuf;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const OVERRIDES: &str = "overrides";
const COPY_BUFFER_SIZE: usize = 256 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportManifest {
    minecraft: ExportMinecraft,
    manifest_type: String,
    manifest_version: u32,
    name: String,
    version: String,
    author: String,
    files: Vec<ExportFile>,
    overrides: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportMinecraft {
    version: String,
    mod_loaders: Vec<ExportLoader>,
}

#[derive(Serialize, Deserialize)]
struct ExportLoader {
    id: String,
    primary: bool,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct ExportFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    required: bool,
}

/// What an export did with the instance's mods and packs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportReport {
    /// Files the CurseForge app downloads itself.
    pub listed: usize,
    /// Of those, files recognised by their fingerprint.
    pub matched: usize,
    /// Mods and packs that aren't on CurseForge, so they're inside the zip.
    pub included: Vec<String>,
    /// Whether CurseForge could be asked about files Pawrinth didn't install from it.
    pub looked_up: bool,
}

/// CurseForge's names for loaders, such as `fabric-0.16.10` and `forge-47.4.0`.
/// NeoForge for 1.20.1 still uses its old `1.20.1-` version names there.
fn loader_id(
    loader: ModLoader,
    loader_version: Option<&str>,
    game_version: &str,
) -> crate::Result<Option<String>> {
    let name = match loader {
        ModLoader::Vanilla => return Ok(None),
        ModLoader::Forge => "forge",
        ModLoader::NeoForge => "neoforge",
        ModLoader::Fabric => "fabric",
        ModLoader::Quilt => "quilt",
    };
    let Some(version) = loader_version.filter(|version| !version.is_empty())
    else {
        anyhow::bail!(ErrorKind::InputError(
            "this instance doesn't say which loader version it uses"
                .to_string()
        ));
    };
    Ok(Some(
        if loader == ModLoader::NeoForge && game_version == "1.20.1" {
            format!("{name}-{game_version}-{version}")
        } else {
            format!("{name}-{version}")
        },
    ))
}

/// MurmurHash2 with seed 1, skipping tabs, line breaks and spaces, which is how
/// CurseForge fingerprints files.
pub(crate) fn fingerprint(bytes: &[u8]) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let data: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|byte| !matches!(byte, 9 | 10 | 13 | 32))
        .collect();
    let mut hash = 1_u32 ^ data.len() as u32;
    let mut chunks = data.chunks_exact(4);
    for chunk in &mut chunks {
        let mut k =
            u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        hash = hash.wrapping_mul(M) ^ k;
    }
    let rest = chunks.remainder();
    if !rest.is_empty() {
        for (index, byte) in rest.iter().enumerate().rev() {
            hash ^= u32::from(*byte) << (8 * index);
        }
        hash = hash.wrapping_mul(M);
    }
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(M);
    hash ^ (hash >> 15)
}

async fn fingerprint_file(path: &Path) -> crate::Result<u32> {
    let path = path.to_path_buf();
    Ok(
        tokio::task::spawn_blocking(move || -> std::io::Result<u32> {
            Ok(fingerprint(&std::fs::read(path)?))
        })
        .await??,
    )
}

fn file_name(path: &str) -> String {
    path.trim_end_matches(".disabled")
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_string()
}

fn write_archive(
    destination: &Path,
    manifest: &[u8],
    overrides: &[(PathBuf, String)],
    mut progress: impl FnMut(u64),
) -> crate::Result<()> {
    let file = std::fs::File::create(destination)?;
    let mut writer = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = vec![0_u8; COPY_BUFFER_SIZE];
    for (source, relative) in overrides {
        writer.start_file(format!("{OVERRIDES}/{relative}"), options)?;
        let mut input = std::fs::File::open(source)?;
        loop {
            let read = input.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            writer.write_all(&buffer[..read])?;
            progress(read as u64);
        }
    }
    writer.start_file("manifest.json", options)?;
    writer.write_all(manifest)?;
    writer.finish()?.flush()?;
    Ok(())
}

/// Writes `export_path` as a CurseForge modpack of the selected files.
pub async fn export(
    instance_id: &str,
    export_path: PathBuf,
    included_paths: Vec<String>,
    excluded_paths: Vec<String>,
    version: Option<String>,
    name: Option<String>,
) -> crate::Result<ExportReport> {
    let state = State::get().await?;
    check_export_destination(&state, &export_path).await?;
    let metadata =
        crate::api::instance::get(instance_id)
            .await?
            .context(format!(
                "tried to export a nonexistent instance {instance_id}"
            ))?;
    let game_version = metadata.applied_content_set.game_version.clone();
    let loader = loader_id(
        metadata.applied_content_set.loader,
        metadata.applied_content_set.loader_version.as_deref(),
        &game_version,
    )?;
    let selection = ExportSelection::new(included_paths, excluded_paths);
    let instance_dir = crate::api::instance::get_full_path(instance_id).await?;
    let collected = collect_export_files(
        &state,
        instance_id,
        &instance_dir,
        &selection,
        &HashSet::new(),
    )
    .await?;
    let origins = api::origins(instance_id, &state).await?;

    let mut files = Vec::new();
    let mut overrides: Vec<(ReadableContent, SafeRelativeUtf8UnixPathBuf)> =
        Vec::new();
    let mut unknown: Vec<(ReadableContent, SafeRelativeUtf8UnixPathBuf)> =
        Vec::new();
    for (content, relative, _) in collected {
        let logical = logical_content_path(&relative)?;
        if !is_managed_content_path(logical.as_str()) {
            overrides.push((content, relative));
            continue;
        }
        let required = !relative.as_str().ends_with(".disabled");
        let origin =
            origins.get(logical.as_str()).and_then(|(project, file)| {
                Some((
                    project.strip_prefix("curseforge:")?.parse::<u64>().ok()?,
                    file.parse::<u64>().ok()?,
                ))
            });
        match origin {
            Some((project_id, file_id)) => files.push(ExportFile {
                project_id,
                file_id,
                required,
            }),
            None => unknown.push((content, relative)),
        }
    }

    let looked_up =
        !unknown.is_empty() && api::data_mode().await == DataMode::Live;
    let mut matched = 0;
    if looked_up {
        let mut fingerprints = Vec::with_capacity(unknown.len());
        for (content, _) in &unknown {
            fingerprints.push(fingerprint_file(content.path()).await?);
        }
        let found =
            api::fetch_fingerprint_matches(&fingerprints, &state).await?;
        let mut still_unknown = Vec::new();
        for ((content, relative), fingerprint) in
            unknown.into_iter().zip(fingerprints)
        {
            match found.get(&fingerprint) {
                Some((project_id, file_id)) => {
                    matched += 1;
                    files.push(ExportFile {
                        project_id: *project_id,
                        file_id: *file_id,
                        required: !relative.as_str().ends_with(".disabled"),
                    });
                }
                None => still_unknown.push((content, relative)),
            }
        }
        unknown = still_unknown;
    }
    let mut seen = HashSet::new();
    files.retain(|file| seen.insert(file.file_id));
    let mut included: Vec<String> = unknown
        .iter()
        .map(|(_, relative)| file_name(relative.as_str()))
        .collect();
    included.sort();
    overrides.extend(unknown);

    let manifest = ExportManifest {
        minecraft: ExportMinecraft {
            version: game_version,
            mod_loaders: loader
                .into_iter()
                .map(|id| ExportLoader { id, primary: true })
                .collect(),
        },
        manifest_type: "minecraftModpack".to_string(),
        manifest_version: 1,
        name: name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| metadata.instance.name.clone()),
        version: version
            .filter(|version| !version.trim().is_empty())
            .unwrap_or_else(|| "1.0.0".to_string()),
        author: String::new(),
        files,
        overrides: OVERRIDES.to_string(),
    };
    let report = ExportReport {
        listed: manifest.files.len(),
        matched,
        included,
        looked_up,
    };
    let manifest = serde_json::to_vec_pretty(&manifest)?;
    let sources: Vec<(PathBuf, String)> = overrides
        .iter()
        .map(|(content, relative)| {
            (content.path().to_path_buf(), relative.as_str().to_string())
        })
        .collect();
    let total = sources
        .iter()
        .filter_map(|(path, _)| std::fs::metadata(path).ok())
        .map(|metadata| metadata.len())
        .sum::<u64>()
        .max(1);
    let bar = init_loading(
        LoadingBarType::PackExport {
            instance_id: metadata.instance.id.clone(),
            instance_name: metadata.instance.name.clone(),
        },
        total as f64,
        "Exporting instance for CurseForge",
    )
    .await?;
    tokio::task::spawn_blocking(move || {
        let _overrides = overrides;
        write_archive(&export_path, &manifest, &sources, |bytes| {
            let _ = emit_loading(&bar, bytes as f64, None);
        })
    })
    .await??;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_ids_use_curseforge_names() {
        assert_eq!(
            loader_id(ModLoader::Fabric, Some("0.16.10"), "1.21.1").unwrap(),
            Some("fabric-0.16.10".to_string())
        );
        assert_eq!(
            loader_id(ModLoader::Forge, Some("47.4.28"), "1.20.1").unwrap(),
            Some("forge-47.4.28".to_string())
        );
        assert_eq!(
            loader_id(ModLoader::NeoForge, Some("21.1.90"), "1.21.1").unwrap(),
            Some("neoforge-21.1.90".to_string())
        );
        assert_eq!(
            loader_id(ModLoader::NeoForge, Some("47.1.106"), "1.20.1").unwrap(),
            Some("neoforge-1.20.1-47.1.106".to_string())
        );
        assert_eq!(
            loader_id(ModLoader::Vanilla, None, "1.21.1").unwrap(),
            None
        );
        assert!(loader_id(ModLoader::Fabric, None, "1.21.1").is_err());
    }

    #[test]
    fn exported_loader_ids_read_back_the_same() {
        for (loader, version, game) in [
            (ModLoader::Fabric, "0.16.10", "1.21.1"),
            (ModLoader::NeoForge, "47.1.106", "1.20.1"),
            (ModLoader::Forge, "47.4.28", "1.20.1"),
        ] {
            let id = loader_id(loader, Some(version), game).unwrap().unwrap();
            assert_eq!(
                super::super::curseforge_pack::parse_loader(&id, game),
                Some((loader, version.to_string()))
            );
        }
    }

    #[test]
    fn fingerprints_ignore_whitespace() {
        assert_eq!(fingerprint(b"a b\tc\r\nd"), fingerprint(b"abcd"));
        assert_ne!(fingerprint(b"abcd"), fingerprint(b"abce"));
    }

    #[test]
    fn fingerprints_cover_every_tail_length() {
        let values: HashSet<u32> =
            (0..8).map(|len| fingerprint(&b"abcdefgh"[..len])).collect();
        assert_eq!(values.len(), 8);
    }

    #[test]
    fn the_manifest_has_the_shape_curseforge_reads() {
        let manifest = ExportManifest {
            minecraft: ExportMinecraft {
                version: "1.21.1".to_string(),
                mod_loaders: vec![ExportLoader {
                    id: "fabric-0.16.10".to_string(),
                    primary: true,
                }],
            },
            manifest_type: "minecraftModpack".to_string(),
            manifest_version: 1,
            name: "Pack".to_string(),
            version: "1.0.0".to_string(),
            author: String::new(),
            files: vec![ExportFile {
                project_id: 1,
                file_id: 2,
                required: true,
            }],
            overrides: OVERRIDES.to_string(),
        };
        let json = serde_json::to_value(&manifest).unwrap();
        assert_eq!(json["manifestType"], "minecraftModpack");
        assert_eq!(json["minecraft"]["modLoaders"][0]["id"], "fabric-0.16.10");
        assert_eq!(json["files"][0]["projectID"], 1);
        assert_eq!(json["files"][0]["fileID"], 2);
        assert_eq!(json["overrides"], "overrides");
    }

    #[test]
    fn the_archive_has_the_manifest_and_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("a.toml");
        std::fs::write(&config, "a = 1").unwrap();
        let out = dir.path().join("pack.zip");
        let mut written = 0;
        write_archive(
            &out,
            br#"{"manifestType":"minecraftModpack"}"#,
            &[(config, "config/a.toml".to_string())],
            |bytes| written += bytes,
        )
        .unwrap();
        assert_eq!(written, 5);
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let mut names: Vec<String> = archive
            .file_names()
            .map(|name| name.unwrap().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["manifest.json", "overrides/config/a.toml"]);
        let mut text = String::new();
        archive
            .by_name("overrides/config/a.toml")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "a = 1");
    }

    #[test]
    fn file_names_drop_the_folder_and_disabled_suffix() {
        assert_eq!(file_name("mods/example.jar.disabled"), "example.jar");
        assert_eq!(file_name("resourcepacks/pack.zip"), "pack.zip");
    }
}
