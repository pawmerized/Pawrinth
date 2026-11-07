import { beforeEach, describe, expect, it, vi } from 'vitest'

const gallery = vi.fn()
const lookup = vi.fn()

vi.mock('@/helpers/plus', () => ({
	plus_skin_gallery: (query: string | null, after: string | null) => gallery(query, after),
	plus_skin_lookup: (query: string) => lookup(query),
}))

function hash(letter: string, number: number): string {
	return `${letter}${number}`.padEnd(64, '0')
}

function page(letter: string, numbers: number[], next: string | null) {
	return {
		status: 'ok',
		skins: numbers.map((number) => ({
			id: `${letter}${number}`,
			name: null,
			texture_hash: hash(letter, number),
		})),
		next,
	}
}

async function freshGallery() {
	vi.resetModules()
	const { useSkinGallery } = await import('./skin-gallery')
	return useSkinGallery()
}

beforeEach(() => {
	gallery.mockReset()
	lookup.mockReset()
	lookup.mockResolvedValue({ status: 'not_found' })
})

describe('the gallery', () => {
	it('opens on the latest skins', async () => {
		gallery.mockResolvedValue(page('a', [1, 2], null))
		const state = await freshGallery()
		await state.searchSkins('')
		expect(gallery).toHaveBeenCalledWith(null, null)
		expect(state.status.value).toBe('ready')
		expect(state.shownSkins.value).toHaveLength(2)
		expect(lookup).not.toHaveBeenCalled()
	})

	it('searches by keyword', async () => {
		gallery.mockResolvedValue(page('a', [1], null))
		const state = await freshGallery()
		await state.searchSkins(' cat ')
		expect(gallery).toHaveBeenCalledWith('cat', null)
		expect(state.activeQuery.value).toBe('cat')
	})

	it('moves between pages and keeps the search', async () => {
		gallery.mockImplementation(async (_query: string, after: string | null) =>
			after === null ? page('a', [1, 2], 'cursor-1') : page('b', [1, 2], null),
		)
		const state = await freshGallery()
		await state.searchSkins('cat')
		expect(state.hasPrevious.value).toBe(false)
		expect(state.hasNext.value).toBe(true)

		await state.showNextPage()
		expect(gallery).toHaveBeenLastCalledWith('cat', 'cursor-1')
		expect(state.firstShownPage.value).toBe(1)
		expect(state.shownSkins.value.map((skin) => skin.sourceId)).toEqual(['b1', 'b2'])
		expect(state.hasNext.value).toBe(false)
		expect(state.activeQuery.value).toBe('cat')

		state.showPreviousPage()
		expect(state.firstShownPage.value).toBe(0)
		expect(state.shownSkins.value.map((skin) => skin.sourceId)).toEqual(['a1', 'a2'])

		await state.showNextPage()
		expect(gallery).toHaveBeenCalledTimes(2)
	})

	it('adds to what is shown when loading more', async () => {
		gallery.mockImplementation(async (_query: string, after: string | null) =>
			after === null ? page('a', [1], 'cursor-1') : page('b', [1], null),
		)
		const state = await freshGallery()
		await state.searchSkins('')
		await state.loadMore()
		expect(state.shownSkins.value.map((skin) => skin.sourceId)).toEqual(['a1', 'b1'])
		expect(state.firstShownPage.value).toBe(0)
		expect(state.lastShownPage.value).toBe(1)
	})

	it('never shows the same skin twice', async () => {
		gallery.mockImplementation(async (_query: string, after: string | null) =>
			after === null ? page('a', [1, 2], 'cursor-1') : page('a', [2, 3], null),
		)
		const state = await freshGallery()
		await state.searchSkins('')
		await state.loadMore()
		expect(state.shownSkins.value.map((skin) => skin.sourceId)).toEqual(['a1', 'a2', 'a3'])
	})

	it('skips a page that only repeats what was shown', async () => {
		gallery
			.mockResolvedValueOnce(page('a', [1], 'cursor-1'))
			.mockResolvedValueOnce(page('a', [1], 'cursor-2'))
			.mockResolvedValueOnce(page('b', [1], null))
		const state = await freshGallery()
		await state.searchSkins('')
		await state.showNextPage()
		expect(gallery).toHaveBeenCalledTimes(3)
		expect(state.shownSkins.value.map((skin) => skin.sourceId)).toEqual(['b1'])
	})

	it('does not invent a page when nothing new is left', async () => {
		gallery
			.mockResolvedValueOnce(page('a', [1], 'cursor-1'))
			.mockResolvedValueOnce(page('a', [1], null))
		const state = await freshGallery()
		await state.searchSkins('')
		await state.showNextPage()
		expect(state.firstShownPage.value).toBe(0)
		expect(state.hasNext.value).toBe(false)
	})

	it('reports having no results', async () => {
		gallery.mockResolvedValue(page('a', [], null))
		const state = await freshGallery()
		await state.searchSkins('nothing')
		expect(state.status.value).toBe('ready')
		expect(state.shownSkins.value).toEqual([])
	})

	it('reports a failure instead of showing an empty page', async () => {
		gallery.mockResolvedValue({ status: 'rate_limited' })
		const state = await freshGallery()
		await state.searchSkins('cat')
		expect(state.status.value).toBe('rate_limited')
	})

	it('keeps what is shown when a later page fails', async () => {
		gallery.mockResolvedValueOnce(page('a', [1], 'cursor-1')).mockResolvedValueOnce({ status: 'unavailable' })
		const state = await freshGallery()
		await state.searchSkins('')
		await state.showNextPage()
		expect(state.status.value).toBe('ready')
		expect(state.moreFailed.value).toBe('unavailable')
		expect(state.shownSkins.value.map((skin) => skin.sourceId)).toEqual(['a1'])
	})

	it('treats a crashed request as unavailable', async () => {
		gallery.mockRejectedValue(new Error('broken'))
		const state = await freshGallery()
		await state.searchSkins('cat')
		expect(state.status.value).toBe('unavailable')
	})

	it('ignores an older search that finishes late', async () => {
		let finishFirst: (value: unknown) => void = () => {}
		gallery
			.mockReturnValueOnce(new Promise((resolve) => (finishFirst = resolve)))
			.mockResolvedValueOnce(page('b', [1], null))
		const state = await freshGallery()
		const first = state.searchSkins('old')
		await state.searchSkins('new')
		finishFirst(page('a', [1], null))
		await first
		expect(state.shownSkins.value.map((skin) => skin.sourceId)).toEqual(['b1'])
		expect(state.activeQuery.value).toBe('new')
	})

	it('only loads once on its own', async () => {
		gallery.mockResolvedValue(page('a', [1], null))
		const state = await freshGallery()
		state.loadOnce()
		await vi.waitFor(() => expect(state.status.value).toBe('ready'))
		state.loadOnce()
		expect(gallery).toHaveBeenCalledTimes(1)
	})

	it('clearing the search goes back to the latest skins', async () => {
		gallery.mockResolvedValue(page('a', [1], null))
		const state = await freshGallery()
		await state.searchSkins('cat')
		await state.searchSkins('')
		expect(gallery).toHaveBeenLastCalledWith(null, null)
		expect(state.activeQuery.value).toBe('')
	})
})

describe('player lookups', () => {
	it('only happens for text that could be a username', async () => {
		gallery.mockResolvedValue(page('a', [1], null))
		const state = await freshGallery()
		await state.searchSkins('among us')
		expect(lookup).not.toHaveBeenCalled()
		expect(state.playerStatus.value).toBe('none')
	})

	it('shows a player alongside the gallery', async () => {
		gallery.mockResolvedValue(page('a', [1], null))
		lookup.mockResolvedValue({
			status: 'found',
			player: { id: 'id', name: 'Notch', model: 'CLASSIC', texture: 'data:x', texture_key: 'k' },
		})
		const state = await freshGallery()
		await state.searchSkins('Notch')
		expect(state.player.value?.name).toBe('Notch')
		expect(state.playerStatus.value).toBe('found')
		expect(state.shownSkins.value).toHaveLength(1)
	})

	it('stays quiet when nobody has that name', async () => {
		gallery.mockResolvedValue(page('a', [1], null))
		const state = await freshGallery()
		await state.searchSkins('hoodie')
		expect(state.playerStatus.value).toBe('none')
	})

	it('says when Mojang can be reached no longer', async () => {
		gallery.mockResolvedValue(page('a', [1], null))
		lookup.mockResolvedValue({ status: 'rate_limited' })
		const state = await freshGallery()
		await state.searchSkins('cat')
		expect(state.playerStatus.value).toBe('rate_limited')
	})
})
