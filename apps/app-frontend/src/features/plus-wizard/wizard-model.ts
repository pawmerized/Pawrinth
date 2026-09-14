import { type CustomTheme, isUsable, type ThemeBase } from '@/features/plus-themes/theme-model'
import { DARK_STARTER, LIGHT_STARTER } from '@/features/plus-themes/theme-presets'

export const WIZARD_STEPS = [
	'welcome',
	'appearance',
	'layout',
	'content',
	'features',
	'finish',
] as const
export type WizardStep = (typeof WIZARD_STEPS)[number]

export const ACCENT_THEME_ID = 'setup-accent'
export const DEFAULT_ACCENT = '#1bd96a'

/**
 * Anyone who already has instances isn't a new user, so the wizard never appears
 * unprompted for them. It stays available from settings.
 */
export function wizardDecision(state: {
	completed: boolean
	instanceCount: number
}): 'show' | 'mark-complete' | 'nothing' {
	if (state.completed) return 'nothing'
	return state.instanceCount === 0 ? 'show' : 'mark-complete'
}

export function nextStep(step: WizardStep): WizardStep {
	const index = WIZARD_STEPS.indexOf(step)
	return WIZARD_STEPS[Math.min(index + 1, WIZARD_STEPS.length - 1)]
}

export function previousStep(step: WizardStep): WizardStep {
	const index = WIZARD_STEPS.indexOf(step)
	return WIZARD_STEPS[Math.max(index - 1, 0)]
}

/** Only colors are replaced, so a chosen accent can't make the app unreadable. */
export function accentTheme(base: ThemeBase, accent: string, name: string): CustomTheme | null {
	const starter = base === 'dark' ? DARK_STARTER : LIGHT_STARTER
	const palette = { ...starter, accent, selection: accent }
	if (!/^#[0-9a-f]{6}$/i.test(accent) || !isUsable(palette)) return null
	return { id: ACCENT_THEME_ID, name, base, palette }
}

/** Adds or replaces the wizard's own theme, leaving every other saved theme alone. */
export function withAccentTheme(themes: CustomTheme[], theme: CustomTheme): CustomTheme[] {
	const others = themes.filter((saved) => saved.id !== ACCENT_THEME_ID)
	return [...others, theme]
}

export interface DetectedLauncher {
	name: string
	path: string
	instances: string[]
}

export interface ImportSelection {
	launcher: string
	path: string
	instanceNames: string[]
}

/** Keys look like `Launcher/Instance`. Nothing is selected unless the user chooses it. */
export function importSelections(
	launchers: DetectedLauncher[],
	selected: ReadonlySet<string>,
): ImportSelection[] {
	return launchers
		.map((launcher) => ({
			launcher: launcher.name,
			path: launcher.path,
			instanceNames: launcher.instances.filter((name) => selected.has(`${launcher.name}/${name}`)),
		}))
		.filter((selection) => selection.instanceNames.length > 0)
}
