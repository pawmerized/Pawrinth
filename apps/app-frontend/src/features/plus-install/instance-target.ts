const MOD_LOADERS = ['fabric', 'forge', 'quilt', 'neoforge']

export type InstanceTargetStatus = 'ready' | 'no-loader' | 'unknown-loader' | 'unknown-version'

export interface InstanceTarget {
	gameVersion: string | null
	/** Only ever a loader the instance actually says it has. */
	loader: string | null
	status: InstanceTargetStatus
}

function cleanText(value: unknown): string | null {
	if (typeof value !== 'string') return null
	const trimmed = value.trim()
	return trimmed.length > 0 ? trimmed : null
}

/**
 * Reads what an instance runs from its metadata. Anything missing or unrecognized is
 * reported as unknown, so the app can ask instead of guessing.
 */
export function readInstanceTarget(
	instance: { game_version?: unknown; loader?: unknown } | null | undefined,
): InstanceTarget {
	const gameVersion = cleanText(instance?.game_version)
	const declared = cleanText(instance?.loader)?.toLowerCase() ?? null
	if (declared === 'vanilla') {
		return { gameVersion, loader: null, status: gameVersion ? 'no-loader' : 'unknown-version' }
	}
	if (!declared || !MOD_LOADERS.includes(declared)) {
		return { gameVersion, loader: null, status: 'unknown-loader' }
	}
	return { gameVersion, loader: declared, status: gameVersion ? 'ready' : 'unknown-version' }
}
