import { describe, expect, it } from 'vitest'

import {
	isGif,
	MAX_OVERLAY,
	MIN_OVERLAY,
	normalizeBackground,
	overlayStylesheet,
	parseColor,
	shouldAnimate,
} from './background-model'

describe('normalizeBackground', () => {
	it('rejects anything without a path', () => {
		expect(normalizeBackground(null)).toBeNull()
		expect(normalizeBackground('text')).toBeNull()
		expect(normalizeBackground({})).toBeNull()
		expect(normalizeBackground({ path: '' })).toBeNull()
	})

	it('fills in defaults', () => {
		const result = normalizeBackground({ path: 'a.png' })
		expect(result?.fit).toBe('cover')
		expect(result?.animate).toBe(true)
	})

	it('keeps the overlay in the readable range', () => {
		expect(normalizeBackground({ path: 'a.png', overlay: 0 })?.overlay).toBe(MIN_OVERLAY)
		expect(normalizeBackground({ path: 'a.png', overlay: 5 })?.overlay).toBe(MAX_OVERLAY)
		expect(normalizeBackground({ path: 'a.png', overlay: 'x' })?.overlay).toBe(0.6)
	})

	it('drops invalid values rather than passing them to CSS', () => {
		const result = normalizeBackground({
			path: 'a.png',
			tint: 'red; } body { display: none',
			fit: 'url(evil)',
			position: 'url(evil)',
		})
		expect(result?.tint).toBeNull()
		expect(result?.fit).toBe('cover')
		expect(result?.position).toBe('center')
	})
})

describe('GIF playback', () => {
	it('recognizes GIFs by extension', () => {
		expect(isGif('C:/a/b.GIF')).toBe(true)
		expect(isGif('C:/a/b.webp')).toBe(false)
	})

	const on = { animate: true, ignoreSystemMotion: false }
	const still = { appReduced: false, systemReduced: false }

	it('pauses for the setting, the app option and a hidden window', () => {
		expect(shouldAnimate(on, still, true)).toBe(true)
		expect(shouldAnimate({ ...on, animate: false }, still, true)).toBe(false)
		expect(shouldAnimate(on, { ...still, appReduced: true }, true)).toBe(false)
		expect(shouldAnimate(on, still, false)).toBe(false)
	})

	it('respects the system preference unless the user overrides it', () => {
		const system = { appReduced: false, systemReduced: true }
		expect(shouldAnimate(on, system, true)).toBe(false)
		expect(shouldAnimate({ ...on, ignoreSystemMotion: true }, system, true)).toBe(true)
	})

	it('still pauses for the app option when overriding the system', () => {
		const both = { appReduced: true, systemReduced: true }
		expect(shouldAnimate({ ...on, ignoreSystemMotion: true }, both, true)).toBe(false)
	})
})

describe('overlay stylesheet', () => {
	it('reads both color formats used by the tokens', () => {
		expect(parseColor('#16181c')).toEqual([22, 24, 28])
		expect(parseColor(' rgb(1, 2, 3) ')).toEqual([1, 2, 3])
		expect(parseColor('rgba(1, 2, 3, 0.5)')).toEqual([1, 2, 3])
		expect(parseColor('var(--x)')).toBeNull()
	})

	it('only makes the given surfaces transparent', () => {
		const css = overlayStylesheet({ '--surface-1': '#16181c', '--bad': 'var(--x)' }, 0.6)
		expect(css).toContain('--surface-1: rgba(22, 24, 28, 0.6)')
		expect(css).not.toContain('--bad')
	})

	it('outranks a custom theme, whose rule is more specific', () => {
		const css = overlayStylesheet({ '--surface-1': '#16181c' }, 0.6)
		expect(css).toContain('--surface-1: rgba(22, 24, 28, 0.6) !important')
	})

	it('keeps headers solid while content scrolls under them', () => {
		const css = overlayStylesheet({ '--surface-1': '#16181c', '--bad': 'var(--x)' }, 0.6)
		expect(css).toContain('html.plus-has-background .sticky { --surface-1: rgb(22, 24, 28) !important;')
		expect(css).not.toContain('--bad')
	})

	it('fills the rounded corner with the frame color', () => {
		const css = overlayStylesheet({}, 0.6)
		expect(css).toContain('.app-contents::after')
		expect(css).toContain('var(--plus-frame-color, var(--color-raised-bg))')
	})
})
