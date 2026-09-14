<template>
	<StreamerModeNotice v-if="streamerMode" />
	<AccountSocialSettings
		v-else
		ref="socialSettings"
		:get-blocked-users="get_blocked_users"
		:get-users="getUsers"
		:unblock-user="unblock_user"
	/>
</template>

<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { AccountSocialSettings, injectModrinthClient } from '@modrinth/ui'
import { onBeforeUnmount, onMounted, ref } from 'vue'

import StreamerModeNotice from '@/features/plus-privacy/streamer-mode-notice.vue'
import { usePrivacy } from '@/features/plus-privacy/use-privacy'
import { injectAppSettingsModal } from '@/features/settings/app-settings-modal/context'
import { get_blocked_users, unblock_user } from '@/helpers/users'

const client = injectModrinthClient()
const settingsModal = injectAppSettingsModal()
const { streamerMode } = usePrivacy()
const socialSettings = ref<InstanceType<typeof AccountSocialSettings> | null>(null)

onMounted(() => {
	settingsModal.registerUnsavedChangesController({
		hasChanges: () => socialSettings.value?.hasChanges ?? false,
		getOriginal: () => socialSettings.value?.originalState ?? {},
		getModified: () => socialSettings.value?.modifiedState ?? {},
		isSaving: () => socialSettings.value?.saving ?? false,
		reset: () => socialSettings.value?.reset(),
		save: () => socialSettings.value?.save(),
	})
})

onBeforeUnmount(() => {
	settingsModal.registerUnsavedChangesController(null)
})

function getUsers(userIds: string[]): Promise<Labrinth.Users.v2.User[]> {
	return client.labrinth.users_v2.getMultiple(userIds)
}
</script>
