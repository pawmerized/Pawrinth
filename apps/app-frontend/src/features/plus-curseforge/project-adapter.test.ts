import { describe, expect, it } from 'vitest'

import type { CfFile, CfProject } from '@/helpers/plus'

import {
	authorPath,
	classSlugOf,
	errorText,
	fileTags,
	installTargets,
	projectPath,
	releaseTypeOf,
	rememberBrowse,
	rememberedBrowse,
	toCreators,
	toProjectView,
	toVersionView,
} from './project-adapter'

function project(overrides: Partial<CfProject> = {}): CfProject {
	return {
		id: 123,
		name: 'Example',
		slug: 'example',
		summary: 'Summary',
		links: { websiteUrl: null, sourceUrl: null },
		classId: 6,
		downloadCount: 1234.6,
		logo: null,
		thumbsUpCount: null,
		dateReleased: null,
		latestFilesIndexes: [],
		latestFiles: [],
		authors: [],
		categories: [],
		dateModified: null,
		allowModDistribution: true,
		...overrides,
	}
}

function file(overrides: Partial<CfFile> = {}): CfFile {
	return {
		id: 9,
		modId: 123,
		displayName: 'Example 1.0',
		fileName: 'example-1.0.jar',
		releaseType: 1,
		fileDate: '2026-01-01T00:00:00Z',
		fileLength: 10,
		downloadUrl: null,
		gameVersions: [],
		dependencies: [],
		...overrides,
	}
}

describe('error text', () => {
	it('drops the backend error category', () => {
		expect(errorText(new Error('invalid input: could not reach CurseForge'))).toBe(
			'could not reach CurseForge',
		)
	})

	it('handles errors that are only text', () => {
		expect(errorText('something broke')).toBe('something broke')
	})
})

describe('file details', () => {
	it('separates game versions and loaders from the rest', () => {
		expect(
			fileTags(file({ gameVersions: ['1.20.1', 'Forge', 'Client', 'Java 17', '1.20'] })),
		).toEqual({ gameVersions: ['1.20.1', '1.20'], loaders: ['forge'] })
	})

	it('reads release types and treats unknown ones as least stable', () => {
		expect(releaseTypeOf({ releaseType: 1 })).toBe('release')
		expect(releaseTypeOf({ releaseType: 2 })).toBe('beta')
		expect(releaseTypeOf({ releaseType: 3 })).toBe('alpha')
		expect(releaseTypeOf({ releaseType: 99 })).toBe('alpha')
	})

	it('lists the versions and loaders the instance picker can match', () => {
		const files = [
			file({ gameVersions: ['1.20.1', 'Fabric', 'Forge', 'Client'] }),
			file({ gameVersions: ['1.20.1', 'Fabric'] }),
			file({ gameVersions: ['1.21.1'] }),
		]
		expect(installTargets(files, 'mod')).toEqual([
			{ game_version: '1.20.1', loader: 'fabric' },
			{ game_version: '1.20.1', loader: 'forge' },
		])
		expect(installTargets(files, 'resourcepack')).toEqual([
			{ game_version: '1.20.1', loader: 'minecraft' },
			{ game_version: '1.21.1', loader: 'minecraft' },
		])
	})

	it('makes up no downloads for a file that has none listed', () => {
		expect(toVersionView(file()).downloads).toBe(0)
	})
})

describe('project details', () => {
	it('keeps only what CurseForge supplied', () => {
		const view = toProjectView(project(), 'mc-mods')
		expect(view.license.name).toBe('')
		expect(view.source_url).toBeNull()
		expect(view.icon_url).toBeNull()
		expect(view.gallery).toEqual([])
		expect(view.followers).toBe(0)
		expect(view.published).toBeNull()
	})

	it('uses the right project type for each kind of project', () => {
		expect(toProjectView(project(), 'mc-mods').project_type).toBe('mod')
		expect(toProjectView(project(), 'texture-packs').project_type).toBe('resourcepack')
	})

	it('lists each category and game version once', () => {
		const category = { id: 1, name: 'Tech', slug: 'tech', classId: 6, isClass: false }
		const view = toProjectView(
			project({
				categories: [category, category],
				latestFilesIndexes: [
					{ gameVersion: '1.20.1', modLoader: 1 },
					{ gameVersion: '1.20.1', modLoader: 4 },
				],
			}),
			'mc-mods',
		)
		expect(view.categories).toEqual(['Tech'])
		expect(view.game_versions).toEqual(['1.20.1'])
		expect(view.loaders).toEqual(['forge', 'fabric'])
	})

	it('drops gallery entries with no image, and invents no date for the rest', () => {
		const view = toProjectView(
			project({
				screenshots: [
					{ url: 'large.png', thumbnailUrl: 'small.png', title: 'Shot', description: null },
					{ url: null, thumbnailUrl: null, title: 'Broken', description: null },
				],
			}),
			'mc-mods',
		)
		expect(view.gallery).toHaveLength(1)
		expect(view.gallery[0].raw_url).toBe('large.png')
		expect(view.gallery[0]).not.toHaveProperty('created')
	})

	it('finds a project’s class', () => {
		const classes = [{ id: 6, name: 'Mods', slug: 'mc-mods', classId: null, isClass: true }]
		expect(classSlugOf(project(), classes)).toBe('mc-mods')
		expect(classSlugOf(project({ classId: 99 }), classes)).toBe('')
	})
})

describe('creators', () => {
	it('does not mark anyone as owner when there is only one listed author', () => {
		const creators = toCreators(project({ authors: [{ id: 1, name: 'Solo', url: null }] }))
		expect(creators[0].user.username).toBe('Solo')
		expect(creators[0].is_owner).toBe(false)
		expect(creators[0].role).toBe('')
	})

	it('leaves a missing avatar empty', () => {
		const creators = toCreators(project({ authors: [{ id: 1, name: 'Solo', url: null }] }))
		expect(creators[0].user.avatar_url).toBe('')
	})

	it('links to the internal page, carrying what the page needs', () => {
		const path = authorPath(
			{ id: 7, name: 'Some One', url: 'https://example.invalid/a', avatarUrl: null },
			'local:abc',
		)
		const parsed = new URL(path ?? '', 'http://x')
		expect(parsed.pathname).toBe('/curseforge/author/7')
		expect(parsed.searchParams.get('name')).toBe('Some One')
		expect(parsed.searchParams.get('i')).toBe('local:abc')
	})

	it('falls back to the website for an author with no id', () => {
		expect(authorPath({ id: null, name: 'A', url: 'https://example.invalid/a' })).toBe(
			'https://example.invalid/a',
		)
		expect(authorPath({ id: null, name: 'A', url: null })).toBeUndefined()
	})
})

describe('navigation', () => {
	it('keeps the instance when opening a project', () => {
		expect(projectPath({ id: 5 })).toBe('/curseforge/project/5')
		expect(projectPath({ id: 5 }, 'local:abc')).toBe('/curseforge/project/5?i=local%3Aabc')
	})

	it('restores the search after coming back', () => {
		const snapshot = { query: 'rockets', filters: [], sort: 'downloads', maxResults: 20, page: 3 }
		rememberBrowse('key', snapshot)
		expect(rememberedBrowse('key')).toEqual(snapshot)
		expect(rememberedBrowse('other')).toBeUndefined()
	})
})
