import { describe, expect, it } from 'vitest'

import type { LibraryMod } from '@/helpers/plus'

import { findSame } from './library-match'

function mod(source: LibraryMod['source'], project_id: string, name: string, slug: string) {
	return { source, project_id, name, slug, icon_url: null, auto_install: true }
}

describe('the mod library', () => {
	it('knows the same mod from the other site', () => {
		const library = [mod('modrinth', 'AANobbMI', 'Sodium', 'sodium')]
		expect(findSame(library, mod('curseforge', '394468', 'Sodium', 'sodium'))).toBe(library[0])
		expect(findSame(library, mod('modrinth', 'AANobbMI', 'Renamed', 'renamed'))).toBe(library[0])
		expect(findSame(library, mod('modrinth', 'gvQqBUqZ', 'Lithium', 'lithium'))).toBeNull()
	})
})
