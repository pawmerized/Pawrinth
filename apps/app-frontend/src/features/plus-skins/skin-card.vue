<script setup lang="ts">
import { CheckIcon, EyeIcon, HeartIcon, PlusIcon } from '@modrinth/assets'
import { Button, defineMessages, IconButton, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import type { DiscoveredSkin } from './discovered-skin'
import { useSkinFavorites } from './skin-favorites'
import SkinThumbnail from './skin-thumbnail.vue'

const props = defineProps<{
	skin: DiscoveredSkin
	previewing?: boolean
	readOnly?: boolean
}>()

const emit = defineEmits<{
	preview: [skin: DiscoveredSkin]
	open: [skin: DiscoveredSkin, event: MouseEvent]
	import: [skin: DiscoveredSkin]
}>()

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { isFavorite, addFavorite, removeFavorite } = useSkinFavorites()

const messages = defineMessages({
	untitled: { id: 'plus.skins.card.untitled', defaultMessage: 'Untitled skin' },
	view: { id: 'plus.skins.card.view', defaultMessage: 'View' },
	previewOnCharacter: { id: 'plus.skins.card.preview', defaultMessage: 'Preview {name}' },
	addToMySkins: { id: 'plus.skins.find.import', defaultMessage: 'Add to my skins' },
	favorite: { id: 'plus.skins.card.favorite', defaultMessage: 'Add to favorites' },
	unfavorite: { id: 'plus.skins.card.unfavorite', defaultMessage: 'Remove from favorites' },
	sourceMineskin: { id: 'plus.skins.source.mineskin', defaultMessage: 'MineSkin' },
	sourceMojang: { id: 'plus.skins.source.mojang', defaultMessage: 'Minecraft player' },
})

const favorited = computed(() => isFavorite(props.skin.key))
const name = computed(() => props.skin.name ?? formatMessage(messages.untitled))
const favoriteLabel = computed(() =>
	formatMessage(favorited.value ? messages.unfavorite : messages.favorite),
)

async function toggleFavorite(): Promise<void> {
	try {
		if (favorited.value) await removeFavorite(props.skin.key)
		else await addFavorite(props.skin)
	} catch (error) {
		handleError(error as Error)
	}
}
</script>

<template>
	<article
		class="flex min-w-0 flex-col gap-2 rounded-2xl border border-solid bg-surface-3 p-2"
		:class="previewing ? 'border-brand' : 'border-surface-5'"
		:data-skin-card="skin.key"
		:data-previewing="previewing || undefined"
	>
		<button
			class="relative block aspect-[31/40] w-full cursor-pointer rounded-xl border-none bg-transparent p-0 transition-[filter] hover:brightness-110 focus-visible:brightness-110"
			:aria-label="formatMessage(messages.previewOnCharacter, { name })"
			:aria-pressed="previewing"
			@click="emit('preview', skin)"
		>
			<SkinThumbnail :skin="skin" :name="name" />
			<span
				v-if="previewing"
				class="absolute right-2 top-2 flex size-6 items-center justify-center rounded-full bg-contrast"
			>
				<CheckIcon class="size-4 invert [stroke-width:3]" aria-hidden="true" />
			</span>
		</button>
		<div class="min-w-0 px-1">
			<p class="m-0 truncate text-sm font-semibold text-contrast" :title="name">{{ name }}</p>
			<p class="m-0 truncate text-xs text-secondary">
				{{ formatMessage(skin.source === 'mineskin' ? messages.sourceMineskin : messages.sourceMojang) }}
			</p>
		</div>
		<div class="flex items-center gap-1">
			<Button class="grow !min-w-0" size="sm" @click="emit('open', skin, $event)">
				<EyeIcon aria-hidden="true" />
				{{ formatMessage(messages.view) }}
			</Button>
			<IconButton
				v-tooltip="formatMessage(messages.addToMySkins)"
				size="sm"
				:label="formatMessage(messages.addToMySkins)"
				:disabled="readOnly"
				@click="emit('import', skin)"
			>
				<PlusIcon aria-hidden="true" />
			</IconButton>
			<IconButton
				v-tooltip="favoriteLabel"
				size="sm"
				:label="favoriteLabel"
				:aria-pressed="favorited"
				@click="toggleFavorite"
			>
				<HeartIcon :class="{ 'fill-current text-brand-red': favorited }" aria-hidden="true" />
			</IconButton>
		</div>
	</article>
</template>
