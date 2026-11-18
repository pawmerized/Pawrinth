import { patchPlusSettings } from '@/features/plus-settings/use-plus-settings'
import { type LibraryMod, plus_settings_get } from '@/helpers/plus'

import { findSame } from './library-match'

export type AddResult = { added: true } | { added: false; existing: LibraryMod }

/** Adds a mod to the library unless it already has the same mod. */
export async function addToLibrary(mod: LibraryMod): Promise<AddResult> {
	const { mod_library: library } = await plus_settings_get()
	const existing = findSame(library.mods, mod)
	if (existing) return { added: false, existing }
	await patchPlusSettings({ mod_library: { mods: [...library.mods, mod] } })
	return { added: true }
}

export async function saveLibrary(mods: LibraryMod[]): Promise<void> {
	await patchPlusSettings({ mod_library: { mods } })
}
