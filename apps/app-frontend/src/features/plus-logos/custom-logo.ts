import { ref } from 'vue'

/**
 * A picture someone chose as Pawrinth's logo. The file is a copy in Pawrinth's own data,
 * made by the same import as backgrounds, which checks it's really an image.
 */
export interface CustomLogo {
	path: string
	/** Its size in the title bar, in pixels. */
	width: number
	height: number
	/** Whether changing one side changes the other to keep the picture's proportions. */
	keep_ratio: boolean
	/** The picture's own width divided by its height. */
	ratio: number
	rounded: boolean
}

export const LOGO_WIDTH = { min: 16, max: 320 } as const
export const LOGO_HEIGHT = { min: 12, max: 44 } as const
const DEFAULT_HEIGHT = 28
const FIRST_PAINT_KEY = 'plus-custom-logo'

function clamp(value: number, range: { min: number; max: number }): number {
	return Math.min(Math.max(Math.round(value), range.min), range.max)
}

/** A size with that height, as wide as the picture's proportions make it, within limits. */
function sizeForHeight(height: number, ratio: number): { width: number; height: number } {
	const width = clamp(height * ratio, LOGO_WIDTH)
	return { width, height: clamp(width / ratio, LOGO_HEIGHT) }
}

/** The size a newly chosen picture starts at. */
export function startingSize(ratio: number): { width: number; height: number } {
	return sizeForHeight(DEFAULT_HEIGHT, ratio)
}

/**
 * Changes one side. With proportions kept, the other side follows, and when a limit stops
 * one side the other stops with it, so the picture never needs stretching.
 */
export function resize(logo: CustomLogo, side: 'width' | 'height', value: number): CustomLogo {
	if (!Number.isFinite(value)) return logo
	if (!logo.keep_ratio) {
		return side === 'width'
			? { ...logo, width: clamp(value, LOGO_WIDTH) }
			: { ...logo, height: clamp(value, LOGO_HEIGHT) }
	}
	const height =
		side === 'height' ? clamp(value, LOGO_HEIGHT) : clamp(value / logo.ratio, LOGO_HEIGHT)
	return { ...logo, ...sizeForHeight(height, logo.ratio) }
}

/** Settings come from disk, so anything malformed counts as no custom logo. */
export function readCustomLogo(value: unknown): CustomLogo | null {
	if (typeof value !== 'object' || value === null) return null
	const candidate = value as Partial<Record<keyof CustomLogo, unknown>>
	const ratio = Number(candidate.ratio)
	if (
		typeof candidate.path !== 'string' ||
		!candidate.path ||
		!(ratio > 0) ||
		!Number.isFinite(ratio)
	) {
		return null
	}
	return {
		path: candidate.path,
		width: clamp(Number(candidate.width) || LOGO_WIDTH.min, LOGO_WIDTH),
		height: clamp(Number(candidate.height) || LOGO_HEIGHT.min, LOGO_HEIGHT),
		keep_ratio: candidate.keep_ratio !== false,
		ratio,
		rounded: candidate.rounded === true,
	}
}

/** Corners round in proportion to the shorter side, so wide logos don't get oval ends. */
export function cornerRadius(logo: Pick<CustomLogo, 'width' | 'height' | 'rounded'>): string {
	return logo.rounded ? `${Math.round(Math.min(logo.width, logo.height) * 0.22)}px` : '0'
}

function readFirstPaint(): CustomLogo | null {
	try {
		const stored = window.localStorage.getItem(FIRST_PAINT_KEY)
		return stored ? readCustomLogo(JSON.parse(stored)) : null
	} catch {
		return null
	}
}

/** The custom logo shown now, if one is in use, remembered for the loading screen. */
export const currentCustomLogo = ref<CustomLogo | null>(readFirstPaint())

export function setCurrentCustomLogo(logo: CustomLogo | null): void {
	currentCustomLogo.value = logo
	try {
		if (logo) window.localStorage.setItem(FIRST_PAINT_KEY, JSON.stringify(logo))
		else window.localStorage.removeItem(FIRST_PAINT_KEY)
	} catch {
		// storage blocked or full
	}
}
