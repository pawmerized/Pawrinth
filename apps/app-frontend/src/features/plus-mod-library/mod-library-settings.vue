<script setup lang="ts">
import { ArrowDownIcon, ArrowUpIcon, TrashIcon } from '@modrinth/assets'
import {
	Avatar,
	defineMessages,
	IconButton,
	injectNotificationManager,
	Toggle,
	useVIntl,
} from '@modrinth/ui'
import { computed } from 'vue'

import { patchPlusSettings, usePlusSettings } from '@/features/plus-settings/use-plus-settings'
import type { LibraryMod } from '@/helpers/plus'

import { saveLibrary } from './mod-library'
import ModLibraryBrowse from './mod-library-browse.vue'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { settings } = usePlusSettings()

const messages = defineMessages({
	title: { id: 'plus.mod-library.title', defaultMessage: 'Mod library' },
	description: {
		id: 'plus.mod-library.description',
		defaultMessage:
			'Mods you use in most instances. Saving one doesn’t install it anywhere: each new instance from Create instance gets the mods with New instances on, which is how they start, in a version for its Minecraft version and mod loader and with what they need. Turn it off for mods you only want to keep in the list. Mods an instance already has, from either site, aren’t added twice.',
	},
	howToAdd: {
		id: 'plus.mod-library.how-to-add',
		defaultMessage:
			'Find mods below, or use a mod’s ⋯ menu in an instance’s content and choose Add to my mod library.',
	},
	enabled: { id: 'plus.mod-library.enabled', defaultMessage: 'Add these to new instances' },
	enabledText: {
		id: 'plus.mod-library.enabled.text',
		defaultMessage: 'Turn this off to keep the list without installing anything.',
	},
	empty: { id: 'plus.mod-library.empty', defaultMessage: 'Your mod library is empty.' },
	autoInstall: { id: 'plus.mod-library.auto-install', defaultMessage: 'New instances' },
	remove: { id: 'plus.mod-library.remove', defaultMessage: 'Remove {name}' },
	moveUp: { id: 'plus.mod-library.move-up', defaultMessage: 'Move {name} up' },
	moveDown: { id: 'plus.mod-library.move-down', defaultMessage: 'Move {name} down' },
	modrinth: { id: 'plus.mod-library.source.modrinth', defaultMessage: 'Modrinth' },
	curseforge: { id: 'plus.mod-library.source.curseforge', defaultMessage: 'CurseForge' },
})

const library = computed(() => settings.value?.mod_library ?? { enabled: true, mods: [] })

async function save(mods: LibraryMod[]): Promise<void> {
	try {
		await saveLibrary(mods)
	} catch (error) {
		handleError(error as Error)
	}
}

async function setEnabled(enabled: boolean): Promise<void> {
	try {
		await patchPlusSettings({ mod_library: { enabled } })
	} catch (error) {
		handleError(error as Error)
	}
}

function update(index: number, change: Partial<LibraryMod>): Promise<void> {
	return save(library.value.mods.map((item, at) => (at === index ? { ...item, ...change } : item)))
}

function move(index: number, by: number): Promise<void> {
	const mods = [...library.value.mods]
	const [item] = mods.splice(index, 1)
	mods.splice(index + by, 0, item)
	return save(mods)
}

function remove(index: number): Promise<void> {
	return save(library.value.mods.filter((_, at) => at !== index))
}
</script>

<template>
	<section v-if="settings" class="flex flex-col gap-4">
		<div>
			<h2 class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
			<p class="m-0 mt-1">{{ formatMessage(messages.description) }}</p>
			<p class="m-0 mt-1 text-sm text-secondary">{{ formatMessage(messages.howToAdd) }}</p>
		</div>
		<label class="flex cursor-pointer items-center justify-between gap-4">
			<span>
				<span class="block font-semibold text-contrast">{{ formatMessage(messages.enabled) }}</span>
				<span class="text-sm text-secondary">{{ formatMessage(messages.enabledText) }}</span>
			</span>
			<Toggle :model-value="library.enabled" @update:model-value="setEnabled" />
		</label>
		<p v-if="library.mods.length === 0" class="m-0 text-secondary">
			{{ formatMessage(messages.empty) }}
		</p>
		<ul v-else class="m-0 flex list-none flex-col gap-2 p-0">
			<li
				v-for="(item, index) in library.mods"
				:key="`${item.source}:${item.project_id}`"
				class="flex items-center gap-3 rounded-xl bg-surface-2 p-2 pr-3"
				:class="{ 'opacity-60': !library.enabled }"
			>
				<Avatar
					:src="item.icon_url ?? undefined"
					size="2.5rem"
					:tint-by="item.project_id"
					alt=""
					pad-transparent-corners
				/>
				<div class="flex min-w-0 flex-1 flex-col">
					<span class="truncate font-semibold text-contrast">{{ item.name }}</span>
					<span class="text-sm text-secondary">{{ formatMessage(messages[item.source]) }}</span>
				</div>
				<label class="flex shrink-0 items-center gap-2 text-sm text-secondary">
					{{ formatMessage(messages.autoInstall) }}
					<Toggle
						:model-value="item.auto_install"
						@update:model-value="(value: boolean) => update(index, { auto_install: value })"
					/>
				</label>
				<IconButton
					v-tooltip="formatMessage(messages.moveUp, { name: item.name })"
					type="quiet"
					:label="formatMessage(messages.moveUp, { name: item.name })"
					:disabled="index === 0"
					@click="move(index, -1)"
				>
					<ArrowUpIcon aria-hidden="true" />
				</IconButton>
				<IconButton
					v-tooltip="formatMessage(messages.moveDown, { name: item.name })"
					type="quiet"
					:label="formatMessage(messages.moveDown, { name: item.name })"
					:disabled="index === library.mods.length - 1"
					@click="move(index, 1)"
				>
					<ArrowDownIcon aria-hidden="true" />
				</IconButton>
				<IconButton
					v-tooltip="formatMessage(messages.remove, { name: item.name })"
					type="quiet"
					color="red"
					:label="formatMessage(messages.remove, { name: item.name })"
					@click="remove(index)"
				>
					<TrashIcon aria-hidden="true" />
				</IconButton>
			</li>
		</ul>

		<ModLibraryBrowse class="mt-2 border-0 border-t border-solid border-divider pt-4" />
	</section>
</template>
