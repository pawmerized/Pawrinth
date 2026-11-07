import { getCurrentWebview } from '@tauri-apps/api/webview'
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window'
import { readonly, ref, shallowRef, watch } from 'vue'

import { setThemeLibrary, startThemeEngine } from '@/features/plus-themes/theme-engine'
import {
	plus_settings_get,
	plus_settings_patch,
	type PlusSettings,
	type PlusSettingsPatch,
} from '@/helpers/plus'

const BASE_MIN_WIDTH = 1100
const BASE_MIN_HEIGHT = 700

const settings = shallowRef<PlusSettings | null>(null)
const loadFailed = ref(false)

function applyPrivacyClasses(value: PlusSettings['privacy']): void {
	const html = document.documentElement
	html.classList.toggle('plus-blur-names', value.streamer_mode || value.blur_usernames)
	html.classList.toggle('plus-streamer-mode', value.streamer_mode)
}

async function applyUiScale(scale: number): Promise<void> {
	await getCurrentWebview().setZoom(scale)
	await getCurrentWindow().setMinSize(
		new LogicalSize(BASE_MIN_WIDTH * Math.max(scale, 1), BASE_MIN_HEIGHT * Math.max(scale, 1)),
	)
}

function applyInterfaceClasses(value: PlusSettings['interface']): void {
	const html = document.documentElement
	html.classList.toggle('plus-reduce-motion', value.reduce_motion)
	html.classList.toggle('plus-density-compact', value.density === 'compact')
}

/** Loads the settings and applies them. Failures leave stock behavior in place. */
export async function initPlusSettings(): Promise<void> {
	startThemeEngine()
	try {
		const loaded = await plus_settings_get()
		settings.value = loaded
		loadFailed.value = false
		setThemeLibrary(loaded.appearance.themes, loaded.appearance.active_theme_id)
		applyInterfaceClasses(loaded.interface)
		applyPrivacyClasses(loaded.privacy)
		if (loaded.interface.ui_scale !== 1) {
			await applyUiScale(loaded.interface.ui_scale)
		}
	} catch (error) {
		loadFailed.value = true
		console.error('Failed to load Pawrinth settings', error)
	}
}

export async function patchPlusSettings(patch: PlusSettingsPatch): Promise<PlusSettings> {
	const updated = await plus_settings_patch(patch)
	settings.value = updated
	return updated
}

watch(settings, (value, previous) => {
	if (!value) return
	applyInterfaceClasses(value.interface)
	applyPrivacyClasses(value.privacy)
	if (previous && previous.interface.ui_scale !== value.interface.ui_scale) {
		void applyUiScale(value.interface.ui_scale).catch((error) => {
			console.error('Failed to apply UI scale', error)
		})
	}
})

export function usePlusSettings() {
	return { settings: readonly(settings), loadFailed: readonly(loadFailed) }
}
