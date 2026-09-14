import { describe, expect, it } from 'vitest'

import {
	type DiscoveredSkin,
	type FavoriteSkin,
	fromFavorite,
	fromGallerySkin,
	fromPlayerSkin,
	newSkinsOnly,
	parseFavorites,
	searchFavorites,
	toFavorite,
	toSelectableSkin,
	withFavorite,
	withoutFavorite,
} from './discovered-skin'

const HASH = '5b0b78732cbf7f6a96925a299cc855ed1ec1e84189fee491f6b57255b43bf17e'
const OTHER_HASH = '754072289ccb7123cd7fe8242fe29458485062f0163a35ff81ef9f48bd85325f'

function favorite(overrides: Partial<FavoriteSkin> = {}): FavoriteSkin {
	return {
		key: HASH,
		name: 'Cat',
		source: 'mineskin',
		model: 'SLIM',
		source_id: 'abc',
		tags: ['cute'],
		added: '2026-11-01T00:00:00Z',
		...overrides,
	}
}

describe('turning results into skins', () => {
	it('takes a gallery skin as it is, with its model still to be found', () => {
		expect(fromGallerySkin({ id: 'abc', name: null, texture_hash: HASH })).toEqual({
			key: HASH,
			name: null,
			source: 'mineskin',
			textureUrl: `https://textures.minecraft.net/texture/${HASH}`,
			model: 'UNKNOWN',
			sourceId: 'abc',
		})
	})

	it('takes a player with a skin', () => {
		const skin = fromPlayerSkin({
			id: 'id',
			name: 'Notch',
			model: 'CLASSIC',
			texture: 'data:image/png;base64,AQID',
			texture_key: HASH,
		})
		expect(skin?.source).toBe('mojang')
		expect(skin?.textureUrl).toBe('data:image/png;base64,AQID')
	})

	it('has nothing to preview for a player on a default skin', () => {
		expect(
			fromPlayerSkin({ id: 'id', name: 'x', model: 'CLASSIC', texture: null, texture_key: null }),
		).toBeNull()
	})

	it('gives the existing skin parts what they expect', () => {
		const skin = toSelectableSkin(fromGallerySkin({ id: 'abc', name: 'Cat', texture_hash: HASH }))
		expect(skin.texture_key).toBe(HASH)
		expect(skin.variant).toBe('UNKNOWN')
		expect(skin.source).toBe('custom_external')
		expect(skin.is_equipped).toBe(false)
	})
})

describe('duplicates', () => {
	const skin = (key: string) => ({ key }) as DiscoveredSkin

	it('keeps the first copy of each skin across pages', () => {
		const seen = new Set<string>()
		expect(newSkinsOnly([skin('a'), skin('b'), skin('a')], seen).map((s) => s.key)).toEqual(['a', 'b'])
		expect(newSkinsOnly([skin('b'), skin('c')], seen).map((s) => s.key)).toEqual(['c'])
	})
})

describe('favorites', () => {
	it('keep where a skin came from', () => {
		const skin = fromGallerySkin({ id: 'abc', name: 'Cat', texture_hash: HASH })
		const saved = toFavorite(skin, ['cute'], '2026-11-01T00:00:00Z')
		expect(saved.source).toBe('mineskin')
		expect(saved.source_id).toBe('abc')
		expect(saved.tags).toEqual(['cute'])
		expect(fromFavorite(saved).textureUrl).toBe(`https://textures.minecraft.net/texture/${HASH}`)
	})

	it('add to the top without repeating', () => {
		const list = withFavorite([favorite(), favorite({ key: OTHER_HASH })], favorite({ name: 'New' }))
		expect(list.map((item) => item.key)).toEqual([HASH, OTHER_HASH])
		expect(list[0].name).toBe('New')
	})

	it('can be removed', () => {
		expect(withoutFavorite([favorite()], HASH)).toEqual([])
		expect(withoutFavorite([favorite()], OTHER_HASH)).toHaveLength(1)
	})

	it('can be searched by name, tag or source', () => {
		const list = [favorite(), favorite({ key: OTHER_HASH, name: 'Dog', tags: [], source: 'mojang' })]
		expect(searchFavorites(list, 'cat').map((item) => item.name)).toEqual(['Cat'])
		expect(searchFavorites(list, 'CUTE').map((item) => item.name)).toEqual(['Cat'])
		expect(searchFavorites(list, 'mojang').map((item) => item.name)).toEqual(['Dog'])
		expect(searchFavorites(list, '  ')).toHaveLength(2)
		expect(searchFavorites(list, 'nothing')).toEqual([])
	})

	it('leave out anything malformed in the settings file', () => {
		expect(parseFavorites('nope')).toEqual([])
		expect(parseFavorites(undefined)).toEqual([])
		const bad = [
			favorite({ key: '../../evil' }),
			favorite({ source: 'elsewhere' as never }),
			favorite({ model: 'HUGE' as never }),
			favorite({ tags: [1] as never }),
			{ key: HASH },
			null,
		]
		expect(parseFavorites([...bad, favorite()])).toEqual([favorite()])
	})

	it('can only ever load pictures from Mojang', () => {
		expect(parseFavorites([favorite({ key: 'https://example.com/x.png' })])).toEqual([])
	})
})
