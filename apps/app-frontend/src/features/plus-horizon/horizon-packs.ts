import type { Labrinth } from '@modrinth/api-client'
import { invoke } from '@tauri-apps/api/core'

import { HORIZON_LOADER } from './horizon'

export type HorizonPackKind = 'resource_pack' | 'shader'

/** One of Horizon's own packs, in the version made for the instance's Minecraft version. */
export interface HorizonPack {
	id: string
	kind: HorizonPackKind
	name: string
	summary: string
	authors: string[]
	license: string | null
	icon: string | null
	categories: string[]
	screenshots: string[]
	downloads: number
	version: string
	size: number
	page_url: string
	/** The file of this pack the instance has. */
	installed: string | null
	update_available: boolean
	/** When this version came out, and when the pack's first one did. */
	updated: string | null
	created: string | null
}

/** The address segment for each kind on Horizon's browse page. */
export const KIND_PATHS: Record<HorizonPackKind, string> = {
	resource_pack: 'resourcepacks',
	shader: 'shaders',
}

/** Where Horizon reads each kind from, inside the instance. */
const KIND_FOLDERS: Record<HorizonPackKind, string> = {
	resource_pack: 'resourcepacks',
	shader: 'shaderpacks',
}

/** The installed file's path in the instance, without `.disabled`, as content lists name it. */
export function installedPath(pack: HorizonPack): string | null {
	return pack.installed
		? `${KIND_FOLDERS[pack.kind]}/${pack.installed.replace(/\.disabled$/, '')}`
		: null
}

export function kindFromPath(path: unknown): HorizonPackKind {
	return path === 'shaders' ? 'shader' : 'resource_pack'
}

/** The browse types Horizon has packs for. */
export function horizonKindFor(projectType: string | undefined): HorizonPackKind | null {
	if (projectType === 'resourcepack' || projectType === 'texture-packs') return 'resource_pack'
	if (projectType === 'shader' || projectType === 'shaders') return 'shader'
	return null
}

/**
 * Where Discover opens for an instance and a kind of content. A Horizon instance only has
 * Horizon's own packs, so shaders open its Shaders and anything else its Resource packs.
 */
export function discoverLocation(
	loader: string,
	instanceId: string,
	projectType: string,
): { path: string; query: { i: string } } {
	const path =
		loader === HORIZON_LOADER
			? `/horizon/${KIND_PATHS[horizonKindFor(projectType) ?? 'resource_pack']}`
			: `/browse/${projectType}`
	return { path, query: { i: instanceId } }
}

export function horizonPacksPath(kind: HorizonPackKind, instanceId: string): string {
	return `/horizon/${KIND_PATHS[kind]}?i=${encodeURIComponent(instanceId)}`
}

/** Packs whose name, summary, author or categories contain every word searched for. */
export function matchesSearch(pack: HorizonPack, search: string): boolean {
	const haystack = [pack.name, pack.summary, ...pack.authors, ...pack.categories]
		.join(' ')
		.toLowerCase()
	return search
		.toLowerCase()
		.split(/\s+/)
		.filter(Boolean)
		.every((word) => haystack.includes(word))
}

export type HorizonSearchHit = Labrinth.Search.v3.ResultSearchProject & { horizon: HorizonPack }

/** Shapes a Horizon pack like a Modrinth search result, so it shows in the same browse layout. */
export function toHorizonHit(pack: HorizonPack): HorizonSearchHit {
	return {
		project_id: `horizon:${pack.id}`,
		slug: pack.id,
		name: pack.name,
		title: pack.name,
		summary: pack.summary,
		description: pack.summary,
		icon_url: pack.icon,
		author: pack.authors.join(', '),
		author_id: null,
		organization: null,
		organization_id: null,
		categories: pack.categories,
		display_categories: pack.categories,
		loaders: [],
		project_types: [pack.kind === 'shader' ? 'shader' : 'resourcepack'],
		downloads: pack.downloads,
		date_created: pack.created ?? '',
		date_modified: pack.updated ?? '',
		featured_gallery: pack.screenshots[0] ?? null,
		project_loader_fields: {},
		horizon: pack,
	} as unknown as HorizonSearchHit
}

/** Modrinth's sorts that Horizon's registry has data for. It has no follower counts. */
export type HorizonSort = 'relevance' | 'downloads' | 'newest' | 'updated'

/**
 * The packs matching a search, in the chosen order. The registry is small, so this runs here.
 * It has no search ranking, so relevance puts the most downloaded matches first.
 */
export function findPacks(packs: HorizonPack[], search: string, sort: HorizonSort): HorizonPack[] {
	const found = packs.filter((pack) => matchesSearch(pack, search))
	const date = (value: string | null) => value ?? ''
	if (sort === 'newest') return found.sort((a, b) => date(b.created).localeCompare(date(a.created)))
	if (sort === 'updated')
		return found.sort((a, b) => date(b.updated).localeCompare(date(a.updated)))
	return found.sort((a, b) => b.downloads - a.downloads)
}

/** One of Modrinth's categories, as its tag list gives them. */
export interface CategoryTag {
	name: string
	project_type: string
	header: string
}

/** Filters for the categories Horizon's packs use, grouped under Modrinth's headings. */
export interface CategoryGroup {
	id: string
	header: string
	/** Resolutions match any choice, like on Modrinth; other groups match all of them. */
	method: 'and' | 'or'
	names: string[]
}

const HEADER_ORDER = ['categories', 'features', 'resolutions', 'performance impact']

/**
 * Horizon's registry uses Modrinth's category names, so its packs get Modrinth's groups. A
 * category Modrinth doesn't list goes under Categories.
 */
export function categoryGroups(
	packs: HorizonPack[],
	tags: CategoryTag[],
	projectType: string,
): CategoryGroup[] {
	const headerOf = new Map(
		tags.filter((tag) => tag.project_type === projectType).map((tag) => [tag.name, tag.header]),
	)
	const groups = new Map<string, CategoryGroup>()
	for (const name of [...new Set(packs.flatMap((pack) => pack.categories))].sort()) {
		const header = headerOf.get(name) ?? 'categories'
		const id = `category_${projectType}_${header}`
		const group = groups.get(id) ?? {
			id,
			header,
			method: header === 'resolutions' ? 'or' : 'and',
			names: [],
		}
		group.names.push(name)
		groups.set(id, group)
	}
	const rank = (header: string) =>
		HEADER_ORDER.includes(header) ? HEADER_ORDER.indexOf(header) : HEADER_ORDER.length
	return [...groups.values()].sort((a, b) => rank(a.header) - rank(b.header))
}

/** Whether a pack has the chosen categories and none of the excluded ones. */
export function matchesFilters(
	pack: HorizonPack,
	filters: { type: string; option: string; negative?: boolean }[],
	groups: CategoryGroup[],
): boolean {
	return groups.every((group) => {
		const chosen = filters.filter((filter) => filter.type === group.id)
		if (chosen.some((filter) => filter.negative && pack.categories.includes(filter.option))) {
			return false
		}
		const included = chosen.filter((filter) => !filter.negative).map((filter) => filter.option)
		if (included.length === 0) return true
		return group.method === 'or'
			? included.some((name) => pack.categories.includes(name))
			: included.every((name) => pack.categories.includes(name))
	})
}

export async function plus_horizon_packs(
	instanceId: string,
	kind: HorizonPackKind,
): Promise<HorizonPack[]> {
	return await invoke('plugin:plus|plus_horizon_packs', { instanceId, kind })
}

/** Resolves to the installed file's name. */
export async function plus_horizon_pack_install(
	instanceId: string,
	kind: HorizonPackKind,
	id: string,
): Promise<string> {
	return await invoke('plugin:plus|plus_horizon_pack_install', { instanceId, kind, id })
}
