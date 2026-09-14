import type { CfAuthor, CfCategory, CfFile, CfInstalledFile, CfProject } from '@/helpers/plus'

import { classInfo, projectLoaders } from './provider-mapping'

const RELEASE_TYPES: Record<number, string> = { 1: 'release', 2: 'beta', 3: 'alpha' }
const LOADER_TAGS: Record<string, string> = {
	Forge: 'forge',
	Fabric: 'fabric',
	Quilt: 'quilt',
	NeoForge: 'neoforge',
}

/** What went wrong in words a person can read, without the backend's error category. */
export function errorText(error: unknown): string {
	return String((error as Error)?.message ?? error).replace(/^invalid input:\s*/, '')
}

export function classSlugOf(project: CfProject, classes: readonly CfCategory[]): string {
	return classes.find((item) => item.id === project.classId)?.slug ?? ''
}

/** A file's `gameVersions` mixes game versions, loaders and things like "Client". */
export function fileTags(file: Pick<CfFile, 'gameVersions'>) {
	return {
		gameVersions: file.gameVersions.filter((tag) => /^\d/.test(tag)),
		loaders: file.gameVersions.flatMap((tag) => (LOADER_TAGS[tag] ? [LOADER_TAGS[tag]] : [])),
	}
}

/** Game version and loader pairs a project's files are marked for, in the instance picker's terms. */
export function installTargets(files: Pick<CfFile, 'gameVersions'>[], projectType: string) {
	const targets = new Map<string, { game_version: string; loader: string }>()
	for (const file of files) {
		const { gameVersions, loaders } = fileTags(file)
		const fileLoaders = projectType === 'mod' ? loaders : ['minecraft']
		for (const game_version of gameVersions) {
			for (const loader of fileLoaders) {
				targets.set(`${game_version}\0${loader}`, { game_version, loader })
			}
		}
	}
	return [...targets.values()]
}

export function releaseTypeOf(file: Pick<CfFile, 'releaseType'>): string {
	return RELEASE_TYPES[file.releaseType] ?? 'alpha'
}

/** How a file installed from CurseForge shows in an instance's content list. */
export function installedRowView(
	installed: CfInstalledFile,
	instanceId: string,
	sourceName: string,
) {
	const { project } = installed
	const author = project?.authors?.[0]
	return {
		title: project?.name,
		iconUrl: project?.logo?.thumbnailUrl ?? undefined,
		projectLink: {
			path: `/curseforge/project/${installed.project_id}`,
			query: { i: instanceId },
		},
		versionNumber: installed.current_file?.displayName,
		owner: author
			? {
					id: String(author.id ?? author.name),
					name: author.name,
					avatar_url: author.avatarUrl ?? undefined,
					type: 'user' as const,
					link: author.id ? `/curseforge/author/${author.id}` : undefined,
				}
			: { id: 'curseforge', name: sourceName, type: 'organization' as const },
	}
}

/** The shape Modrinth's version table expects, with only what CurseForge supplies. */
export function toVersionView(file: CfFile) {
	const { gameVersions, loaders } = fileTags(file)
	return {
		id: String(file.id),
		project_id: String(file.modId),
		name: file.displayName,
		version_number: file.displayName,
		version_type: releaseTypeOf(file),
		game_versions: gameVersions,
		loaders,
		date_published: file.fileDate,
		downloads: file.downloadCount ?? 0,
		files: [],
		dependencies: [],
		displayUrlEnding: String(file.id),
		curseforge: file,
	}
}

/**
 * The shape Modrinth's project page components expect. Fields CurseForge has no equivalent
 * for are empty, and the components hide them, rather than being filled in with a guess.
 */
export function toProjectView(project: CfProject, classSlug: string) {
	const indexes = project.latestFilesIndexes ?? []
	return {
		id: String(project.id),
		slug: project.slug,
		title: project.name,
		description: project.summary,
		status: 'approved',
		project_type: classInfo(classSlug).projectType,
		downloads: Math.round(project.downloadCount),
		followers: project.thumbsUpCount ?? 0,
		categories: [...new Set(project.categories.map((category) => category.name))],
		additional_categories: [],
		icon_url: project.logo?.thumbnailUrl ?? project.logo?.url ?? null,
		raw_icon_url: project.logo?.url ?? null,
		loaders: projectLoaders(project),
		game_versions: [...new Set(indexes.map((index) => index.gameVersion))],
		versions: indexes.map((index, position) => `${project.id}-${position}`),
		client_side: 'unknown',
		server_side: 'unknown',
		published: project.dateReleased ?? project.dateCreated ?? null,
		updated: project.dateModified ?? null,
		license: { id: '', name: '', url: null },
		issues_url: project.links.issuesUrl ?? null,
		source_url: project.links.sourceUrl ?? null,
		wiki_url: project.links.wikiUrl ?? null,
		discord_url: null,
		donation_urls: [],
		gallery: (project.screenshots ?? [])
			.filter((image) => !!image.url)
			.map((image) => ({
				url: image.thumbnailUrl ?? image.url,
				raw_url: image.url,
				title: image.title ?? '',
				description: image.description ?? '',
				featured: false,
			})),
	}
}

export function toCreators(project: CfProject) {
	return project.authors.map((author, position) => ({
		id: String(author.id ?? author.name),
		role: '',
		is_owner: position === 0 && project.authors.length > 1,
		accepted: true,
		user: {
			id: String(author.id ?? author.name),
			username: author.name,
			avatar_url: author.avatarUrl ?? '',
		},
	}))
}

/** Authors without an id can't be looked up, so they lead to their CurseForge page. */
export function authorPath(author: CfAuthor, instanceId?: string): string | undefined {
	if (author.id == null) return author.url ?? undefined
	const params = new URLSearchParams({ name: author.name })
	if (author.url) params.set('url', author.url)
	if (author.avatarUrl) params.set('avatar', author.avatarUrl)
	if (instanceId) params.set('i', instanceId)
	return `/curseforge/author/${author.id}?${params}`
}

export function projectPath(project: Pick<CfProject, 'id'>, instanceId?: string): string {
	const suffix = instanceId ? `?i=${encodeURIComponent(instanceId)}` : ''
	return `/curseforge/project/${project.id}${suffix}`
}

export interface BrowseSnapshot {
	query: string
	filters: { type: string; option: string; negative?: boolean }[]
	sort: string
	maxResults: number
	page: number
}

const snapshots = new Map<string, BrowseSnapshot>()

/** Kept for this session, so coming back from a project restores the search and filters. */
export function rememberBrowse(key: string, snapshot: BrowseSnapshot): void {
	snapshots.set(key, snapshot)
}

export function rememberedBrowse(key: string): BrowseSnapshot | undefined {
	return snapshots.get(key)
}
