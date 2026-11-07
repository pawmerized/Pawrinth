import type { Labrinth } from '@modrinth/api-client'

import type { CfCategory, CfProject } from '@/helpers/plus'

/** Display order and Modrinth equivalent of CurseForge's project classes. */
const CLASS_ORDER: { slug: string; projectType: string; installable: boolean }[] = [
	{ slug: 'modpacks', projectType: 'modpack', installable: true },
	{ slug: 'mc-mods', projectType: 'mod', installable: true },
	{ slug: 'texture-packs', projectType: 'resourcepack', installable: true },
	{ slug: 'data-packs', projectType: 'datapack', installable: true },
	{ slug: 'shaders', projectType: 'shader', installable: true },
	{ slug: 'worlds', projectType: 'project', installable: false },
	{ slug: 'bukkit-plugins', projectType: 'plugin', installable: false },
	{ slug: 'customization', projectType: 'project', installable: false },
	{ slug: 'mc-addons', projectType: 'project', installable: false },
]

export function orderClasses(classes: CfCategory[]): CfCategory[] {
	const position = (slug: string) => {
		const index = CLASS_ORDER.findIndex((item) => item.slug === slug)
		return index === -1 ? CLASS_ORDER.length : index
	}
	return [...classes].sort((a, b) => position(a.slug) - position(b.slug))
}

export function classInfo(slug: string) {
	return (
		CLASS_ORDER.find((item) => item.slug === slug) ?? {
			slug,
			projectType: 'project',
			installable: false,
		}
	)
}

/** What can go into an instance, following the same rules as the Modrinth tab. */
export function classesForInstance(
	classes: CfCategory[],
	instance: { loader: string; game_version: string },
	gameVersions: string[],
): CfCategory[] {
	return classes.filter(({ slug }) => {
		if (!classInfo(slug).installable || slug === 'modpacks') return false
		if (slug === 'mc-mods') return instance.loader !== 'vanilla'
		if (slug === 'data-packs') {
			return gameVersions.indexOf(instance.game_version) <= gameVersions.indexOf('1.13')
		}
		return true
	})
}

export function modrinthPathFor(classSlug: string): string {
	const projectType = classInfo(classSlug).projectType
	return ['modpack', 'mod', 'resourcepack', 'datapack', 'shader'].includes(projectType)
		? `/browse/${projectType}`
		: '/browse/mod'
}

export function curseforgePathFor(projectType: string): string {
	const match = CLASS_ORDER.find(
		(item) => item.projectType === projectType && item.slug !== 'worlds',
	)
	return `/curseforge/${match && match.projectType !== 'project' ? match.slug : 'mc-mods'}`
}

/** CurseForge's `modLoader` values, from its API documentation. */
export const LOADER_NAMES: Record<number, string> = {
	1: 'forge',
	4: 'fabric',
	5: 'quilt',
	6: 'neoforge',
}

export const LOADER_TYPE_IDS: Record<string, number> = {
	forge: 1,
	fabric: 4,
	quilt: 5,
	neoforge: 6,
}

export function projectLoaders(project: CfProject): string[] {
	const loaders = new Set<string>()
	for (const index of project.latestFilesIndexes ?? []) {
		const name = index.modLoader === null ? undefined : LOADER_NAMES[index.modLoader]
		if (name) loaders.add(name)
	}
	return [...loaders]
}

/** Only says something when the project's files actually list a side. */
export function projectEnvironment(project: CfProject): string | undefined {
	const tags = new Set((project.latestFiles ?? []).flatMap((file) => file.gameVersions))
	const client = tags.has('Client')
	const server = tags.has('Server')
	if (client && server) return 'client_and_server'
	if (client) return 'client_only'
	if (server) return 'server_only'
	return undefined
}

export type CfSearchHit = Labrinth.Search.v3.ResultSearchProject & {
	curseforge: CfProject
}

/**
 * CurseForge reports no likes for almost every project, so an empty count is left out rather
 * than shown as 0 on every card.
 */
export function toSearchHit(project: CfProject): CfSearchHit {
	const categories = project.categories.map((category) => category.name)
	const environment = projectEnvironment(project)
	return {
		project_id: `curseforge:${project.id}`,
		slug: project.slug,
		name: project.name,
		title: project.name,
		summary: project.summary,
		description: project.summary,
		icon_url: project.logo?.thumbnailUrl ?? project.logo?.url ?? null,
		author: project.authors[0]?.name ?? '',
		author_id: null,
		organization: null,
		organization_id: null,
		categories,
		display_categories: categories,
		loaders: projectLoaders(project),
		project_types: [],
		downloads: Math.round(project.downloadCount),
		follows: project.thumbsUpCount || undefined,
		date_created: project.dateReleased ?? project.dateModified ?? '',
		date_modified: project.dateModified ?? '',
		project_loader_fields: environment ? { environment: [environment] } : {},
		curseforge: project,
	} as unknown as CfSearchHit
}
