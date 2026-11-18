import { describe, expect, it } from 'vitest'

import type { TechnicPackSummary } from '@/helpers/plus'

import { toTechnicHit } from './technic-hit'

const pack: TechnicPackSummary = {
	slug: 'tekkit',
	name: 'Tekkit',
	author: 'Technic',
	summary: 'Machines and more',
	icon_url: 'https://cdn.example/tekkit.png',
	game_version: '1.12.2',
	installs: 152800,
	ratings: 900,
	tags: ['tech', 'magic'],
}

describe('toTechnicHit', () => {
	it('fills the fields the browse cards show', () => {
		const hit = toTechnicHit(pack)
		expect(hit.project_id).toBe('technic:tekkit')
		expect(hit.name).toBe('Tekkit')
		expect(hit.author).toBe('Technic')
		expect(hit.icon_url).toBe('https://cdn.example/tekkit.png')
		expect(hit.downloads).toBe(152800)
		expect(hit.follows).toBe(900)
		expect(hit.display_categories).toEqual(['Minecraft 1.12.2', 'tech', 'magic'])
		expect(hit.technic).toBe(pack)
	})

	it('leaves out what Technic did not send', () => {
		const hit = toTechnicHit({
			...pack,
			author: null,
			summary: null,
			game_version: null,
			installs: null,
			ratings: null,
			tags: [],
		})
		expect(hit.author).toBe('')
		expect(hit.summary).toBe('')
		expect(hit.downloads).toBeUndefined()
		expect(hit.follows).toBeUndefined()
		expect(hit.display_categories).toEqual([])
	})
})
