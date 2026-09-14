<script setup lang="ts">
import { DownloadIcon } from '@modrinth/assets'
import {
	Button,
	Checkbox,
	defineMessages,
	injectNotificationManager,
	LoadingIndicator,
	NewModal,
	useVIntl,
} from '@modrinth/ui'
import { ref } from 'vue'

import {
	type CfAvailableUpdate,
	plus_cf_apply_updates,
	plus_cf_check_updates,
} from '@/helpers/plus'

const props = defineProps<{
	instanceId: string
}>()

const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()

const messages = defineMessages({
	title: { id: 'plus.curseforge.updates', defaultMessage: 'CurseForge updates' },
	none: {
		id: 'plus.curseforge.updates.none',
		defaultMessage: 'Everything installed from CurseForge is up to date.',
	},
	blocked: {
		id: 'plus.curseforge.updates.blocked',
		defaultMessage: "This author doesn't allow downloads through other apps.",
	},
	note: {
		id: 'plus.curseforge.updates.note',
		defaultMessage: 'Old versions are kept, so you can restore them from Update history.',
	},
	apply: {
		id: 'plus.curseforge.updates.apply',
		defaultMessage: '{count, plural, one {Update # project} other {Update # projects}}',
	},
	done: { id: 'plus.curseforge.updates.done', defaultMessage: 'CurseForge updates finished' },
})

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const updates = ref<CfAvailableUpdate[]>([])
const selected = ref<Set<string>>(new Set())
const loading = ref(false)
const working = ref(false)

async function show(event?: MouseEvent): Promise<void> {
	modal.value?.show(event)
	loading.value = true
	try {
		updates.value = await plus_cf_check_updates(props.instanceId)
		selected.value = new Set(
			updates.value.filter((item) => item.can_download).map((item) => item.relative_path),
		)
	} catch (error) {
		handleError(error as Error)
	} finally {
		loading.value = false
	}
}

function toggle(path: string, value: boolean): void {
	const next = new Set(selected.value)
	if (value) next.add(path)
	else next.delete(path)
	selected.value = next
}

async function apply(): Promise<void> {
	working.value = true
	try {
		await plus_cf_apply_updates(
			props.instanceId,
			updates.value
				.filter((item) => selected.value.has(item.relative_path))
				.map((item) => ({
					relative_path: item.relative_path,
					project_id: item.project.id,
					file_id: item.file.id,
				})),
		)
		addNotification({ type: 'success', title: formatMessage(messages.done) })
		modal.value?.hide()
	} catch (error) {
		handleError(error as Error)
	} finally {
		working.value = false
	}
}

defineExpose({ show })
</script>

<template>
	<NewModal ref="modal" :header="formatMessage(messages.title)" max-width="640px">
		<LoadingIndicator v-if="loading" />
		<p v-else-if="updates.length === 0" class="m-0">{{ formatMessage(messages.none) }}</p>
		<div v-else class="flex flex-col gap-3">
			<p class="m-0 text-secondary">{{ formatMessage(messages.note) }}</p>
			<label
				v-for="item in updates"
				:key="item.relative_path"
				class="flex items-start gap-3 rounded-xl bg-surface-2 p-3"
			>
				<Checkbox
					:model-value="selected.has(item.relative_path)"
					:disabled="!item.can_download"
					@update:model-value="toggle(item.relative_path, $event)"
				/>
				<span class="min-w-0">
					<span class="block font-semibold text-contrast">{{ item.project.name }}</span>
					<span class="block break-all text-sm text-secondary">
						{{ item.current_file_name }} → {{ item.file.fileName }}
					</span>
					<span v-if="!item.can_download" class="block text-sm text-orange">
						{{ formatMessage(messages.blocked) }}
					</span>
				</span>
			</label>
			<div class="flex justify-end">
				<Button
					type="colored"
					color="brand"
					:disabled="working || selected.size === 0"
					@click="apply"
				>
					<DownloadIcon aria-hidden="true" />
					{{ formatMessage(messages.apply, { count: selected.size }) }}
				</Button>
			</div>
		</div>
	</NewModal>
</template>
