<script setup lang="ts">
import {
	Button,
	Checkbox,
	Combobox,
	defineMessages,
	injectNotificationManager,
	NewModal,
	useVIntl,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

import { list } from '@/helpers/instance'
import {
	type ManifestComparison,
	plus_manifest_compare,
	plus_sync_from_instance,
	type VersionedManifest,
} from '@/helpers/plus'

const props = defineProps<{
	instanceId: string
}>()

const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()

const messages = defineMessages({
	title: { id: 'plus.compare.title', defaultMessage: 'Compare instances' },
	choose: { id: 'plus.compare.choose', defaultMessage: 'Compare this instance with' },
	placeholder: { id: 'plus.compare.placeholder', defaultMessage: 'Select an instance' },
	thisInstance: { id: 'plus.compare.this', defaultMessage: 'This instance' },
	other: { id: 'plus.compare.other', defaultMessage: 'Other instance' },
	setup: {
		id: 'plus.compare.setup',
		defaultMessage: '{loader} {loaderVersion} on Minecraft {game}',
	},
	same: {
		id: 'plus.compare.same',
		defaultMessage:
			'{count, plural, one {# file is the same in both} other {# files are the same in both}}',
	},
	different: { id: 'plus.compare.different', defaultMessage: 'Different versions' },
	differentSources: { id: 'plus.compare.different-sources', defaultMessage: 'Different sources' },
	onlyHere: { id: 'plus.compare.only-here', defaultMessage: 'Only in this instance' },
	onlyThere: { id: 'plus.compare.only-there', defaultMessage: 'Only in the other instance' },
	incompatible: {
		id: 'plus.compare.incompatible',
		defaultMessage:
			'These instances use a different game version or loader, so content cannot be copied between them.',
	},
	copy: {
		id: 'plus.compare.copy',
		defaultMessage:
			'{count, plural, one {Copy # file to this instance} other {Copy # files to this instance}}',
	},
	copied: { id: 'plus.compare.copied', defaultMessage: 'Files copied' },
	copiedText: {
		id: 'plus.compare.copied-text',
		defaultMessage:
			'{copied, plural, one {# file copied} other {# files copied}}. The other instance was not changed.',
	},
})

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const options = ref<{ value: string; label: string }[]>([])
const otherId = ref<string | null>(null)
const left = ref<VersionedManifest | null>(null)
const right = ref<VersionedManifest | null>(null)
const comparison = ref<ManifestComparison | null>(null)
const selected = ref<Set<string>>(new Set())
const working = ref(false)

const compatible = computed(
	() =>
		!!left.value &&
		!!right.value &&
		left.value.game_version === right.value.game_version &&
		left.value.loader === right.value.loader,
)

async function compare(): Promise<void> {
	if (!otherId.value) return
	try {
		const [leftManifest, rightManifest, result] = await plus_manifest_compare(
			props.instanceId,
			otherId.value,
		)
		left.value = leftManifest
		right.value = rightManifest
		comparison.value = result
		selected.value = new Set()
	} catch (error) {
		handleError(error as Error)
	}
}

watch(otherId, compare)

async function show(event?: MouseEvent): Promise<void> {
	try {
		const instances = await list()
		options.value = instances
			.filter((instance) => instance.id !== props.instanceId)
			.map((instance) => ({ value: instance.id, label: instance.name }))
	} catch (error) {
		handleError(error as Error)
	}
	otherId.value = null
	comparison.value = null
	modal.value?.show(event)
}

function toggle(path: string, value: boolean): void {
	const next = new Set(selected.value)
	if (value) next.add(path)
	else next.delete(path)
	selected.value = next
}

async function copySelected(): Promise<void> {
	if (!otherId.value || selected.value.size === 0) return
	working.value = true
	try {
		const report = await plus_sync_from_instance(otherId.value, props.instanceId, [
			...selected.value,
		])
		addNotification({
			type: 'success',
			title: formatMessage(messages.copied),
			text: formatMessage(messages.copiedText, { copied: report.copied.length }),
		})
		await compare()
	} catch (error) {
		handleError(error as Error)
	} finally {
		working.value = false
	}
}

defineExpose({ show })
</script>

<template>
	<NewModal ref="modal" :header="formatMessage(messages.title)" max-width="720px">
		<div class="flex flex-col gap-4">
			<label class="flex flex-col gap-2">
				<span class="font-semibold text-contrast">{{ formatMessage(messages.choose) }}</span>
				<Combobox
					v-model="otherId"
					:options="options"
					:placeholder="formatMessage(messages.placeholder)"
				/>
			</label>

			<template v-if="comparison && left && right">
				<div class="grid grid-cols-2 gap-3">
					<div
						v-for="[title, manifest] in [
							[messages.thisInstance, left],
							[messages.other, right],
						] as const"
						:key="title.id"
						class="rounded-xl bg-surface-2 p-3"
					>
						<p class="m-0 font-semibold text-contrast">{{ formatMessage(title) }}</p>
						<p class="m-0 mt-1 text-sm capitalize text-secondary">
							{{
								formatMessage(messages.setup, {
									loader: manifest.loader,
									loaderVersion: manifest.loader_version ?? '',
									game: manifest.game_version,
								})
							}}
						</p>
					</div>
				</div>
				<p v-if="!compatible" class="m-0 text-orange">{{ formatMessage(messages.incompatible) }}</p>
				<p class="m-0 text-secondary">
					{{ formatMessage(messages.same, { count: comparison.same.length }) }}
				</p>

				<section v-if="comparison.different_versions.length > 0">
					<h3 class="m-0 mb-2 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.different) }}
					</h3>
					<p
						v-for="difference in comparison.different_versions"
						:key="difference.left.file_id"
						class="m-0 break-all text-sm"
					>
						{{ difference.left.file_name }} ↔ {{ difference.right.file_name }}
					</p>
				</section>

				<section v-if="comparison.different_sources.length > 0">
					<h3 class="m-0 mb-2 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.differentSources) }}
					</h3>
					<p
						v-for="difference in comparison.different_sources"
						:key="difference.left.file_id"
						class="m-0 break-all text-sm"
					>
						{{ difference.left.file_name }} ({{ difference.left.provider }}) ↔
						{{ difference.right.file_name }} ({{ difference.right.provider }})
					</p>
				</section>

				<section v-if="comparison.only_left.length > 0">
					<h3 class="m-0 mb-2 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.onlyHere) }}
					</h3>
					<p
						v-for="entry in comparison.only_left"
						:key="entry.file_id"
						class="m-0 break-all text-sm"
					>
						{{ entry.file_name }}
					</p>
				</section>

				<section v-if="comparison.only_right.length > 0">
					<h3 class="m-0 mb-2 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.onlyThere) }}
					</h3>
					<label
						v-for="entry in comparison.only_right"
						:key="entry.file_id"
						class="flex items-center gap-2 py-1 text-sm"
					>
						<Checkbox
							:model-value="selected.has(entry.relative_path)"
							:disabled="!compatible"
							@update:model-value="toggle(entry.relative_path, $event)"
						/>
						<span class="break-all">{{ entry.file_name }}</span>
					</label>
					<div class="mt-3 flex justify-end">
						<Button
							type="colored"
							color="brand"
							:disabled="working || selected.size === 0 || !compatible"
							@click="copySelected"
						>
							{{ formatMessage(messages.copy, { count: selected.size }) }}
						</Button>
					</div>
				</section>
			</template>
		</div>
	</NewModal>
</template>
