/** Sites Pawrinth can only link to, because their terms or missing APIs rule out anything more. */
export const LINKED_SITE_IDS = ['namemc', 'skinmc', 'minecraftskins'] as const
export type LinkedSiteId = (typeof LINKED_SITE_IDS)[number]

export const GALLERY_SOURCE_ID = 'mineskin'
export const SKIN_SOURCE_IDS = [GALLERY_SOURCE_ID, ...LINKED_SITE_IDS] as const
export type SkinSourceId = (typeof SKIN_SOURCE_IDS)[number]

export type SkinSearchKind = 'everything' | 'players' | 'skins' | 'creations' | 'creators'

export interface SkinSearchLink {
	kind: SkinSearchKind
	url: string
}

export interface PlayerPageLink {
	source: LinkedSiteId
	url: string
}

const BROWSE_URLS: Record<LinkedSiteId, string> = {
	namemc: 'https://namemc.com/minecraft-skins',
	skinmc: 'https://skinmc.net/skins',
	minecraftskins: 'https://www.minecraftskins.com/',
}

const MAX_NAME_LENGTH = 16
const MAX_RECENT_SEARCHES = 5

export function isSkinSourceId(value: unknown): value is SkinSourceId {
	return SKIN_SOURCE_IDS.includes(value as SkinSourceId)
}

export function isLinkedSiteId(value: unknown): value is LinkedSiteId {
	return LINKED_SITE_IDS.includes(value as LinkedSiteId)
}

export function skinSourceBrowseUrl(source: LinkedSiteId): string {
	return BROWSE_URLS[source]
}

/** The search pages each site offers its own visitors. */
export function skinSourceSearchLinks(source: LinkedSiteId, query: string): SkinSearchLink[] {
	const text = encodeURIComponent(query.trim())
	if (!text) return []
	switch (source) {
		case 'namemc':
			return [{ kind: 'everything', url: `https://namemc.com/search?q=${text}` }]
		case 'skinmc':
			return [
				{ kind: 'players', url: `https://skinmc.net/s?search=${text}&tab=profiles` },
				{ kind: 'skins', url: `https://skinmc.net/s?search=${text}&tab=skins` },
				{ kind: 'creations', url: `https://skinmc.net/s?search=${text}&tab=skin_creations` },
			]
		case 'minecraftskins':
			return [
				{ kind: 'skins', url: `https://www.minecraftskins.com/search/skin/${text}/1/` },
				{ kind: 'creators', url: `https://www.minecraftskins.com/search/user/${text}/1/` },
			]
	}
}

function dashedId(id: string): string {
	return id.replace(/^(.{8})(.{4})(.{4})(.{4})(.{12})$/, '$1-$2-$3-$4-$5')
}

export function playerPageLinks(player: { id: string; name: string }): PlayerPageLink[] {
	return [
		{ source: 'namemc', url: `https://namemc.com/profile/${dashedId(player.id)}` },
		{
			source: 'skinmc',
			url: `https://skinmc.net/s?search=${encodeURIComponent(player.name)}&tab=profiles`,
		},
	]
}

export function mineskinPageUrl(id: string): string {
	return `https://mineskin.org/skins/${encodeURIComponent(id)}`
}

/** Whether Mojang could know this as a player, so anything else never leaves the app. */
export function couldBePlayer(query: string): boolean {
	const text = query.trim()
	return (
		new RegExp(`^[A-Za-z0-9_]{1,${MAX_NAME_LENGTH}}$`).test(text) ||
		/^[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12}$/i.test(text)
	)
}

export function withRecentSearch(recent: readonly string[], query: string): string[] {
	const text = query.trim()
	if (!text) return [...recent]
	const others = recent.filter((item) => item.toLowerCase() !== text.toLowerCase())
	return [text, ...others].slice(0, MAX_RECENT_SEARCHES)
}

export function bytesFromDataUrl(dataUrl: string): Uint8Array {
	const binary = atob(dataUrl.slice(dataUrl.indexOf(',') + 1))
	return Uint8Array.from(binary, (letter) => letter.charCodeAt(0))
}
