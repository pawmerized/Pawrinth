<script setup lang="ts">
import { Admonition, defineMessages, injectNotificationManager, Input, Toggle, useVIntl } from '@modrinth/ui'
import { useDebounceFn } from '@vueuse/core'
import { computed, ref, watch } from 'vue'

import { cleanDisplayName } from '@/features/plus-privacy/privacy-model'
import { usePrivacy } from '@/features/plus-privacy/use-privacy'

import { patchPlusSettings } from './use-plus-settings'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { privacy } = usePrivacy()

const messages = defineMessages({
	title: { id: 'plus.privacy.title', defaultMessage: 'Privacy and streaming' },
	description: {
		id: 'plus.privacy.description',
		defaultMessage: 'Keep your names and details off your screen while streaming or recording.',
	},
	streamerMode: { id: 'plus.privacy.streamer', defaultMessage: 'Streamer mode' },
	streamerModeText: {
		id: 'plus.privacy.streamer.text',
		defaultMessage:
			'Replaces your Minecraft and Modrinth names with "Hidden account" and hides your profile details. Blurs avatars, friends, screenshots and folder paths, and removes account names, your user folder and IP addresses from logs, errors and notifications. Discord only says "Playing Minecraft", and your CurseForge key hint is hidden too.',
	},
	blur: { id: 'plus.privacy.blur', defaultMessage: 'Blur usernames' },
	blurText: {
		id: 'plus.privacy.blur.text',
		defaultMessage: 'Blurs your Minecraft and Modrinth names wherever Pawrinth shows them.',
	},
	blurOnByStreamer: {
		id: 'plus.privacy.blur.streamer',
		defaultMessage: 'Streamer mode already does this.',
	},
	displayName: { id: 'plus.privacy.display-name', defaultMessage: 'Name to show instead' },
	displayNameText: {
		id: 'plus.privacy.display-name.text',
		defaultMessage: 'Shows this in place of your account names. Leave it empty to use the real ones.',
	},
	placeholder: { id: 'plus.privacy.display-name.placeholder', defaultMessage: 'Your real name is used' },
	limitsHeader: { id: 'plus.privacy.limits', defaultMessage: 'This only changes what you see here' },
	limitsText: {
		id: 'plus.privacy.limits.text',
		defaultMessage:
			"You stay signed in and your accounts aren't changed. The game, servers and other players still see your real Minecraft username, and it can still appear inside the game window, in chat and on the tab list. Hide those separately if you're streaming the game.",
	},
})

const nameDraft = ref(privacy.value.display_name ?? '')
const blurFromStreamer = computed(() => privacy.value.streamer_mode)

watch(
	() => privacy.value.display_name,
	(value) => {
		if ((value ?? '') !== nameDraft.value.trim()) nameDraft.value = value ?? ''
	},
)

async function update(change: Record<string, unknown>): Promise<void> {
	try {
		await patchPlusSettings({ privacy: change })
	} catch (error) {
		handleError(error as Error)
	}
}

const saveName = useDebounceFn((value: string) => {
	void update({ display_name: cleanDisplayName(value) })
}, 400)
</script>

<template>
	<section>
		<h2 class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
		<p class="m-0 mt-1">{{ formatMessage(messages.description) }}</p>

		<div class="mt-4 flex flex-col gap-6">
			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.streamerMode) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.streamerModeText) }}</p>
				</div>
				<Toggle
					id="plus-streamer-mode"
					:model-value="privacy.streamer_mode"
					class="shrink-0"
					@update:model-value="update({ streamer_mode: $event })"
				/>
			</div>

			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">{{ formatMessage(messages.blur) }}</h3>
					<p class="m-0 mt-1">
						{{ formatMessage(blurFromStreamer ? messages.blurOnByStreamer : messages.blurText) }}
					</p>
				</div>
				<Toggle
					id="plus-blur-usernames"
					:model-value="privacy.blur_usernames || blurFromStreamer"
					:disabled="blurFromStreamer"
					class="shrink-0"
					@update:model-value="update({ blur_usernames: $event })"
				/>
			</div>

			<div>
				<h3 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.displayName) }}
				</h3>
				<p class="m-0 mt-1">{{ formatMessage(messages.displayNameText) }}</p>
				<Input
					id="plus-display-name"
					v-model="nameDraft"
					class="mt-2 max-w-80"
					:maxlength="24"
					:spellcheck="false"
					autocomplete="off"
					:placeholder="formatMessage(messages.placeholder)"
					@update:model-value="saveName(String($event ?? ''))"
				/>
			</div>

			<Admonition type="info" :header="formatMessage(messages.limitsHeader)">
				{{ formatMessage(messages.limitsText) }}
			</Admonition>
		</div>
	</section>
</template>
