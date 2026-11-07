import { computed, ref, watch } from 'vue'

import { useTheme } from '@/composables/use-theme'

import { type CustomTheme, isValidTheme, themeStylesheet } from './theme-model'
import { THEME_PRESETS } from './theme-presets'

const STYLE_ELEMENT_ID = 'plus-theme-overlay'
const FIRST_PAINT_KEY = 'plus-theme-overlay'
const OVERLAY_CLASS = 'plus-custom'

const savedThemes = ref<CustomTheme[]>([])
const activeThemeId = ref<string | null>(null)
const previewTheme = ref<CustomTheme | null>(null)

export const allThemes = computed(() => [...THEME_PRESETS, ...savedThemes.value])

const selectedTheme = computed(
	() =>
		previewTheme.value ?? allThemes.value.find((theme) => theme.id === activeThemeId.value) ?? null,
)

function styleElement(): HTMLStyleElement {
	const existing = document.getElementById(STYLE_ELEMENT_ID)
	if (existing) return existing as HTMLStyleElement
	const element = document.createElement('style')
	element.id = STYLE_ELEMENT_ID
	document.head.appendChild(element)
	return element
}

function rememberForFirstPaint(css: string | null): void {
	try {
		if (css) window.localStorage.setItem(FIRST_PAINT_KEY, css)
		else window.localStorage.removeItem(FIRST_PAINT_KEY)
	} catch {
		// storage blocked or full
	}
}

/**
 * Applies the last overlay before the settings load, which avoids a flash of the
 * stock theme on startup. The stylesheet was generated from validated colors.
 */
export function applyFirstPaintOverlay(): void {
	try {
		const css = window.localStorage.getItem(FIRST_PAINT_KEY)
		if (css) {
			styleElement().textContent = css
			document.documentElement.classList.add(OVERLAY_CLASS)
		}
	} catch {
		// storage blocked or full
	}
}

let started = false

/**
 * The overlay is active only while the app theme matches the custom theme's base,
 * so `settings.theme` and account sync only ever carry stock theme names.
 */
export function startThemeEngine(): void {
	if (started) return
	started = true
	const appTheme = useTheme()

	watch(
		[selectedTheme, () => appTheme.active],
		([theme, active]) => {
			const html = document.documentElement
			if (!theme || theme.base !== active) {
				html.classList.remove(OVERLAY_CLASS)
				styleElement().textContent = ''
				if (!previewTheme.value) rememberForFirstPaint(null)
				return
			}
			const css = themeStylesheet(theme)
			styleElement().textContent = css
			html.classList.add(OVERLAY_CLASS)
			if (!previewTheme.value) rememberForFirstPaint(css)
		},
		{ immediate: true },
	)
}

export function setThemeLibrary(themes: unknown[], activeId: string | null): void {
	savedThemes.value = themes.filter(isValidTheme)
	activeThemeId.value = activeId
}

export function setThemePreview(theme: CustomTheme | null): void {
	previewTheme.value = theme
}

export function useThemeLibrary() {
	return { savedThemes, activeThemeId, previewTheme, selectedTheme, allThemes }
}
