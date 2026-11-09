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
import { ref } from 'vue'

import { errorText } from '@/features/plus-curseforge/project-adapter'
import { plus_technic_install, plus_technic_pack, plus_technic_prepare } from '@/helpers/plus'

import { technicPreparing } from './technic-state'

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()

const messages = defineMessages({
	insecureHeader: {
		id: 'plus.technic.insecure.header',
		defaultMessage: 'This pack downloads insecurely',
	},
	insecureText: {
		id: 'plus.technic.insecure.text',
		defaultMessage:
			'{name} downloads over plain HTTP, and Technic gives no way to check its files. Someone on your network could swap them for something harmful. Only install it if you trust the pack and your connection.',
	},
	installAnyway: { id: 'plus.technic.insecure.install', defaultMessage: 'Install anyway' },
	nothingToDownload: {
		id: 'plus.technic.nothing-to-download',
		defaultMessage: '{name} has nothing to download on Technic.',
	},
	installFailed: { id: 'plus.technic.install-failed', defaultMessage: "Couldn't install {name}" },
	unverifiedTitle: {
		id: 'plus.technic.unverified.title',
		defaultMessage: 'Some files of {name} didn’t match their checksums',
	},
	unverifiedText: {
		id: 'plus.technic.unverified.text',
		defaultMessage:
			'{files} didn’t match the checksums the pack’s server lists, even after downloading again. They came over a secure connection, so the pack’s creator most likely changed them without updating the list.',
	},
})

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const pendingName = ref('')
let decide: ((install: boolean) => void) | null = null

function askAboutInsecure(name: string): Promise<boolean> {
	pendingName.value = name
	return new Promise((resolve) => {
		decide = resolve
		modal.value?.show()
	})
}

function answer(install: boolean): void {
	const resolve = decide
	decide = null
	modal.value?.hide()
	resolve?.(install)
}

function onHide(): void {
	answer(false)
}

/** Downloads a Technic pack, converts it and starts installing it as a new instance. */
async function install(slug: string, name: string): Promise<void> {
	if (technicPreparing.value !== null) return
	technicPreparing.value = slug
	try {
		const details = await plus_technic_pack(slug)
		if (!details.downloadable) {
			addNotification({
				type: 'error',
				title: formatMessage(messages.installFailed, { name }),
				text: formatMessage(messages.nothingToDownload, { name: details.name }),
			})
			return
		}
		let allowInsecure = false
		if (details.insecure) {
			technicPreparing.value = null
			allowInsecure = await askAboutInsecure(details.name)
			if (!allowInsecure || technicPreparing.value !== null) return
			technicPreparing.value = slug
		}
		const prepared = await plus_technic_prepare(slug, allowInsecure)
		await plus_technic_install(prepared.token)
		if (prepared.unverified.length > 0) {
			addNotification({
				type: 'warning',
				title: formatMessage(messages.unverifiedTitle, { name: prepared.name }),
				text: formatMessage(messages.unverifiedText, { files: prepared.unverified.join(', ') }),
			})
		}
	} catch (error) {
		addNotification({
			type: 'error',
			title: formatMessage(messages.installFailed, { name }),
			text: errorText(error),
		})
	} finally {
		if (technicPreparing.value === slug) technicPreparing.value = null
	}
}

defineExpose({ install })
</script>

<template>
	<NewModal
		ref="modal"
		:header="formatMessage(messages.insecureHeader)"
		max-width="520px"
		:on-hide="onHide"
	>
		<p class="m-0 text-secondary">
			{{ formatMessage(messages.insecureText, { name: pendingName }) }}
		</p>
		<template #actions>
			<div class="flex justify-end gap-2">
				<Button type="outlined" @click="answer(false)">
					<XIcon aria-hidden="true" />
					{{ formatMessage(commonMessages.cancelButton) }}
				</Button>
				<Button type="colored" color="red" @click="answer(true)">
					<DownloadIcon aria-hidden="true" />
					{{ formatMessage(messages.installAnyway) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>
