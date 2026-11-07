<script setup lang="ts">
import { ExternalIcon, EyeIcon, EyeOffIcon, RefreshCwIcon, TrashIcon } from '@modrinth/assets'
import {
	Admonition,
	Button,
	defineMessages,
	IconButton,
	injectNotificationManager,
	Input,
	useVIntl,
} from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import { notifyCurseforgeKeyChanged } from '@/features/plus-curseforge/key-state'
import MineskinSettings from '@/features/plus-settings/mineskin-settings.vue'
import {
	type CfKeyOutcome,
	type CfKeyStatus,
	plus_cf_key_remove,
	plus_cf_key_save,
	plus_cf_key_status,
	plus_cf_key_test,
} from '@/helpers/plus'

const CONSOLE_URL = 'https://console.curseforge.com/'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
	title: { id: 'plus.curseforge.title', defaultMessage: 'CurseForge' },
	description: {
		id: 'plus.curseforge.description',
		defaultMessage:
			'Searching CurseForge, installing from it and checking for updates all need an API key from CurseForge. Pawrinth keeps yours in your system’s credential store, which on Windows is Credential Manager. It’s only ever sent to CurseForge.',
	},
	connectedHeader: { id: 'plus.curseforge.live', defaultMessage: 'Connected to CurseForge' },
	connectedText: {
		id: 'plus.curseforge.live.text',
		defaultMessage: 'Project search, downloads and updates come from the official API.',
	},
	noKeyHeader: { id: 'plus.curseforge.no-key', defaultMessage: 'No API key saved' },
	noKeyText: {
		id: 'plus.curseforge.no-key.text',
		defaultMessage: 'The CurseForge tab can’t show any projects until you add a key.',
	},
	rejectedHeader: {
		id: 'plus.curseforge.saved-rejected',
		defaultMessage: 'CurseForge rejected the saved key',
	},
	rejectedText: {
		id: 'plus.curseforge.saved-rejected.text',
		defaultMessage: 'It may have been revoked. Replace it to use CurseForge again.',
	},
	uncheckedHeader: { id: 'plus.curseforge.unchecked', defaultMessage: 'Key saved but not checked' },
	uncheckedText: {
		id: 'plus.curseforge.unchecked.text',
		defaultMessage: 'CurseForge couldn’t be reached just now. Test the connection to try again.',
	},
	developmentHeader: {
		id: 'plus.curseforge.development',
		defaultMessage: 'Using a development key',
	},
	developmentText: {
		id: 'plus.curseforge.development.text',
		defaultMessage:
			'This development build found a key in CURSEFORGE_API_KEY. Release builds ignore that variable. A key saved here takes priority.',
	},
	storeHeader: {
		id: 'plus.curseforge.store-unavailable',
		defaultMessage: 'Your credential store isn’t available',
	},
	storeText: {
		id: 'plus.curseforge.store-unavailable.text',
		defaultMessage:
			'Keys are only ever kept in your system’s credential store, so one can’t be saved right now.',
	},
	keyLabel: { id: 'plus.curseforge.key.label', defaultMessage: 'API key' },
	savedKey: { id: 'plus.curseforge.key.saved', defaultMessage: 'Saved key' },
	placeholder: { id: 'plus.curseforge.key.placeholder', defaultMessage: 'Paste your API key' },
	show: { id: 'plus.curseforge.key.show', defaultMessage: 'Show key' },
	hide: { id: 'plus.curseforge.key.hide', defaultMessage: 'Hide key' },
	save: { id: 'plus.curseforge.key.save', defaultMessage: 'Save key' },
	test: { id: 'plus.curseforge.key.test', defaultMessage: 'Test connection' },
	replace: { id: 'plus.curseforge.key.replace', defaultMessage: 'Replace' },
	remove: { id: 'plus.curseforge.key.remove', defaultMessage: 'Remove' },
	cancel: { id: 'plus.curseforge.key.cancel', defaultMessage: 'Cancel' },
	savedHeader: { id: 'plus.curseforge.result.saved', defaultMessage: 'Key saved' },
	savedText: {
		id: 'plus.curseforge.result.saved.text',
		defaultMessage: 'CurseForge accepted it. The CurseForge tab now shows real projects.',
	},
	savedUncheckedText: {
		id: 'plus.curseforge.result.saved-unchecked.text',
		defaultMessage: 'It’s saved, but CurseForge couldn’t confirm it just now.',
	},
	removedHeader: { id: 'plus.curseforge.result.removed', defaultMessage: 'Key removed' },
	removedText: {
		id: 'plus.curseforge.result.removed.text',
		defaultMessage: 'It’s been deleted from your credential store.',
	},
	worksHeader: { id: 'plus.curseforge.result.works', defaultMessage: 'This key works' },
	worksText: {
		id: 'plus.curseforge.result.works.text',
		defaultMessage: 'Save it to start using it.',
	},
	connectedResultText: {
		id: 'plus.curseforge.result.connected.text',
		defaultMessage: 'CurseForge accepted the saved key.',
	},
	refusedHeader: {
		id: 'plus.curseforge.result.refused',
		defaultMessage: 'CurseForge rejected this key',
	},
	refusedText: {
		id: 'plus.curseforge.result.refused.text',
		defaultMessage: 'Check that you copied all of it. A rejected key is never saved.',
	},
	refusedCutOffText: {
		id: 'plus.curseforge.result.refused-cut-off.text',
		defaultMessage:
			'Keys normally start with $2a$, and this one doesn’t, so the beginning may have been cut off when it was copied. A rejected key is never saved.',
	},
	refusedSavedText: {
		id: 'plus.curseforge.result.refused-saved.text',
		defaultMessage: 'It may have been revoked. Replace it to use CurseForge again.',
	},
	malformedHeader: {
		id: 'plus.curseforge.result.malformed',
		defaultMessage: 'That doesn’t look like an API key',
	},
	malformedText: {
		id: 'plus.curseforge.result.malformed.text',
		defaultMessage: 'Keys have no spaces and are at most 256 characters long.',
	},
	limitedHeader: {
		id: 'plus.curseforge.result.limited',
		defaultMessage: 'CurseForge is limiting requests',
	},
	limitedText: {
		id: 'plus.curseforge.result.limited.text',
		defaultMessage: 'Too many requests were sent. Try again in a few minutes.',
	},
	serviceHeader: {
		id: 'plus.curseforge.result.service',
		defaultMessage: 'CurseForge had a problem',
	},
	serviceText: {
		id: 'plus.curseforge.result.service.text',
		defaultMessage: 'This isn’t caused by your key. Try again later.',
	},
	unreachableHeader: {
		id: 'plus.curseforge.result.unreachable',
		defaultMessage: 'Couldn’t reach CurseForge',
	},
	unreachableText: {
		id: 'plus.curseforge.result.unreachable.text',
		defaultMessage: 'Check your internet connection and try again.',
	},
	gettingKey: { id: 'plus.curseforge.getting-key', defaultMessage: 'Getting a key' },
	gettingKeyText: {
		id: 'plus.curseforge.getting-key.text',
		defaultMessage:
			'CurseForge issues keys through its developer console. A key is tied to you, and CurseForge’s terms don’t allow sharing it.',
	},
	openConsole: { id: 'plus.curseforge.getting-key.open', defaultMessage: 'Open CurseForge console' },
	rules: { id: 'plus.curseforge.rules', defaultMessage: 'What to expect' },
	rulesText: {
		id: 'plus.curseforge.rules.text',
		defaultMessage:
			"Some authors don't allow downloads through other apps; those projects link to the CurseForge website instead.",
	},
})

type ResultType = 'success' | 'warning' | 'critical'
type Result = { type: ResultType; header: string; text: string }

const status = ref<CfKeyStatus | null>(null)
const draft = ref('')
const revealed = ref(false)
const replacing = ref(false)
const busy = ref(false)
const result = ref<Result | null>(null)

const hasSavedKey = computed(() => status.value?.source === 'user')
const showForm = computed(() => !hasSavedKey.value || replacing.value)
const canUseDraft = computed(() => draft.value.trim().length > 0 && !busy.value)
const storeAvailable = computed(() => status.value?.store_available !== false)

const summary = computed<Result | null>(() => {
	const current = status.value
	if (!current) return null
	if (!current.store_available) {
		return {
			type: 'critical',
			header: formatMessage(messages.storeHeader),
			text: formatMessage(messages.storeText),
		}
	}
	if (current.source === null) return null
	if (current.check === 'rejected') {
		return {
			type: 'critical',
			header: formatMessage(messages.rejectedHeader),
			text: formatMessage(messages.rejectedText),
		}
	}
	if (current.check !== 'valid') {
		return {
			type: 'warning',
			header: formatMessage(messages.uncheckedHeader),
			text: formatMessage(messages.uncheckedText),
		}
	}
	return {
		type: 'success',
		header: formatMessage(messages.connectedHeader),
		text: formatMessage(messages.connectedText),
	}
})

function clearDraft(): void {
	draft.value = ''
	revealed.value = false
}

function describeProblem(outcome: CfKeyOutcome): Result | null {
	switch (outcome.check) {
		case 'malformed':
			return {
				type: 'critical',
				header: formatMessage(messages.malformedHeader),
				text: formatMessage(messages.malformedText),
			}
		case 'rate_limited':
			return {
				type: 'warning',
				header: formatMessage(messages.limitedHeader),
				text: formatMessage(messages.limitedText),
			}
		case 'service_error':
			return {
				type: 'warning',
				header: formatMessage(messages.serviceHeader),
				text: formatMessage(messages.serviceText),
			}
		case 'unreachable':
			return {
				type: 'warning',
				header: formatMessage(messages.unreachableHeader),
				text: formatMessage(messages.unreachableText),
			}
		default:
			return null
	}
}

function describeRefusal(outcome: CfKeyOutcome, savedKey: boolean): Result {
	const text = savedKey
		? messages.refusedSavedText
		: outcome.unusual_format
			? messages.refusedCutOffText
			: messages.refusedText
	return {
		type: 'critical',
		header: formatMessage(messages.refusedHeader),
		text: formatMessage(text),
	}
}

async function refresh(): Promise<void> {
	status.value = await plus_cf_key_status()
}

async function run(action: () => Promise<void>): Promise<void> {
	busy.value = true
	result.value = null
	try {
		await action()
	} catch (error) {
		handleError(error as Error)
	} finally {
		busy.value = false
	}
}

function save(): Promise<void> {
	return run(async () => {
		const outcome = await plus_cf_key_save(draft.value)
		if (!outcome.saved) {
			result.value =
				outcome.check === 'rejected' ? describeRefusal(outcome, false) : describeProblem(outcome)
			return
		}
		clearDraft()
		replacing.value = false
		await refresh()
		notifyCurseforgeKeyChanged()
		result.value =
			outcome.check === 'valid'
				? {
						type: 'success',
						header: formatMessage(messages.savedHeader),
						text: formatMessage(messages.savedText),
					}
				: {
						type: 'warning',
						header: formatMessage(messages.savedHeader),
						text: `${formatMessage(messages.savedUncheckedText)} ${describeProblem(outcome)?.text ?? ''}`,
					}
	})
}

function testDraft(): Promise<void> {
	return run(async () => {
		const outcome = await plus_cf_key_test(draft.value)
		if (outcome.check === 'valid') {
			result.value = {
				type: 'success',
				header: formatMessage(messages.worksHeader),
				text: formatMessage(messages.worksText),
			}
		} else if (outcome.check === 'rejected') {
			result.value = describeRefusal(outcome, false)
		} else {
			result.value = describeProblem(outcome)
		}
	})
}

function testSaved(): Promise<void> {
	return run(async () => {
		const outcome = await plus_cf_key_test(null)
		await refresh()
		notifyCurseforgeKeyChanged()
		if (outcome.check === 'valid') {
			result.value = {
				type: 'success',
				header: formatMessage(messages.connectedHeader),
				text: formatMessage(messages.connectedResultText),
			}
		} else if (outcome.check === 'rejected') {
			result.value = describeRefusal(outcome, true)
		} else {
			result.value = describeProblem(outcome)
		}
	})
}

function remove(): Promise<void> {
	return run(async () => {
		await plus_cf_key_remove()
		clearDraft()
		replacing.value = false
		await refresh()
		notifyCurseforgeKeyChanged()
		result.value = {
			type: 'success',
			header: formatMessage(messages.removedHeader),
			text: formatMessage(messages.removedText),
		}
	})
}

function startReplacing(): void {
	result.value = null
	replacing.value = true
}

function cancelReplacing(): void {
	result.value = null
	clearDraft()
	replacing.value = false
}

onMounted(() => run(refresh))
onBeforeUnmount(clearDraft)
</script>

<template>
	<section>
		<h2 class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
		<p class="m-0 mt-1">{{ formatMessage(messages.description) }}</p>

		<Admonition
			v-if="summary && !result"
			class="mt-4"
			:type="summary.type"
			:header="summary.header"
		>
			{{ summary.text }}
		</Admonition>
		<Admonition
			v-else-if="status && status.source === null && !result && storeAvailable"
			class="mt-4"
			type="info"
			:header="formatMessage(messages.noKeyHeader)"
		>
			{{ formatMessage(messages.noKeyText) }}
		</Admonition>
		<Admonition v-if="result" class="mt-4" :type="result.type" :header="result.header">
			{{ result.text }}
		</Admonition>
		<Admonition
			v-if="status?.source === 'development'"
			class="mt-4"
			type="info"
			:header="formatMessage(messages.developmentHeader)"
		>
			{{ formatMessage(messages.developmentText) }}
		</Admonition>

		<div v-if="status && hasSavedKey && !replacing" class="mt-4">
			<h3 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.savedKey) }}
			</h3>
			<p class="plus-private-stream m-0 mt-1 font-mono text-contrast">{{ status.hint }}</p>
			<div class="mt-3 flex flex-wrap gap-2">
				<Button :disabled="busy" @click="testSaved">
					<RefreshCwIcon aria-hidden="true" />
					{{ formatMessage(messages.test) }}
				</Button>
				<Button :disabled="busy || !storeAvailable" @click="startReplacing">
					{{ formatMessage(messages.replace) }}
				</Button>
				<Button type="colored" color="red" :disabled="busy" @click="remove">
					<TrashIcon aria-hidden="true" />
					{{ formatMessage(messages.remove) }}
				</Button>
			</div>
		</div>

		<form v-else-if="status && showForm" class="mt-4" @submit.prevent="save">
			<label for="plus-curseforge-key" class="block text-lg font-semibold text-contrast">
				{{ formatMessage(messages.keyLabel) }}
			</label>
			<div class="mt-2 flex items-center gap-2">
				<Input
					id="plus-curseforge-key"
					v-model="draft"
					:type="revealed ? 'text' : 'password'"
					:placeholder="formatMessage(messages.placeholder)"
					:disabled="!storeAvailable"
					:spellcheck="false"
					autocomplete="off"
					wrapper-class="w-full"
				/>
				<IconButton
					v-tooltip="formatMessage(revealed ? messages.hide : messages.show)"
					:label="formatMessage(revealed ? messages.hide : messages.show)"
					native-type="button"
					@click="revealed = !revealed"
				>
					<EyeOffIcon v-if="revealed" />
					<EyeIcon v-else />
				</IconButton>
			</div>
			<div class="mt-3 flex flex-wrap gap-2">
				<Button
					type="colored"
					color="brand"
					native-type="submit"
					:disabled="!canUseDraft || !storeAvailable"
				>
					{{ formatMessage(messages.save) }}
				</Button>
				<Button native-type="button" :disabled="!canUseDraft" @click="testDraft">
					<RefreshCwIcon aria-hidden="true" />
					{{ formatMessage(messages.test) }}
				</Button>
				<Button v-if="hasSavedKey" native-type="button" :disabled="busy" @click="cancelReplacing">
					{{ formatMessage(messages.cancel) }}
				</Button>
			</div>
		</form>

		<h3 class="m-0 mt-6 text-lg font-semibold text-contrast">
			{{ formatMessage(messages.gettingKey) }}
		</h3>
		<p class="m-0 mt-1">{{ formatMessage(messages.gettingKeyText) }}</p>
		<Button class="mt-3" native-type="button" @click="openUrl(CONSOLE_URL)">
			{{ formatMessage(messages.openConsole) }}
			<ExternalIcon aria-hidden="true" />
		</Button>

		<h3 class="m-0 mt-6 text-lg font-semibold text-contrast">
			{{ formatMessage(messages.rules) }}
		</h3>
		<p class="m-0 mt-1">{{ formatMessage(messages.rulesText) }}</p>

		<MineskinSettings class="mt-8" />
	</section>
</template>
