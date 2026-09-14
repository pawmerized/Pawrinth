<script setup lang="ts">
import { ImageIcon, RotateCounterClockwiseIcon, TrashIcon } from '@modrinth/assets'
import {
	Admonition,
	Button,
	Checkbox,
	defineMessages,
	IconButton,
	injectNotificationManager,
	Input,
	NewModal,
	useFormatBytes,
	useRelativeTime,
	useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import {
	type Operation,
	type OperationItem,
	plus_operation_delete,
	plus_operation_preview,
	plus_operation_restore,
	plus_operations_list,
	plus_snapshot_create,
	plus_storage_usage,
	type RestorePreviewItem,
	type RestoreStatus,
	type StorageUsage,
} from '@/helpers/plus'

const props = defineProps<{
	instanceId: string
	instanceBusy: boolean
}>()

const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const formatBytes = useFormatBytes()
const relativeTime = useRelativeTime()

const messages = defineMessages({
	title: { id: 'plus.history.title', defaultMessage: 'Update history' },
	empty: {
		id: 'plus.history.empty',
		defaultMessage: 'No updates recorded yet. Updates and snapshots for this instance appear here.',
	},
	storage: {
		id: 'plus.history.storage',
		defaultMessage: 'Recovery data uses {used} of your {limit} allowance.',
	},
	overBudget: {
		id: 'plus.history.over-budget',
		defaultMessage:
			'Recovery data is over your allowance. Nothing is deleted automatically; remove old entries to free space.',
	},
	updateAll: { id: 'plus.history.kind.update-all', defaultMessage: 'Update all' },
	updateOne: { id: 'plus.history.kind.update-one', defaultMessage: 'Single update' },
	rollback: { id: 'plus.history.kind.rollback', defaultMessage: 'Restore' },
	snapshot: { id: 'plus.history.kind.snapshot', defaultMessage: 'Snapshot' },
	applied: { id: 'plus.history.state.applied', defaultMessage: 'Completed' },
	partial: { id: 'plus.history.state.partial', defaultMessage: 'Partly completed' },
	captured: { id: 'plus.history.state.captured', defaultMessage: 'Saved' },
	applying: { id: 'plus.history.state.applying', defaultMessage: 'In progress' },
	needsAttention: { id: 'plus.history.state.needs-attention', defaultMessage: 'Needs attention' },
	changedCount: {
		id: 'plus.history.changed-count',
		defaultMessage: '{count, plural, one {# file changed} other {# files changed}}',
	},
	restore: { id: 'plus.history.restore', defaultMessage: 'Restore previous versions' },
	restoreSnapshot: { id: 'plus.history.restore-snapshot', defaultMessage: 'Restore snapshot' },
	remove: { id: 'plus.history.remove', defaultMessage: 'Remove from history' },
	removeWarning: {
		id: 'plus.history.remove-warning',
		defaultMessage: 'This deletes the only recovery copy of these files. Remove it anyway?',
	},
	confirmRemove: { id: 'plus.history.confirm-remove', defaultMessage: 'Remove' },
	keep: { id: 'plus.history.keep', defaultMessage: 'Keep' },
	snapshotName: { id: 'plus.history.snapshot-name', defaultMessage: 'Snapshot name' },
	createSnapshot: { id: 'plus.history.create-snapshot', defaultMessage: 'Create snapshot' },
	snapshotNote: {
		id: 'plus.history.snapshot-note',
		defaultMessage:
			'Snapshots save mods, resource packs, shaders and datapacks. Worlds and configs are not included.',
	},
	reviewTitle: { id: 'plus.history.review', defaultMessage: 'Review what will change' },
	reviewNote: {
		id: 'plus.history.review-note',
		defaultMessage:
			'Only the selected files change. Worlds, configs and other content stay as they are.',
	},
	nothingToChange: {
		id: 'plus.history.nothing',
		defaultMessage: 'The instance already matches this point, so there is nothing to restore.',
	},
	back: { id: 'plus.history.back', defaultMessage: 'Back' },
	confirmRestore: {
		id: 'plus.history.confirm-restore',
		defaultMessage: '{count, plural, one {Restore # file} other {Restore # files}}',
	},
	replaceVersion: { id: 'plus.history.change.updated', defaultMessage: '{after} → {before}' },
	removeFile: { id: 'plus.history.change.added', defaultMessage: 'Remove {name}' },
	bringBack: { id: 'plus.history.change.removed', defaultMessage: 'Bring back {name}' },
	restorable: { id: 'plus.history.status.restorable', defaultMessage: 'Ready to restore' },
	alreadyRestored: { id: 'plus.history.status.already', defaultMessage: 'Already restored' },
	changedSince: {
		id: 'plus.history.status.changed',
		defaultMessage: 'Changed since this update. Select it to overwrite your change.',
	},
	removedSince: {
		id: 'plus.history.status.removed',
		defaultMessage: 'Removed since this update. Select it to bring it back anyway.',
	},
	pathOccupied: {
		id: 'plus.history.status.occupied',
		defaultMessage: 'Another file now has this name. Select it to replace that file.',
	},
	backupDamaged: {
		id: 'plus.history.status.damaged',
		defaultMessage: 'The preserved copy is damaged, so this file cannot be restored.',
	},
	restoredTitle: { id: 'plus.history.restored', defaultMessage: 'Restore complete' },
	restoredText: {
		id: 'plus.history.restored-text',
		defaultMessage:
			'{restored, plural, one {# file restored} other {# files restored}} and verified. {skipped, plural, =0 {} one {# file skipped.} other {# files skipped.}}',
	},
	busy: { id: 'plus.history.busy', defaultMessage: 'Wait for the current install to finish.' },
})

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const operations = ref<Operation[]>([])
const usage = ref<StorageUsage | null>(null)
const snapshotName = ref('')
const working = ref(false)
const removing = ref<string | null>(null)

const reviewing = ref<Operation | null>(null)
const preview = ref<RestorePreviewItem[]>([])
const selected = ref<Set<number>>(new Set())

const overBudget = computed(() => !!usage.value && usage.value.used_bytes > usage.value.limit_bytes)
const actionable = computed(() =>
	preview.value.filter(
		(item) => item.status !== 'already_restored' && item.status !== 'backup_damaged',
	),
)

const kindLabels = {
	update_all: messages.updateAll,
	update_one: messages.updateOne,
	rollback: messages.rollback,
	snapshot: messages.snapshot,
}

const stateLabels = {
	applied: messages.applied,
	partial: messages.partial,
	captured: messages.captured,
	applying: messages.applying,
	needs_attention: messages.needsAttention,
	reverted: messages.applied,
}

const statusLabels: Record<RestoreStatus, (typeof messages)[keyof typeof messages]> = {
	restorable: messages.restorable,
	already_restored: messages.alreadyRestored,
	changed_since: messages.changedSince,
	removed_since: messages.removedSince,
	path_occupied: messages.pathOccupied,
	backup_damaged: messages.backupDamaged,
}

function isConflict(status: RestoreStatus): boolean {
	return status === 'changed_since' || status === 'removed_since' || status === 'path_occupied'
}

function describe(item: OperationItem): string {
	if (item.before && item.after) {
		return formatMessage(messages.replaceVersion, {
			after: item.after.file_name,
			before: item.before.file_name,
		})
	}
	if (item.after) return formatMessage(messages.removeFile, { name: item.after.file_name })
	return formatMessage(messages.bringBack, { name: item.before?.file_name ?? '' })
}

async function load(): Promise<void> {
	try {
		const [loaded, storage] = await Promise.all([
			plus_operations_list(props.instanceId),
			plus_storage_usage(),
		])
		operations.value = loaded
		usage.value = storage
	} catch (error) {
		handleError(error as Error)
	}
}

async function show(event?: MouseEvent): Promise<void> {
	reviewing.value = null
	removing.value = null
	await load()
	modal.value?.show(event)
}

async function review(operation: Operation): Promise<void> {
	working.value = true
	try {
		preview.value = await plus_operation_preview(operation.id)
		selected.value = new Set(
			preview.value.filter((item) => item.status === 'restorable').map((item) => item.ordinal),
		)
		reviewing.value = operation
	} catch (error) {
		handleError(error as Error)
	} finally {
		working.value = false
	}
}

function toggle(ordinal: number, value: boolean): void {
	const next = new Set(selected.value)
	if (value) next.add(ordinal)
	else next.delete(ordinal)
	selected.value = next
}

async function confirmRestore(): Promise<void> {
	if (!reviewing.value) return
	working.value = true
	const ordinals = [...selected.value]
	const overwrite = preview.value
		.filter((item) => selected.value.has(item.ordinal) && isConflict(item.status))
		.map((item) => item.ordinal)
	try {
		const report = await plus_operation_restore(reviewing.value.id, ordinals, overwrite)
		addNotification({
			type: 'success',
			title: formatMessage(messages.restoredTitle),
			text: formatMessage(messages.restoredText, {
				restored: report.restored.length,
				skipped: report.skipped.length,
			}),
		})
		reviewing.value = null
		await load()
	} catch (error) {
		handleError(error as Error)
	} finally {
		working.value = false
	}
}

async function createSnapshot(): Promise<void> {
	const label = snapshotName.value.trim()
	if (!label) return
	working.value = true
	try {
		await plus_snapshot_create(props.instanceId, label)
		snapshotName.value = ''
		await load()
	} catch (error) {
		handleError(error as Error)
	} finally {
		working.value = false
	}
}

async function remove(operation: Operation): Promise<void> {
	try {
		await plus_operation_delete(operation.id)
		removing.value = null
		await load()
	} catch (error) {
		handleError(error as Error)
	}
}

defineExpose({ show })
</script>

<template>
	<NewModal ref="modal" :header="formatMessage(messages.title)" max-width="720px">
		<div v-if="reviewing" class="flex flex-col gap-3">
			<h3 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.reviewTitle) }}
			</h3>
			<p class="m-0 text-secondary">{{ formatMessage(messages.reviewNote) }}</p>
			<p v-if="preview.length === 0" class="m-0">{{ formatMessage(messages.nothingToChange) }}</p>
			<div
				v-for="item in preview"
				:key="item.ordinal"
				class="flex items-start gap-3 rounded-xl bg-surface-2 p-3"
			>
				<Checkbox
					:model-value="selected.has(item.ordinal)"
					:disabled="item.status === 'already_restored' || item.status === 'backup_damaged'"
					@update:model-value="toggle(item.ordinal, $event)"
				/>
				<div class="min-w-0">
					<p class="m-0 break-all font-semibold text-contrast">{{ describe(item) }}</p>
					<p
						class="m-0 mt-1 text-sm"
						:class="
							isConflict(item.status) || item.status === 'backup_damaged'
								? 'text-orange'
								: 'text-secondary'
						"
					>
						{{ formatMessage(statusLabels[item.status]) }}
					</p>
				</div>
			</div>
			<div class="flex justify-end gap-2">
				<Button type="quiet" @click="reviewing = null">{{ formatMessage(messages.back) }}</Button>
				<Button
					type="colored"
					color="brand"
					:disabled="working || selected.size === 0 || actionable.length === 0 || instanceBusy"
					@click="confirmRestore"
				>
					<RotateCounterClockwiseIcon aria-hidden="true" />
					{{ formatMessage(messages.confirmRestore, { count: selected.size }) }}
				</Button>
			</div>
		</div>

		<div v-else class="flex flex-col gap-4">
			<p v-if="usage" class="m-0 text-secondary">
				{{
					formatMessage(messages.storage, {
						used: formatBytes(usage.used_bytes, 1),
						limit: formatBytes(usage.limit_bytes, 1),
					})
				}}
			</p>
			<Admonition v-if="overBudget" type="warning">{{
				formatMessage(messages.overBudget)
			}}</Admonition>

			<div class="flex flex-col gap-2">
				<div class="flex items-center gap-2">
					<Input
						v-model="snapshotName"
						:placeholder="formatMessage(messages.snapshotName)"
						:maxlength="60"
						wrapper-class="flex-1"
					/>
					<Button :disabled="working || !snapshotName.trim()" @click="createSnapshot">
						<ImageIcon aria-hidden="true" />
						{{ formatMessage(messages.createSnapshot) }}
					</Button>
				</div>
				<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.snapshotNote) }}</p>
			</div>

			<p v-if="operations.length === 0" class="m-0">{{ formatMessage(messages.empty) }}</p>
			<div v-for="operation in operations" :key="operation.id" class="rounded-xl bg-surface-2 p-3">
				<div class="flex flex-wrap items-center justify-between gap-2">
					<div class="min-w-0">
						<p class="m-0 font-semibold text-contrast">
							{{ operation.label ?? formatMessage(kindLabels[operation.kind]) }}
						</p>
						<p class="m-0 text-sm text-secondary">
							{{ relativeTime(new Date(operation.created_at * 1000)) }}
							· {{ formatMessage(stateLabels[operation.state]) }}
							<template v-if="operation.kind !== 'snapshot'">
								· {{ formatMessage(messages.changedCount, { count: operation.items.length }) }}
							</template>
						</p>
					</div>
					<div class="flex shrink-0 items-center gap-2">
						<IconButton
							v-tooltip="formatMessage(messages.remove)"
							type="quiet"
							color="red"
							:label="formatMessage(messages.remove)"
							@click="removing = operation.id"
						>
							<TrashIcon aria-hidden="true" />
						</IconButton>
						<Button
							v-tooltip="instanceBusy ? formatMessage(messages.busy) : undefined"
							:disabled="working || instanceBusy || operation.state === 'applying'"
							@click="review(operation)"
						>
							<RotateCounterClockwiseIcon aria-hidden="true" />
							{{
								formatMessage(
									operation.kind === 'snapshot' ? messages.restoreSnapshot : messages.restore,
								)
							}}
						</Button>
					</div>
				</div>
				<p v-if="operation.last_error" class="m-0 mt-2 text-sm text-orange">
					{{ operation.last_error }}
				</p>
				<ul v-if="operation.items.length > 0" class="m-0 mt-2 flex flex-col gap-1 pl-4 text-sm">
					<li v-for="item in operation.items" :key="item.ordinal" class="break-all">
						{{ item.after?.file_name ?? item.before?.file_name }}
					</li>
				</ul>
				<div
					v-if="removing === operation.id"
					class="mt-3 flex flex-wrap items-center justify-between gap-2"
				>
					<span class="text-orange">{{ formatMessage(messages.removeWarning) }}</span>
					<div class="flex gap-2">
						<Button type="quiet" @click="removing = null">{{
							formatMessage(messages.keep)
						}}</Button>
						<Button type="colored" color="red" @click="remove(operation)">{{
							formatMessage(messages.confirmRemove)
						}}</Button>
					</div>
				</div>
			</div>
		</div>
	</NewModal>
</template>
