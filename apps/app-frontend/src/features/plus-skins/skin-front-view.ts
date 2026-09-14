/** How many canvas pixels stand for one skin pixel. */
export const THUMBNAIL_SCALE = 8
export const THUMBNAIL_WIDTH = 16 * THUMBNAIL_SCALE
export const THUMBNAIL_HEIGHT = 32 * THUMBNAIL_SCALE

const LEGACY_SKIN_HEIGHT = 32
const CLASSIC_ARM_WIDTH = 4
const SLIM_ARM_WIDTH = 3
const SLIM_CHECK_COLUMN = 54
const SLIM_CHECK_ROW = 20
const SLIM_CHECK_HEIGHT = 12
const OPAQUE_ALPHA_INDEX = 3
const RGBA_CHANNELS = 4

interface Part {
	x: number
	y: number
	width: number
	height: number
	toX: number
	toY: number
	mirrored?: boolean
}

/** Slim skins leave the last column of each arm empty. */
export function isSlimTexture(pixels: Uint8ClampedArray): boolean {
	for (let index = OPAQUE_ALPHA_INDEX; index < pixels.length; index += RGBA_CHANNELS) {
		if (pixels[index] !== 0) return false
	}
	return true
}

/** The front of each body part, base layer first, then the outer layer over it. */
export function frontViewParts(textureHeight: number, slim: boolean): Part[] {
	const legacy = textureHeight <= LEGACY_SKIN_HEIGHT
	const arm = slim && !legacy ? SLIM_ARM_WIDTH : CLASSIC_ARM_WIDTH
	const rightArmX = CLASSIC_ARM_WIDTH - arm
	const parts: Part[] = [
		{ x: 8, y: 8, width: 8, height: 8, toX: 4, toY: 0 },
		{ x: 20, y: 20, width: 8, height: 12, toX: 4, toY: 8 },
		{ x: 44, y: 20, width: arm, height: 12, toX: rightArmX, toY: 8 },
		{ x: 4, y: 20, width: 4, height: 12, toX: 4, toY: 20 },
	]
	if (legacy) {
		parts.push(
			{ x: 44, y: 20, width: arm, height: 12, toX: 12, toY: 8, mirrored: true },
			{ x: 4, y: 20, width: 4, height: 12, toX: 8, toY: 20, mirrored: true },
			{ x: 40, y: 8, width: 8, height: 8, toX: 4, toY: 0 },
		)
		return parts
	}
	parts.push(
		{ x: 36, y: 52, width: arm, height: 12, toX: 12, toY: 8 },
		{ x: 20, y: 52, width: 4, height: 12, toX: 8, toY: 20 },
		{ x: 40, y: 8, width: 8, height: 8, toX: 4, toY: 0 },
		{ x: 20, y: 36, width: 8, height: 12, toX: 4, toY: 8 },
		{ x: 44, y: 36, width: arm, height: 12, toX: rightArmX, toY: 8 },
		{ x: 52, y: 52, width: arm, height: 12, toX: 12, toY: 8 },
		{ x: 4, y: 36, width: 4, height: 12, toX: 4, toY: 20 },
		{ x: 4, y: 52, width: 4, height: 12, toX: 8, toY: 20 },
	)
	return parts
}

export function drawFrontView(target: CanvasRenderingContext2D, image: HTMLImageElement): void {
	const reader = document.createElement('canvas')
	reader.width = image.naturalWidth
	reader.height = image.naturalHeight
	const source = reader.getContext('2d', { willReadFrequently: true })
	if (!source) throw new Error('Failed to create canvas rendering context.')
	source.drawImage(image, 0, 0)
	const slim = isSlimTexture(
		source.getImageData(SLIM_CHECK_COLUMN, SLIM_CHECK_ROW, 2, SLIM_CHECK_HEIGHT).data,
	)

	target.clearRect(0, 0, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT)
	target.imageSmoothingEnabled = false
	for (const part of frontViewParts(image.naturalHeight, slim)) {
		target.save()
		if (part.mirrored) {
			target.translate((part.toX + part.width) * THUMBNAIL_SCALE, part.toY * THUMBNAIL_SCALE)
			target.scale(-1, 1)
		} else {
			target.translate(part.toX * THUMBNAIL_SCALE, part.toY * THUMBNAIL_SCALE)
		}
		target.drawImage(
			image,
			part.x,
			part.y,
			part.width,
			part.height,
			0,
			0,
			part.width * THUMBNAIL_SCALE,
			part.height * THUMBNAIL_SCALE,
		)
		target.restore()
	}
}
