import { ref } from 'vue'

export const LOGO_IDS = ['paw', 'mochi', 'nova'] as const

export type LogoId = (typeof LOGO_IDS)[number]

export interface LogoLayer {
	d: string
	fill: string
	opacity?: number
}

/** Places a logo's 512-unit artwork inside the 200-unit mark area. */
export interface LogoFrame {
	x: number
	y: number
	scale: number
}

const BRAND = 'var(--color-brand)'
const CELL = 32
const GRID_SIZE = 16
const MARK_SIZE = 200
const MARK_WIDTH = 168
const MARK_HEIGHT = 150
const FIRST_PAINT_KEY = 'plus-logo'

/**
 * Letters shade the accent the way the paw does: `B` is the accent, `H` lights it,
 * and `S`, `M`, `N` and `D` darken it. The rest are fixed colors.
 */
const SHADES: Record<string, { fill: string; opacity: number }> = {
	H: { fill: '#fff', opacity: 0.3 },
	S: { fill: '#000', opacity: 0.2 },
	M: { fill: '#000', opacity: 0.45 },
	N: { fill: '#000', opacity: 0.6 },
	D: { fill: '#000', opacity: 0.8 },
}

const SOLIDS: Record<string, string> = {
	W: '#ffffff',
	P: '#ff8aad',
	Y: '#ffd65c',
	y: '#e6a834',
}

export const LOGO_GRIDS: Record<Exclude<LogoId, 'paw'>, readonly string[]> = {
	mochi: [
		'................',
		'.H............H.',
		'.HH..........HH.',
		'.HPH........HPH.',
		'.BPPHHHHHHHHPPB.',
		'.BBBBBBBBBBBBBB.',
		'HBBBBBBBBBBBBBBS',
		'HBBWDBBBBBBWDBBS',
		'HBBDDBBBBBBDDBBS',
		'HBBDDBBBBBBDDBBS',
		'HBBBBBBPPBBBBBBS',
		'HBPBBBDBBDBBBPBS',
		'.SBBBBBDDBBBBBS.',
		'..SSBBBBBBBBSS..',
		'....SSSSSSSS....',
		'................',
	],
	nova: [
		'..............Y.',
		'.............Yy.',
		'......H.....Yy..',
		'.....MHB...Yy...',
		'....MMBBHHYy....',
		'...MMMBBBBBH....',
		'...MMMBBBBBBH...',
		'..MNMBBBBWDBBH..',
		'..MMMBBBBDDBBBH.',
		'.MNMMBBBBBBBBBBH',
		'.MMMBBBBBBBBBBBB',
		'MNMMBBBBBBBBBDBB',
		'MMMBBBBBSSBBBBBS',
		'MNMBBBBS..SSSSS.',
		'MMMBBBBS........',
		'MMBBBBBS........',
	],
}

const PAW_LAYERS: LogoLayer[] = [
	{
		d: 'M32 160H96V192H128V288H64V256H32ZM96 320H128V288H160V256H192V224H320V256H352V288H384V192H416V160H480V256H448V288H384V320H416V416H384V448H288V416H224V448H128V416H96ZM128 96H160V64H224V192H160V160H128ZM288 64H352V96H384V160H352V192H288Z',
		fill: BRAND,
	},
	{
		d: 'M32 160H96V192H128V224H96V192H32ZM96 320H128V352H96ZM128 96H160V64H224V96H160V128H128ZM128 288H160V256H192V288H160V320H128ZM192 224H320V256H352V288H384V320H352V288H320V256H192ZM288 64H352V96H384V128H352V96H288ZM384 192H416V224H384ZM384 320H416V352H384ZM416 160H480V192H416Z',
		fill: '#fff',
		opacity: 0.3,
	},
	{
		d: 'M32 224H64V256H32ZM64 256H128V288H64ZM96 384H128V416H224V384H288V416H384V448H288V416H224V448H128V416H96ZM128 128H160V160H224V192H160V160H128ZM288 160H352V192H288ZM352 128H384V160H352ZM384 256H448V288H384ZM384 384H416V416H384ZM448 224H480V256H448Z',
		fill: '#000',
		opacity: 0.2,
	},
]

const PAW_FRAME: LogoFrame = { x: 4, y: 4, scale: 0.375 }

export function isLogoId(value: unknown): value is LogoId {
	return typeof value === 'string' && (LOGO_IDS as readonly string[]).includes(value)
}

/** Logos this build doesn't know, such as one chosen in a newer build, show the paw. */
export function resolveLogo(value: unknown): LogoId {
	return isLogoId(value) ? value : 'paw'
}

function runsPath(grid: readonly string[], matches: (cell: string) => boolean): string {
	let d = ''
	grid.forEach((row, y) => {
		let x = 0
		while (x < row.length) {
			if (!matches(row[x])) {
				x++
				continue
			}
			const start = x
			while (x < row.length && matches(row[x])) x++
			d += `M${start * CELL} ${y * CELL}h${(x - start) * CELL}v${CELL}h${(start - x) * CELL}Z`
		}
	})
	return d
}

/** Turns a pixel grid into SVG layers on a 512-unit square, one layer per color. */
export function gridLayers(grid: readonly string[]): LogoLayer[] {
	const layers: LogoLayer[] = []
	const accent = runsPath(grid, (cell) => cell === 'B' || cell in SHADES)
	if (accent) layers.push({ d: accent, fill: BRAND })
	for (const [letter, shade] of Object.entries(SHADES)) {
		const d = runsPath(grid, (cell) => cell === letter)
		if (d) layers.push({ d, ...shade })
	}
	for (const [letter, fill] of Object.entries(SOLIDS)) {
		const d = runsPath(grid, (cell) => cell === letter)
		if (d) layers.push({ d, fill })
	}
	return layers
}

export function logoLayers(id: LogoId): LogoLayer[] {
	return id === 'paw' ? PAW_LAYERS : gridLayers(LOGO_GRIDS[id])
}

interface GridBounds {
	left: number
	top: number
	width: number
	height: number
}

export function gridBounds(grid: readonly string[]): GridBounds {
	let left = GRID_SIZE
	let right = -1
	let top = GRID_SIZE
	let bottom = -1
	grid.forEach((row, y) => {
		for (let x = 0; x < row.length; x++) {
			if (row[x] === '.') continue
			left = Math.min(left, x)
			right = Math.max(right, x)
			top = Math.min(top, y)
			bottom = Math.max(bottom, y)
		}
	})
	return { left, top, width: right - left + 1, height: bottom - top + 1 }
}

/** Sizes a character like the paw: centered, and no wider or taller than the paw is. */
export function logoFrame(id: LogoId): LogoFrame {
	if (id === 'paw') return PAW_FRAME
	const bounds = gridBounds(LOGO_GRIDS[id])
	const scale = Math.min(MARK_WIDTH / (bounds.width * CELL), MARK_HEIGHT / (bounds.height * CELL))
	const centerX = (bounds.left + bounds.width / 2) * CELL
	const centerY = (bounds.top + bounds.height / 2) * CELL
	return {
		x: MARK_SIZE / 2 - centerX * scale,
		y: MARK_SIZE / 2 - centerY * scale,
		scale,
	}
}

const ICON_ACCENT: [number, number, number] = [27, 217, 106]
const ICON_TOP: [number, number, number] = [34, 38, 45]
const ICON_BOTTOM: [number, number, number] = [21, 23, 27]

function hexColor(hex: string): [number, number, number] {
	const digits =
		hex.length === 4 ? [...hex.slice(1)].map((digit) => digit + digit).join('') : hex.slice(1)
	const value = Number.parseInt(digits, 16)
	return [(value >> 16) & 255, (value >> 8) & 255, value & 255]
}

function blend(
	base: [number, number, number],
	over: [number, number, number],
	amount: number,
): [number, number, number] {
	return [0, 1, 2].map((i) => Math.round(base[i] * (1 - amount) + over[i] * amount)) as [
		number,
		number,
		number,
	]
}

function iconCellColor(cell: string): [number, number, number] | null {
	if (cell === 'B') return ICON_ACCENT
	const shade = SHADES[cell]
	if (shade) return blend(ICON_ACCENT, hexColor(shade.fill), shade.opacity)
	const solid = SOLIDS[cell]
	return solid ? hexColor(solid) : null
}

/** How much of a pixel lies inside the icon's rounded square, from 0 to 1. */
function cornerCoverage(x: number, y: number, size: number, radius: number): number {
	const px = x + 0.5
	const py = y + 0.5
	const cx = Math.min(Math.max(px, radius), size - radius)
	const cy = Math.min(Math.max(py, radius), size - radius)
	const distance = Math.hypot(px - cx, py - cy)
	return Math.min(Math.max(radius - distance + 0.5, 0), 1)
}

/**
 * Draws a character as an app icon in the paw icon's colors: a dark rounded square
 * with the character's pixels kept sharp. Returns RGBA bytes, row by row.
 */
export function iconPixels(id: Exclude<LogoId, 'paw'>, size: number): Uint8Array {
	const grid = LOGO_GRIDS[id]
	const bounds = gridBounds(grid)
	const cellPx = Math.max(1, Math.floor((size * 0.75) / Math.max(bounds.width, bounds.height)))
	const offsetX = Math.round((size - bounds.width * cellPx) / 2) - bounds.left * cellPx
	const offsetY = Math.round((size - bounds.height * cellPx) / 2) - bounds.top * cellPx
	const radius = size * 0.21
	const pixels = new Uint8Array(size * size * 4)
	for (let y = 0; y < size; y++) {
		const background = blend(ICON_TOP, ICON_BOTTOM, y / Math.max(size - 1, 1))
		for (let x = 0; x < size; x++) {
			const coverage = cornerCoverage(x, y, size, radius)
			const gridX = Math.floor((x - offsetX) / cellPx)
			const gridY = Math.floor((y - offsetY) / cellPx)
			const cell = grid[gridY]?.[gridX] ?? '.'
			const color = iconCellColor(cell) ?? background
			const index = (y * size + x) * 4
			pixels[index] = color[0]
			pixels[index + 1] = color[1]
			pixels[index + 2] = color[2]
			pixels[index + 3] = Math.round(coverage * 255)
		}
	}
	return pixels
}

function readFirstPaintLogo(): LogoId {
	try {
		return resolveLogo(window.localStorage.getItem(FIRST_PAINT_KEY))
	} catch {
		return 'paw'
	}
}

/** The logo shown now, remembered so the loading screen shows it before settings load. */
export const currentLogo = ref<LogoId>(readFirstPaintLogo())

export function setCurrentLogo(id: LogoId): void {
	currentLogo.value = id
	try {
		if (id === 'paw') window.localStorage.removeItem(FIRST_PAINT_KEY)
		else window.localStorage.setItem(FIRST_PAINT_KEY, id)
	} catch {
		// storage blocked or full
	}
}
