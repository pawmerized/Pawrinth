<script setup lang="ts">
import { ExternalIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'

import type { CfMissingFile } from '@/helpers/plus'

defineProps<{ files: CfMissingFile[] }>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
	notDistributable: {
		id: 'plus.curseforge.pack.missing.not-distributable',
		defaultMessage: "Its author only allows downloads from CurseForge's website",
	},
	unavailable: {
		id: 'plus.curseforge.pack.missing.unavailable',
		defaultMessage: "CurseForge doesn't offer this file any more",
	},
	unsupported: {
		id: 'plus.curseforge.pack.missing.unsupported',
		defaultMessage: "Pawrinth can't install this kind of file yet",
	},
	downloadPage: {
		id: 'plus.curseforge.pack.missing.download-page',
		defaultMessage: 'Download page',
	},
})

function reasonText(file: CfMissingFile): string {
	const reasons = {
		not_distributable: messages.notDistributable,
		unavailable: messages.unavailable,
		unsupported: messages.unsupported,
	}
	return formatMessage(reasons[file.reason])
}
</script>

<template>
	<ul class="m-0 flex max-h-64 list-none flex-col gap-1 overflow-y-auto p-0">
		<li
			v-for="file in files"
			:key="file.file_id"
			class="flex items-center justify-between gap-3 rounded-xl bg-surface-2 px-3 py-2"
		>
			<div class="flex min-w-0 flex-col">
				<span class="truncate font-semibold text-contrast">{{ file.name }}</span>
				<span v-if="file.file_name" class="truncate text-sm text-secondary">
					{{ file.file_name }}
				</span>
				<span class="text-sm text-secondary">{{ reasonText(file) }}</span>
			</div>
			<a
				v-if="file.download_page"
				class="inline-flex shrink-0 items-center gap-1 text-link hover:underline"
				:href="file.download_page"
				target="_blank"
			>
				<ExternalIcon aria-hidden="true" />
				{{ formatMessage(messages.downloadPage) }}
			</a>
		</li>
	</ul>
</template>
