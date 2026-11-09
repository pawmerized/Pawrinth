<script setup lang="ts">
import { DatabaseBackupIcon, RotateCounterClockwiseIcon, TrashIcon, XIcon } from '@modrinth/assets'
import {
	Button,
	Combobox,
	commonMessages,
	defineMessages,
	injectNotificationManager,
	LoadingIndicator,
	NewModal,
	Toggle,
	useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import {
	type InstanceBackups,
	plus_world_backup_delete,
	plus_world_backup_now,
	plus_world_backup_restore,
	plus_world_backups_list,
	plus_world_backups_set_schedule,
	type WorldBackup,
	type WorldBackupSchedule,
} from '@/helpers/plus'

const props = defineProps<{
	instanceId: string
	instanceRunning: boolean
}>()

const emit = defineEmits<{
	restored: []
}>()

const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()

const messages = defineMessages({
	title: { id: 'plus.world-backups.title', defaultMessage: 'World backups' },
	automatic: {
		id: 'plus.world-backups.automatic',
		defaultMessage: 'Back up worlds automatically',
	},
	automaticText: {
		id: 'plus.world-backups.automatic.text',
		defaultMessage:
			'Backups are kept in Pawrinth’s data folder, apart from Update history. A world is only backed up again after it changes, never while Minecraft has it open, and a backup missed while Pawrinth was closed happens soon after it opens.',
	},
	every: { id: 'plus.world-backups.every', defaultMessage: 'How often' },
	keep: { id: 'plus.world-backups.keep', defaultMessage: 'Automatic backups to keep per world' },
	hourly: { id: 'plus.world-backups.hourly', defaultMessage: 'Every hour' },
	sixHours: { id: 'plus.world-backups.six-hours', defaultMessage: 'Every 6 hours' },
	daily: { id: 'plus.world-backups.daily', defaultMessage: 'Every day' },
	weekly: { id: 'plus.world-backups.weekly', defaultMessage: 'Every week' },
	keepCount: {
		id: 'plus.world-backups.keep-count',
		defaultMessage: '{count, plural, one {# backup} other {# backups}}',
	},
	noWorlds: {
		id: 'plus.world-backups.no-worlds',
		defaultMessage: 'This instance has no singleplayer worlds yet.',
	},
	noBackups: { id: 'plus.world-backups.none', defaultMessage: 'No backups yet.' },
	missingWorld: {
		id: 'plus.world-backups.missing-world',
		defaultMessage: 'This world isn’t in the instance any more. Restoring a backup brings it back.',
	},
	backUpNow: { id: 'plus.world-backups.now', defaultMessage: 'Back up now' },
	backedUp: { id: 'plus.world-backups.backed-up', defaultMessage: 'Backed up {world}' },
	kindAutomatic: { id: 'plus.world-backups.kind.automatic', defaultMessage: 'Automatic' },
	kindManual: { id: 'plus.world-backups.kind.manual', defaultMessage: 'Made by you' },
	kindBeforeRestore: {
		id: 'plus.world-backups.kind.before-restore',
		defaultMessage: 'Before a restore',
	},
	restore: { id: 'plus.world-backups.restore', defaultMessage: 'Restore' },
	confirmRestore: {
		id: 'plus.world-backups.restore.confirm',
		defaultMessage:
			'Replace {world} with this backup from {date}? The world as it is now is backed up first, so you can undo this.',
	},
	closeFirst: {
		id: 'plus.world-backups.close-first',
		defaultMessage: 'Close Minecraft to restore a world.',
	},
	restored: {
		id: 'plus.world-backups.restored',
		defaultMessage: 'Restored {world}. Its previous state was backed up first.',
	},
	delete: { id: 'plus.world-backups.delete', defaultMessage: 'Delete' },
	confirmDelete: {
		id: 'plus.world-backups.delete.confirm',
		defaultMessage: 'Delete this backup? This can’t be undone.',
	},
})

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const data = ref<InstanceBackups | null>(null)
const loading = ref(false)
const working = ref<string | null>(null)
const confirming = ref<{ action: 'restore' | 'delete'; key: string } | null>(null)
const dates = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' })

const intervalOptions = computed(() => [
	{ value: 1, label: formatMessage(messages.hourly) },
	{ value: 6, label: formatMessage(messages.sixHours) },
	{ value: 24, label: formatMessage(messages.daily) },
	{ value: 168, label: formatMessage(messages.weekly) },
])
const keepOptions = computed(() =>
	[3, 5, 10, 20, 50].map((count) => ({
		value: count,
		label: formatMessage(messages.keepCount, { count }),
	})),
)

async function load(): Promise<void> {
	loading.value = true
	try {
		data.value = await plus_world_backups_list(props.instanceId)
	} catch (error) {
		handleError(error as Error)
	} finally {
		loading.value = false
	}
}

async function show(event?: MouseEvent): Promise<void> {
	confirming.value = null
	modal.value?.show(event)
	await load()
}

async function updateSchedule(change: Partial<WorldBackupSchedule>): Promise<void> {
	if (!data.value) return
	try {
		data.value.schedule = await plus_world_backups_set_schedule(props.instanceId, {
			...data.value.schedule,
			...change,
		})
	} catch (error) {
		handleError(error as Error)
	}
}

function kindLabel(backup: WorldBackup): string {
	switch (backup.kind) {
		case 'manual':
			return formatMessage(messages.kindManual)
		case 'before_restore':
			return formatMessage(messages.kindBeforeRestore)
		default:
			return formatMessage(messages.kindAutomatic)
	}
}

function size(bytes: number): string {
	if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`
	if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
	return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`
}

function keyOf(world: string, backup: WorldBackup): string {
	return `${world}/${backup.file_name}`
}

async function run(key: string, action: () => Promise<void>): Promise<void> {
	working.value = key
	try {
		await action()
		await load()
	} catch (error) {
		handleError(error as Error)
	} finally {
		working.value = null
		confirming.value = null
	}
}

function backUpNow(world: string): Promise<void> {
	return run(`${world}/now`, async () => {
		await plus_world_backup_now(props.instanceId, world)
		addNotification({ type: 'success', title: formatMessage(messages.backedUp, { world }) })
	})
}

function restore(world: string, backup: WorldBackup): Promise<void> {
	return run(keyOf(world, backup), async () => {
		await plus_world_backup_restore(props.instanceId, world, backup.file_name)
		addNotification({ type: 'success', title: formatMessage(messages.restored, { world }) })
		emit('restored')
	})
}

function remove(world: string, backup: WorldBackup): Promise<void> {
	return run(keyOf(world, backup), () =>
		plus_world_backup_delete(props.instanceId, world, backup.file_name),
	)
}

defineExpose({ show })
</script>

<template>
	<NewModal ref="modal" :header="formatMessage(messages.title)" max-width="720px">
		<LoadingIndicator v-if="loading && !data" />
		<div v-else-if="data" class="flex flex-col gap-5">
			<section class="flex flex-col gap-3 rounded-xl bg-surface-2 p-4">
				<div class="flex items-center justify-between gap-4">
					<div>
						<h3 class="m-0 text-lg font-semibold text-contrast">
							{{ formatMessage(messages.automatic) }}
						</h3>
						<p class="m-0 mt-1 text-sm text-secondary">
							{{ formatMessage(messages.automaticText) }}
						</p>
					</div>
					<Toggle
						id="plus-world-backups"
						:model-value="data.schedule.enabled"
						class="shrink-0"
						@update:model-value="updateSchedule({ enabled: $event })"
					/>
				</div>
				<div v-if="data.schedule.enabled" class="grid gap-3 sm:grid-cols-2">
					<label class="flex flex-col gap-1">
						<span class="text-sm font-semibold text-contrast">{{
							formatMessage(messages.every)
						}}</span>
						<Combobox
							:model-value="data.schedule.interval_hours"
							:options="intervalOptions"
							@update:model-value="updateSchedule({ interval_hours: Number($event) })"
						/>
					</label>
					<label class="flex flex-col gap-1">
						<span class="text-sm font-semibold text-contrast">{{
							formatMessage(messages.keep)
						}}</span>
						<Combobox
							:model-value="data.schedule.keep"
							:options="keepOptions"
							@update:model-value="updateSchedule({ keep: Number($event) })"
						/>
					</label>
				</div>
			</section>

			<p v-if="data.worlds.length === 0" class="m-0 text-secondary">
				{{ formatMessage(messages.noWorlds) }}
			</p>
			<section
				v-for="world in data.worlds"
				:key="world.world"
				class="flex flex-col gap-2 rounded-xl bg-surface-2 p-4"
			>
				<div class="flex flex-wrap items-center justify-between gap-2">
					<h3 class="m-0 break-all text-lg font-semibold text-contrast">{{ world.world }}</h3>
					<Button v-if="world.exists" :disabled="working !== null" @click="backUpNow(world.world)">
						<DatabaseBackupIcon aria-hidden="true" />
						{{ formatMessage(messages.backUpNow) }}
					</Button>
				</div>
				<p v-if="!world.exists" class="m-0 text-sm text-secondary">
					{{ formatMessage(messages.missingWorld) }}
				</p>
				<p v-if="world.backups.length === 0" class="m-0 text-sm text-secondary">
					{{ formatMessage(messages.noBackups) }}
				</p>
				<div
					v-for="backup in world.backups"
					:key="backup.file_name"
					class="flex flex-wrap items-center justify-between gap-2 rounded-lg bg-surface-3 px-3 py-2"
				>
					<span class="min-w-0">
						<span class="block text-sm font-semibold text-contrast">
							{{ dates.format(new Date(backup.created)) }}
						</span>
						<span class="block text-xs text-secondary">
							{{ kindLabel(backup) }} · {{ size(backup.size) }}
						</span>
					</span>
					<span
						v-if="confirming?.key === keyOf(world.world, backup)"
						class="flex flex-col items-end gap-2"
					>
						<span class="max-w-80 text-right text-xs text-secondary">
							{{
								confirming.action === 'restore'
									? formatMessage(messages.confirmRestore, {
											world: world.world,
											date: dates.format(new Date(backup.created)),
										})
									: formatMessage(messages.confirmDelete)
							}}
						</span>
						<span class="flex gap-2">
							<Button type="outlined" @click="confirming = null">
								<XIcon aria-hidden="true" />
								{{ formatMessage(commonMessages.cancelButton) }}
							</Button>
							<Button
								v-if="confirming.action === 'restore'"
								type="colored"
								color="brand"
								:disabled="working !== null || instanceRunning"
								@click="restore(world.world, backup)"
							>
								<RotateCounterClockwiseIcon aria-hidden="true" />
								{{ formatMessage(messages.restore) }}
							</Button>
							<Button
								v-else
								type="colored"
								color="red"
								:disabled="working !== null"
								@click="remove(world.world, backup)"
							>
								<TrashIcon aria-hidden="true" />
								{{ formatMessage(messages.delete) }}
							</Button>
						</span>
						<span
							v-if="confirming.action === 'restore' && instanceRunning"
							class="text-xs text-orange"
						>
							{{ formatMessage(messages.closeFirst) }}
						</span>
					</span>
					<span v-else class="flex gap-2">
						<Button
							:disabled="working !== null"
							@click="confirming = { action: 'restore', key: keyOf(world.world, backup) }"
						>
							<RotateCounterClockwiseIcon aria-hidden="true" />
							{{ formatMessage(messages.restore) }}
						</Button>
						<Button
							:disabled="working !== null"
							@click="confirming = { action: 'delete', key: keyOf(world.world, backup) }"
						>
							<TrashIcon aria-hidden="true" />
							{{ formatMessage(messages.delete) }}
						</Button>
					</span>
				</div>
			</section>
		</div>
	</NewModal>
</template>
