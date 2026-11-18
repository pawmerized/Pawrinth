import { type InstalledIdentity, plus_installed_identity } from '@/helpers/plus'

/** Shorter names are too likely to belong to a different mod. */
const MIN_NAME_LENGTH = 3

/** "Just Enough Items (JEI)" and "just-enough-items-jei" both become "justenoughitemsjei". */
export function nameKey(text: string): string {
	return text.replace(/[^A-Za-z0-9]/g, '').toLowerCase()
}

/** Whether a project with any of these slugs or names is already in the instance. */
export function hasInstalledName(
	names: ReadonlySet<string>,
	...texts: (string | null | undefined)[]
): boolean {
	return texts.some((text) => {
		const key = text ? nameKey(text) : ''
		return key.length >= MIN_NAME_LENGTH && names.has(key)
	})
}

export const EMPTY_IDENTITY: InstalledIdentity = { curseforge_ids: [], names: [] }

/** What an instance has installed, or nothing when it can't be read. */
export async function loadInstalledIdentity(instanceId: string | null | undefined) {
	return instanceId
		? await plus_installed_identity(instanceId).catch(() => EMPTY_IDENTITY)
		: EMPTY_IDENTITY
}
