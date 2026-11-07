import { describe, expect, it } from 'vitest'

import {
	findTagFilter,
	toggleTagFilter,
} from '../../../../../packages/ui/src/layouts/shared/browse-tab/tag-filters'
import {
	DEFAULT_PROJECT_TOOLS,
	filterProjects,
	pageCountOf,
	pageOf,
	PROJECTS_PER_PAGE,
	projectFilterOptions,
	rememberedProjectTools,
	rememberProjectTools,
} from '../../../../../packages/ui/src/layouts/shared/user-profile/project-tools'

const projects = [
	{
		name: 'Alpha Mod',
		summary: 'Adds rockets',
		downloads: 10,
		followers: 300,
		updated: '2026-01-01T00:00:00Z',
		categories: ['technology'],
		loaders: ['fabric'],
		game_versions: ['1.20.1', '1.9'],
	},
	{
		name: 'Beta Mod',
		summary: 'Adds trees',
		downloads: 500,
		followers: 5,
		updated: '2026-03-01T00:00:00Z',
		categories: ['worldgen'],
		additional_categories: ['technology'],
		loaders: ['forge'],
		game_versions: ['1.21.1'],
	},
]

const names = (list: { name: string }[]) => list.map((project) => project.name)

describe('searching a creator’s projects', () => {
	it('sorts by downloads by default', () => {
		expect(names(filterProjects(projects, DEFAULT_PROJECT_TOOLS))).toEqual([
			'Beta Mod',
			'Alpha Mod',
		])
	})

	it('sorts by popularity, recent updates and name', () => {
		const sorted = (sort: 'followers' | 'updated' | 'name') =>
			names(filterProjects(projects, { ...DEFAULT_PROJECT_TOOLS, sort }))
		expect(sorted('followers')).toEqual(['Alpha Mod', 'Beta Mod'])
		expect(sorted('updated')).toEqual(['Beta Mod', 'Alpha Mod'])
		expect(sorted('name')).toEqual(['Alpha Mod', 'Beta Mod'])
	})

	it('searches names and summaries', () => {
		const found = (search: string) =>
			names(filterProjects(projects, { ...DEFAULT_PROJECT_TOOLS, search }))
		expect(found('alpha')).toEqual(['Alpha Mod'])
		expect(found('TREES')).toEqual(['Beta Mod'])
		expect(found('nothing like this')).toEqual([])
	})

	it('filters by category, including additional ones', () => {
		expect(
			names(filterProjects(projects, { ...DEFAULT_PROJECT_TOOLS, category: 'technology' })),
		).toEqual(['Beta Mod', 'Alpha Mod'])
		expect(
			names(filterProjects(projects, { ...DEFAULT_PROJECT_TOOLS, category: 'worldgen' })),
		).toEqual(['Beta Mod'])
	})

	it('filters by loader and game version', () => {
		expect(names(filterProjects(projects, { ...DEFAULT_PROJECT_TOOLS, loader: 'forge' }))).toEqual([
			'Beta Mod',
		])
		expect(
			names(filterProjects(projects, { ...DEFAULT_PROJECT_TOOLS, gameVersion: '1.20.1' })),
		).toEqual(['Alpha Mod'])
	})

	it('copes with projects that are missing data', () => {
		expect(
			filterProjects([{ name: 'Bare' }], { ...DEFAULT_PROJECT_TOOLS, loader: 'forge' }),
		).toEqual([])
		expect(names(filterProjects([{ name: 'Bare' }], DEFAULT_PROJECT_TOOLS))).toEqual(['Bare'])
	})

	it('does not change the list it was given', () => {
		const original = [...projects]
		filterProjects(projects, DEFAULT_PROJECT_TOOLS)
		expect(projects).toEqual(original)
	})

	it('pages through many projects', () => {
		const many = Array.from({ length: PROJECTS_PER_PAGE + 3 }, (_, index) => index)
		expect(pageOf(many, 1)).toHaveLength(PROJECTS_PER_PAGE)
		expect(pageOf(many, 2)).toEqual([PROJECTS_PER_PAGE, PROJECTS_PER_PAGE + 1, PROJECTS_PER_PAGE + 2])
		expect(pageCountOf(many.length)).toBe(2)
		expect(pageCountOf(0)).toBe(1)
	})

	it('only offers filters the projects can match, with versions newest first', () => {
		const options = projectFilterOptions(projects)
		expect(options.categories).toEqual(['technology', 'worldgen'])
		expect(options.loaders).toEqual(['fabric', 'forge'])
		expect(options.gameVersions).toEqual(['1.21.1', '1.20.1', '1.9'])
	})

	it('restores the search and filters when coming back', () => {
		rememberProjectTools('creator', { ...DEFAULT_PROJECT_TOOLS, search: 'rockets', page: 3 })
		expect(rememberedProjectTools('creator').search).toBe('rockets')
		expect(rememberedProjectTools('creator').page).toBe(3)
		expect(rememberedProjectTools('someone-else').search).toBe('')
	})
})

describe('clicking a tag on a project card', () => {
	const filters = [
		{ id: 'mod_loader', options: [{ id: 'forge', value: 'forge' }] },
		{
			id: 'category_curseforge',
			options: [{ id: '423', value: '423', formatted_name: 'Map and Information' }],
		},
		{ id: 'hidden', display: 'none', options: [{ id: 'secret' }] },
	]

	it('finds the filter a tag belongs to, by id or by display name', () => {
		expect(findTagFilter('forge', filters)).toEqual({ type: 'mod_loader', option: 'forge' })
		expect(findTagFilter('Map and Information', filters)).toEqual({
			type: 'category_curseforge',
			option: '423',
		})
	})

	it('does nothing for a tag with no filter, or a hidden one', () => {
		expect(findTagFilter('not a filter', filters)).toBeUndefined()
		expect(findTagFilter('secret', filters)).toBeUndefined()
	})

	it('selects on the first click and clears on the second, leaving others alone', () => {
		const other = { type: 'game_version', option: '1.20.1' }
		const match = { type: 'mod_loader', option: 'forge' }
		const selected = toggleTagFilter([other], match)
		expect(selected).toEqual([other, match])
		expect(toggleTagFilter(selected, match)).toEqual([other])
	})
})
