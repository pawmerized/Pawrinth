import { describe, expect, it, vi } from 'vitest'

import { horizonDiscoverRedirect } from './discover-guard'

vi.mock('@/features/plus-settings/use-plus-settings', () => ({ usePlusSettings: () => ({}) }))
vi.mock('@/helpers/instance', () => ({ get: async () => null }))

const horizon = { id: 'h', loader: 'vanilla', game_version: '26.3' }
const plain = { id: 'p', loader: 'vanilla', game_version: '26.3' }
const on = { h: true }

describe("a Horizon instance's Discover", () => {
	it("never opens Modrinth's or CurseForge's pages", () => {
		const modrinth = (projectType: string) => ({
			name: 'Discover content',
			params: { projectType },
			query: { i: 'h' },
		})
		expect(horizonDiscoverRedirect(modrinth('datapack'), horizon, on)?.path).toBe(
			'/horizon/resourcepacks',
		)
		expect(horizonDiscoverRedirect(modrinth('shader'), horizon, on)?.path).toBe('/horizon/shaders')
		expect(
			horizonDiscoverRedirect(
				{ name: 'Discover on CurseForge', params: { classSlug: 'shaders' }, query: { i: 'h' } },
				horizon,
				on,
			),
		).toEqual({ path: '/horizon/shaders', query: { i: 'h' } })
	})

	it('leaves other instances and pages alone', () => {
		const to = { name: 'Discover content', params: { projectType: 'shader' }, query: { i: 'p' } }
		expect(horizonDiscoverRedirect(to, plain, on)).toBeNull()
		expect(horizonDiscoverRedirect(to, horizon, {})).toBeNull()
		expect(horizonDiscoverRedirect(to, null, on)).toBeNull()
		expect(
			horizonDiscoverRedirect({ name: 'Project', params: {}, query: { i: 'h' } }, horizon, on),
		).toBeNull()
	})
})
