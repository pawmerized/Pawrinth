import { defaultWindowIcon } from '@tauri-apps/api/app'
import { Image } from '@tauri-apps/api/image'
import { getCurrentWindow } from '@tauri-apps/api/window'

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
