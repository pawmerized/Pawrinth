import { shallowRef } from 'vue'

import { type PackUpdate, plus_pack_updates } from '@/helpers/plus'

const ASK_AGAIN_AFTER_MS = 60_000

/** Instances whose modpack has a newer version, by instance id. */
export const packUpdates = shallowRef(new Map<string, PackUpdate>())

let asked = 0

/**
 * Finds out which modpacks have updates. Asking again within a minute does nothing unless
 * `force` is set, which also makes the sites be asked again. Failures keep what was known.
 */
export async function refreshPackUpdates(force = false): Promise<void> {
	if (!force && Date.now() - asked < ASK_AGAIN_AFTER_MS) return
	asked = Date.now()
	try {
		const updates = await plus_pack_updates(force)
		packUpdates.value = new Map(updates.map((update) => [update.instance_id, update]))
	} catch {
		// offline, or a site is down
	}
}
