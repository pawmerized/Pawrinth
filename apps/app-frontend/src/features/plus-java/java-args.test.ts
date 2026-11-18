import { describe, expect, it } from 'vitest'

import {
	applyPreset,
	checkArgs,
	isBlocked,
	sizeInMb,
	splitArgs,
	suggestedPresets,
} from './java-args'

const java17 = { javaMajor: 17, memoryMb: 4096, loader: 'fabric' }

function kinds(args: string, context = java17) {
	return checkArgs(splitArgs(args), context).map((issue) => issue.kind)
}

describe('checking Java arguments', () => {
	it('accepts Minecraft launcher defaults on every Java', () => {
		const [preset] = suggestedPresets(java17)
		expect(checkArgs(preset.args, java17)).toEqual([])
		expect(checkArgs(preset.args, { ...java17, javaMajor: 8 })).toEqual([])
	})

	it('stops on what keeps the game from starting', () => {
		expect(kinds('4G')).toEqual(['not-an-option'])
		expect(kinds('-XX:+UseG1GC -XX:+UseZGC')).toContain('many-collectors')
		expect(kinds('-XX:G1NewSizePercent=20 -XX:+UnlockExperimentalVMOptions')).toEqual([
			'experimental',
		])
		expect(kinds('-Xms8G')).toEqual(['heap-too-large'])
		expect(kinds('-XX:+UseZGC -XX:+ZGenerational')).toEqual(['needs-java'])
		expect(isBlocked(checkArgs(['-XX:+UseZGC'], { ...java17, javaMajor: 11 }))).toBe(true)
	})

	it('fixes an experimental option by unlocking it first', () => {
		const [issue] = checkArgs(['-XX:G1NewSizePercent=20'], java17)
		expect(issue.fixed).toEqual(['-XX:+UnlockExperimentalVMOptions', '-XX:G1NewSizePercent=20'])
	})

	it('warns about options for another Java, collector or loader', () => {
		expect(kinds('-XX:MaxPermSize=256m')).toEqual(['removed'])
		expect(kinds('-XX:MaxPermSize=256m', { ...java17, javaMajor: 7 })).toEqual([])
		expect(kinds('-XX:+UseZGC -XX:G1HeapRegionSize=32M')).toEqual(['other-collector'])
		expect(kinds('-Dfml.ignorePatchDiscrepancies=true')).toEqual(['wrong-loader'])
		expect(kinds('-Dfabric.debug=true', { ...java17, loader: 'quilt' })).toEqual([])
		expect(kinds('-Xmx2G -Xmx4G')).toEqual(['memory-flag', 'duplicate', 'memory-flag'])
		expect(kinds('-XX:+UseZGC -XX:+ZGenerational', { ...java17, javaMajor: 23 })).toEqual([
			'default-already',
		])
	})

	it('reads memory sizes', () => {
		expect(sizeInMb('4G')).toBe(4096)
		expect(sizeInMb('512m')).toBe(512)
		expect(sizeInMb('four')).toBeNull()
	})
})

describe('suggestions', () => {
	it('only suggest ZGC on Java 21 with enough memory', () => {
		expect(suggestedPresets(java17).map((preset) => preset.id)).toEqual(['mojang-g1'])
		expect(
			suggestedPresets({ ...java17, javaMajor: 21, memoryMb: 2048 }).map((preset) => preset.id),
		).toEqual(['mojang-g1'])
		const zgc = suggestedPresets({ ...java17, javaMajor: 21 })[1]
		expect(zgc.args).toEqual(['-XX:+UseZGC', '-XX:+ZGenerational'])
		expect(suggestedPresets({ ...java17, javaMajor: 24 })[1].args).toEqual(['-XX:+UseZGC'])
	})

	it('replace the old collector settings and keep the rest', () => {
		const [g1, zgc] = suggestedPresets({ ...java17, javaMajor: 21 })
		const before = applyPreset(['-Dfoo=bar', '-Xss2M'], g1)
		expect(applyPreset(before, zgc)).toEqual([
			'-Dfoo=bar',
			'-Xss2M',
			'-XX:+UseZGC',
			'-XX:+ZGenerational',
		])
	})
})
