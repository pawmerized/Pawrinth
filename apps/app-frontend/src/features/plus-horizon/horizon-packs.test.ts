import { describe, expect, it } from 'vitest'

import {
	categoryGroups,
	discoverLocation,
	findPacks,
	horizonKindFor,
	type HorizonPack,
	installedPath,
	kindFromPath,
	matchesFilters,
	matchesSearch,
	toHorizonHit,
} from './horizon-packs'

const aurora: HorizonPack = {
	id: 'aurora',
	kind: 'shader',
	name: 'Aurora Shaders Horizon',
	summary: 'A shader pack for Horizon',
	authors: ['iLoong'],
	license: 'Apache-2.0',
	icon: null,
	categories: ['semi-realistic', 'atmosphere'],
	screenshots: [],
	downloads: 20470,
	version: '5.9.3',
	size: 604159,
	page_url: 'https://playhorizon.dev/mods/aurora',
	installed: null,
	update_available: false,
	updated: '2026-10-01T00:00:00Z',
	created: '2026-01-01T00:00:00Z',
}

describe("Horizon's packs", () => {
	it('replace only resource packs and shaders', () => {
		expect(horizonKindFor('resourcepack')).toBe('resource_pack')
		expect(horizonKindFor('shaders')).toBe('shader')
		expect(horizonKindFor('mod')).toBeNull()
		expect(horizonKindFor('datapack')).toBeNull()
		expect(kindFromPath('shaders')).toBe('shader')
		expect(kindFromPath('anything')).toBe('resource_pack')
	})

	it('match every word searched for', () => {
		expect(matchesSearch(aurora, 'aurora')).toBe(true)
		expect(matchesSearch(aurora, 'iloong atmosphere')).toBe(true)
		expect(matchesSearch(aurora, 'aurora cartoon')).toBe(false)
		expect(matchesSearch(aurora, '  ')).toBe(true)
	})

	it('sort by downloads or dates, after the search', () => {
		const daybreak = {
			...aurora,
			id: 'daybreak',
			name: 'Daybreak',
			downloads: 63000,
			updated: '2026-11-01T00:00:00Z',
			created: '2026-05-01T00:00:00Z',
		}
		const montarc = {
			...aurora,
			id: 'montarc',
			name: 'Montarc',
			downloads: 51000,
			summary: '',
			updated: null,
			created: '2026-08-01T00:00:00Z',
		}
		const packs = [montarc, aurora, daybreak]
		expect(findPacks(packs, '', 'downloads').map((pack) => pack.id)).toEqual([
			'daybreak',
			'montarc',
			'aurora',
		])
		expect(findPacks(packs, '', 'newest').map((pack) => pack.id)).toEqual([
			'montarc',
			'daybreak',
			'aurora',
		])
		expect(findPacks(packs, '', 'updated').map((pack) => pack.id)).toEqual([
			'daybreak',
			'aurora',
			'montarc',
		])
		expect(findPacks(packs, 'horizon', 'relevance').map((pack) => pack.id)).toEqual([
			'daybreak',
			'aurora',
		])
		expect(packs.map((pack) => pack.id)).toEqual(['montarc', 'aurora', 'daybreak'])
	})

	it('show as a browse result with their own page and first screenshot', () => {
		const hit = toHorizonHit({ ...aurora, screenshots: ['https://example.invalid/a.png'] })
		expect(hit.project_id).toBe('horizon:aurora')
		expect(hit.author).toBe('iLoong')
		expect(hit.project_types).toEqual(['shader'])
		expect(hit.featured_gallery).toBe('https://example.invalid/a.png')
		expect(hit.horizon.page_url).toBe('https://playhorizon.dev/mods/aurora')
	})

	it('are matched to content list rows by their file, turned on or off', () => {
		expect(installedPath(aurora)).toBeNull()
		expect(installedPath({ ...aurora, installed: 'aurora-5.9.3.hshader.disabled' })).toBe(
			'shaderpacks/aurora-5.9.3.hshader',
		)
		expect(installedPath({ ...aurora, kind: 'resource_pack', installed: 'aurora-5.9.3.zip' })).toBe(
			'resourcepacks/aurora-5.9.3.zip',
		)
	})

	it("filter by Modrinth's category groups", () => {
		const tags = [
			{ name: 'semi-realistic', project_type: 'shader', header: 'categories' },
			{ name: 'atmosphere', project_type: 'shader', header: 'features' },
			{ name: 'shadows', project_type: 'shader', header: 'features' },
			{ name: 'high', project_type: 'shader', header: 'performance impact' },
			{ name: 'atmosphere', project_type: 'resourcepack', header: 'categories' },
		]
		const wisdom = { ...aurora, id: 'wisdom', categories: ['semi-realistic', 'shadows', 'high'] }
		const groups = categoryGroups([aurora, wisdom], tags, 'shader')
		expect(groups.map((group) => [group.header, group.names])).toEqual([
			['categories', ['semi-realistic']],
			['features', ['atmosphere', 'shadows']],
			['performance impact', ['high']],
		])
		const features = 'category_shader_features'
		expect(matchesFilters(aurora, [], groups)).toBe(true)
		expect(matchesFilters(aurora, [{ type: features, option: 'atmosphere' }], groups)).toBe(true)
		expect(matchesFilters(wisdom, [{ type: features, option: 'atmosphere' }], groups)).toBe(false)
		expect(
			matchesFilters(wisdom, [{ type: features, option: 'shadows', negative: true }], groups),
		).toBe(false)
	})

	it('are the only Discover a Horizon instance has', () => {
		expect(discoverLocation('horizon', 'a', 'shader')).toEqual({
			path: '/horizon/shaders',
			query: { i: 'a' },
		})
		for (const type of ['resourcepack', 'datapack', 'mod', 'texture-packs', 'mc-mods']) {
			expect(discoverLocation('horizon', 'a', type).path).toBe('/horizon/resourcepacks')
		}
		expect(discoverLocation('horizon', 'a', 'shaders').path).toBe('/horizon/shaders')
		expect(discoverLocation('vanilla', 'b', 'datapack')).toEqual({
			path: '/browse/datapack',
			query: { i: 'b' },
		})
	})
})
