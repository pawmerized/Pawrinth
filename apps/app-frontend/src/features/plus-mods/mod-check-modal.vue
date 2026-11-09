<script setup lang="ts">
import { CheckIcon, TrashIcon, XIcon } from '@modrinth/assets'
import {
	Admonition,
	Button,
	commonMessages,
	defineMessages,
	injectNotificationManager,
	LoadingIndicator,
	NewModal,
	useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import {
	type CheckedModFile,
	type ModReport,
	plus_mods_check,
	plus_mods_remove_duplicate,
	plus_mods_switch_to_modrinth,
} from '@/helpers/plus'

const props = defineProps<{
	instanceId: string
}>()

const emit = defineEmits<{
	changed: []
}>()

const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()

const messages = defineMessages({
	title: { id: 'plus.mods.check.title', defaultMessage: 'Check mods' },
	intro: {
		id: 'plus.mods.check.intro',
		defaultMessage:
			'Pawrinth recognises mods that weren’t installed through it by their exact file, links them to Modrinth or CurseForge so they get updates, and finds copies of the same mod.',
	},
	recognised: {
		id: 'plus.mods.check.recognised',
		defaultMessage:
			'{modrinth} from Modrinth, {curseforge} from CurseForge, {unknown, plural, one {# not recognised} other {# not recognised}}',
	},
	linkedHeader: {
		id: 'plus.mods.check.linked',
		defaultMessage:
			'{count, plural, one {Linked # mod to CurseForge} other {Linked # mods to CurseForge}}',
	},
	linkedText: {
		id: 'plus.mods.check.linked.text',
		defaultMessage: 'CurseForge updates manage them from now on. Nothing about the files changed.',
	},
	noCurseforgeHeader: {
		id: 'plus.mods.check.no-curseforge',
		defaultMessage: 'CurseForge wasn’t checked',
	},
	noCurseforgeText: {
		id: 'plus.mods.check.no-curseforge.text',
		defaultMessage:
			'Recognising mods from CurseForge needs an API key or curse.tools, under Settings, API configuration.',
	},
	duplicatesHeader: { id: 'plus.mods.check.duplicates', defaultMessage: 'Duplicate mods' },
	duplicatesText: {
		id: 'plus.mods.check.duplicates.text',
		defaultMessage:
			'These files are the same mod, found by the mod ids inside them or their project. Minecraft can crash with two copies, so keep one.',
	},
	noDuplicates: {
		id: 'plus.mods.check.no-duplicates',
		defaultMessage: 'No mod is installed twice.',
	},
	keep: { id: 'plus.mods.check.keep', defaultMessage: 'Keep' },
	remove: { id: 'plus.mods.check.remove', defaultMessage: 'Remove' },
	confirmRemove: {
		id: 'plus.mods.check.remove.confirm',
		defaultMessage:
			'Remove {file}? Pawrinth takes a snapshot first, so you can bring it back from Update history.',
	},
	removed: {
		id: 'plus.mods.check.removed',
		defaultMessage: 'Removed {file}. You can bring it back from Update history.',
	},
	cantRemove: {
		id: 'plus.mods.check.cant-remove',
		defaultMessage:
			'Pawrinth can’t confirm that any copy works with this instance’s Minecraft version and loader, so it won’t remove one. Remove the wrong copy yourself if you’re sure.',
	},
	equivalentsHeader: {
		id: 'plus.mods.check.equivalents',
		defaultMessage: 'Also on Modrinth at the same version',
	},
	equivalentsText: {
		id: 'plus.mods.check.equivalents.text',
		defaultMessage:
			'These CurseForge mods are on Modrinth with the same version for this Minecraft version and loader, from the same source code. Switching keeps the version you have.',
	},
	switch: { id: 'plus.mods.check.switch', defaultMessage: 'Switch to Modrinth' },
	confirmSwitch: {
		id: 'plus.mods.check.switch.confirm',
		defaultMessage:
			'Replace {file} with {replacement} from Modrinth? Pawrinth checks the Modrinth file is the same mod first, and takes a snapshot so you can undo it.',
	},
	switched: {
		id: 'plus.mods.check.switched',
		defaultMessage: '{name} now comes from Modrinth.',
	},
	unknownHeader: {
		id: 'plus.mods.check.unknown',
		defaultMessage: 'Not recognised',
	},
	unknownText: {
		id: 'plus.mods.check.unknown.text',
		defaultMessage:
			'No exact match on Modrinth or CurseForge, so these stay as they are and you manage them yourself.',
	},
	modrinth: { id: 'plus.mods.check.source.modrinth', defaultMessage: 'Modrinth' },
	curseforge: { id: 'plus.mods.check.source.curseforge', defaultMessage: 'CurseForge' },
	unknownSource: { id: 'plus.mods.check.source.unknown', defaultMessage: 'Not recognised' },
	works: { id: 'plus.mods.check.works', defaultMessage: 'Works with this instance' },
	doesntWork: { id: 'plus.mods.check.doesnt-work', defaultMessage: 'Not for this instance' },
	unsure: { id: 'plus.mods.check.unsure', defaultMessage: 'Can’t tell' },
	again: { id: 'plus.mods.check.again', defaultMessage: 'Check again' },
})

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const report = ref<ModReport | null>(null)
const loading = ref(false)
const working = ref<string | null>(null)
const confirming = ref<string | null>(null)

const counts = computed(() => {
	const files = report.value?.files ?? []
	return {
		modrinth: files.filter((file) => file.source.provider === 'modrinth').length,
		curseforge: files.filter((file) => file.source.provider === 'curseforge').length,
		unknown: files.filter((file) => file.source.provider === 'unknown').length,
	}
})
const unknownFiles = computed(
	() => report.value?.files.filter((file) => file.source.provider === 'unknown') ?? [],
)

async function check(): Promise<void> {
	loading.value = true
	confirming.value = null
	try {
		report.value = await plus_mods_check(props.instanceId)
		if (report.value.newly_linked.length > 0) emit('changed')
	} catch (error) {
		handleError(error as Error)
	} finally {
		loading.value = false
	}
}

async function show(event?: MouseEvent): Promise<void> {
	modal.value?.show(event)
	await check()
}

function sourceLabel(file: CheckedModFile): string {
	switch (file.source.provider) {
		case 'modrinth':
			return formatMessage(messages.modrinth)
		case 'curseforge':
			return formatMessage(messages.curseforge)
		default:
			return formatMessage(messages.unknownSource)
	}
}

function compatibilityLabel(file: CheckedModFile): string {
	switch (file.compatibility) {
		case 'compatible':
			return formatMessage(messages.works)
		case 'incompatible':
			return formatMessage(messages.doesntWork)
		default:
			return formatMessage(messages.unsure)
	}
}

async function removeDuplicate(file: CheckedModFile, keepPath: string): Promise<void> {
	working.value = file.relative_path
	try {
		await plus_mods_remove_duplicate(props.instanceId, file.relative_path, keepPath)
		addNotification({
			type: 'success',
			title: formatMessage(messages.removed, { file: file.file_name }),
		})
		emit('changed')
		await check()
	} catch (error) {
		handleError(error as Error)
	} finally {
		working.value = null
		confirming.value = null
	}
}

async function switchToModrinth(path: string, versionId: string, name: string): Promise<void> {
	working.value = path
	try {
		await plus_mods_switch_to_modrinth(props.instanceId, path, versionId)
		addNotification({ type: 'success', title: formatMessage(messages.switched, { name }) })
		emit('changed')
		await check()
	} catch (error) {
		handleError(error as Error)
	} finally {
		working.value = null
		confirming.value = null
	}
}

defineExpose({ show })
</script>

<template>
	<NewModal ref="modal" :header="formatMessage(messages.title)" max-width="720px">
		<div class="flex flex-col gap-4">
			<p class="m-0 text-secondary">{{ formatMessage(messages.intro) }}</p>
			<LoadingIndicator v-if="loading && !report" />
			<template v-else-if="report">
				<p class="m-0 font-semibold text-contrast">
					{{ formatMessage(messages.recognised, counts) }}
				</p>

				<Admonition
					v-if="report.newly_linked.length > 0"
					type="success"
					:header="formatMessage(messages.linkedHeader, { count: report.newly_linked.length })"
				>
					{{ formatMessage(messages.linkedText) }}
					<span class="mt-1 block break-all text-sm">{{ report.newly_linked.join(', ') }}</span>
				</Admonition>
				<Admonition
					v-if="!report.curseforge_checked"
					type="info"
					:header="formatMessage(messages.noCurseforgeHeader)"
				>
					{{ formatMessage(messages.noCurseforgeText) }}
				</Admonition>

				<section class="flex flex-col gap-2">
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.duplicatesHeader) }}
					</h3>
					<p v-if="report.duplicates.length === 0" class="m-0 text-secondary">
						{{ formatMessage(messages.noDuplicates) }}
					</p>
					<template v-else>
						<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.duplicatesText) }}</p>
						<div
							v-for="group in report.duplicates"
							:key="group.keep"
							class="flex flex-col gap-2 rounded-xl bg-surface-2 p-3"
						>
							<span class="font-semibold text-contrast">{{ group.name }}</span>
							<div
								v-for="file in group.files"
								:key="file.relative_path"
								class="flex flex-wrap items-center justify-between gap-2 rounded-lg bg-surface-3 px-3 py-2"
							>
								<span class="min-w-0">
									<span class="block break-all text-sm text-contrast">{{ file.file_name }}</span>
									<span class="block text-xs text-secondary">
										{{
											[file.version, sourceLabel(file), compatibilityLabel(file)]
												.filter(Boolean)
												.join(' · ')
										}}
									</span>
								</span>
								<span
									v-if="file.relative_path === group.keep"
									class="flex items-center gap-1 text-sm font-semibold text-brand"
								>
									<CheckIcon aria-hidden="true" />
									{{ formatMessage(messages.keep) }}
								</span>
								<template v-else-if="group.can_remove">
									<span
										v-if="confirming === file.relative_path"
										class="flex flex-col items-end gap-2"
									>
										<span class="max-w-80 text-right text-xs text-secondary">
											{{ formatMessage(messages.confirmRemove, { file: file.file_name }) }}
										</span>
										<span class="flex gap-2">
											<Button type="outlined" @click="confirming = null">
												<XIcon aria-hidden="true" />
												{{ formatMessage(commonMessages.cancelButton) }}
											</Button>
											<Button
												type="colored"
												color="red"
												:disabled="working !== null"
												@click="removeDuplicate(file, group.keep)"
											>
												<TrashIcon aria-hidden="true" />
												{{ formatMessage(messages.remove) }}
											</Button>
										</span>
									</span>
									<Button
										v-else
										:disabled="working !== null"
										@click="confirming = file.relative_path"
									>
										<TrashIcon aria-hidden="true" />
										{{ formatMessage(messages.remove) }}
									</Button>
								</template>
							</div>
							<p v-if="!group.can_remove" class="m-0 text-xs text-secondary">
								{{ formatMessage(messages.cantRemove) }}
							</p>
						</div>
					</template>
				</section>

				<section v-if="report.modrinth_equivalents.length > 0" class="flex flex-col gap-2">
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.equivalentsHeader) }}
					</h3>
					<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.equivalentsText) }}</p>
					<div
						v-for="item in report.modrinth_equivalents"
						:key="item.relative_path"
						class="flex flex-wrap items-center justify-between gap-2 rounded-xl bg-surface-2 p-3"
					>
						<span class="min-w-0">
							<span class="block font-semibold text-contrast"
								>{{ item.name }} {{ item.version }}</span
							>
							<span class="block break-all text-xs text-secondary">
								{{ item.file_name }} → {{ item.modrinth_file_name }}
							</span>
						</span>
						<span v-if="confirming === item.relative_path" class="flex flex-col items-end gap-2">
							<span class="max-w-80 text-right text-xs text-secondary">
								{{
									formatMessage(messages.confirmSwitch, {
										file: item.file_name,
										replacement: item.modrinth_file_name,
									})
								}}
							</span>
							<span class="flex gap-2">
								<Button type="outlined" @click="confirming = null">
									<XIcon aria-hidden="true" />
									{{ formatMessage(commonMessages.cancelButton) }}
								</Button>
								<Button
									type="colored"
									color="brand"
									:disabled="working !== null"
									@click="switchToModrinth(item.relative_path, item.version_id, item.name)"
								>
									{{ formatMessage(messages.switch) }}
								</Button>
							</span>
						</span>
						<Button v-else :disabled="working !== null" @click="confirming = item.relative_path">
							{{ formatMessage(messages.switch) }}
						</Button>
					</div>
				</section>

				<section v-if="unknownFiles.length > 0" class="flex flex-col gap-2">
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.unknownHeader) }}
					</h3>
					<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.unknownText) }}</p>
					<ul class="m-0 flex max-h-48 list-none flex-col gap-1 overflow-y-auto p-0">
						<li
							v-for="file in unknownFiles"
							:key="file.relative_path"
							class="truncate rounded-lg bg-surface-2 px-3 py-1.5 text-sm text-primary"
						>
							{{ file.file_name }}
						</li>
					</ul>
				</section>

				<div class="flex justify-end">
					<Button :disabled="loading || working !== null" @click="check">
						{{ formatMessage(messages.again) }}
					</Button>
				</div>
			</template>
		</div>
	</NewModal>
</template>
