import { describe, expect, it } from 'vitest'

import { frontViewParts, isSlimTexture } from './skin-front-view'

const CANVAS_WIDTH = 16
const CANVAS_HEIGHT = 32

function pixels(alpha: number): Uint8ClampedArray {
	return Uint8ClampedArray.from([0, 0, 0, alpha, 0, 0, 0, alpha])
}

describe('telling slim skins apart', () => {
	it('sees an empty last arm column as slim', () => {
		expect(isSlimTexture(pixels(0))).toBe(true)
	})

	it('sees a filled one as classic', () => {
		expect(isSlimTexture(pixels(255))).toBe(false)
	})
})

describe('the front view', () => {
	it.each([
		['modern classic', 64, false],
		['modern slim', 64, true],
		['old classic', 32, false],
	])('stays inside the thumbnail for %s skins', (_, height, slim) => {
		for (const part of frontViewParts(height, slim)) {
			expect(part.toX).toBeGreaterThanOrEqual(0)
			expect(part.toX + part.width).toBeLessThanOrEqual(CANVAS_WIDTH)
			expect(part.toY + part.height).toBeLessThanOrEqual(CANVAS_HEIGHT)
			expect(part.x + part.width).toBeLessThanOrEqual(64)
			expect(part.y + part.height).toBeLessThanOrEqual(height)
		}
	})

	it('draws the outer layer over the base', () => {
		const parts = frontViewParts(64, false)
		const headBase = parts.findIndex((part) => part.x === 8 && part.y === 8)
		const headOuter = parts.findIndex((part) => part.x === 40 && part.y === 8)
		expect(headOuter).toBeGreaterThan(headBase)
	})

	it('narrows both arms on a slim skin and keeps them against the body', () => {
		const arms = frontViewParts(64, true).filter((part) => part.toY === 8 && part.toX !== 4)
		expect(arms.every((part) => part.width === 3)).toBe(true)
		expect(arms.some((part) => part.toX === 1)).toBe(true)
		expect(arms.some((part) => part.toX === 12)).toBe(true)
	})

	it('mirrors the only arm and leg an old skin has', () => {
		const mirrored = frontViewParts(32, false).filter((part) => part.mirrored)
		expect(mirrored.map((part) => part.toX).sort()).toEqual([12, 8])
	})

	it('has no second layer to draw on an old skin, apart from the head', () => {
		const parts = frontViewParts(32, false)
		expect(parts.every((part) => part.y < 32)).toBe(true)
	})
})
