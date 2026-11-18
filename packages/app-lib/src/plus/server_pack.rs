//! One-click server packs: an instance's mods and configs, without the mods that only
//! work in the game client, plus the mod loader's server and start scripts, in a new
//! folder. A mod is only left out when something says it's client-only: its own
//! `fabric.mod.json` or `quilt.mod.json`, Modrinth's server-side field, or CurseForge's
//! Client tag. Everything else stays, so the pack errs towards having too much.
//!
//! Server files come from Mojang, Fabric, Quilt, Forge and NeoForge over HTTPS. The
//! Minecraft EULA is never accepted here; the server asks for it on its first start.

use super::curseforge::{self as cf, DataMode};
use crate::state::{CachedEntry, ModLoader, SideType, State};
use crate::util::fetch::REQWEST_CLIENT;
use crate::{ErrorKind, state::instances::adapters::sqlite::content_rows};
use anyhow::bail;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Folders the server needs as they are, besides mods.
const COPIED_FOLDERS: [&str; 4] =
    ["config", "defaultconfigs", "kubejs", "scripts"];
const DEFAULT_MEMORY_MB: u32 = 4096;
const MOJANG_MANIFEST: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const MAX_METADATA_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LeftOutReason {
    /// The mod's own metadata says it's for the client.
    DeclaredClient,
    /// Modrinth says the mod doesn't work on servers.
    ModrinthClientOnly,
    /// CurseForge tags the file for the client only.
    CurseforgeClientOnly,
}

#[derive(Clone, Debug, Serialize)]
pub struct LeftOut {
    pub file: String,
    pub reason: LeftOutReason,
}

#[derive(Clone, Debug, Serialize)]
pub struct ServerPackReport {
    pub folder: String,
    pub mods: usize,
    pub left_out: Vec<LeftOut>,
    /// The Java the server needs, from Mojang's version details.
    pub java_version: Option<u32>,
}

/// Whether a Fabric or Quilt mod declares itself client-only. Forge and NeoForge
/// metadata has no such field.
fn declared_client_only(path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(std::io::BufReader::new(file))
    else {
        return false;
    };
    let mut read = |name: &str| -> Option<serde_json::Value> {
        let entry = archive.by_name(name).ok()?;
        let mut text = String::new();
        entry
            .take(MAX_METADATA_BYTES)
            .read_to_string(&mut text)
            .ok()?;
        serde_json::from_str(&text).ok()
    };
    if let Some(fabric) = read("fabric.mod.json") {
        return fabric.get("environment").and_then(|value| value.as_str())
            == Some("client");
    }
    read("quilt.mod.json")
        .and_then(|quilt| {
            quilt
                .pointer("/minecraft/environment")
                .and_then(|value| value.as_str())
                .map(|environment| environment == "client")
        })
        .unwrap_or(false)
}

/// CurseForge tags files with `Client` and `Server` where the author said so.
fn curseforge_client_only(game_versions: &[String]) -> bool {
    let has = |tag: &str| {
        game_versions
            .iter()
            .any(|version| version.eq_ignore_ascii_case(tag))
    };
    has("Client") && !has("Server")
}

/// Before 1.17, Forge's installer makes a server jar to start instead of run scripts.
fn uses_run_scripts(game_version: &str) -> bool {
    match game_version.strip_prefix("1.") {
        Some(rest) => rest
            .split('.')
            .next()
            .and_then(|minor| minor.parse::<u32>().ok())
            .is_none_or(|minor| minor >= 17),
        None => true,
    }
}

/// Forge's loader version, without the Minecraft version some records put in front.
fn bare_loader_version<'a>(version: &'a str, game_version: &str) -> &'a str {
    version
        .strip_prefix(&format!("{game_version}-"))
        .unwrap_or(version)
}

async fn get_json(url: &str) -> crate::Result<serde_json::Value> {
    Ok(REQWEST_CLIENT
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

async fn download(url: &str, destination: &Path) -> crate::Result<()> {
    if !url.starts_with("https://") {
        bail!(ErrorKind::InputError(format!(
            "refusing to download a server file over plain HTTP: {url}"
        )));
    }
    let bytes = REQWEST_CLIENT
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    tokio::fs::write(destination, &bytes).await?;
    Ok(())
}

/// Mojang's details for a Minecraft version, for its server jar and Java version.
async fn mojang_version(
    game_version: &str,
) -> crate::Result<serde_json::Value> {
    let manifest = get_json(MOJANG_MANIFEST).await?;
    let url = manifest["versions"]
        .as_array()
        .and_then(|versions| {
            versions
                .iter()
                .find(|version| version["id"] == game_version)
        })
        .and_then(|version| version["url"].as_str())
        .ok_or_else(|| {
            ErrorKind::InputError(format!(
                "Mojang doesn't list Minecraft {game_version}"
            ))
        })?
        .to_string();
    get_json(&url).await
}

struct Launch {
    /// Run once, before the first start, when the loader has to install itself.
    install: Option<String>,
    /// What starts the server, with `{memory}` for the memory flag.
    start: String,
    /// Forge and NeoForge read Java options from this file instead.
    jvm_args_file: bool,
}

async fn loader_files(
    folder: &Path,
    loader: ModLoader,
    loader_version: Option<&str>,
    game_version: &str,
    mojang: &serde_json::Value,
) -> crate::Result<Launch> {
    let need_version = || {
        loader_version
            .filter(|version| !version.is_empty())
            .ok_or_else(|| {
                ErrorKind::InputError(
                    "this instance doesn't say which loader version it uses"
                        .to_string(),
                )
            })
    };
    let java = "java {memory}";
    match loader {
        ModLoader::Vanilla => {
            let url = mojang["downloads"]["server"]["url"]
                .as_str()
                .ok_or_else(|| {
                    ErrorKind::InputError(format!(
                        "Mojang has no server for Minecraft {game_version}"
                    ))
                })?;
            download(url, &folder.join("server.jar")).await?;
            Ok(Launch {
                install: None,
                start: format!("{java} -jar server.jar nogui"),
                jvm_args_file: false,
            })
        }
        ModLoader::Fabric => {
            let installers =
                get_json("https://meta.fabricmc.net/v2/versions/installer")
                    .await?;
            let installer = installers
                .as_array()
                .and_then(|list| {
                    list.iter().find(|item| item["stable"] == true)
                })
                .and_then(|item| item["version"].as_str())
                .ok_or_else(|| {
                    ErrorKind::InputError(
                        "Fabric's installer list couldn't be read".to_string(),
                    )
                })?
                .to_string();
            let loader = need_version()?;
            download(
                &format!(
                    "https://meta.fabricmc.net/v2/versions/loader/{game_version}/{loader}/{installer}/server/jar"
                ),
                &folder.join("fabric-server-launch.jar"),
            )
            .await?;
            Ok(Launch {
                install: None,
                start: format!("{java} -jar fabric-server-launch.jar nogui"),
                jvm_args_file: false,
            })
        }
        ModLoader::Quilt => {
            let installers =
                get_json("https://meta.quiltmc.org/v3/versions/installer")
                    .await?;
            let installer = installers
                .as_array()
                .and_then(|list| list.first())
                .and_then(|item| item["version"].as_str())
                .ok_or_else(|| {
                    ErrorKind::InputError(
                        "Quilt's installer list couldn't be read".to_string(),
                    )
                })?
                .to_string();
            let loader = need_version()?;
            download(
                &format!(
                    "https://maven.quiltmc.org/repository/release/org/quiltmc/quilt-installer/{installer}/quilt-installer-{installer}.jar"
                ),
                &folder.join("quilt-installer.jar"),
            )
            .await?;
            Ok(Launch {
                install: Some(format!(
                    "java -jar quilt-installer.jar install server {game_version} {loader} --download-server --install-dir=."
                )),
                start: format!("{java} -jar quilt-server-launch.jar nogui"),
                jvm_args_file: false,
            })
        }
        ModLoader::Forge | ModLoader::NeoForge => {
            let version = bare_loader_version(need_version()?, game_version);
            let url = match loader {
                ModLoader::Forge => format!(
                    "https://maven.minecraftforge.net/net/minecraftforge/forge/{game_version}-{version}/forge-{game_version}-{version}-installer.jar"
                ),
                _ if game_version == "1.20.1" => format!(
                    "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-{version}/forge-1.20.1-{version}-installer.jar"
                ),
                _ => format!(
                    "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
                ),
            };
            download(&url, &folder.join("installer.jar")).await?;
            let install =
                Some("java -jar installer.jar --installServer".to_string());
            Ok(if uses_run_scripts(game_version) {
                Launch {
                    install,
                    start: "run".to_string(),
                    jvm_args_file: true,
                }
            } else {
                Launch {
                    install,
                    start: format!(
                        "{java} -jar forge-{game_version}-{version}.jar nogui"
                    ),
                    jvm_args_file: false,
                }
            })
        }
    }
}

fn start_scripts(launch: &Launch, memory_mb: u32) -> (String, String) {
    let memory = format!("-Xmx{memory_mb}M");
    let start = launch.start.replace("{memory}", &memory);
    let mut windows = String::from("@echo off\r\ncd /d \"%~dp0\"\r\n");
    let mut unix =
        String::from("#!/usr/bin/env sh\ncd \"$(dirname \"$0\")\"\n");
    if let Some(install) = &launch.install {
        let marker = if launch.jvm_args_file {
            "user_jvm_args.txt"
        } else {
            ".pawrinth-installed"
        };
        windows.push_str(&format!(
            "if not exist {marker} (\r\n  {install} || exit /b 1\r\n  type nul >> {marker}\r\n)\r\n"
        ));
        unix.push_str(&format!(
            "if [ ! -f {marker} ]; then\n  {install} || exit 1\n  touch {marker}\nfi\n"
        ));
    }
    if launch.jvm_args_file {
        windows.push_str(&format!(
            "findstr /b /c:\"-Xmx\" user_jvm_args.txt >nul || echo {memory}>> user_jvm_args.txt\r\ncall run.bat nogui\r\n"
        ));
        unix.push_str(&format!(
            "grep -q '^-Xmx' user_jvm_args.txt || echo '{memory}' >> user_jvm_args.txt\nsh ./run.sh nogui\n"
        ));
    } else {
        windows.push_str(&format!("{start}\r\npause\r\n"));
        unix.push_str(&format!("{start}\n"));
    }
    (windows, unix)
}

fn readme(
    name: &str,
    game_version: &str,
    loader: ModLoader,
    java_version: Option<u32>,
    left_out: &[LeftOut],
) -> String {
    let java = java_version
        .map(|version| format!("Java {version}"))
        .unwrap_or_else(|| "the Java this Minecraft version needs".to_string());
    let loader = match loader {
        ModLoader::Vanilla => "no mod loader",
        ModLoader::Forge => "Forge",
        ModLoader::NeoForge => "NeoForge",
        ModLoader::Fabric => "Fabric",
        ModLoader::Quilt => "Quilt",
    };
    let mut text = format!(
        "Server for {name} (Minecraft {game_version}, {loader}), made by Pawrinth.\n\n\
         1. Install {java} on the server.\n\
         2. Run start.bat on Windows, or start.sh elsewhere. The first start sets up the mod loader, then stops to ask you to accept the Minecraft EULA.\n\
         3. Read https://aka.ms/MinecraftEULA, then set eula=true in eula.txt and start again.\n"
    );
    if !left_out.is_empty() {
        text.push_str(
            "\nThese mods were left out because they only work in the game client:\n",
        );
        for item in left_out {
            text.push_str(&format!("- {}\n", item.file));
        }
        text.push_str("Players can still use them in their own game.\n");
    }
    text
}

pub(super) fn copy_folder(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_folder(&entry.path(), &target)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Makes a server folder for an instance inside `parent`. The folder is new, so
/// nothing that's already there is changed.
pub async fn create(
    instance_id: &str,
    parent: PathBuf,
) -> crate::Result<ServerPackReport> {
    let state = State::get().await?;
    let Some(metadata) = crate::api::instance::get(instance_id).await? else {
        bail!(ErrorKind::InputError("unknown instance".to_string()));
    };
    let instance_dir = crate::api::instance::get_full_path(instance_id).await?;
    let content_set = &metadata.applied_content_set;
    let game_version = content_set.game_version.clone();
    let loader = content_set.loader;
    let name = metadata.instance.name.clone();

    let folder_name: String = format!("{name} server")
        .chars()
        .map(|char| {
            if char.is_control() || "<>:\"/\\|?*".contains(char) {
                '_'
            } else {
                char
            }
        })
        .collect();
    let folder = parent.join(folder_name.trim().trim_end_matches('.'));
    if folder.exists() {
        bail!(ErrorKind::InputError(format!(
            "{} already exists; choose another place or remove it first",
            folder.display()
        )));
    }

    let mojang = mojang_version(&game_version).await?;
    let java_version = mojang["javaVersion"]["majorVersion"]
        .as_u64()
        .and_then(|version| u32::try_from(version).ok());

    let files = content_rows::get_instance_files(instance_id, &state.pool)
        .await?
        .into_iter()
        .filter(|file| {
            !file.missing
                && file.enabled
                && file.relative_path.starts_with("mods/")
                && file.relative_path.ends_with(".jar")
        })
        .collect::<Vec<_>>();

    let hashes: Vec<&str> =
        files.iter().map(|file| file.sha1.as_str()).collect();
    let modrinth_projects: HashMap<String, String> =
        CachedEntry::get_file_many(
            &hashes,
            None,
            &state.pool,
            &state.api_semaphore,
        )
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|file| (file.hash, file.project_id))
        .collect();
    let project_ids: Vec<&str> =
        modrinth_projects.values().map(String::as_str).collect();
    let client_only_projects: Vec<String> = CachedEntry::get_project_many(
        &project_ids,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await
    .unwrap_or_default()
    .into_iter()
    .filter(|project| matches!(project.server_side, SideType::Unsupported))
    .map(|project| project.id)
    .collect();

    let origins = cf::origins(instance_id, &state).await.unwrap_or_default();
    let curseforge_files: HashMap<u64, Vec<String>> =
        if cf::data_mode().await == DataMode::Live {
            let ids: Vec<u64> = files
                .iter()
                .filter_map(|file| origins.get(&file.relative_path))
                .filter_map(|(_, file_id)| file_id.parse().ok())
                .collect();
            cf::fetch_files_by_id(&ids, &state)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|file| (file.id, file.game_versions))
                .collect()
        } else {
            HashMap::new()
        };

    let mut kept = Vec::new();
    let mut left_out = Vec::new();
    for file in &files {
        let path = instance_dir.join(&file.relative_path);
        let declared = {
            let path = path.clone();
            tokio::task::spawn_blocking(move || declared_client_only(&path))
                .await?
        };
        let reason = if declared {
            Some(LeftOutReason::DeclaredClient)
        } else if modrinth_projects
            .get(&file.sha1)
            .is_some_and(|project| client_only_projects.contains(project))
        {
            Some(LeftOutReason::ModrinthClientOnly)
        } else if origins
            .get(&file.relative_path)
            .and_then(|(_, file_id)| file_id.parse::<u64>().ok())
            .and_then(|file_id| curseforge_files.get(&file_id))
            .is_some_and(|tags| curseforge_client_only(tags))
        {
            Some(LeftOutReason::CurseforgeClientOnly)
        } else {
            None
        };
        match reason {
            Some(reason) => left_out.push(LeftOut {
                file: file.file_name.clone(),
                reason,
            }),
            None => kept.push(path),
        }
    }

    tokio::fs::create_dir_all(folder.join("mods")).await?;
    let result = async {
        let launch = loader_files(
            &folder,
            loader,
            content_set.loader_version.as_deref(),
            &game_version,
            &mojang,
        )
        .await?;
        let copy_folder_path = folder.clone();
        let copy_instance = instance_dir.clone();
        let copy_kept = kept.clone();
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            for path in &copy_kept {
                if let Some(file_name) = path.file_name() {
                    std::fs::copy(
                        path,
                        copy_folder_path.join("mods").join(file_name),
                    )?;
                }
            }
            for name in COPIED_FOLDERS {
                let source = copy_instance.join(name);
                if source.is_dir() {
                    copy_folder(&source, &copy_folder_path.join(name))?;
                }
            }
            Ok(())
        })
        .await??;
        let memory = metadata
            .launch_overrides
            .memory
            .map(|memory| memory.maximum)
            .unwrap_or(DEFAULT_MEMORY_MB);
        let (windows, unix) = start_scripts(&launch, memory);
        tokio::fs::write(folder.join("start.bat"), windows).await?;
        tokio::fs::write(folder.join("start.sh"), unix).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(
                folder.join("start.sh"),
                std::fs::Permissions::from_mode(0o755),
            )
            .await?;
        }
        tokio::fs::write(
            folder.join("README.txt"),
            readme(&name, &game_version, loader, java_version, &left_out),
        )
        .await?;
        crate::Result::Ok(())
    }
    .await;
    if let Err(error) = result {
        let _ = tokio::fs::remove_dir_all(&folder).await;
        return Err(error);
    }

    Ok(ServerPackReport {
        folder: folder.to_string_lossy().into_owned(),
        mods: kept.len(),
        left_out,
        java_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jar(entry: &str, text: &str) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut zip = zip::ZipWriter::new(file.reopen().unwrap());
        zip.start_file(entry, zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut zip, text.as_bytes()).unwrap();
        zip.finish().unwrap();
        file
    }

    #[test]
    fn only_mods_that_say_client_are_left_out() {
        let client =
            jar("fabric.mod.json", r#"{"id":"a","environment":"client"}"#);
        let both = jar("fabric.mod.json", r#"{"id":"b","environment":"*"}"#);
        let quilt = jar(
            "quilt.mod.json",
            r#"{"quilt_loader":{"id":"c"},"minecraft":{"environment":"client"}}"#,
        );
        let forge = jar("META-INF/mods.toml", "modLoader=\"javafml\"");
        assert!(declared_client_only(client.path()));
        assert!(!declared_client_only(both.path()));
        assert!(declared_client_only(quilt.path()));
        assert!(!declared_client_only(forge.path()));
    }

    #[test]
    fn curseforge_tags_need_client_without_server() {
        let tags = |list: &[&str]| -> Vec<String> {
            list.iter().map(|tag| tag.to_string()).collect()
        };
        assert!(curseforge_client_only(&tags(&[
            "1.20.1", "Client", "Forge"
        ])));
        assert!(!curseforge_client_only(&tags(&[
            "1.20.1", "Client", "Server"
        ])));
        assert!(!curseforge_client_only(&tags(&["1.20.1", "Forge"])));
    }

    #[test]
    fn forge_versions_and_scripts_follow_the_minecraft_version() {
        assert!(!uses_run_scripts("1.16.5"));
        assert!(uses_run_scripts("1.17.1"));
        assert!(uses_run_scripts("1.21.1"));
        assert!(uses_run_scripts("26.3"));
        assert_eq!(bare_loader_version("1.20.1-47.2.0", "1.20.1"), "47.2.0");
        assert_eq!(bare_loader_version("47.2.0", "1.20.1"), "47.2.0");
    }

    #[test]
    fn the_readme_names_the_loader_and_what_was_left_out() {
        let text = readme(
            "Pack",
            "1.21.1",
            ModLoader::NeoForge,
            Some(21),
            &[LeftOut {
                file: "sodium.jar".to_string(),
                reason: LeftOutReason::DeclaredClient,
            }],
        );
        assert!(text.contains("(Minecraft 1.21.1, NeoForge)"));
        assert!(text.contains("Install Java 21"));
        assert!(text.contains("- sodium.jar\n"));
        assert!(text.lines().all(|line| !line.starts_with(' ')));
    }

    #[test]
    fn scripts_install_once_and_set_memory() {
        let launch = Launch {
            install: Some(
                "java -jar installer.jar --installServer".to_string(),
            ),
            start: "run".to_string(),
            jvm_args_file: true,
        };
        let (windows, unix) = start_scripts(&launch, 6144);
        assert!(windows.contains("if not exist user_jvm_args.txt"));
        assert!(windows.contains("echo -Xmx6144M>> user_jvm_args.txt"));
        assert!(unix.contains("sh ./run.sh nogui"));
        let fabric = Launch {
            install: None,
            start: "java {memory} -jar fabric-server-launch.jar nogui"
                .to_string(),
            jvm_args_file: false,
        };
        let (windows, _) = start_scripts(&fabric, 4096);
        assert!(
            windows
                .contains("java -Xmx4096M -jar fabric-server-launch.jar nogui")
        );
    }
}
