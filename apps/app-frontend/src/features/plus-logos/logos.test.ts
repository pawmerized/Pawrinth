import { describe, expect, it } from 'vitest'

import {
	gridBounds,
	gridLayers,
	iconPixels,
	LOGO_GRIDS,
	LOGO_IDS,
	logoFrame,
	logoLayers,
	resolveLogo,
} from './logos'

const KNOWN_CELLS = new Set([...'.BHSMNDWPYy'])

describe('logo grids', () => {
	for (const [id, grid] of Object.entries(LOGO_GRIDS)) {
		it(`${id} is a 16 by 16 grid of known colors`, () => {
			expect(grid).toHaveLength(16)
			for (const row of grid) {
				expect(row).toHaveLength(16)
				for (const cell of row) expect(KNOWN_CELLS.has(cell)).toBe(true)
			}
		})
	}
})

describe('resolveLogo', () => {
	it('keeps the logos this build has', () => {
		for (const id of LOGO_IDS) expect(resolveLogo(id)).toBe(id)
	})

	it('shows the paw for anything else', () => {
		expect(resolveLogo(null)).toBe('paw')
		expect(resolveLogo('from-a-newer-build')).toBe('paw')
		expect(resolveLogo(3)).toBe('paw')
	})
})

describe('gridLayers', () => {
	it('draws every shaded cell on the accent and merges runs in a row', () => {
		const grid = ['BBH.', '..SY']
		const layers = gridLayers(grid)
		expect(layers[0]).toEqual({
			d: 'M0 0h96v32h-96ZM64 32h32v32h-32Z',
			fill: 'var(--color-brand)',
		})
		expect(layers).toContainEqual({ d: 'M64 0h32v32h-32Z', fill: '#fff', opacity: 0.3 })
		expect(layers).toContainEqual({ d: 'M64 32h32v32h-32Z', fill: '#000', opacity: 0.2 })
		expect(layers).toContainEqual({ d: 'M96 32h32v32h-32Z', fill: '#ffd65c' })
	})
})

describe('logoFrame', () => {
	it('keeps the paw where it always was', () => {
		expect(logoFrame('paw')).toEqual({ x: 4, y: 4, scale: 0.375 })
		expect(logoLayers('paw')).toHaveLength(3)
	})

	it('fits each character inside the paw-sized area, centered', () => {
		for (const id of ['mochi', 'nova'] as const) {
			const bounds = gridBounds(LOGO_GRIDS[id])
			const frame = logoFrame(id)
			const left = frame.x + bounds.left * 32 * frame.scale
			const top = frame.y + bounds.top * 32 * frame.scale
			const width = bounds.width * 32 * frame.scale
			const height = bounds.height * 32 * frame.scale
			expect(width).toBeLessThanOrEqual(168 + 1e-9)
			expect(height).toBeLessThanOrEqual(150 + 1e-9)
			expect(left + width / 2).toBeCloseTo(100)
			expect(top + height / 2).toBeCloseTo(100)
		}
	})
})

describe('iconPixels', () => {
	const size = 128

	function pixel(pixels: Uint8Array, x: number, y: number): number[] {
		const index = (y * size + x) * 4
		return [...pixels.slice(index, index + 4)]
	}

	it('rounds the corners and fills the middle', () => {
		const pixels = iconPixels('mochi', size)
		expect(pixels).toHaveLength(size * size * 4)
		expect(pixel(pixels, 0, 0)[3]).toBe(0)
		expect(pixel(pixels, size - 1, size - 1)[3]).toBe(0)
		expect(pixel(pixels, size / 2, 2)[3]).toBe(255)
		expect(pixel(pixels, 2, size / 2)[3]).toBe(255)
	})

	it('draws whole cells in the icon accent', () => {
		const pixels = iconPixels('mochi', size)
		const cell = 6
		const left = (size - 16 * cell) / 2
		const top = Math.round((size - 14 * cell) / 2) - cell
		const accentX = left + 5 * cell
		const accentY = top + 5 * cell
		for (const [dx, dy] of [
			[0, 0],
			[cell - 1, cell - 1],
		]) {
			expect(pixel(pixels, accentX + dx, accentY + dy)).toEqual([27, 217, 106, 255])
		}
		const eyeShine = pixel(pixels, left + 3 * cell, top + 7 * cell)
		expect(eyeShine).toEqual([255, 255, 255, 255])
	})
})
