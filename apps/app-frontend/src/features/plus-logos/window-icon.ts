import { defaultWindowIcon } from '@tauri-apps/api/app'
import { convertFileSrc } from '@tauri-apps/api/core'
import { Image } from '@tauri-apps/api/image'
import { getCurrentWindow } from '@tauri-apps/api/window'

import { cornerRadius, type CustomLogo } from './custom-logo'
import { iconPixels, type LogoId } from './logos'

const ICON_SIZE = 128

/** Shows the logo as the window's icon, which the taskbar uses unless the app is pinned. */
export async function applyWindowIcon(id: LogoId): Promise<void> {
	const icon =
		id === 'paw'
			? await defaultWindowIcon()
			: await Image.new(iconPixels(id, ICON_SIZE), ICON_SIZE, ICON_SIZE)
	if (!icon) return
	try {
		await getCurrentWindow().setIcon(icon)
	} finally {
		await icon.close()
	}
}

/** Draws a picture fitted into the icon square, in proportion, the way the title bar shows it. */
async function customIconPixels(logo: CustomLogo): Promise<Uint8Array> {
	const picture = new window.Image()
	picture.crossOrigin = 'anonymous'
	picture.src = convertFileSrc(logo.path)
	await picture.decode()
	const canvas = document.createElement('canvas')
	canvas.width = ICON_SIZE
	canvas.height = ICON_SIZE
	const context = canvas.getContext('2d')
	if (!context) throw new Error('no 2D canvas')
	const scale = Math.min(ICON_SIZE / picture.naturalWidth, ICON_SIZE / picture.naturalHeight)
	const width = picture.naturalWidth * scale
	const height = picture.naturalHeight * scale
	const x = (ICON_SIZE - width) / 2
	const y = (ICON_SIZE - height) / 2
	if (logo.rounded) {
		const radius = Number.parseFloat(cornerRadius({ width, height, rounded: true }))
		context.beginPath()
		context.roundRect(x, y, width, height, radius)
		context.clip()
	}
	context.drawImage(picture, x, y, width, height)
	return new Uint8Array(context.getImageData(0, 0, ICON_SIZE, ICON_SIZE).data)
}

/** Uses a custom logo as the window's icon, keeping Pawrinth's own when it can't be drawn. */
export async function applyCustomWindowIcon(logo: CustomLogo): Promise<void> {
	const icon = await customIconPixels(logo)
		.then((pixels) => Image.new(pixels, ICON_SIZE, ICON_SIZE))
		.catch(() => defaultWindowIcon())
	if (!icon) return
	try {
		await getCurrentWindow().setIcon(icon)
	} finally {
		await icon.close()
	}
}
