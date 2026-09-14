import { useTheme } from '@/composables/use-theme'
import { patchPlusSettings } from '@/features/plus-settings/use-plus-settings'
import { get, set } from '@/helpers/settings'

import { setThemeLibrary } from './theme-engine'
import type { CustomTheme, ThemeBase } from './theme-model'

export type StockTheme = 'dark' | 'light' | 'oled'

export async function persistThemes(themes: CustomTheme[], activeId: string | null): Promise<void> {
	const updated = await patchPlusSettings({
		appearance: { themes, active_theme_id: activeId },
	})
	setThemeLibrary(updated.appearance.themes, updated.appearance.active_theme_id)
}

/** A custom theme only shows while the app theme matches its base. */
export async function matchAppTheme(base: ThemeBase): Promise<void> {
	const appTheme = useTheme()
	if (appTheme.preferred === base) return
	await set({ ...(await get()), theme: base })
	appTheme.preferred = base
}

export async function applyStockTheme(
	value: StockTheme,
	savedThemes: CustomTheme[],
): Promise<void> {
	const appTheme = useTheme()
	await set({ ...(await get()), theme: value })
	appTheme.preferred = value
	if (value !== 'light') appTheme.preferredDark = value
	await persistThemes(savedThemes, null)
}

export async function applyCustomTheme(
	theme: CustomTheme,
	savedThemes: CustomTheme[],
): Promise<void> {
	await matchAppTheme(theme.base)
	await persistThemes(savedThemes, theme.id)
}
