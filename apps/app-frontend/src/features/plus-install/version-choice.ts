const STABILITY_ORDER = ['release', 'beta', 'alpha']

/** A stable release wins over anything experimental, however new. Expects newest first. */
export function preferStableVersion<T extends { version_type?: string }>(
	newestFirst: readonly T[],
): T | undefined {
	for (const versionType of STABILITY_ORDER) {
		const match = newestFirst.find((version) => version.version_type === versionType)
		if (match) return match
	}
	return newestFirst[0]
}
