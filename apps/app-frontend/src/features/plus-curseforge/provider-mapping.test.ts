import { describe, expect, it } from 'vitest'

import type { CfCategory, CfProject } from '@/helpers/plus'

import {
	classesForInstance,
	classInfo,
	curseforgePathFor,
	modrinthPathFor,
	orderClasses,
	projectEnvironment,
	projectLoaders,
	toSearchHit,
} from './provider-mapping'

function project(overrides: Partial<CfProject> = {}): CfProject {
	return {
		id: 123,
		name: 'Example',
		slug: 'example',
		summary: 'Summary',
		links: { websiteUrl: null, sourceUrl: null },
		classId: 6,
		downloadCount: 1234.6,
		logo: { url: 'large.png', thumbnailUrl: 'thumb.png' },
		thumbsUpCount: 7,
		dateReleased: '2026-01-01T00:00:00Z',
		latestFilesIndexes: [],
		latestFiles: [],
		authors: [{ name: 'someone', url: 'https://example.invalid' }],
		categories: [],
		dateModified: '2026-02-01T00:00:00Z',
		allowModDistribution: true,
		...overrides,
	}
}

function category(slug: string): CfCategory {
	return { id: 1, name: slug, slug, classId: null, isClass: true }
}

describe('classes', () => {
	it('puts shared classes first, in Modrinth order', () => {
		const ordered = orderClasses(
			['worlds', 'shaders', 'mc-mods', 'unknown-class', 'modpacks'].map(category),
		)
		expect(ordered.map((item) => item.slug)).toEqual([
			'modpacks',
			'mc-mods',
			'shaders',
			'worlds',
			'unknown-class',
		])
	})

	it('only allows installs for classes the install code supports', () => {
		expect(classInfo('mc-mods').installable).toBe(true)
		expect(classInfo('modpacks').installable).toBe(true)
		expect(classInfo('worlds').installable).toBe(false)
		expect(classInfo('never-seen-before').installable).toBe(false)
	})

	it('maps the provider switch to the matching tab', () => {
		expect(curseforgePathFor('shader')).toBe('/curseforge/shaders')
		expect(curseforgePathFor('server')).toBe('/curseforge/mc-mods')
		expect(modrinthPathFor('texture-packs')).toBe('/browse/resourcepack')
		expect(modrinthPathFor('worlds')).toBe('/browse/mod')
	})

	describe('inside an instance', () => {
		const all = ['modpacks', 'mc-mods', 'texture-packs', 'data-packs', 'shaders', 'worlds'].map(
			category,
		)
		const versions = ['1.21.1', '1.20.1', '1.13', '1.12.2']
		const slugs = (loader: string, game_version: string) =>
			classesForInstance(all, { loader, game_version }, versions).map((item) => item.slug)

		it('only lists what can be installed into it', () => {
			expect(slugs('forge', '1.20.1')).toEqual([
				'mc-mods',
				'texture-packs',
				'data-packs',
				'shaders',
			])
		})

		it('hides mods from a vanilla instance', () => {
			expect(slugs('vanilla', '1.20.1')).toEqual(['texture-packs', 'data-packs', 'shaders'])
		})

		it('hides data packs before Minecraft 1.13', () => {
			expect(slugs('forge', '1.12.2')).toEqual(['mc-mods', 'texture-packs', 'shaders'])
		})
	})
})

describe('project mapping', () => {
	it('reads loaders from the API enum and ignores unknown ones', () => {
		const loaders = projectLoaders(
			project({
				latestFilesIndexes: [
					{ gameVersion: '1.21.1', modLoader: 4 },
					{ gameVersion: '1.21.1', modLoader: 4 },
					{ gameVersion: '1.20.1', modLoader: 6 },
					{ gameVersion: '1.7.10', modLoader: 3 },
					{ gameVersion: '1.7.10', modLoader: null },
				],
			}),
		)
		expect(loaders).toEqual(['fabric', 'neoforge'])
	})

	it('only reports a side when files list one', () => {
		expect(projectEnvironment(project())).toBeUndefined()
		expect(projectEnvironment(project({ latestFiles: [{ gameVersions: ['1.21.1'] }] }))).toBe(
			undefined,
		)
		expect(projectEnvironment(project({ latestFiles: [{ gameVersions: ['Client'] }] }))).toBe(
			'client_only',
		)
		expect(
			projectEnvironment(project({ latestFiles: [{ gameVersions: ['Client', 'Server'] }] })),
		).toBe('client_and_server')
	})

	it('uses the like count CurseForge documents', () => {
		expect(toSearchHit(project()).follows).toBe(7)
		expect(toSearchHit(project({ thumbsUpCount: null })).follows).toBeUndefined()
		expect(toSearchHit(project({ thumbsUpCount: 0 })).follows).toBeUndefined()
	})

	it('keeps a CurseForge id apart from Modrinth ids', () => {
		const hit = toSearchHit(project())
		expect(hit.project_id).toBe('curseforge:123')
		expect(hit.downloads).toBe(1235)
		expect(hit.icon_url).toBe('thumb.png')
		expect(hit.curseforge.id).toBe(123)
	})

	it('does not invent an environment or author', () => {
		const hit = toSearchHit(project({ authors: [] }))
		expect(hit.author).toBe('')
		expect(hit.project_loader_fields).toEqual({})
	})
})
