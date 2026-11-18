import bundled from './gallery.json'
import { type CustomTheme, parseThemeImport } from './theme-model'

/** A shared theme, checked the same way as an imported theme file. */
export interface GalleryTheme {
	id: string
	author: string
	theme: Omit<CustomTheme, 'id'>
	/** The theme file, for the same preview and save steps as an import. */
	document: string
}

const MAX_THEMES = 200

/** Keeps only themes that pass every check an imported theme file has to pass. */
export function readGallery(value: unknown): GalleryTheme[] {
	const gallery = value as { format?: unknown; themes?: unknown } | null
	if (gallery?.format !== 'pawrinth-theme-gallery' || !Array.isArray(gallery.themes)) return []
	const seen = new Set<string>()
	const themes: GalleryTheme[] = []
	for (const entry of gallery.themes.slice(0, MAX_THEMES)) {
		const { id, author, theme } = (entry ?? {}) as Record<string, unknown>
		if (typeof id !== 'string' || !/^[a-z0-9-]{1,48}$/.test(id) || seen.has(id)) continue
		const document = JSON.stringify(theme ?? null)
		const parsed = parseThemeImport(document)
		if (!parsed.ok) continue
		seen.add(id)
		themes.push({
			id,
			author: typeof author === 'string' ? author.slice(0, 48) : '',
			theme: parsed.theme,
			document,
		})
	}
	return themes
}

/**
 * Themes made for the gallery, which ships with the app until the gallery has somewhere
 * to be shared from. Only a sample is shown, behind Coming soon.
 */
export const bundledGallery = readGallery(bundled)
