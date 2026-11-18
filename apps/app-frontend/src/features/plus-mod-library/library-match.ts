import { hasInstalledName, nameKey } from '@/features/plus-mods/installed-names'
import type { LibraryMod } from '@/helpers/plus'

/**
 * The library entry that's the same mod, from either site: the same project, or a
 * project with the same name or slug.
 */
export function findSame(mods: readonly LibraryMod[], mod: LibraryMod): LibraryMod | null {
	return (
		mods.find(
			(item) =>
				(item.source === mod.source && item.project_id === mod.project_id) ||
				hasInstalledName(
					new Set([nameKey(item.name), ...(item.slug ? [nameKey(item.slug)] : [])]),
					mod.slug,
					mod.name,
				),
		) ?? null
	)
}
