import { describe, expect, it } from 'vitest'

import { hasInstalledName, nameKey } from './installed-names'

describe('installed names', () => {
	it('match however a site writes them', () => {
		expect(nameKey('Just Enough Items (JEI)')).toBe('justenoughitemsjei')
		const names = new Set(['fabricapi', 'jei'])
		expect(hasInstalledName(names, 'fabric-api', 'Fabric API')).toBe(true)
		expect(hasInstalledName(names, 'jei', 'Just Enough Items (JEI)')).toBe(true)
		expect(hasInstalledName(names, 'jeiaddon', 'JEI Addon')).toBe(false)
		expect(hasInstalledName(new Set(['fa']), 'fa', null)).toBe(false)
	})
})
