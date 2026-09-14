export type BackgroundFit = 'cover' | 'contain' | 'tile'
export type BackgroundPosition = 'center' | 'top' | 'bottom' | 'left' | 'right'

export interface BackgroundSettings {
	path: string
	fit: BackgroundFit
	position: BackgroundPosition
	/** How visible the image is, from 0 to 1. */
	opacity: number
	blur: number
	brightness: number
	/** How strongly the app's own background covers the image, from 0.45 to 0.95. */
	overlay: number
	tint: string | null
	tintStrength: number
	animate: boolean
	/** Plays even when Windows asks apps to reduce motion. */
	ignoreSystemMotion: boolean
}

export const MIN_OVERLAY = 0.45
export const MAX_OVERLAY = 0.95

export const DEFAULT_BACKGROUND: Omit<BackgroundSettings, 'path'> = {
	fit: 'cover',
	position: 'center',
	opacity: 1,
	blur: 0,
	brightness: 1,
	overlay: 0.6,
	tint: null,
	tintStrength: 0.25,
	animate: true,
	ignoreSystemMotion: false,
}

const HEX_COLOR = /^#[0-9a-f]{6}$/i
const FITS: BackgroundFit[] = ['cover', 'contain', 'tile']
const POSITIONS: BackgroundPosition[] = ['center', 'top', 'bottom', 'left', 'right']

function clamp(value: unknown, min: number, max: number, fallback: number): number {
	return typeof value === 'number' && Number.isFinite(value)
		? Math.min(max, Math.max(min, value))
		: fallback
}

/** Settings are read back from storage, so nothing about them is assumed. */
export function normalizeBackground(value: unknown): BackgroundSettings | null {
	if (typeof value !== 'object' || value === null) return null
	const raw = value as Partial<BackgroundSettings>
	if (typeof raw.path !== 'string' || raw.path.length === 0) return null
	return {
		path: raw.path,
		fit: FITS.includes(raw.fit as BackgroundFit) ? (raw.fit as BackgroundFit) : 'cover',
		position: POSITIONS.includes(raw.position as BackgroundPosition)
			? (raw.position as BackgroundPosition)
			: 'center',
		opacity: clamp(raw.opacity, 0.1, 1, DEFAULT_BACKGROUND.opacity),
		blur: clamp(raw.blur, 0, 40, DEFAULT_BACKGROUND.blur),
		brightness: clamp(raw.brightness, 0.2, 1.5, DEFAULT_BACKGROUND.brightness),
		overlay: clamp(raw.overlay, MIN_OVERLAY, MAX_OVERLAY, DEFAULT_BACKGROUND.overlay),
		tint: typeof raw.tint === 'string' && HEX_COLOR.test(raw.tint) ? raw.tint : null,
		tintStrength: clamp(raw.tintStrength, 0, 0.8, DEFAULT_BACKGROUND.tintStrength),
		animate: raw.animate !== false,
		ignoreSystemMotion: raw.ignoreSystemMotion === true,
	}
}

export function isGif(path: string): boolean {
	return path.toLowerCase().endsWith('.gif')
}

/** A GIF plays only when the settings, the OS preference and the window all allow it. */
export function shouldAnimate(
	settings: Pick<BackgroundSettings, 'animate' | 'ignoreSystemMotion'>,
	motion: { appReduced: boolean; systemReduced: boolean },
	windowVisible: boolean,
): boolean {
	if (!settings.animate || motion.appReduced || !windowVisible) return false
	return settings.ignoreSystemMotion || !motion.systemReduced
}

/** Reads `#rrggbb` or `rgb(...)`, which is how the design tokens are written. */
export function parseColor(value: string): [number, number, number] | null {
	const trimmed = value.trim()
	const hex = trimmed.match(/^#([0-9a-f]{6})$/i)
	if (hex) {
		const number = Number.parseInt(hex[1], 16)
		return [(number >> 16) & 255, (number >> 8) & 255, number & 255]
	}
	const rgb = trimmed.match(/^rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)/i)
	return rgb ? [Number(rgb[1]), Number(rgb[2]), Number(rgb[3])] : null
}

/**
 * Only the deepest surfaces turn see-through. Cards stay solid, so text stays readable,
 * and so do sticky headers, because content scrolls underneath them.
 * The colors are `!important` because a custom theme's rule is more specific than this
 * one. The content area's rounded corner has nothing behind it once the frame stops
 * painting the whole window, so it gets filled in with the frame's color.
 */
export function overlayStylesheet(surfaces: Record<string, string>, overlay: number): string {
	const colors = Object.entries(surfaces).flatMap(([token, value]) => {
		const color = parseColor(value)
		return color ? [{ token, channels: color.join(', ') }] : []
	})
	const seeThrough = colors
		.map(({ token, channels }) => `${token}: rgba(${channels}, ${overlay}) !important;`)
		.join(' ')
	const solid = colors
		.map(({ token, channels }) => `${token}: rgb(${channels}) !important;`)
		.join(' ')
	return [
		`html.plus-has-background { ${seeThrough} background: transparent; }`,
		`html.plus-has-background .sticky { ${solid} }`,
		'html.plus-has-background body, html.plus-has-background #app, html.plus-has-background .app-grid-layout { background: transparent; }',
		'html.plus-has-background .app-sidebar { background: var(--surface-2); }',
		"html.plus-has-background .app-contents::after { content: ''; position: absolute; top: 0; left: 0; z-index: 30; width: var(--radius-xl); height: var(--radius-xl); background: radial-gradient(circle at 100% 100%, transparent calc(var(--radius-xl) - 0.5px), var(--plus-frame-color, var(--color-raised-bg)) var(--radius-xl)); pointer-events: none; }",
	].join('\n')
}
