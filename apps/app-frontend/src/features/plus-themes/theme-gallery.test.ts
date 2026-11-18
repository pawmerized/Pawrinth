import { describe, expect, it } from 'vitest'

import gallery from './gallery.json'
import { readGallery } from './theme-gallery'
import { contrastIssues } from './theme-model'
import { THEME_PRESETS } from './theme-presets'

describe('the theme gallery', () => {
	it('ships only themes that pass every check, with no warnings', () => {
		const themes = readGallery(gallery)
		expect(themes).toHaveLength(gallery.themes.length)
		for (const { theme } of themes) {
			expect(contrastIssues(theme.palette), theme.name).toEqual([])
		}
	})

	it('has its own themes, not copies of the presets', () => {
		const presets = new Set(THEME_PRESETS.map((theme) => theme.name))
		expect(readGallery(gallery).filter(({ theme }) => presets.has(theme.name))).toEqual([])
	})

	it('drops entries that would fail an import', () => {
		const themes = readGallery({
			format: 'pawrinth-theme-gallery',
			themes: [
				{ id: 'Bad Id', theme: gallery.themes[0].theme },
				{ id: 'bad-colors', theme: { ...gallery.themes[0].theme, palette: { text: 'red' } } },
				{ id: 'fine', author: 'Someone', theme: gallery.themes[0].theme },
				{ id: 'fine', theme: gallery.themes[1].theme },
			],
		})
		expect(themes.map((theme) => theme.id)).toEqual(['fine'])
		expect(readGallery({ format: 'something-else', themes: [] })).toEqual([])
	})
})
