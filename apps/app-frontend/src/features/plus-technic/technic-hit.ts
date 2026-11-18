import type { Labrinth } from '@modrinth/api-client'

import type { TechnicPackSummary } from '@/helpers/plus'

export type TechnicSearchHit = Labrinth.Search.v3.ResultSearchProject & {
	technic: TechnicPackSummary
}

/**
 * Shapes a Technic pack like a Modrinth search result, so it shows in the same browse layout.
 * Its Minecraft version leads the tags, and counts Technic didn't send stay hidden rather than 0.
 */
export function toTechnicHit(pack: TechnicPackSummary): TechnicSearchHit {
	const tags = [...(pack.game_version ? [`Minecraft ${pack.game_version}`] : []), ...pack.tags]
	return {
		project_id: `technic:${pack.slug}`,
		slug: pack.slug,
		name: pack.name,
		title: pack.name,
		summary: pack.summary ?? '',
		description: pack.summary ?? '',
		icon_url: pack.icon_url,
		author: pack.author ?? '',
		author_id: null,
		organization: null,
		organization_id: null,
		categories: tags,
		display_categories: tags,
		loaders: [],
		project_types: ['modpack'],
		downloads: pack.installs ?? undefined,
		follows: pack.ratings ?? undefined,
		date_created: '',
		date_modified: '',
		project_loader_fields: {},
		technic: pack,
	} as unknown as TechnicSearchHit
}
