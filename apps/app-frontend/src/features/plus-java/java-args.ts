/**
 * Checks custom Java arguments against the Java version, memory and mod loader an
 * instance uses. Every rule here is about what that Java version does with the option,
 * so nothing is flagged or suggested just because it's popular.
 */

export type ArgSeverity = 'blocking' | 'warning' | 'info'

export type ArgIssueKind =
	| 'not-an-option'
	| 'quoted'
	| 'memory-flag'
	| 'heap-too-large'
	| 'duplicate'
	| 'many-collectors'
	| 'needs-java'
	| 'experimental'
	| 'removed'
	| 'default-already'
	| 'other-collector'
	| 'some-builds'
	| 'wrong-loader'
	| 'debug'
	| 'agent'
	| 'large-pages'
	| 'pre-touch'

export interface ArgIssue {
	arg: string
	kind: ArgIssueKind
	severity: ArgSeverity
	/** A Java version or a loader name the message names. */
	detail?: string
	/** The whole argument list with this fixed, when there's a safe fix. */
	fixed?: string[]
}

export interface ArgContext {
	/** The major version of the Java the instance runs on, when known. */
	javaMajor: number | null
	/** The memory the instance gets, in MB. */
	memoryMb: number | null
	loader: string
}

export type PresetId = 'mojang-g1' | 'generational-zgc'

export interface ArgPreset {
	id: PresetId
	args: string[]
}

/** Pawrinth passes arguments to Java split at spaces, with nothing else done to them. */
export function splitArgs(text: string): string[] {
	return text.trim().split(/\s+/).filter(Boolean)
}

const COLLECTORS: Record<string, string> = {
	UseG1GC: 'G1',
	UseZGC: 'ZGC',
	UseShenandoahGC: 'Shenandoah',
	UseParallelGC: 'Parallel',
	UseParallelOldGC: 'Parallel',
	UseSerialGC: 'Serial',
	UseConcMarkSweepGC: 'CMS',
}

/** Options HotSpot only accepts after `-XX:+UnlockExperimentalVMOptions`. */
const EXPERIMENTAL = new Set([
	'G1NewSizePercent',
	'G1MaxNewSizePercent',
	'G1MixedGCLiveThresholdPercent',
])

/** Options Java dropped, and the version that dropped them. */
const REMOVED: { test: (name: string, arg: string) => boolean; version: number }[] = [
	{ test: (name) => name === 'PermSize' || name === 'MaxPermSize', version: 8 },
	{ test: (_, arg) => arg === '-Xincgc', version: 9 },
	{ test: (_, arg) => arg === '-d64' || arg === '-d32', version: 10 },
	{ test: (name) => name === 'AggressiveOpts', version: 11 },
	{ test: (name) => name === 'UseConcMarkSweepGC' || name.startsWith('CMS'), version: 14 },
]

/** The option an argument sets, so the same option set twice can be found. */
export function optionName(arg: string): string | null {
	const xx = /^-XX:[+-]?([A-Za-z0-9_]+)/.exec(arg)
	if (xx) return xx[1]
	const property = /^-D([^=]+)/.exec(arg)
	if (property) return `D:${property[1]}`
	const size = /^-X(mx|ms|ss|mn)/.exec(arg)
	if (size) return `X${size[1]}`
	return null
}

/** `-Xms4G` and `-Xms4096m` both come to 4096. */
export function sizeInMb(value: string): number | null {
	const match = /^(\d+)([kKmMgGtT]?)$/.exec(value)
	if (!match) return null
	const amount = Number(match[1])
	switch (match[2].toLowerCase()) {
		case 'k':
			return amount / 1024
		case 'm':
			return amount
		case 'g':
			return amount * 1024
		case 't':
			return amount * 1024 * 1024
		default:
			return amount / (1024 * 1024)
	}
}

function enabledFlag(arg: string): string | null {
	return /^-XX:\+([A-Za-z0-9_]+)$/.exec(arg)?.[1] ?? null
}

function without(args: string[], index: number): string[] {
	return args.filter((_, position) => position !== index)
}

function selectedCollectors(args: string[]): string[] {
	const chosen = new Set<string>()
	for (const arg of args) {
		const flag = enabledFlag(arg)
		if (flag && COLLECTORS[flag]) chosen.add(COLLECTORS[flag])
	}
	return [...chosen]
}

function isG1Tuning(name: string): boolean {
	return name.startsWith('G1') || name === 'InitiatingHeapOccupancyPercent'
}

export function checkArgs(args: string[], context: ArgContext): ArgIssue[] {
	const issues: ArgIssue[] = []
	const java = context.javaMajor
	const loader = context.loader.toLowerCase()
	const collectors = selectedCollectors(args)
	const unlockAt = args.indexOf('-XX:+UnlockExperimentalVMOptions')
	const customMax = args
		.map((arg) => /^-Xmx(.+)$/.exec(arg)?.[1])
		.filter((value): value is string => !!value)
		.map(sizeInMb)
		.at(-1)
	const maxMb = customMax ?? context.memoryMb
	const seen = new Map<string, number>()

	args.forEach((arg, index) => {
		const add = (issue: Omit<ArgIssue, 'arg'>) => issues.push({ arg, ...issue })
		const remove = () => without(args, index)

		if (!arg.startsWith('-')) {
			add({ kind: 'not-an-option', severity: 'blocking', fixed: remove() })
			return
		}
		if (/["']/.test(arg)) add({ kind: 'quoted', severity: 'warning' })

		const name = optionName(arg)
		if (name) {
			const earlier = seen.get(name)
			if (earlier !== undefined) {
				add({ kind: 'duplicate', severity: 'warning', fixed: without(args, earlier) })
			}
			seen.set(name, index)
		}

		if (arg.startsWith('-Xmx')) {
			add({ kind: 'memory-flag', severity: 'warning', fixed: remove() })
		}
		const initial = /^-Xms(.+)$/.exec(arg)?.[1]
		const initialMb = initial ? sizeInMb(initial) : null
		if (initialMb !== null && maxMb != null && initialMb > maxMb) {
			add({ kind: 'heap-too-large', severity: 'blocking', fixed: remove() })
		}

		const xxName = /^-XX:[+-]?([A-Za-z0-9_]+)/.exec(arg)?.[1] ?? null
		if (java !== null) {
			const removed = REMOVED.find((rule) => rule.test(xxName ?? '', arg))
			if (removed && java >= removed.version) {
				add({
					kind: 'removed',
					severity: 'warning',
					detail: String(removed.version),
					fixed: remove(),
				})
			}
		}

		if (xxName === 'UseZGC' && enabledFlag(arg) && java !== null && java < 15) {
			add({ kind: 'needs-java', severity: 'blocking', detail: '15', fixed: remove() })
		}
		if (xxName === 'ZGenerational' && java !== null) {
			if (java < 21) {
				add({ kind: 'needs-java', severity: 'blocking', detail: '21', fixed: remove() })
			} else if (java >= 23) {
				add({ kind: 'default-already', severity: 'info', detail: '23', fixed: remove() })
			}
		}
		if (xxName === 'UseShenandoahGC' && enabledFlag(arg)) {
			add({ kind: 'some-builds', severity: 'warning' })
		}

		if (xxName && EXPERIMENTAL.has(xxName) && (unlockAt === -1 || unlockAt > index)) {
			const fixed = args.filter((other) => other !== '-XX:+UnlockExperimentalVMOptions')
			fixed.splice(fixed.indexOf(arg), 0, '-XX:+UnlockExperimentalVMOptions')
			add({ kind: 'experimental', severity: 'blocking', fixed })
		}

		if (
			xxName &&
			isG1Tuning(xxName) &&
			collectors.length === 1 &&
			collectors[0] !== 'G1' &&
			collectors[0] !== 'CMS'
		) {
			add({
				kind: 'other-collector',
				severity: 'warning',
				detail: collectors[0],
				fixed: remove(),
			})
		}

		const property = /^-D([^=]+)/.exec(arg)?.[1]?.toLowerCase()
		if (property) {
			const forFabric = property.startsWith('fabric.') || property.startsWith('fabric-')
			const forQuilt = property.startsWith('quilt.') || property.startsWith('loader.')
			const forForge = property.startsWith('fml.') || property.startsWith('forge.')
			if (forFabric && loader !== 'fabric' && loader !== 'quilt') {
				add({ kind: 'wrong-loader', severity: 'warning', detail: 'Fabric', fixed: remove() })
			} else if (forQuilt && loader !== 'quilt') {
				add({ kind: 'wrong-loader', severity: 'warning', detail: 'Quilt', fixed: remove() })
			} else if (forForge && loader !== 'forge' && loader !== 'neoforge') {
				add({ kind: 'wrong-loader', severity: 'warning', detail: 'Forge', fixed: remove() })
			}
			if (property.startsWith('mixin.debug')) {
				add({ kind: 'debug', severity: 'warning', fixed: remove() })
			}
		}
		if (arg.startsWith('-javaagent:')) add({ kind: 'agent', severity: 'info' })
		if (xxName === 'UseLargePages' && enabledFlag(arg)) {
			add({ kind: 'large-pages', severity: 'info' })
		}
		if (xxName === 'AlwaysPreTouch' && enabledFlag(arg)) {
			add({ kind: 'pre-touch', severity: 'info', fixed: remove() })
		}
	})

	if (collectors.length > 1) {
		issues.unshift({
			arg: collectors.join(' + '),
			kind: 'many-collectors',
			severity: 'blocking',
		})
	}
	const order: Record<ArgSeverity, number> = { blocking: 0, warning: 1, info: 2 }
	return issues.sort((a, b) => order[a.severity] - order[b.severity])
}

export function isBlocked(issues: ArgIssue[]): boolean {
	return issues.some((issue) => issue.severity === 'blocking')
}

const MOJANG_G1 = [
	'-XX:+UnlockExperimentalVMOptions',
	'-XX:+UseG1GC',
	'-XX:G1NewSizePercent=20',
	'-XX:G1ReservePercent=20',
	'-XX:MaxGCPauseMillis=50',
	'-XX:G1HeapRegionSize=32M',
]

/** ZGC needs room above what the game uses, so it isn't suggested with less. */
const ZGC_MIN_MEMORY_MB = 4096

/**
 * Suggestions for this Java version and memory. Minecraft's own launcher settings work
 * on every Java Minecraft runs on; generational ZGC needs Java 21 and enough memory.
 */
export function suggestedPresets(context: ArgContext): ArgPreset[] {
	const presets: ArgPreset[] = [{ id: 'mojang-g1', args: MOJANG_G1 }]
	const java = context.javaMajor
	if (java !== null && java >= 21 && (context.memoryMb ?? 0) >= ZGC_MIN_MEMORY_MB) {
		presets.push({
			id: 'generational-zgc',
			args: java < 23 ? ['-XX:+UseZGC', '-XX:+ZGenerational'] : ['-XX:+UseZGC'],
		})
	}
	return presets
}

/**
 * The arguments with a preset in place of the collector settings they had. Everything
 * else, such as properties and stack size, is kept.
 */
export function applyPreset(args: string[], preset: ArgPreset): string[] {
	const kept = args.filter((arg) => {
		const name = /^-XX:[+-]?([A-Za-z0-9_]+)/.exec(arg)?.[1]
		return !(
			name &&
			(COLLECTORS[name] ||
				isG1Tuning(name) ||
				name === 'MaxGCPauseMillis' ||
				name === 'ZGenerational' ||
				name === 'UnlockExperimentalVMOptions')
		)
	})
	return [...kept, ...preset.args]
}

export function usesPreset(args: string[], preset: ArgPreset): boolean {
	return preset.args.every((arg) => args.includes(arg))
}
