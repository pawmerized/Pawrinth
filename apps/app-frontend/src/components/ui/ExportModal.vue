<script setup>
import { FolderOpenIcon, XIcon } from '@modrinth/assets'
import {
	Button,
	Chips,
	commonMessages,
	defineMessages,
	FileTreeSelect,
	injectNotificationManager,
	injectPopupNotificationManager,
	Input,
	NewModal,
	Textarea,
	useVIntl,
} from '@modrinth/ui'
import { save } from '@tauri-apps/plugin-dialog'
import { ref, shallowRef } from 'vue'

import { PackageIcon } from '@/assets/icons'
import { export_instance_mrpack, get_pack_export_candidates } from '@/helpers/instance'
import { plus_cf_export } from '@/helpers/plus'
import { highlightInFolder } from '@/helpers/utils'

const { handleError } = injectNotificationManager()
const popupNotificationManager = injectPopupNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	header: { id: 'app.export-modal.header', defaultMessage: 'Export modpack' },
	modpackNameLabel: { id: 'app.export-modal.modpack-name-label', defaultMessage: 'Modpack name' },
	modpackNamePlaceholder: {
		id: 'app.export-modal.modpack-name-placeholder',
		defaultMessage: 'Modpack name',
	},
	versionNumberLabel: {
		id: 'app.export-modal.version-number-label',
		defaultMessage: 'Version number',
	},
	versionNumberPlaceholder: {
		id: 'app.export-modal.version-number-placeholder',
		defaultMessage: '1.0.0',
	},
	descriptionPlaceholder: {
		id: 'app.export-modal.description-placeholder',
		defaultMessage: 'Enter modpack description...',
	},
	exportButton: { id: 'app.export-modal.export-button', defaultMessage: 'Export' },
	exportComplete: {
		id: 'app.export-modal.export-complete',
		defaultMessage: 'Export complete',
	},
	exportCompleteDescription: {
		id: 'app.export-modal.export-complete-description',
		defaultMessage: '{name} was exported successfully.',
	},
	formatLabel: { id: 'plus.export-modal.format', defaultMessage: 'Format' },
	formatModrinth: { id: 'plus.export-modal.format.modrinth', defaultMessage: 'Modrinth (.mrpack)' },
	formatCurseforge: {
		id: 'plus.export-modal.format.curseforge',
		defaultMessage: 'CurseForge (.zip)',
	},
	formatModrinthText: {
		id: 'plus.export-modal.format.modrinth.text',
		defaultMessage: 'For Pawrinth, Modrinth App and other launchers that open .mrpack files.',
	},
	formatCurseforgeText: {
		id: 'plus.export-modal.format.curseforge.text',
		defaultMessage:
			"For the CurseForge app. Mods from CurseForge are listed so it downloads them itself, and anything that isn't on CurseForge goes inside the zip.",
	},
	curseforgeComplete: {
		id: 'plus.export-modal.curseforge-complete',
		defaultMessage: '{name} lists {listed, plural, one {# file} other {# files}} from CurseForge.',
	},
	curseforgeIncluded: {
		id: 'plus.export-modal.curseforge-included',
		defaultMessage:
			"{count, plural, one {# file isn't} other {# files aren't}} on CurseForge, so {count, plural, one {it's} other {they're}} inside the zip: {names}.",
	},
	curseforgeNotLookedUp: {
		id: 'plus.export-modal.curseforge-not-looked-up',
		defaultMessage:
			'Add a CurseForge API key in Settings, so files from elsewhere can be matched to CurseForge too.',
	},
})

const FORMATS = ['mrpack', 'curseforge']

const props = defineProps({
	instance: {
		type: Object,
		required: true,
	},
})

defineExpose({
	show: () => {
		resetExportState()
		exportModal.value.show()
		void initFiles().catch(handleError)
	},
})

const exportModal = ref(null)
const format = ref('mrpack')
const nameInput = ref(props.instance.name)
const exportDescription = ref('')
const versionInput = ref('1.0.0')
const files = shallowRef([])
const includedFilePaths = ref([])
const excludedFilePaths = ref([])
const fileTreeKey = ref(0)
const filesLoadId = ref(0)
const directoryEntries = new Map()
const currentDirectory = ref('')

async function initFiles() {
	const loadId = ++filesLoadId.value
	const exportCandidates = await get_pack_export_candidates(props.instance.id)
	if (loadId !== filesLoadId.value) return

	files.value = exportCandidates
	directoryEntries.set('', exportCandidates)
	currentDirectory.value = ''
	includedFilePaths.value = files.value
		.filter((file) => !file.disabled && file.defaultSelected)
		.map((file) => file.path)
}

const exportPack = async () => {
	const curseforge = format.value === 'curseforge'
	const outputPath = await save({
		defaultPath: `${nameInput.value} ${versionInput.value}.${curseforge ? 'zip' : 'mrpack'}`,
		filters: [
			curseforge
				? { name: 'CurseForge Modpack', extensions: ['zip'] }
				: { name: 'Modrinth Modpack', extensions: ['mrpack'] },
		],
	})

	if (outputPath) {
		exportModal.value.hide()

		try {
			const fileName = outputPath.split(/[\\/]/).pop() ?? outputPath
			let text = formatMessage(messages.exportCompleteDescription, { name: fileName })
			if (curseforge) {
				const report = await plus_cf_export(
					props.instance.id,
					outputPath,
					includedFilePaths.value,
					excludedFilePaths.value,
					versionInput.value,
					nameInput.value,
				)
				text = [
					formatMessage(messages.curseforgeComplete, { name: fileName, listed: report.listed }),
					report.included.length > 0
						? formatMessage(messages.curseforgeIncluded, {
								count: report.included.length,
								names: report.included.join(', '),
							})
						: '',
					report.included.length > 0 && !report.looked_up
						? formatMessage(messages.curseforgeNotLookedUp)
						: '',
				]
					.filter(Boolean)
					.join(' ')
			} else {
				await export_instance_mrpack(
					props.instance.id,
					outputPath,
					includedFilePaths.value,
					excludedFilePaths.value,
					versionInput.value,
					exportDescription.value,
					nameInput.value,
				)
			}

			popupNotificationManager.addPopupNotification({
				title: formatMessage(messages.exportComplete),
				text,
				type: 'success',
				buttons: [
					{
						label: formatMessage(commonMessages.openInFolderButton),
						icon: FolderOpenIcon,
						action: () => highlightInFolder(outputPath).catch(handleError),
					},
				],
			})
		} catch (error) {
			handleError(error)
		}
	}
}

function resetExportState() {
	format.value = 'mrpack'
	nameInput.value = props.instance.name
	exportDescription.value = ''
	versionInput.value = '1.0.0'
	files.value = []
	includedFilePaths.value = []
	excludedFilePaths.value = []
	fileTreeKey.value += 1
	directoryEntries.clear()
	currentDirectory.value = ''
}

async function loadExportDirectory(path) {
	const normalizedPath = normalizeExportPath(path)
	currentDirectory.value = normalizedPath

	const cachedEntries = directoryEntries.get(normalizedPath)
	if (cachedEntries) {
		files.value = cachedEntries
		return
	}

	const loadId = filesLoadId.value
	files.value = []

	try {
		const childItems = await get_pack_export_candidates(
			props.instance.id,
			normalizedPath || undefined,
		)
		if (loadId !== filesLoadId.value) return

		directoryEntries.set(normalizedPath, childItems)
		if (currentDirectory.value === normalizedPath) {
			files.value = childItems
		}
	} catch {
		if (currentDirectory.value === normalizedPath) files.value = []
	}
}

function normalizeExportPath(path) {
	return path.replaceAll('\\', '/').split('/').filter(Boolean).join('/')
}
</script>

<template>
	<NewModal
		ref="exportModal"
		:header="formatMessage(messages.header)"
		scrollable
		width="46rem"
		max-width="calc(100vw - 2rem)"
	>
		<div class="flex flex-col gap-4">
			<div class="flex flex-col gap-2">
				<p class="m-0 text-contrast font-semibold">{{ formatMessage(messages.formatLabel) }}</p>
				<Chips
					v-model="format"
					:items="[...FORMATS]"
					:format-label="
						(item) =>
							formatMessage(
								item === 'curseforge' ? messages.formatCurseforge : messages.formatModrinth,
							)
					"
					:never-empty="true"
					:capitalize="false"
					data-export-format
				/>
				<p class="m-0 text-secondary">
					{{
						formatMessage(
							format === 'curseforge' ? messages.formatCurseforgeText : messages.formatModrinthText,
						)
					}}
				</p>
			</div>
			<div class="grid grid-cols-2 gap-4">
				<div class="labeled_input w-full">
					<p class="text-contrast font-semibold">{{ formatMessage(messages.modpackNameLabel) }}</p>
					<Input
						v-model="nameInput"
						type="text"
						:placeholder="formatMessage(messages.modpackNamePlaceholder)"
						clearable
						wrapper-class="w-full"
					/>
				</div>
				<div class="labeled_input w-full">
					<p class="text-contrast font-semibold">
						{{ formatMessage(messages.versionNumberLabel) }}
					</p>
					<Input
						v-model="versionInput"
						type="text"
						:placeholder="formatMessage(messages.versionNumberPlaceholder)"
						clearable
						wrapper-class="w-full"
					/>
				</div>
			</div>
			<div v-if="format === 'mrpack'" class="flex flex-col gap-2 min-w-0">
				<p class="m-0 text-contrast font-semibold">
					{{ formatMessage(commonMessages.descriptionLabel) }}
				</p>
				<Textarea
					v-model="exportDescription"
					:placeholder="formatMessage(messages.descriptionPlaceholder)"
					wrapper-class="w-full"
				/>
			</div>
			<FileTreeSelect
				:key="fileTreeKey"
				v-model="includedFilePaths"
				v-model:excluded-paths="excludedFilePaths"
				class="min-w-0"
				:items="files"
				lazy
				@navigate="loadExportDirectory"
			/>
		</div>
		<template #actions>
			<div class="flex items-center justify-end gap-2">
				<Button type="outlined" @click="exportModal.hide">
					<XIcon />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button type="colored" color="brand" @click="exportPack">
					<PackageIcon />
					{{ formatMessage(messages.exportButton) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>
