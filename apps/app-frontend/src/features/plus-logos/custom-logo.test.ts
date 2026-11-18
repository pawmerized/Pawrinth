import { describe, expect, it } from 'vitest'

import { cornerRadius, type CustomLogo, readCustomLogo, resize, startingSize } from './custom-logo'

const wide: CustomLogo = {
	path: 'logo.png',
	width: 112,
	height: 28,
	keep_ratio: true,
	ratio: 4,
	rounded: false,
}

describe('custom logos', () => {
	it('start at the title bar height in their own proportions', () => {
		expect(startingSize(4)).toEqual({ width: 112, height: 28 })
		expect(startingSize(1)).toEqual({ width: 28, height: 28 })
		expect(startingSize(20)).toEqual({ width: 320, height: 16 })
	})

	it('keep their proportions while resizing, even at a limit', () => {
		expect(resize(wide, 'height', 40)).toMatchObject({ width: 160, height: 40 })
		expect(resize(wide, 'width', 80)).toMatchObject({ width: 80, height: 20 })
		const capped = resize(wide, 'height', 400)
		expect(capped.width / capped.height).toBeCloseTo(4, 0)
		expect(capped.height).toBeLessThanOrEqual(44)
	})

	it('change one side alone only when proportions are unlocked', () => {
		expect(resize({ ...wide, keep_ratio: false }, 'width', 50)).toMatchObject({
			width: 50,
			height: 28,
		})
		expect(resize(wide, 'width', Number.NaN)).toBe(wide)
	})

	it('ignore malformed settings and fix out-of-range sizes', () => {
		expect(readCustomLogo(null)).toBeNull()
		expect(readCustomLogo({ path: '', ratio: 1 })).toBeNull()
		expect(readCustomLogo({ path: 'a.png', ratio: 0 })).toBeNull()
		expect(readCustomLogo({ path: 'a.png', ratio: 2, width: 9999, height: 1 })).toMatchObject({
			width: 320,
			height: 12,
			keep_ratio: true,
			rounded: false,
		})
	})

	it('round corners from the shorter side', () => {
		expect(cornerRadius({ width: 112, height: 28, rounded: true })).toBe('6px')
		expect(cornerRadius({ width: 112, height: 28, rounded: false })).toBe('0')
	})
})
