import { describe, expect, it } from 'vitest'

import {
	contrastIssues,
	contrastRatio,
	type CustomTheme,
	exportTheme,
	paletteFromTokens,
	THEME_FORMAT_VERSION,
	themeFileName,
	uniqueThemeName,
	isUsable,
	isValidTheme,
	parseThemeImport,
	readableOn,
	themeStylesheet,
	themeTokens,
} from './theme-model'
import { DARK_STARTER, LIGHT_STARTER, THEME_PRESETS } from './theme-presets'

const theme: CustomTheme = { id: 'a', name: 'Test', base: 'dark', palette: DARK_STARTER }

describe('contrast', () => {
	it('matches known WCAG values', () => {
		expect(contrastRatio('#000000', '#ffffff')).toBeCloseTo(21)
		expect(contrastRatio('#ffffff', '#ffffff')).toBeCloseTo(1)
	})

	it('picks the readable foreground', () => {
		expect(readableOn('#000000')).toBe('#ffffff')
		expect(readableOn('#ffffff')).toBe('#000000')
	})

	it('blocks palettes whose text cannot be read', () => {
		const palette = { ...DARK_STARTER, text: DARK_STARTER.background }
		expect(isUsable(palette)).toBe(false)
		expect(contrastIssues(palette).some((issue) => issue.severity === 'blocking')).toBe(true)
	})

	it('every preset and starter stays readable', () => {
		for (const palette of [DARK_STARTER, LIGHT_STARTER, ...THEME_PRESETS.map((p) => p.palette)]) {
			expect(isUsable(palette)).toBe(true)
		}
	})
})

describe('tokens', () => {
	it('uses the accent for brand tokens', () => {
		const tokens = themeTokens({ ...theme, palette: { ...DARK_STARTER, accent: '#ff0000' } })
		expect(tokens['--color-brand']).toBe('#ff0000')
		expect(tokens['--color-brand-highlight']).toBe('rgba(255, 0, 0, 0.25)')
	})

	it('replaces the green sidebar gradient so no stock color shows through', () => {
		const tokens = themeTokens({ ...theme, palette: { ...DARK_STARTER, accent: '#ff0000' } })
		expect(tokens['--brand-gradient-fade-out-color']).not.toContain('213')
		expect(tokens['--brand-gradient-bg']).toContain('255, 0, 0')
	})

	it('scopes the stylesheet to the base theme and overlay class', () => {
		expect(themeStylesheet(theme)).toContain('html.dark-mode.plus-custom')
		expect(themeStylesheet({ ...theme, base: 'light' })).toContain('html.light-mode.plus-custom')
	})

	it('refuses to generate CSS from anything but validated colors', () => {
		const hostile = {
			...theme,
			palette: {
				...DARK_STARTER,
				accent: 'red; } body { background: url(https://evil.test/) } a {',
			},
		}
		expect(themeStylesheet(hostile)).toBe('')
		expect(isValidTheme(hostile)).toBe(false)
	})
})

describe('theme files', () => {
	const adjustments = {
		path: 'C:/private/image.png',
		fit: 'contain',
		position: 'top',
		opacity: 0.5,
		blur: 4,
		brightness: 0.8,
		overlay: 0.7,
		tint: '#112233',
		tintStrength: 0.3,
		animate: false,
		ignoreSystemMotion: false,
	} as const

	it('carries background adjustments but never the image path', () => {
		const text = exportTheme(theme, adjustments)
		expect(text).not.toContain('private')
		const result = parseThemeImport(text)
		expect(result.ok && result.background?.blur).toBe(4)
		expect(result.ok && result.background?.fit).toBe('contain')
	})

	it('still reads a version 1 file, which has no background', () => {
		const document = JSON.parse(exportTheme(theme))
		document.version = 1
		const result = parseThemeImport(JSON.stringify(document))
		expect(result.ok).toBe(true)
		expect(result.ok && result.background).toBeNull()
	})

	it('refuses a file from a newer version, and says so', () => {
		const document = JSON.parse(exportTheme(theme))
		document.version = THEME_FORMAT_VERSION + 1
		expect(parseThemeImport(JSON.stringify(document))).toEqual({
			ok: false,
			error: 'unsupported-version',
		})
	})

	it('refuses nonsense versions', () => {
		for (const version of [0, -1, 'two', null]) {
			const document = JSON.parse(exportTheme(theme))
			document.version = version
			expect(parseThemeImport(JSON.stringify(document))).toEqual({
				ok: false,
				error: 'wrong-format',
			})
		}
	})

	it('cleans bad background values instead of passing them on', () => {
		const document = JSON.parse(exportTheme(theme, adjustments))
		document.background.opacity = 99
		document.background.tint = 'url(evil)'
		const result = parseThemeImport(JSON.stringify(document))
		expect(result.ok && result.background?.opacity).toBe(1)
		expect(result.ok && result.background?.tint).toBeNull()
	})

	it('makes file names that are safe on every system', () => {
		expect(themeFileName('My Theme')).toBe('My Theme.pawrinth-theme.json')
		expect(themeFileName('a/b:c*d?')).toBe('abcd.pawrinth-theme.json')
		expect(themeFileName('***')).toBe('theme.pawrinth-theme.json')
	})

	it('finds a free name when keeping both', () => {
		expect(uniqueThemeName('Nord', [])).toBe('Nord')
		expect(uniqueThemeName('Nord', ['nord'])).toBe('Nord (2)')
		expect(uniqueThemeName('Nord', ['Nord', 'Nord (2)'])).toBe('Nord (3)')
		expect(uniqueThemeName('x'.repeat(48), ['x'.repeat(48)])).toHaveLength(48)
	})

	it('reads a stock theme from its tokens', () => {
		const palette = paletteFromTokens((token) =>
			token === '--color-brand' ? 'rgb(27, 217, 106)' : '#112233',
		)
		expect(palette?.background).toBe('#112233')
		expect(palette?.accent).toBe('#1bd96a')
	})

	it('gives up rather than guess when a token can’t be read', () => {
		expect(paletteFromTokens(() => 'var(--unresolved)')).toBeNull()
	})
})

describe('import and export', () => {
	it('round trips', () => {
		const result = parseThemeImport(exportTheme(theme))
		expect(result).toEqual({
			ok: true,
			theme: { name: 'Test', base: 'dark', palette: DARK_STARTER },
			background: null,
		})
	})

	it('drops unknown palette keys', () => {
		const document = JSON.parse(exportTheme(theme))
		document.palette.extra = 'url(https://evil.test/)'
		const result = parseThemeImport(JSON.stringify(document))
		expect(result.ok && 'extra' in result.theme.palette).toBe(false)
	})

	it.each([
		['not json', 'not-json'],
		['{"format":"something-else"}', 'wrong-format'],
		[
			JSON.stringify({
				format: 'pawrinth-theme',
				version: 1,
				name: 'x',
				base: 'dark',
				palette: {},
			}),
			'invalid-colors',
		],
		[
			JSON.stringify({
				format: 'pawrinth-theme',
				version: 1,
				name: 'x',
				base: 'dark',
				palette: { ...DARK_STARTER, text: DARK_STARTER.background },
			}),
			'unreadable',
		],
	])('rejects %s', (text, error) => {
		expect(parseThemeImport(text)).toEqual({ ok: false, error })
	})
})
