<script setup lang="ts">
import {
	defineMessages,
	injectNotificationManager,
	Slider,
	Toggle,
	useFormatBytes,
	useVIntl,
} from '@modrinth/ui'
import { useDebounceFn } from '@vueuse/core'
import { onMounted, ref } from 'vue'

import { plus_storage_usage, type PlusSettingsPatch, type StorageUsage } from '@/helpers/plus'

import { patchPlusSettings, usePlusSettings } from './use-plus-settings'

const GIB = 1024 * 1024 * 1024

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { settings } = usePlusSettings()
const formatBytes = useFormatBytes()
const usage = ref<StorageUsage | null>(null)

const messages = defineMessages({
	title: { id: 'plus.recovery.title', defaultMessage: 'Updates and recovery' },
	enabledTitle: {
		id: 'plus.recovery.enabled',
		defaultMessage: 'Save recovery data before updating',
	},
	enabledDescription: {
		id: 'plus.recovery.enabled.description',
		defaultMessage:
			'Keeps the files each update replaces, so you can restore them from Update history even if the old version is no longer available.',
	},
	usage: {
		id: 'plus.recovery.usage',
		defaultMessage:
			'Currently using {used} across {count, plural, one {# entry} other {# entries}}.',
	},
	mclogsTitle: { id: 'plus.diagnostics.mclogs', defaultMessage: 'Also check crashes with mclo.gs' },
	mclogsDescription: {
		id: 'plus.diagnostics.mclogs.description',
		defaultMessage:
			"When Pawrinth's own checks find nothing, sends the game's log to the mclo.gs service for analysis. Logs can contain personal details.",
	},
	limitTitle: { id: 'plus.recovery.limit', defaultMessage: 'Storage allowance' },
	limitDescription: {
		id: 'plus.recovery.limit.description',
		defaultMessage:
			'You are warned when recovery data goes over this. Nothing is deleted because of it.',
	},
	cleanupTitle: { id: 'plus.recovery.cleanup', defaultMessage: 'Remove old entries automatically' },
	cleanupDescription: {
		id: 'plus.recovery.cleanup.description',
		defaultMessage:
			"Removes entries past the limits below. Each instance's newest entry and all snapshots are always kept.",
	},
	countTitle: { id: 'plus.recovery.count', defaultMessage: 'Entries to keep per instance' },
	daysTitle: { id: 'plus.recovery.days', defaultMessage: 'Days to keep entries' },
	daysDescription: {
		id: 'plus.recovery.days.description',
		defaultMessage: '0 keeps them until you remove them.',
	},
})

async function setMclogs(send_logs_to_mclogs: boolean): Promise<void> {
	try {
		await patchPlusSettings({ diagnostics: { send_logs_to_mclogs } })
	} catch (error) {
		handleError(error as Error)
	}
}

async function update(value: NonNullable<PlusSettingsPatch['rollback']>): Promise<void> {
	try {
		await patchPlusSettings({ rollback: value })
	} catch (error) {
		handleError(error as Error)
	}
}

const updateDebounced = useDebounceFn(update, 400)

onMounted(async () => {
	try {
		usage.value = await plus_storage_usage()
	} catch (error) {
		handleError(error as Error)
	}
})
</script>

<template>
	<section v-if="settings">
		<h2 class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
		<div class="mt-4 flex flex-col gap-6">
			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.enabledTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.enabledDescription) }}</p>
					<p v-if="usage" class="m-0 mt-1 text-secondary">
						{{
							formatMessage(messages.usage, {
								used: formatBytes(usage.used_bytes, 1),
								count: usage.operations,
							})
						}}
					</p>
				</div>
				<Toggle
					id="plus-recovery-enabled"
					:model-value="settings.rollback.enabled"
					@update:model-value="update({ enabled: $event })"
				/>
			</div>

			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.mclogsTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.mclogsDescription) }}</p>
				</div>
				<Toggle
					id="plus-diagnostics-mclogs"
					:model-value="settings.diagnostics.send_logs_to_mclogs"
					@update:model-value="setMclogs($event)"
				/>
			</div>

			<div>
				<h3 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.limitTitle) }}
				</h3>
				<p class="m-0 mt-1">{{ formatMessage(messages.limitDescription) }}</p>
				<Slider
					class="mt-3"
					:model-value="Math.round(settings.rollback.max_storage_bytes / GIB)"
					:min="1"
					:max="100"
					:step="1"
					unit="GiB"
					@update:model-value="updateDebounced({ max_storage_bytes: $event * GIB })"
				/>
			</div>

			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.cleanupTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.cleanupDescription) }}</p>
				</div>
				<Toggle
					id="plus-recovery-cleanup"
					:model-value="settings.rollback.auto_cleanup"
					@update:model-value="update({ auto_cleanup: $event })"
				/>
			</div>

			<template v-if="settings.rollback.auto_cleanup">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.countTitle) }}
					</h3>
					<Slider
						class="mt-3"
						:model-value="settings.rollback.max_operations_per_instance"
						:min="1"
						:max="100"
						:step="1"
						@update:model-value="updateDebounced({ max_operations_per_instance: $event })"
					/>
				</div>
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.daysTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.daysDescription) }}</p>
					<Slider
						class="mt-3"
						:model-value="settings.rollback.retention_days"
						:min="0"
						:max="365"
						:step="1"
						@update:model-value="updateDebounced({ retention_days: $event })"
					/>
				</div>
			</template>
		</div>
	</section>
</template>
