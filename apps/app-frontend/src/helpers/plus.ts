import { invoke } from '@tauri-apps/api/core'

import type { InstallJobSnapshot } from '@/generated/app-events/InstallJobSnapshot'

export type Density = 'comfortable' | 'compact'

export interface PlusSettings {
	appearance: {
		active_theme_id: string | null
		themes: unknown[]
		background: unknown
	}
	interface: {
		ui_scale: number
		reduce_motion: boolean
		density: Density
		remember_last_page: boolean
	}
	rollback: {
		enabled: boolean
		max_operations_per_instance: number
		max_storage_bytes: number
		retention_days: number
		auto_cleanup: boolean
	}
	diagnostics: {
		send_logs_to_mclogs: boolean
	}
	onboarding: {
		wizard_completed: boolean
	}
	privacy: {
		streamer_mode: boolean
		blur_usernames: boolean
		display_name: string | null
	}
	discovery: {
		remember_search_history: boolean
		search_history: string[]
		recently_viewed: unknown[]
	}
	skins: {
		favorites: unknown[]
	}
	discord: {
		use_custom: boolean
		application_id: string | null
		show_instance: boolean
		show_version: boolean
		show_elapsed: boolean
		show_loader_badge: boolean
		idle_text: string | null
		idle_state_text: string | null
		playing_text: string | null
		playing_state_text: string | null
		logo_image: string | null
		logo_text: string | null
		button_label: string | null
		button_url: string | null
		second_button_label: string | null
		second_button_url: string | null
	}
}

type DeepPartial<T> = {
	[K in keyof T]?: T[K] extends unknown[] ? T[K] : T[K] extends object ? DeepPartial<T[K]> : T[K]
}

export type PlusSettingsPatch = DeepPartial<PlusSettings>

export async function plus_settings_get(): Promise<PlusSettings> {
	return await invoke<PlusSettings>('plugin:plus|plus_settings_get')
}

export async function plus_settings_patch(patch: PlusSettingsPatch): Promise<PlusSettings> {
	return await invoke<PlusSettings>('plugin:plus|plus_settings_patch', { patch })
}

export type ItemChange = 'updated' | 'added' | 'removed' | 'toggled'
export type ContentProvider = 'modrinth' | 'local' | 'curseforge'

export interface ManifestEntry {
	file_id: string
	relative_path: string
	file_name: string
	enabled: boolean
	sha512: string
	size: number
	provider: ContentProvider
	project_id: string | null
	version_id: string | null
	project_type: string
	source_kind: string
}

export interface OperationItem {
	ordinal: number
	change: ItemChange
	before: ManifestEntry | null
	after: ManifestEntry | null
}

export type OperationKind = 'update_all' | 'update_one' | 'rollback' | 'snapshot'
export type OperationState =
	| 'applying'
	| 'applied'
	| 'partial'
	| 'reverted'
	| 'captured'
	| 'needs_attention'

export interface Operation {
	id: string
	instance_id: string
	kind: OperationKind
	state: OperationState
	parent_id: string | null
	label: string | null
	game_version: string
	loader: string
	loader_version: string | null
	last_error: string | null
	created_at: number
	finished_at: number | null
	items: OperationItem[]
}

export type RestoreStatus =
	| 'restorable'
	| 'already_restored'
	| 'changed_since'
	| 'removed_since'
	| 'path_occupied'
	| 'backup_damaged'

export interface RestorePreviewItem extends OperationItem {
	status: RestoreStatus
}

export interface RestoreReport {
	restored: string[]
	skipped: string[]
	rollback_operation_id: string | null
}

export interface StorageUsage {
	used_bytes: number
	limit_bytes: number
	operations: number
}

export interface UpdatePreflight {
	required_bytes: number
	available_bytes: number
	enough_space: boolean
	rollback_enabled: boolean
	over_budget: boolean
	usage: StorageUsage
}

export interface VersionedManifest {
	game_version: string
	loader: string
	loader_version: string | null
	entries: ManifestEntry[]
}

export interface VersionDifference {
	left: ManifestEntry
	right: ManifestEntry
}

export interface ManifestComparison {
	same: ManifestEntry[]
	only_left: ManifestEntry[]
	only_right: ManifestEntry[]
	different_versions: VersionDifference[]
	different_sources: VersionDifference[]
}

export interface SyncReport {
	copied: string[]
	skipped: string[]
}

export type FindingKind =
	| 'missing_dependency'
	| 'version_mismatch'
	| 'wrong_loader'
	| 'java_version'
	| 'out_of_memory'
	| 'mod_conflict'
	| 'mixin_failure'
	| 'corrupt_config'

export interface Finding {
	kind: FindingKind
	confidence: 'confirmed' | 'possible'
	evidence: string
	subject: string | null
	related_files: string[]
}

export async function plus_operations_list(instanceId: string): Promise<Operation[]> {
	return await invoke<Operation[]>('plugin:plus|plus_operations_list', { instanceId })
}

export async function plus_operation_preview(operationId: string): Promise<RestorePreviewItem[]> {
	return await invoke<RestorePreviewItem[]>('plugin:plus|plus_operation_preview', { operationId })
}

export async function plus_operation_restore(
	operationId: string,
	ordinals: number[],
	overwriteConflicts: number[],
): Promise<RestoreReport> {
	return await invoke<RestoreReport>('plugin:plus|plus_operation_restore', {
		operationId,
		ordinals,
		overwriteConflicts,
	})
}

export async function plus_operation_delete(operationId: string): Promise<void> {
	await invoke('plugin:plus|plus_operation_delete', { operationId })
}

export async function plus_storage_usage(): Promise<StorageUsage> {
	return await invoke<StorageUsage>('plugin:plus|plus_storage_usage')
}

export async function plus_update_preflight(instanceId: string): Promise<UpdatePreflight> {
	return await invoke<UpdatePreflight>('plugin:plus|plus_update_preflight', { instanceId })
}

export async function plus_snapshot_create(instanceId: string, label: string): Promise<string> {
	return await invoke<string>('plugin:plus|plus_snapshot_create', { instanceId, label })
}

export async function plus_manifest_compare(
	left: string,
	right: string,
): Promise<[VersionedManifest, VersionedManifest, ManifestComparison]> {
	return await invoke('plugin:plus|plus_manifest_compare', { left, right })
}

export async function plus_sync_from_instance(
	sourceId: string,
	targetId: string,
	paths: string[],
): Promise<SyncReport> {
	return await invoke<SyncReport>('plugin:plus|plus_sync_from_instance', {
		sourceId,
		targetId,
		paths,
	})
}

export async function plus_diagnose_log(
	instanceId: string,
	log: string,
): Promise<{ findings: Finding[] }> {
	return await invoke('plugin:plus|plus_diagnose_log', { instanceId, log })
}

export interface CfCategory {
	id: number
	name: string
	slug: string
	classId: number | null
	isClass: boolean | null
}

export interface CfAuthor {
	id?: number | null
	name: string
	url: string | null
	avatarUrl?: string | null
}

export interface CfImage {
	url: string | null
	thumbnailUrl: string | null
	title?: string | null
	description?: string | null
}

export interface CfProject {
	id: number
	name: string
	slug: string
	summary: string
	links: {
		websiteUrl: string | null
		sourceUrl: string | null
		wikiUrl?: string | null
		issuesUrl?: string | null
	}
	classId: number | null
	downloadCount: number
	logo: CfImage | null
	screenshots?: CfImage[]
	thumbsUpCount: number | null
	dateCreated?: string | null
	dateReleased: string | null
	latestFilesIndexes: { gameVersion: string; modLoader: number | null }[]
	latestFiles: { gameVersions: string[] }[]
	authors: CfAuthor[]
	categories: CfCategory[]
	dateModified: string | null
	allowModDistribution: boolean | null
}

export interface CfFile {
	id: number
	modId: number
	displayName: string
	fileName: string
	releaseType: number
	fileDate: string
	fileLength: number
	downloadCount?: number
	downloadUrl: string | null
	gameVersions: string[]
	dependencies: { modId: number; relationType: number }[]
}

export interface CfSearchQuery {
	classId?: number | null
	searchFilter?: string | null
	gameVersion?: string | null
	modLoaderType?: number | null
	categoryId?: number | null
	authorId?: number | null
	sortField?: number | null
	sortOrder?: 'asc' | 'desc' | null
	index: number
	pageSize: number
}

export interface CfSearchResults {
	data: CfProject[]
	pagination: { index: number; pageSize: number; resultCount: number; totalCount: number }
}

export type CfCompatibility =
	| { status: 'compatible' }
	| { status: 'unknown'; reason: string }
	| { status: 'incompatible'; reason: string }

export interface CfFileOption {
	file: CfFile
	compatibility: CfCompatibility
}

export interface CfInstallOptions {
	files: CfFileOption[]
	recommended_file_id: number | null
}

export interface CfPlannedDependency {
	project_id: number
	name: string
	website_url: string | null
	status: 'will_install' | 'already_installed' | 'unavailable'
	file: CfFile | null
	reason: string | null
}

export interface CfInstallPlan {
	project: CfProject
	file: CfFile
	compatibility: CfCompatibility
	can_download: boolean
	required: CfPlannedDependency[]
	optional: CfPlannedDependency[]
	conflicts: string[]
}

export interface CfInstallReport {
	installed: string[]
	already_installed: string[]
	unavailable: string[]
}

export interface CfAvailableUpdate {
	relative_path: string
	project: CfProject
	current_file_name: string
	current_file_id: number
	file: CfFile
	can_download: boolean
}

export type CfDataMode = 'live' | 'sample' | 'needs_key' | 'key_rejected'

export async function plus_cf_data_mode(): Promise<CfDataMode> {
	return await invoke('plugin:plus|plus_cf_data_mode')
}

export type CfKeyCheck =
	| 'valid'
	| 'rejected'
	| 'rate_limited'
	| 'service_error'
	| 'unreachable'
	| 'malformed'

export interface CfKeyStatus {
	source: 'user' | 'development' | null
	/** Only the last few characters. No command ever returns the key itself. */
	hint: string | null
	check: CfKeyCheck | null
	store_available: boolean
}

export interface CfKeyOutcome {
	check: CfKeyCheck
	saved: boolean
	unusual_format: boolean
}

export async function plus_cf_key_status(): Promise<CfKeyStatus> {
	return await invoke('plugin:plus|plus_cf_key_status')
}

export async function plus_cf_key_save(key: string): Promise<CfKeyOutcome> {
	return await invoke('plugin:plus|plus_cf_key_save', { key })
}

/** Tests a key that hasn't been saved, or the saved one when given `null`. */
export async function plus_cf_key_test(key: string | null): Promise<CfKeyOutcome> {
	return await invoke('plugin:plus|plus_cf_key_test', { key })
}

export async function plus_cf_key_remove(): Promise<void> {
	await invoke('plugin:plus|plus_cf_key_remove')
}

export interface CfGameVersion {
	versionString: string
	gameVersionTypeId: number | null
	approved: boolean | null
}

export async function plus_cf_game_versions(): Promise<CfGameVersion[]> {
	return await invoke('plugin:plus|plus_cf_game_versions')
}

export async function plus_cf_search(query: CfSearchQuery): Promise<CfSearchResults> {
	return await invoke('plugin:plus|plus_cf_search', { query })
}

export async function plus_cf_classes(): Promise<CfCategory[]> {
	return await invoke('plugin:plus|plus_cf_classes')
}

export async function plus_cf_categories(classId: number): Promise<CfCategory[]> {
	return await invoke('plugin:plus|plus_cf_categories', { classId })
}

export async function plus_cf_project(id: number): Promise<CfProject> {
	return await invoke('plugin:plus|plus_cf_project', { id })
}

export async function plus_cf_description(id: number): Promise<string> {
	return await invoke('plugin:plus|plus_cf_description', { id })
}

export async function plus_cf_files(
	projectId: number,
	gameVersion: string | null,
	loaderType: number | null,
): Promise<CfFile[]> {
	return await invoke('plugin:plus|plus_cf_files', { projectId, gameVersion, loaderType })
}

export async function plus_cf_changelog(projectId: number, fileId: number): Promise<string> {
	return await invoke('plugin:plus|plus_cf_changelog', { projectId, fileId })
}

export async function plus_cf_install_options(
	instanceId: string,
	projectId: number,
	includeIncompatible: boolean,
	loaderOverride: string | null,
): Promise<CfInstallOptions> {
	return await invoke('plugin:plus|plus_cf_install_options', {
		instanceId,
		projectId,
		includeIncompatible,
		loaderOverride,
	})
}

export async function plus_cf_plan_install(
	instanceId: string,
	projectId: number,
	fileId: number,
	loaderOverride: string | null,
): Promise<CfInstallPlan> {
	return await invoke('plugin:plus|plus_cf_plan_install', {
		instanceId,
		projectId,
		fileId,
		loaderOverride,
	})
}

export async function plus_cf_install(
	instanceId: string,
	projectId: number,
	fileId: number,
	allowIncompatible: boolean,
	skipUnavailable: boolean,
	loaderOverride: string | null,
): Promise<CfInstallReport> {
	return await invoke('plugin:plus|plus_cf_install', {
		loaderOverride,
		instanceId,
		projectId,
		fileId,
		allowIncompatible,
		skipUnavailable,
	})
}

export interface CfInstalledFile {
	relative_path: string
	project_id: number
	file_id: number
	project: CfProject | null
	current_file: CfFile | null
	update: CfFile | null
}

export async function plus_cf_installed(instanceId: string): Promise<CfInstalledFile[]> {
	return await invoke('plugin:plus|plus_cf_installed', { instanceId })
}

export async function plus_cf_check_updates(instanceId: string): Promise<CfAvailableUpdate[]> {
	return await invoke('plugin:plus|plus_cf_check_updates', { instanceId })
}

export async function plus_cf_apply_updates(
	instanceId: string,
	selections: { relative_path: string; project_id: number; file_id: number }[],
): Promise<string> {
	return await invoke('plugin:plus|plus_cf_apply_updates', { instanceId, selections })
}

export type CfMissingReason = 'not_distributable' | 'unavailable' | 'unsupported'

export interface CfMissingFile {
	project_id: number
	file_id: number
	name: string
	file_name: string | null
	download_page: string | null
	reason: CfMissingReason
}

export interface CfPreparedPack {
	token: string
	project_id: number
	file_id: number
	name: string
	version: string
	game_version: string
	loader: string
	loader_version: string | null
	file_count: number
	total_bytes: number
	missing: CfMissingFile[]
	optional_skipped: number
	kept: string[]
}

export interface CfPackInfo {
	instance_id: string
	project_id: number
	file_id: number
	name: string
	version: string
	missing: CfMissingFile[]
	kept: string[]
}

export async function plus_cf_pack_prepare(
	projectId: number,
	fileId: number | null,
	instanceId: string | null,
): Promise<CfPreparedPack> {
	return await invoke('plugin:plus|plus_cf_pack_prepare', { projectId, fileId, instanceId })
}

export async function plus_cf_pack_install(
	token: string,
	instanceId: string | null,
): Promise<InstallJobSnapshot> {
	return await invoke('plugin:plus|plus_cf_pack_install', { token, instanceId })
}

export async function plus_cf_installed_in(projectId: number): Promise<string[]> {
	return await invoke('plugin:plus|plus_cf_installed_in', { projectId })
}

export async function plus_cf_installed_projects(instanceId: string): Promise<number[]> {
	return await invoke('plugin:plus|plus_cf_installed_projects', { instanceId })
}

export interface CfExportReport {
	listed: number
	matched: number
	included: string[]
	looked_up: boolean
}

export async function plus_cf_export(
	instanceId: string,
	exportPath: string,
	includedPaths: string[],
	excludedPaths: string[],
	version: string | null,
	name: string | null,
): Promise<CfExportReport> {
	return await invoke('plugin:plus|plus_cf_export', {
		instanceId,
		exportPath,
		includedPaths,
		excludedPaths,
		version,
		name,
	})
}

export async function plus_cf_pack_list(): Promise<CfPackInfo[]> {
	return await invoke('plugin:plus|plus_cf_pack_list')
}

export async function plus_theme_file_read(path: string): Promise<string> {
	return await invoke('plugin:plus|plus_theme_file_read', { path })
}

export async function plus_theme_file_save(path: string, contents: string): Promise<void> {
	await invoke('plugin:plus|plus_theme_file_save', { path, contents })
}

export type NewsCategory =
	| 'release'
	| 'update'
	| 'feature'
	| 'maintenance'
	| 'compatibility'
	| 'community'
	| 'announcement'

export interface PlusNewsArticle {
	title: string
	summary: string
	date: string
	link: string
	thumbnail: string | null
	category: NewsCategory
}

export interface PlusNewsFeed {
	configured: boolean
	articles: PlusNewsArticle[]
	all_news_url: string | null
}

export async function plus_news_feed(): Promise<PlusNewsFeed> {
	return await invoke('plugin:plus|plus_news_feed')
}

export type DiscordProblem =
	| { kind: 'not_running' | 'blocked' | 'application_refused' | 'no_reply' }
	| { kind: 'activity_refused'; message: string }

export interface DiscordStatus {
	enabled: boolean
	application: 'built_in' | 'custom'
	connected: boolean
	problem: DiscordProblem | null
	install_button: boolean
}

export interface DiscordCheck {
	status: DiscordStatus
	application_known: boolean | null
	application_name: string | null
	logo_image: string
	logo_uploaded: boolean | null
}

export interface DiscordPresence {
	details: string
	state: string | null
	started_at: number | null
	logo_image: string
	logo_text: string
	badge: { image: string; text: string } | null
	buttons: { label: string; url: string }[]
}

export interface DiscordPreview {
	idle: DiscordPresence
	playing: DiscordPresence
}

export async function plus_discord_status(): Promise<DiscordStatus> {
	return await invoke('plugin:plus|plus_discord_status')
}

export async function plus_discord_preview(): Promise<DiscordPreview> {
	return await invoke('plugin:plus|plus_discord_preview')
}

export async function plus_discord_refresh(): Promise<void> {
	await invoke('plugin:plus|plus_discord_refresh')
}

export async function plus_discord_check(): Promise<DiscordCheck> {
	return await invoke('plugin:plus|plus_discord_check')
}

export interface PlayerSkin {
	id: string
	name: string
	model: 'CLASSIC' | 'SLIM'
	texture: string | null
	texture_key: string | null
}

export type SkinLookup =
	| { status: 'found'; player: PlayerSkin }
	| { status: 'not_found' | 'rate_limited' | 'unavailable' }

export async function plus_skin_lookup(query: string): Promise<SkinLookup> {
	return await invoke('plugin:plus|plus_skin_lookup', { query })
}

export interface GallerySkin {
	id: string
	name: string | null
	texture_hash: string
}

export type GalleryPage =
	| { status: 'ok'; skins: GallerySkin[]; next: string | null }
	| { status: 'rate_limited' | 'key_rejected' | 'unavailable' }

export interface GallerySkinDetails {
	model: 'CLASSIC' | 'SLIM' | null
	tags: string[]
	views: number | null
}

export async function plus_skin_gallery(
	query: string | null,
	after: string | null,
): Promise<GalleryPage> {
	return await invoke('plugin:plus|plus_skin_gallery', { query, after })
}

export async function plus_skin_gallery_details(id: string): Promise<GallerySkinDetails | null> {
	return await invoke('plugin:plus|plus_skin_gallery_details', { id })
}

export interface MineskinKeyStatus {
	hint: string | null
	store_available: boolean
}

export interface MineskinKeyOutcome {
	check: 'valid' | 'rejected' | 'rate_limited' | 'service_error' | 'unreachable' | 'malformed'
	saved: boolean
}

export async function plus_mineskin_key_status(): Promise<MineskinKeyStatus> {
	return await invoke('plugin:plus|plus_mineskin_key_status')
}

export async function plus_mineskin_key_save(key: string): Promise<MineskinKeyOutcome> {
	return await invoke('plugin:plus|plus_mineskin_key_save', { key })
}

export async function plus_mineskin_key_remove(): Promise<void> {
	await invoke('plugin:plus|plus_mineskin_key_remove')
}

export async function plus_skin_file_save(path: string, bytes: Uint8Array): Promise<void> {
	await invoke('plugin:plus|plus_skin_file_save', { path, bytes: Array.from(bytes) })
}

export async function plus_background_import(path: string): Promise<string> {
	return await invoke('plugin:plus|plus_background_import', { path })
}

export interface MinecraftFolder {
	path: string
	saves: number
	versions: number
}

export async function plus_detect_minecraft_folder(): Promise<MinecraftFolder | null> {
	return await invoke('plugin:plus|plus_detect_minecraft_folder')
}
