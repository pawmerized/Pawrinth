import { describe, expect, it } from 'vitest'

import { THEME_PRESETS } from '@/features/plus-themes/theme-presets'

import {
	ACCENT_THEME_ID,
	accentTheme,
	importSelections,
	nextStep,
	previousStep,
	withAccentTheme,
	wizardDecision,
} from './wizard-model'

describe('when the wizard appears', () => {
	it('shows only for a new user', () => {
		expect(wizardDecision({ completed: false, instanceCount: 0 })).toBe('show')
	})

	it('never interrupts someone who already has instances', () => {
		expect(wizardDecision({ completed: false, instanceCount: 3 })).toBe('mark-complete')
	})

	it('does not come back once finished or skipped', () => {
		expect(wizardDecision({ completed: true, instanceCount: 0 })).toBe('nothing')
		expect(wizardDecision({ completed: true, instanceCount: 3 })).toBe('nothing')
	})
})

describe('step navigation', () => {
	it('moves between steps and stops at both ends', () => {
		expect(nextStep('welcome')).toBe('appearance')
		expect(previousStep('appearance')).toBe('welcome')
		expect(previousStep('welcome')).toBe('welcome')
		expect(nextStep('finish')).toBe('finish')
	})
})

describe('accent color', () => {
	it('builds a usable theme from a readable accent', () => {
		const theme = accentTheme('dark', '#ff8800', 'Mine')
		expect(theme?.palette.accent).toBe('#ff8800')
		expect(theme?.base).toBe('dark')
	})

	it('refuses an accent that would be unreadable', () => {
		expect(accentTheme('dark', '#16181c', 'Mine')).toBeNull()
		expect(accentTheme('light', '#ffffff', 'Mine')).toBeNull()
	})

	it('refuses anything that is not a color', () => {
		expect(accentTheme('dark', 'red; } body { display: none', 'Mine')).toBeNull()
	})

	it('replaces only its own saved theme', () => {
		const first = accentTheme('dark', '#ff8800', 'Mine')!
		const second = accentTheme('dark', '#00aaff', 'Mine')!
		const themes = withAccentTheme(withAccentTheme([THEME_PRESETS[0]], first), second)
		expect(themes).toHaveLength(2)
		expect(themes[0].id).toBe(THEME_PRESETS[0].id)
		expect(themes.find((theme) => theme.id === ACCENT_THEME_ID)?.palette.accent).toBe('#00aaff')
	})
})

describe('importing existing instances', () => {
	const launchers = [
		{ name: 'PrismLauncher', path: 'C:/prism', instances: ['A', 'B'] },
		{ name: 'Curseforge', path: 'C:/cf', instances: ['A'] },
	]

	it('imports nothing unless chosen', () => {
		expect(importSelections(launchers, new Set())).toEqual([])
	})

	it('keeps same-named instances from different launchers apart', () => {
		expect(importSelections(launchers, new Set(['Curseforge/A']))).toEqual([
			{ launcher: 'Curseforge', path: 'C:/cf', instanceNames: ['A'] },
		])
	})
})
