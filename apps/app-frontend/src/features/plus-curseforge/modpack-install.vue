<script setup lang="ts">
import { DownloadIcon, XIcon } from '@modrinth/assets'
import {
	Button,
	commonMessages,
	defineMessages,
	injectNotificationManager,
	NewModal,
	useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import ModpackAlreadyInstalledModal from '@/components/ui/modal/ModpackAlreadyInstalledModal.vue'
import { useAppSettings } from '@/composables/use-app-settings.ts'
import { list } from '@/helpers/instance'
import {
	type CfPreparedPack,
	plus_cf_pack_install,
	plus_cf_pack_list,
	plus_cf_pack_prepare,
} from '@/helpers/plus'

import MissingFilesList from './missing-files-list.vue'
import { errorText } from './project-adapter'

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const appSettings = useAppSettings()
const router = useRouter()

const messages = defineMessages({
	missingHeader: {
		id: 'plus.curseforge.pack.missing.header',
		defaultMessage: "Some files can't be downloaded",
	},
	missingText: {
		id: 'plus.curseforge.pack.missing.text',
		defaultMessage:
			"{count, plural, one {# file} other {# files}} in {pack} can't be installed through Pawrinth, so the pack may not work properly without {count, plural, one {it} other {them}}. You can download {count, plural, one {it} other {them}} from CurseForge and add {count, plural, one {it} other {them}} to the instance yourself.",
	},
	keptHeader: { id: 'plus.curseforge.pack.kept.header', defaultMessage: 'Your changes stay' },
	keptText: {
		id: 'plus.curseforge.pack.kept.text',
		defaultMessage:
			'These were changed or added in this instance, so the update leaves them as they are:',
	},
	installWithout: {
		id: 'plus.curseforge.pack.install-without',
		defaultMessage: 'Install without them',
	},
	updateWithout: {
		id: 'plus.curseforge.pack.update-without',
		defaultMessage: 'Update without them',
	},
	update: { id: 'plus.curseforge.pack.update', defaultMessage: 'Update' },
	installFailed: {
		id: 'plus.curseforge.pack.install-failed',
		defaultMessage: "Couldn't install {name}",
	},
	updateFailed: {
		id: 'plus.curseforge.pack.update-failed',
		defaultMessage: "Couldn't update {name}",
	},
})

const confirmModal = ref<InstanceType<typeof NewModal> | null>(null)
const duplicateModal = ref<InstanceType<typeof ModpackAlreadyInstalledModal> | null>(null)
const preparing = ref<number | null>(null)
const prepared = ref<CfPreparedPack | null>(null)
const preparedFor = ref<string | null>(null)
const starting = ref(false)
let duplicateRequest: { projectId: number; name: string; fileId: number | null } | null = null

const isUpdate = computed(() => preparedFor.value !== null)
const missing = computed(() => prepared.value?.missing ?? [])
const kept = computed(() => prepared.value?.kept ?? [])
const header = computed(() =>
	formatMessage(missing.value.length > 0 ? messages.missingHeader : messages.keptHeader),
)
const confirmLabel = computed(() =>
	formatMessage(
		missing.value.length === 0
			? messages.update
			: isUpdate.value
				? messages.updateWithout
				: messages.installWithout,
	),
)

async function start(pack: CfPreparedPack, instanceId: string | null): Promise<void> {
	await plus_cf_pack_install(pack.token, instanceId)
}

async function prepareAndStart(
	projectId: number,
	name: string,
	fileId: number | null,
	instanceId: string | null,
): Promise<void> {
	if (preparing.value !== null) return
	preparing.value = projectId
	try {
		const pack = await plus_cf_pack_prepare(projectId, fileId, instanceId)
		if (pack.missing.length > 0 || pack.kept.length > 0) {
			prepared.value = pack
			preparedFor.value = instanceId
			confirmModal.value?.show()
			return
		}
		await start(pack, instanceId)
	} catch (error) {
		addNotification({
			type: 'error',
			title: formatMessage(instanceId ? messages.updateFailed : messages.installFailed, { name }),
			text: errorText(error),
		})
	} finally {
		preparing.value = null
	}
}

/** Installs a CurseForge modpack as a new instance, newest release unless a file is given. */
async function install(
	project: { id: number; name: string },
	fileId: number | null = null,
): Promise<void> {
	if (preparing.value !== null) return
	if (!appSettings.skipNonEssentialWarnings) {
		const [packs, instances] = await Promise.all([plus_cf_pack_list(), list()]).catch(() => [
			[],
			[],
		])
		const existing = instances.find(
			(instance) =>
				instance.link?.type === 'imported_modpack' &&
				packs.some((pack) => pack.instance_id === instance.id && pack.project_id === project.id),
		)
		if (existing) {
			duplicateRequest = { projectId: project.id, name: project.name, fileId }
			duplicateModal.value?.show(existing.name, existing.id)
			return
		}
	}
	await prepareAndStart(project.id, project.name, fileId, null)
}

/** Moves an instance installed from a CurseForge modpack to another version of it. */
async function update(
	instanceId: string,
	project: { id: number; name: string },
	fileId: number,
): Promise<void> {
	await prepareAndStart(project.id, project.name, fileId, instanceId)
}

async function confirm(): Promise<void> {
	const pack = prepared.value
	if (!pack || starting.value) return
	starting.value = true
	try {
		await start(pack, preparedFor.value)
		confirmModal.value?.hide()
	} catch (error) {
		addNotification({
			type: 'error',
			title: formatMessage(isUpdate.value ? messages.updateFailed : messages.installFailed, {
				name: pack.name,
			}),
			text: errorText(error),
		})
	} finally {
		starting.value = false
	}
}

function onCreateAnyway(): void {
	const request = duplicateRequest
	duplicateRequest = null
	if (request) void prepareAndStart(request.projectId, request.name, request.fileId, null)
}

function onGoToInstance(instanceId: string): void {
	duplicateRequest = null
	void router.push(`/instance/${encodeURIComponent(instanceId)}`)
}

defineExpose({ install, update, preparing })
</script>

<template>
	<NewModal ref="confirmModal" :header="header" max-width="560px">
		<div v-if="prepared" class="flex flex-col gap-3">
			<template v-if="missing.length > 0">
				<p class="m-0 text-secondary">
					{{ formatMessage(messages.missingText, { count: missing.length, pack: prepared.name }) }}
				</p>
				<MissingFilesList :files="missing" />
			</template>
			<template v-if="kept.length > 0">
				<p class="m-0 text-secondary">{{ formatMessage(messages.keptText) }}</p>
				<ul class="m-0 flex max-h-48 list-none flex-col gap-1 overflow-y-auto p-0">
					<li
						v-for="path in kept"
						:key="path"
						class="truncate rounded-xl bg-surface-2 px-3 py-2 font-mono text-sm text-primary"
					>
						{{ path }}
					</li>
				</ul>
			</template>
		</div>
		<template #actions>
			<div class="flex justify-end gap-2">
				<Button type="outlined" @click="confirmModal?.hide()">
					<XIcon aria-hidden="true" />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button type="colored" color="brand" :disabled="starting" @click="confirm">
					<DownloadIcon aria-hidden="true" />
					{{ confirmLabel }}
				</Button>
			</div>
		</template>
	</NewModal>
	<ModpackAlreadyInstalledModal
		ref="duplicateModal"
		@create-anyway="onCreateAnyway"
		@go-to-instance="onGoToInstance"
	/>
</template>
