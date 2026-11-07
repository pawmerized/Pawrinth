import { ref } from 'vue'

/** Counts requests, so settings can reopen the wizard from anywhere. */
export const wizardOpenRequests = ref(0)

export function openSetupWizard(): void {
	wizardOpenRequests.value++
}
