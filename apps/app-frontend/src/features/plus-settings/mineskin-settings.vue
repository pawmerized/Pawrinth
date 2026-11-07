<script setup lang="ts">
import { ExternalIcon, TrashIcon } from '@modrinth/assets'
import {
	Admonition,
	Button,
	defineMessages,
	injectNotificationManager,
	Input,
	useVIntl,
} from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { onMounted, ref } from 'vue'

import {
	type MineskinKeyOutcome,
	type MineskinKeyStatus,
	plus_mineskin_key_remove,
	plus_mineskin_key_save,
	plus_mineskin_key_status,
} from '@/helpers/plus'

const ACCOUNT_URL = 'https://account.mineskin.org'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
	title: { id: 'plus.mineskin.settings.title', defaultMessage: 'MineSkin' },
	description: {
		id: 'plus.mineskin.settings.description',
		defaultMessage:
			"The skin gallery uses MineSkin. It works without a key, with stricter limits, but MineSkin's terms ask apps to use one, so you can add your own. It stays in your system's credential store and is only sent to MineSkin.",
	},
	saved: { id: 'plus.mineskin.settings.saved', defaultMessage: 'Saved key' },
	noKey: { id: 'plus.mineskin.settings.no-key', defaultMessage: 'No key saved. The gallery is using MineSkin without one.' },
	storeUnavailable: {
		id: 'plus.mineskin.settings.store-unavailable',
		defaultMessage: "Your system's credential store can't be reached, so a key can't be saved right now.",
	},
	placeholder: { id: 'plus.mineskin.settings.placeholder', defaultMessage: 'Paste your MineSkin API key' },
	save: { id: 'plus.mineskin.settings.save', defaultMessage: 'Save key' },
	remove: { id: 'plus.mineskin.settings.remove', defaultMessage: 'Remove key' },
	getKey: { id: 'plus.mineskin.settings.get-key', defaultMessage: 'Get a key from MineSkin' },
	accepted: { id: 'plus.mineskin.settings.accepted', defaultMessage: 'MineSkin accepted this key, and it is saved.' },
	rejected: { id: 'plus.mineskin.settings.rejected', defaultMessage: "MineSkin rejected this key, so it wasn't saved." },
	unchecked: {
		id: 'plus.mineskin.settings.unchecked',
		defaultMessage: "The key is saved, but MineSkin couldn't check it right now.",
	},
	malformed: {
		id: 'plus.mineskin.settings.malformed',
		defaultMessage: "That can't be an API key. Check it was copied whole.",
	},
})

const status = ref<MineskinKeyStatus | null>(null)
const draft = ref('')
const outcome = ref<MineskinKeyOutcome | null>(null)
const busy = ref(false)

async function refresh(): Promise<void> {
	status.value = await plus_mineskin_key_status()
}

async function saveKey(): Promise<void> {
	busy.value = true
	try {
		outcome.value = await plus_mineskin_key_save(draft.value)
		if (outcome.value.saved) draft.value = ''
		await refresh()
	} catch (error) {
		handleError(error as Error)
	} finally {
		busy.value = false
	}
}

async function removeKey(): Promise<void> {
	busy.value = true
	try {
		await plus_mineskin_key_remove()
		outcome.value = null
		await refresh()
	} catch (error) {
		handleError(error as Error)
	} finally {
		busy.value = false
	}
}

function outcomeText(result: MineskinKeyOutcome): string {
	if (result.check === 'malformed') return formatMessage(messages.malformed)
	if (!result.saved) return formatMessage(messages.rejected)
	return formatMessage(result.check === 'valid' ? messages.accepted : messages.unchecked)
}

onMounted(() => void refresh().catch(handleError))
</script>

<template>
	<section data-mineskin-settings>
		<h2 class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
		<p class="m-0 mt-1">{{ formatMessage(messages.description) }}</p>

		<Admonition v-if="status && !status.store_available" class="mt-3" type="warning">
			{{ formatMessage(messages.storeUnavailable) }}
		</Admonition>
		<template v-else-if="status">
			<div v-if="status.hint" class="mt-3 flex flex-wrap items-center justify-between gap-2">
				<div>
					<h3 class="m-0 text-base font-semibold text-contrast">{{ formatMessage(messages.saved) }}</h3>
					<p class="plus-private-stream m-0 mt-1 font-mono text-contrast">{{ status.hint }}</p>
				</div>
				<Button color="red" type="outlined" :disabled="busy" @click="removeKey">
					<TrashIcon aria-hidden="true" />
					{{ formatMessage(messages.remove) }}
				</Button>
			</div>
			<p v-else class="m-0 mt-3 text-secondary">{{ formatMessage(messages.noKey) }}</p>

			<form class="mt-3 flex flex-wrap items-center gap-2" @submit.prevent="saveKey">
				<Input
					v-model="draft"
					type="password"
					wrapper-class="grow"
					autocomplete="off"
					:spellcheck="false"
					:placeholder="formatMessage(messages.placeholder)"
				/>
				<Button color="brand" type="colored" native-type="submit" :disabled="busy || !draft.trim()">
					{{ formatMessage(messages.save) }}
				</Button>
			</form>
			<p v-if="outcome" class="m-0 mt-2 text-sm text-primary" role="status">{{ outcomeText(outcome) }}</p>
		</template>

		<Button class="mt-3" native-type="button" @click="openUrl(ACCOUNT_URL)">
			{{ formatMessage(messages.getKey) }}
			<ExternalIcon aria-hidden="true" />
		</Button>
	</section>
</template>
