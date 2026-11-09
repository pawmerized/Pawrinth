use crate::api::Result;
use theseus::plus::curseforge::{
    self, AvailableUpdate, Category, DataMode, GameVersion, InstallOptions,
    InstallPlan, InstallReport, Project, ProjectFile, SearchQuery,
    SearchResults, UpdateSelection,
};
use theseus::plus::curseforge_export::{self, ExportReport};
use theseus::plus::curseforge_key::{self, KeyOutcome, KeyStatus};
use theseus::plus::curseforge_pack::{self, PackInfo, PreparedPack};
use theseus::plus::diagnostics::{self, CrashDiagnosis};
use theseus::plus::discord::{
    self, DiscordCheck, DiscordPreview, DiscordStatus,
};
use theseus::plus::manifest::{
    ManifestComparison, VersionedManifest, compare_manifests,
};
use theseus::plus::memory::{self, MemoryStatus};
use theseus::plus::mod_matching::{self, ModReport, SwitchReport};
use theseus::plus::news::{self, NewsFeed};
use theseus::plus::operations::{
    self, Operation, RestorePreviewItem, RestoreReport, StorageUsage,
    SyncReport, UpdatePreflight,
};
use theseus::plus::settings::{self, PlusSettings};
use theseus::plus::skin_gallery::{
    self, GalleryKeyOutcome, GalleryKeyStatus, GalleryPage, GallerySkinDetails,
};
use theseus::plus::skin_lookup::{self, SkinLookup};
use theseus::plus::technic::{self, PackDetails, PackSummary};
use theseus::plus::theme_files;
use theseus::plus::world_backups::{
    self, InstanceBackups, RestoreReport as WorldRestoreReport, WorldBackup,
};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("plus")
        .invoke_handler(tauri::generate_handler![
            plus_settings_get,
            plus_settings_patch,
            plus_operations_list,
            plus_operation_preview,
            plus_operation_restore,
            plus_operation_delete,
            plus_storage_usage,
            plus_update_preflight,
            plus_snapshot_create,
            plus_manifest_compare,
            plus_sync_from_instance,
            plus_diagnose_log,
            plus_detect_minecraft_folder,
            plus_background_import,
            plus_theme_file_read,
            plus_theme_file_save,
            plus_news_feed,
            plus_discord_status,
            plus_discord_preview,
            plus_discord_refresh,
            plus_discord_check,
            plus_skin_lookup,
            plus_skin_file_save,
            plus_skin_gallery,
            plus_skin_gallery_details,
            plus_mineskin_key_status,
            plus_mineskin_key_save,
            plus_mineskin_key_remove,
            plus_cf_data_mode,
            plus_cf_key_status,
            plus_cf_key_save,
            plus_cf_key_test,
            plus_cf_key_remove,
            plus_cf_game_versions,
            plus_cf_search,
            plus_cf_classes,
            plus_cf_categories,
            plus_cf_project,
            plus_cf_description,
            plus_cf_files,
            plus_cf_changelog,
            plus_cf_install_options,
            plus_cf_plan_install,
            plus_cf_install,
            plus_cf_check_updates,
            plus_cf_installed,
            plus_cf_apply_updates,
            plus_cf_pack_prepare,
            plus_cf_pack_install,
            plus_cf_pack_list,
            plus_cf_export,
            plus_cf_installed_in,
            plus_cf_installed_projects,
            plus_technic_browse,
            plus_technic_pack,
            plus_technic_prepare,
            plus_technic_install,
            plus_memory_status,
            plus_memory_set_manual,
            plus_memory_use_automatic,
            plus_ftb_app_instances,
            plus_mods_check,
            plus_mods_remove_duplicate,
            plus_mods_switch_to_modrinth,
            plus_world_backups_list,
            plus_world_backups_set_schedule,
            plus_world_backup_now,
            plus_world_backup_restore,
            plus_world_backup_delete,
        ])
        .build()
}

#[tauri::command]
pub async fn plus_settings_get() -> Result<PlusSettings> {
    Ok(settings::get().await?)
}

#[tauri::command]
pub async fn plus_settings_patch(
    patch: serde_json::Value,
) -> Result<PlusSettings> {
    Ok(settings::patch(patch).await?)
}

#[tauri::command]
pub async fn plus_operations_list(
    instance_id: String,
) -> Result<Vec<Operation>> {
    Ok(operations::list_operations(&instance_id).await?)
}

#[tauri::command]
pub async fn plus_operation_preview(
    operation_id: String,
) -> Result<Vec<RestorePreviewItem>> {
    Ok(operations::preview_restore(&operation_id).await?)
}

#[tauri::command]
pub async fn plus_operation_restore(
    operation_id: String,
    ordinals: Vec<i64>,
    overwrite_conflicts: Vec<i64>,
) -> Result<RestoreReport> {
    Ok(
        operations::restore(&operation_id, &ordinals, &overwrite_conflicts)
            .await?,
    )
}

#[tauri::command]
pub async fn plus_operation_delete(operation_id: String) -> Result<()> {
    Ok(operations::delete_operation(&operation_id).await?)
}

#[tauri::command]
pub async fn plus_storage_usage() -> Result<StorageUsage> {
    Ok(operations::storage_usage().await?)
}

#[tauri::command]
pub async fn plus_update_preflight(
    instance_id: String,
) -> Result<UpdatePreflight> {
    Ok(operations::preflight_update(&instance_id).await?)
}

#[tauri::command]
pub async fn plus_snapshot_create(
    instance_id: String,
    label: String,
) -> Result<String> {
    Ok(operations::create_snapshot(&instance_id, &label).await?)
}

/// Each side is an instance id, or a snapshot's operation id.
#[tauri::command]
pub async fn plus_manifest_compare(
    left: String,
    right: String,
) -> Result<(VersionedManifest, VersionedManifest, ManifestComparison)> {
    async fn read(id: &str) -> theseus::Result<VersionedManifest> {
        if id.starts_with("local:") {
            operations::instance_manifest(id).await
        } else {
            operations::snapshot_manifest(id).await
        }
    }
    let left = read(&left).await?;
    let right = read(&right).await?;
    let comparison = compare_manifests(&left.entries, &right.entries);
    Ok((left, right, comparison))
}

#[tauri::command]
pub async fn plus_sync_from_instance(
    source_id: String,
    target_id: String,
    paths: Vec<String>,
) -> Result<SyncReport> {
    Ok(operations::sync_from_instance(&source_id, &target_id, &paths).await?)
}

#[tauri::command]
pub async fn plus_diagnose_log(
    instance_id: String,
    log: String,
) -> Result<CrashDiagnosis> {
    Ok(diagnostics::diagnose_instance_log(&instance_id, &log).await?)
}

#[tauri::command]
pub async fn plus_cf_data_mode() -> Result<DataMode> {
    Ok(curseforge::data_mode().await)
}

#[tauri::command]
pub async fn plus_cf_key_status() -> Result<KeyStatus> {
    Ok(curseforge_key::status().await)
}

#[tauri::command]
pub async fn plus_cf_key_save(key: String) -> Result<KeyOutcome> {
    Ok(curseforge_key::save(&key).await?)
}

#[tauri::command]
pub async fn plus_cf_key_test(key: Option<String>) -> Result<KeyOutcome> {
    Ok(curseforge_key::test(key.as_deref()).await?)
}

#[tauri::command]
pub async fn plus_cf_key_remove() -> Result<()> {
    Ok(curseforge_key::remove().await?)
}

#[tauri::command]
pub async fn plus_cf_game_versions() -> Result<Vec<GameVersion>> {
    Ok(curseforge::game_versions().await?)
}

#[tauri::command]
pub async fn plus_cf_search(query: SearchQuery) -> Result<SearchResults> {
    Ok(curseforge::search(query).await?)
}

#[tauri::command]
pub async fn plus_cf_classes() -> Result<Vec<Category>> {
    Ok(curseforge::classes().await?)
}

#[tauri::command]
pub async fn plus_cf_categories(class_id: u32) -> Result<Vec<Category>> {
    Ok(curseforge::categories(class_id).await?)
}

#[tauri::command]
pub async fn plus_cf_project(id: u64) -> Result<Project> {
    Ok(curseforge::project(id).await?)
}

#[tauri::command]
pub async fn plus_cf_description(id: u64) -> Result<String> {
    Ok(curseforge::description(id).await?)
}

#[tauri::command]
pub async fn plus_cf_files(
    project_id: u64,
    game_version: Option<String>,
    loader_type: Option<u32>,
) -> Result<Vec<ProjectFile>> {
    Ok(curseforge::files(project_id, game_version, loader_type).await?)
}

#[tauri::command]
pub async fn plus_cf_changelog(
    project_id: u64,
    file_id: u64,
) -> Result<String> {
    Ok(curseforge::changelog(project_id, file_id).await?)
}

#[tauri::command]
pub async fn plus_cf_install_options(
    instance_id: String,
    project_id: u64,
    include_incompatible: bool,
    loader_override: Option<String>,
) -> Result<InstallOptions> {
    Ok(curseforge::install_options(
        &instance_id,
        project_id,
        include_incompatible,
        loader_override.as_deref(),
    )
    .await?)
}

#[tauri::command]
pub async fn plus_cf_plan_install(
    instance_id: String,
    project_id: u64,
    file_id: u64,
    loader_override: Option<String>,
) -> Result<InstallPlan> {
    Ok(curseforge::plan_install(
        &instance_id,
        project_id,
        file_id,
        loader_override.as_deref(),
    )
    .await?)
}

#[tauri::command]
pub async fn plus_cf_install(
    instance_id: String,
    project_id: u64,
    file_id: u64,
    allow_incompatible: bool,
    skip_unavailable: bool,
    loader_override: Option<String>,
) -> Result<InstallReport> {
    Ok(curseforge::install(
        &instance_id,
        project_id,
        file_id,
        allow_incompatible,
        skip_unavailable,
        loader_override.as_deref(),
    )
    .await?)
}

#[tauri::command]
pub async fn plus_cf_installed(
    instance_id: String,
) -> Result<Vec<curseforge::InstalledFile>> {
    Ok(curseforge::installed_files(&instance_id).await?)
}

#[tauri::command]
pub async fn plus_cf_check_updates(
    instance_id: String,
) -> Result<Vec<AvailableUpdate>> {
    Ok(curseforge::check_updates(&instance_id).await?)
}

#[tauri::command]
pub async fn plus_cf_apply_updates(
    instance_id: String,
    selections: Vec<UpdateSelection>,
) -> Result<String> {
    Ok(curseforge::apply_updates(&instance_id, selections).await?)
}

#[tauri::command]
pub async fn plus_cf_pack_prepare(
    project_id: u64,
    file_id: Option<u64>,
    instance_id: Option<String>,
) -> Result<PreparedPack> {
    Ok(curseforge_pack::prepare(project_id, file_id, instance_id).await?)
}

#[tauri::command]
pub async fn plus_cf_pack_install(
    token: String,
    instance_id: Option<String>,
) -> Result<theseus::install::InstallJobSnapshot> {
    Ok(curseforge_pack::install(&token, instance_id).await?)
}

#[tauri::command]
pub async fn plus_cf_installed_in(project_id: u64) -> Result<Vec<String>> {
    Ok(curseforge::installed_in(project_id).await?)
}

#[tauri::command]
pub async fn plus_cf_installed_projects(
    instance_id: String,
) -> Result<Vec<u64>> {
    Ok(curseforge::installed_projects(&instance_id).await?)
}

#[tauri::command]
pub async fn plus_cf_export(
    instance_id: String,
    export_path: std::path::PathBuf,
    included_paths: Vec<String>,
    excluded_paths: Vec<String>,
    version: Option<String>,
    name: Option<String>,
) -> Result<ExportReport> {
    Ok(curseforge_export::export(
        &instance_id,
        export_path,
        included_paths,
        excluded_paths,
        version,
        name,
    )
    .await?)
}

#[tauri::command]
pub async fn plus_cf_pack_list() -> Result<Vec<PackInfo>> {
    Ok(curseforge_pack::installed_packs().await?)
}

#[tauri::command]
pub async fn plus_technic_browse(
    query: Option<String>,
) -> Result<Vec<PackSummary>> {
    Ok(technic::browse(query).await?)
}

#[tauri::command]
pub async fn plus_technic_pack(slug: String) -> Result<PackDetails> {
    Ok(technic::pack(slug).await?)
}

#[tauri::command]
pub async fn plus_technic_prepare(
    slug: String,
    allow_insecure: bool,
) -> Result<technic::PreparedPack> {
    Ok(technic::prepare(slug, allow_insecure).await?)
}

#[tauri::command]
pub async fn plus_technic_install(
    token: String,
) -> Result<theseus::install::InstallJobSnapshot> {
    Ok(technic::install(token).await?)
}

/// FTB App instances in a folder, with their own names, since their folders are
/// named by id.
#[tauri::command]
pub async fn plus_ftb_app_instances(
    path: std::path::PathBuf,
) -> Result<Vec<theseus::pack::import::ftb_app::FtbAppInstance>> {
    Ok(theseus::pack::import::ftb_app::list_instances(&path).await?)
}

/// Links unknown mods to CurseForge and lists duplicates and Modrinth equivalents.
#[tauri::command]
pub async fn plus_mods_check(instance_id: String) -> Result<ModReport> {
    Ok(mod_matching::check(&instance_id).await?)
}

#[tauri::command]
pub async fn plus_mods_remove_duplicate(
    instance_id: String,
    remove_path: String,
    keep_path: String,
) -> Result<String> {
    Ok(
        mod_matching::remove_duplicate(&instance_id, &remove_path, &keep_path)
            .await?,
    )
}

#[tauri::command]
pub async fn plus_mods_switch_to_modrinth(
    instance_id: String,
    relative_path: String,
    version_id: String,
) -> Result<SwitchReport> {
    Ok(mod_matching::switch_to_modrinth(
        &instance_id,
        &relative_path,
        &version_id,
    )
    .await?)
}

#[tauri::command]
pub async fn plus_world_backups_list(
    instance_id: String,
) -> Result<InstanceBackups> {
    Ok(world_backups::list(&instance_id).await?)
}

#[tauri::command]
pub async fn plus_world_backups_set_schedule(
    instance_id: String,
    schedule: settings::WorldBackupSchedule,
) -> Result<settings::WorldBackupSchedule> {
    Ok(world_backups::set_schedule(&instance_id, schedule).await?)
}

#[tauri::command]
pub async fn plus_world_backup_now(
    instance_id: String,
    world: String,
) -> Result<WorldBackup> {
    Ok(world_backups::back_up_now(&instance_id, &world).await?)
}

/// Replaces a world with one of its backups, after backing up the world as it is.
#[tauri::command]
pub async fn plus_world_backup_restore(
    instance_id: String,
    world: String,
    file_name: String,
) -> Result<WorldRestoreReport> {
    Ok(world_backups::restore(&instance_id, &world, &file_name).await?)
}

#[tauri::command]
pub async fn plus_world_backup_delete(
    instance_id: String,
    world: String,
    file_name: String,
) -> Result<()> {
    Ok(world_backups::delete(&instance_id, &world, &file_name).await?)
}

#[tauri::command]
pub async fn plus_memory_status(instance_id: String) -> Result<MemoryStatus> {
    Ok(memory::status(&instance_id).await?)
}

#[tauri::command]
pub async fn plus_memory_set_manual(instance_id: String) -> Result<()> {
    Ok(memory::set_manual(&instance_id).await?)
}

#[tauri::command]
pub async fn plus_memory_use_automatic(
    instance_id: String,
) -> Result<MemoryStatus> {
    Ok(memory::use_automatic(&instance_id).await?)
}

#[tauri::command]
pub async fn plus_background_import(
    path: std::path::PathBuf,
) -> Result<std::path::PathBuf> {
    Ok(theseus::plus::backgrounds::import(&path).await?)
}

#[tauri::command]
pub async fn plus_theme_file_read(path: std::path::PathBuf) -> Result<String> {
    Ok(theme_files::read(&path).await?)
}

#[tauri::command]
pub async fn plus_theme_file_save(
    path: std::path::PathBuf,
    contents: String,
) -> Result<()> {
    Ok(theme_files::save(&path, &contents).await?)
}

#[tauri::command]
pub async fn plus_news_feed() -> Result<NewsFeed> {
    Ok(news::feed().await?)
}

#[tauri::command]
pub async fn plus_discord_status() -> Result<DiscordStatus> {
    Ok(discord::status().await?)
}

#[tauri::command]
pub async fn plus_discord_preview() -> Result<DiscordPreview> {
    Ok(discord::preview().await?)
}

#[tauri::command]
pub async fn plus_discord_refresh() -> Result<()> {
    Ok(discord::refresh().await?)
}

#[tauri::command]
pub async fn plus_discord_check() -> Result<DiscordCheck> {
    Ok(discord::check().await?)
}

#[tauri::command]
pub async fn plus_skin_lookup(query: String) -> Result<SkinLookup> {
    Ok(skin_lookup::lookup(&query).await)
}

#[tauri::command]
pub async fn plus_skin_gallery(
    query: Option<String>,
    after: Option<String>,
) -> Result<GalleryPage> {
    Ok(skin_gallery::list(query.as_deref(), after.as_deref()).await)
}

#[tauri::command]
pub async fn plus_skin_gallery_details(
    id: String,
) -> Result<Option<GallerySkinDetails>> {
    Ok(skin_gallery::details(&id).await)
}

#[tauri::command]
pub async fn plus_mineskin_key_status() -> Result<GalleryKeyStatus> {
    Ok(skin_gallery::key_status().await)
}

#[tauri::command]
pub async fn plus_mineskin_key_save(key: String) -> Result<GalleryKeyOutcome> {
    Ok(skin_gallery::key_save(&key).await?)
}

#[tauri::command]
pub async fn plus_mineskin_key_remove() -> Result<()> {
    Ok(skin_gallery::key_remove().await?)
}

#[tauri::command]
pub async fn plus_skin_file_save(
    path: std::path::PathBuf,
    bytes: Vec<u8>,
) -> Result<()> {
    Ok(skin_lookup::save_png(&path, &bytes).await?)
}

#[tauri::command]
pub async fn plus_detect_minecraft_folder()
-> Result<Option<theseus::plus::setup::MinecraftFolder>> {
    Ok(theseus::plus::setup::detect_minecraft_folder().await)
}
