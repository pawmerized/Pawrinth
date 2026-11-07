import { type BackgroundSettings, normalizeBackground, parseColor } from './background-model'

export type ThemeBase = 'dark' | 'light'

export const PALETTE_KEYS = [
	'background',
	'panel',
	'surface',
	'raised',
	'sidebar',
	'button',
	'border',
	'text',
	'textMuted',
	'accent',
	'selection',
] as const

export type PaletteKey = (typeof PALETTE_KEYS)[number]
export type ThemePalette = Record<PaletteKey, string>

export interface CustomTheme {
	id: string
	name: string
	base: ThemeBase
	palette: ThemePalette
}

export type BackgroundAdjustments = Omit<BackgroundSettings, 'path'>

/**
 * The theme file format. Version 1 has `name`, `base` and `palette`, where a palette is
 * every key in `PALETTE_KEYS` as `#rrggbb`. Version 2 adds an optional `background` with
 * how a background image is adjusted. The image itself is never part of a theme file.
 */
export interface ThemeExport {
	format: 'pawrinth-theme'
	version: number
	name: string
	base: ThemeBase
	palette: ThemePalette
	background?: BackgroundAdjustments
}

export const THEME_FORMAT_VERSION = 2

const HEX_COLOR = /^#[0-9a-f]{6}$/i
const MAX_NAME_LENGTH = 48

export const MIN_READABLE_CONTRAST = 3
export const RECOMMENDED_CONTRAST = 4.5

function channels(hex: string): [number, number, number] {
	const value = Number.parseInt(hex.slice(1), 16)
	return [(value >> 16) & 255, (value >> 8) & 255, value & 255]
}

function toHex(rgb: number[]): string {
	return `#${rgb.map((channel) => Math.round(channel).toString(16).padStart(2, '0')).join('')}`
}

export function mix(from: string, to: string, amount: number): string {
	const a = channels(from)
	const b = channels(to)
	return toHex(a.map((channel, index) => channel + (b[index] - channel) * amount))
}

export function withAlpha(hex: string, alpha: number): string {
	const [r, g, b] = channels(hex)
	return `rgba(${r}, ${g}, ${b}, ${alpha})`
}

function relativeLuminance(hex: string): number {
	const [r, g, b] = channels(hex).map((channel) => {
		const value = channel / 255
		return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
	})
	return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

export function contrastRatio(a: string, b: string): number {
	const [light, dark] = [relativeLuminance(a), relativeLuminance(b)].sort((x, y) => y - x)
	return (light + 0.05) / (dark + 0.05)
}

/** The foreground, black or white, that reads best on `background`. */
export function readableOn(background: string): string {
	return contrastRatio(background, '#ffffff') >= contrastRatio(background, '#000000')
		? '#ffffff'
		: '#000000'
}

export interface ContrastIssue {
	label: 'text' | 'textMuted' | 'accent'
	on: 'background' | 'surface' | 'sidebar'
	ratio: number
	severity: 'warning' | 'blocking'
}

/** Pairs of foreground and background that the interface relies on. */
export function contrastIssues(palette: ThemePalette): ContrastIssue[] {
	const pairs: [ContrastIssue['label'], ContrastIssue['on']][] = [
		['text', 'background'],
		['text', 'surface'],
		['text', 'sidebar'],
		['textMuted', 'background'],
		['textMuted', 'surface'],
		['accent', 'background'],
		['accent', 'surface'],
	]
	const issues: ContrastIssue[] = []
	for (const [label, on] of pairs) {
		const ratio = contrastRatio(palette[label], palette[on])
		if (ratio < RECOMMENDED_CONTRAST) {
			issues.push({
				label,
				on,
				ratio,
				severity: ratio < MIN_READABLE_CONTRAST ? 'blocking' : 'warning',
			})
		}
	}
	return issues
}

export function isUsable(palette: ThemePalette): boolean {
	return !contrastIssues(palette).some((issue) => issue.severity === 'blocking')
}

/**
 * Maps a palette onto the design tokens in `packages/assets/styles/variables.scss`.
 * Every other color in the app derives from these variables.
 */
export function themeTokens(theme: Pick<CustomTheme, 'base' | 'palette'>): Record<string, string> {
	const { base, palette } = theme
	const isDark = base === 'dark'
	const accentContrast = readableOn(palette.accent)
	return {
		'--surface-1': palette.background,
		'--surface-1-5': mix(palette.background, palette.panel, 0.5),
		'--surface-2': palette.panel,
		'--surface-2-5': mix(palette.panel, palette.surface, 0.5),
		'--surface-3': palette.surface,
		'--surface-4': palette.raised,
		'--surface-5': palette.border,
		'--plus-frame-color': palette.sidebar,
		'--color-bg': palette.background,
		'--color-raised-bg': palette.surface,
		'--color-super-raised-bg': palette.raised,
		'--color-button-bg': palette.button,
		'--color-button-border': withAlpha(palette.border, 0.5),
		'--color-divider': palette.border,
		'--color-divider-dark': palette.border,
		'--color-text-primary': palette.text,
		'--color-text-default': mix(palette.textMuted, palette.text, 0.35),
		'--color-text-tertiary': palette.textMuted,
		'--color-contrast': palette.text,
		'--color-base': mix(palette.textMuted, palette.text, 0.35),
		'--color-secondary': palette.textMuted,
		'--color-brand': palette.accent,
		'--color-brand-highlight': withAlpha(palette.accent, 0.25),
		'--color-brand-shadow': withAlpha(palette.accent, 0.7),
		'--color-accent-contrast': accentContrast,
		'--color-button-bg-selected': isDark ? withAlpha(palette.accent, 0.25) : palette.accent,
		'--color-button-text-selected': isDark ? palette.accent : accentContrast,
		'--brand-gradient-bg': `linear-gradient(0deg, ${withAlpha(palette.accent, 0.175)} 0%, ${withAlpha(palette.accent, 0.125)} 100%)`,
		'--brand-gradient-border': withAlpha(palette.border, 0.6),
		'--brand-gradient-fade-out-color': `linear-gradient(to bottom, ${withAlpha(palette.panel, 0)} 0%, ${mix(palette.panel, palette.accent, 0.12)} 80%)`,
		'--brand-gradient-strong-bg': `linear-gradient(270deg, ${withAlpha(palette.accent, 0.175)} 0%, ${withAlpha(palette.accent, 0.12)} 100%)`,
	}
}

/**
 * The palette is validated before this runs, so no user text reaches the
 * stylesheet except colors that passed `HEX_COLOR`.
 */
export function themeStylesheet(theme: CustomTheme): string {
	if (!isValidPalette(theme.palette)) return ''
	const declarations = Object.entries(themeTokens(theme))
		.map(([token, value]) => `${token}: ${value};`)
		.join(' ')
	const root = `html.${theme.base}-mode.plus-custom`
	return [
		`${root} { ${declarations} }`,
		`${root} .app-grid-navbar, ${root} .app-grid-statusbar { --color-raised-bg: ${theme.palette.sidebar}; background-color: ${theme.palette.sidebar}; }`,
		`${root} ::selection { background-color: ${withAlpha(theme.palette.selection, 0.45)}; }`,
	].join('\n')
}

export function isValidPalette(palette: unknown): palette is ThemePalette {
	if (typeof palette !== 'object' || palette === null) return false
	return PALETTE_KEYS.every((key) => {
		const value = (palette as Record<string, unknown>)[key]
		return typeof value === 'string' && HEX_COLOR.test(value)
	})
}

export function isValidTheme(theme: unknown): theme is CustomTheme {
	if (typeof theme !== 'object' || theme === null) return false
	const candidate = theme as Partial<CustomTheme>
	return (
		typeof candidate.id === 'string' &&
		candidate.id.length > 0 &&
		typeof candidate.name === 'string' &&
		candidate.name.trim().length > 0 &&
		candidate.name.length <= MAX_NAME_LENGTH &&
		(candidate.base === 'dark' || candidate.base === 'light') &&
		isValidPalette(candidate.palette)
	)
}

export function exportTheme(
	theme: Pick<CustomTheme, 'name' | 'base' | 'palette'>,
	background?: BackgroundSettings | null,
): string {
	const document: ThemeExport = {
		format: 'pawrinth-theme',
		version: THEME_FORMAT_VERSION,
		name: theme.name,
		base: theme.base,
		palette: theme.palette,
	}
	if (background) {
		const { path: _path, ...adjustments } = background
		document.background = adjustments
	}
	return JSON.stringify(document, null, 2)
}

/** Safe on every system, and easy to recognize in a folder. */
export function themeFileName(name: string): string {
	const safe = name
		.replace(/[<>:"/\\|?*\u0000-\u001f]/g, '')
		.replace(/\s+/g, ' ')
		.trim()
		.slice(0, MAX_NAME_LENGTH)
	return `${safe || 'theme'}.pawrinth-theme.json`
}

/** Finds a name that isn't taken, for keeping both themes. */
export function uniqueThemeName(name: string, existing: readonly string[]): string {
	const taken = new Set(existing.map((item) => item.toLowerCase()))
	if (!taken.has(name.toLowerCase())) return name
	for (let number = 2; ; number++) {
		const suffix = ` (${number})`
		const candidate = `${name.slice(0, MAX_NAME_LENGTH - suffix.length)}${suffix}`
		if (!taken.has(candidate.toLowerCase())) return candidate
	}
}

/** Reads a stock theme's current colors from its tokens, so it can be exported too. */
export function paletteFromTokens(read: (token: string) => string): ThemePalette | null {
	const sources: Record<PaletteKey, string> = {
		background: '--surface-1',
		panel: '--surface-2',
		surface: '--surface-3',
		raised: '--surface-4',
		sidebar: '--color-raised-bg',
		button: '--color-button-bg',
		border: '--surface-5',
		text: '--color-contrast',
		textMuted: '--color-secondary',
		accent: '--color-brand',
		selection: '--color-brand',
	}
	const palette = {} as ThemePalette
	for (const key of PALETTE_KEYS) {
		const color = parseColor(read(sources[key]))
		if (!color) return null
		palette[key] = toHex(color)
	}
	return palette
}

export type ImportError =
	| 'not-json'
	| 'wrong-format'
	| 'unsupported-version'
	| 'invalid-colors'
	| 'unreadable'

export type ImportResult =
	| { ok: true; theme: Omit<CustomTheme, 'id'>; background: BackgroundAdjustments | null }
	| { ok: false; error: ImportError }

export function parseThemeImport(text: string): ImportResult {
	let parsed: unknown
	try {
		parsed = JSON.parse(text)
	} catch {
		return { ok: false, error: 'not-json' }
	}
	const candidate = parsed as Partial<ThemeExport> | null
	if (!candidate || candidate.format !== 'pawrinth-theme') {
		return { ok: false, error: 'wrong-format' }
	}
	if (typeof candidate.version !== 'number' || candidate.version < 1) {
		return { ok: false, error: 'wrong-format' }
	}
	if (candidate.version > THEME_FORMAT_VERSION) {
		return { ok: false, error: 'unsupported-version' }
	}
	if (
		typeof candidate.name !== 'string' ||
		candidate.name.trim().length === 0 ||
		candidate.name.length > MAX_NAME_LENGTH ||
		(candidate.base !== 'dark' && candidate.base !== 'light')
	) {
		return { ok: false, error: 'wrong-format' }
	}
	if (!isValidPalette(candidate.palette)) {
		return { ok: false, error: 'invalid-colors' }
	}
	const palette = Object.fromEntries(
		PALETTE_KEYS.map((key) => [key, candidate.palette![key]]),
	) as ThemePalette
	if (!isUsable(palette)) {
		return { ok: false, error: 'unreadable' }
	}
	const normalized = candidate.background
		? normalizeBackground({ ...candidate.background, path: 'unused' })
		: null
	let background: BackgroundAdjustments | null = null
	if (normalized) {
		const { path: _path, ...adjustments } = normalized
		background = adjustments
	}
	return { ok: true, theme: { name: candidate.name.trim(), base: candidate.base, palette }, background }
}
