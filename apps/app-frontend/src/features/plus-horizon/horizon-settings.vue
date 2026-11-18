<script setup lang="ts">
import { DownloadIcon, RefreshCwIcon, TrashIcon } from '@modrinth/assets'
import {
	Button,
	defineMessages,
	injectNotificationManager,
	ProgressBar,
	Toggle,
	useVIntl,
} from '@modrinth/ui'
import { computed, onBeforeUnmount, ref } from 'vue'

import { injectInstanceSettings } from '@/features/instances/settings-modal/instance-settings-context'
import { reloadPlusSettings } from '@/features/plus-settings/use-plus-settings'

import {
	type HorizonStatus,
	horizonView,
	plus_horizon_remove,
	plus_horizon_set_enabled,
	plus_horizon_set_up,
	plus_horizon_status,
} from './horizon'

const { instance } = injectInstanceSettings()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

const messages = defineMessages({
	intro: {
		id: 'plus.horizon.intro',
		defaultMessage:
			'Horizon is a third-party fan project: a rebuild of Minecraft: Java Edition 26.3 in Rust by the Horizon project, not made by Mojang, Microsoft or Pawrinth. When you play with it, Pawrinth gives it your Minecraft sign-in, just as it does for Minecraft. It plays this instance’s worlds, resource packs and data packs, but not Java mods. Back up worlds you care about before trying it.',
	},
	play: { id: 'plus.horizon.play', defaultMessage: 'Play with Horizon' },
	playDescription: {
		id: 'plus.horizon.play.description',
		defaultMessage:
			'The Play button starts Horizon instead of Java Minecraft, signed in with your Minecraft account. Turn this off to go back.',
	},
	files: { id: 'plus.horizon.files', defaultMessage: 'Horizon' },
	installed: { id: 'plus.horizon.installed', defaultMessage: 'Installed: {version}' },
	notSetUp: { id: 'plus.horizon.not-set-up', defaultMessage: 'Not set up yet.' },
	needsSetup: {
		id: 'plus.horizon.needs-setup',
		defaultMessage: 'Horizon needs preparing again, which usually happens after an update.',
	},
	unsupported: {
		id: 'plus.horizon.unsupported',
		defaultMessage: 'Horizon has no build for this computer yet.',
	},
	ineligible: {
		id: 'plus.horizon.ineligible',
		defaultMessage:
			'Horizon only plays vanilla Minecraft 26.3. Change this instance in Installation to use it.',
	},
	setUp: { id: 'plus.horizon.set-up', defaultMessage: 'Set up Horizon' },
	update: { id: 'plus.horizon.update', defaultMessage: 'Check for updates' },
	setUpDescription: {
		id: 'plus.horizon.set-up.description',
		defaultMessage:
			'Downloads Horizon (about 60 MB) from its official GitHub release, and installs it only if it’s signed with Horizon’s release key, which is built into Pawrinth. Then it gets Minecraft 26.3’s server file from Mojang and builds Horizon’s files from your Minecraft, including about 380 MB of sounds. It takes a few minutes, and every 26.3 instance shares it.',
	},
	remove: { id: 'plus.horizon.remove', defaultMessage: 'Remove Horizon' },
	removeDescription: {
		id: 'plus.horizon.remove.description',
		defaultMessage:
			'Deletes Horizon and the files it prepared, and turns it off for every instance. Worlds stay in their instances.',
	},
	removeButton: { id: 'plus.horizon.remove.button', defaultMessage: 'Remove' },
})

const status = ref<HorizonStatus | null>(null)
const working = ref(false)
let poll: ReturnType<typeof setInterval> | undefined

function stopPolling() {
	if (poll) {
		clearInterval(poll)
		poll = undefined
	}
}

async function refresh() {
	try {
		status.value = await plus_horizon_status(instance.value.id)
	} catch (error) {
		handleError(error as Error)
	}
	if (status.value?.setting_up || working.value) {
		poll ??= setInterval(refresh, 700)
	} else {
		stopPolling()
	}
}

onBeforeUnmount(stopPolling)
await refresh()

const view = computed(() => (status.value ? horizonView(status.value) : null))

const statusText = computed(() => {
	const current = status.value
	if (!current || !view.value) return ''
	switch (view.value.note) {
		case 'unsupported':
			return formatMessage(messages.unsupported)
		case 'ineligible':
			return formatMessage(messages.ineligible)
		case 'not-set-up':
			return formatMessage(messages.notSetUp)
		case 'needs-setup':
			return formatMessage(messages.needsSetup)
		default:
			return current.version ? formatMessage(messages.installed, { version: current.version }) : ''
	}
})

async function run(action: () => Promise<HorizonStatus | void>) {
	working.value = true
	poll ??= setInterval(refresh, 700)
	try {
		const result = await action()
		if (result) status.value = result
	} catch (error) {
		handleError(error as Error)
	} finally {
		working.value = false
		await refresh()
		await reloadPlusSettings().catch(() => {})
	}
}

function setUp() {
	const firstTime = !status.value?.version
	return run(async () => {
		const result = await plus_horizon_set_up(instance.value.id)
		if (firstTime && result.ready && !result.enabled) {
			await plus_horizon_set_enabled(instance.value.id, true)
		}
		return result
	})
}

function setEnabled(enabled: boolean) {
	return run(() => plus_horizon_set_enabled(instance.value.id, enabled))
}

function remove() {
	return run(() => plus_horizon_remove())
}
</script>

<template>
	<div v-if="status && view" class="flex flex-col gap-6">
		<p class="m-0">{{ formatMessage(messages.intro) }}</p>

		<div
			class="flex items-center justify-between gap-4 border-0 border-b border-solid border-surface-5 pb-6"
		>
			<div class="flex flex-col gap-1">
				<label for="plus-horizon" class="text-lg font-semibold text-contrast">
					{{ formatMessage(messages.play) }}
				</label>
				<p class="m-0">{{ formatMessage(messages.playDescription) }}</p>
			</div>
			<Toggle
				id="plus-horizon"
				:model-value="status.enabled"
				:disabled="!view.canToggle || working"
				@update:model-value="setEnabled"
			/>
		</div>

		<div class="flex flex-col gap-4">
			<div class="flex items-center gap-4 justify-between">
				<div class="flex flex-col gap-1">
					<h2 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.files) }}
					</h2>
					<p class="m-0">{{ statusText }}</p>
				</div>
				<Button
					v-if="status.supported && status.eligible"
					class="shrink-0"
					:color="view.setUpIsUpdate ? undefined : 'brand'"
					:type="view.setUpIsUpdate ? undefined : 'colored'"
					:disabled="!view.canSetUp || working"
					@click="setUp"
				>
					<RefreshCwIcon v-if="view.setUpIsUpdate" aria-hidden="true" />
					<DownloadIcon v-else aria-hidden="true" />
					{{ formatMessage(view.setUpIsUpdate ? messages.update : messages.setUp) }}
				</Button>
			</div>
			<p v-if="status.supported && status.eligible" class="m-0 text-sm text-secondary">
				{{ formatMessage(messages.setUpDescription) }}
			</p>
			<ProgressBar
				v-if="status.setting_up"
				:progress="status.setting_up.fraction ?? 0"
				:waiting="status.setting_up.fraction === null"
				:label="status.setting_up.message"
				full-width
				show-progress
			/>
		</div>

		<div
			v-if="view.canRemove || (working && status.version)"
			class="flex items-center gap-4 justify-between"
		>
			<div class="flex flex-col gap-1">
				<h2 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.remove) }}
				</h2>
				<p class="m-0">{{ formatMessage(messages.removeDescription) }}</p>
			</div>
			<Button
				class="shrink-0"
				type="colored"
				color="red"
				:disabled="!view.canRemove || working"
				@click="remove"
			>
				<TrashIcon aria-hidden="true" />
				{{ formatMessage(messages.removeButton) }}
			</Button>
		</div>
	</div>
</template>
