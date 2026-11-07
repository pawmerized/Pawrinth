import { ref } from 'vue'

/** Goes up when the saved key changes, so open pages can reload with or without it. */
export const curseforgeKeyRevision = ref(0)

export function notifyCurseforgeKeyChanged(): void {
	curseforgeKeyRevision.value++
}
