import type { GallerySkin, PlayerSkin } from '@/helpers/plus'
import type { Skin, SkinModel } from '@/helpers/skins'

import { bytesFromDataUrl } from './skin-sources'

const TEXTURE_BASE_URL = 'https://textures.minecraft.net/texture/'
const TEXTURE_HASH = /^[0-9a-f]{32,64}$/

export type DiscoveredSource = 'mineskin' | 'mojang'

/** A skin found through a live source or saved as a favorite, whichever it came from. */
export interface DiscoveredSkin {
	key: string
	name: string | null
	source: DiscoveredSource
	textureUrl: string
	model: SkinModel
	/** MineSkin's id for a gallery skin, or the player's UUID for a Mojang one. */
	sourceId: string
}

export interface FavoriteSkin {
	key: string
	name: string | null
	source: DiscoveredSource
	model: SkinModel
	source_id: string
	tags: string[]
	added: string
}

export function textureUrlFor(hash: string): string {
	return `${TEXTURE_BASE_URL}${hash}`
}

export function fromGallerySkin(skin: GallerySkin): DiscoveredSkin {
	return {
		key: skin.texture_hash,
		name: skin.name,
		source: 'mineskin',
		textureUrl: textureUrlFor(skin.texture_hash),
		model: 'UNKNOWN',
		sourceId: skin.id,
	}
}

/** `null` for a player on a default skin, who has nothing to preview. */
export function fromPlayerSkin(player: PlayerSkin): DiscoveredSkin | null {
	if (!player.texture || !player.texture_key) return null
	return {
		key: player.texture_key,
		name: player.name,
		source: 'mojang',
		textureUrl: player.texture,
		model: player.model,
		sourceId: player.id,
	}
}

export function toSelectableSkin(skin: DiscoveredSkin): Skin {
	return {
		texture_key: skin.key,
		name: skin.name ?? undefined,
		variant: skin.model,
		texture: skin.textureUrl,
		source: 'custom_external',
		is_equipped: false,
	}
}

export async function skinBytes(skin: DiscoveredSkin): Promise<Uint8Array> {
	if (skin.textureUrl.startsWith('data:')) return bytesFromDataUrl(skin.textureUrl)
	const response = await fetch(skin.textureUrl)
	if (!response.ok) throw new Error(`Couldn't download this skin (${response.status}).`)
	return new Uint8Array(await response.arrayBuffer())
}

export function toFavorite(
	skin: DiscoveredSkin,
	tags: readonly string[],
	added: string,
): FavoriteSkin {
	return {
		key: skin.key,
		name: skin.name,
		source: skin.source,
		model: skin.model,
		source_id: skin.sourceId,
		tags: [...tags],
		added,
	}
}

export function fromFavorite(favorite: FavoriteSkin): DiscoveredSkin {
	return {
		key: favorite.key,
		name: favorite.name,
		source: favorite.source,
		textureUrl: textureUrlFor(favorite.key),
		model: favorite.model,
		sourceId: favorite.source_id,
	}
}

function isFavorite(value: unknown): value is FavoriteSkin {
	if (typeof value !== 'object' || value === null) return false
	const item = value as Partial<FavoriteSkin>
	return (
		typeof item.key === 'string' &&
		TEXTURE_HASH.test(item.key) &&
		(item.name === null || typeof item.name === 'string') &&
		(item.source === 'mineskin' || item.source === 'mojang') &&
		(item.model === 'CLASSIC' || item.model === 'SLIM' || item.model === 'UNKNOWN') &&
		typeof item.source_id === 'string' &&
		Array.isArray(item.tags) &&
		item.tags.every((tag) => typeof tag === 'string') &&
		typeof item.added === 'string'
	)
}

/** Favorites come from a settings file, so anything malformed is left out. */
export function parseFavorites(saved: unknown): FavoriteSkin[] {
	return Array.isArray(saved) ? saved.filter(isFavorite) : []
}

export function withFavorite(list: readonly FavoriteSkin[], favorite: FavoriteSkin): FavoriteSkin[] {
	return [favorite, ...list.filter((item) => item.key !== favorite.key)]
}

export function withoutFavorite(list: readonly FavoriteSkin[], key: string): FavoriteSkin[] {
	return list.filter((item) => item.key !== key)
}

export function searchFavorites(list: readonly FavoriteSkin[], query: string): FavoriteSkin[] {
	const text = query.trim().toLowerCase()
	if (!text) return [...list]
	return list.filter((item) =>
		[item.name ?? '', item.source, ...item.tags].some((value) => value.toLowerCase().includes(text)),
	)
}

/** Keeps the first copy of each skin, across every page shown so far. */
export function newSkinsOnly(
	skins: readonly DiscoveredSkin[],
	seen: Set<string>,
): DiscoveredSkin[] {
	return skins.filter((skin) => {
		if (seen.has(skin.key)) return false
		seen.add(skin.key)
		return true
	})
}
