<script setup lang="ts">
import { SearchIcon } from '@modrinth/assets'
import { defineMessages, Input, useVIntl } from '@modrinth/ui'
import { computed, ref } from 'vue'

import { type DiscoveredSkin, fromFavorite, searchFavorites } from './discovered-skin'
import SkinCard from './skin-card.vue'
import { useSkinFavorites } from './skin-favorites'

defineProps<{
	previewingKey?: string
	readOnly?: boolean
}>()

const emit = defineEmits<{
	preview: [skin: DiscoveredSkin]
	open: [skin: DiscoveredSkin, event: MouseEvent]
	import: [skin: DiscoveredSkin]
}>()

const { formatMessage } = useVIntl()
const { favorites } = useSkinFavorites()

const messages = defineMessages({
	title: { id: 'plus.skins.favorites.title', defaultMessage: 'Favorites' },
	saved: {
		id: 'plus.skins.favorites.saved',
		defaultMessage:
			'Saved on this computer. Pictures load from Mojang when you view them, and each skin remembers where it came from.',
	},
	search: { id: 'plus.skins.favorites.search', defaultMessage: 'Search your favorites' },
	empty: {
		id: 'plus.skins.favorites.empty',
		defaultMessage: 'No favorites yet. Press the heart on a skin to keep it here.',
	},
	noMatches: {
		id: 'plus.skins.favorites.no-matches',
		defaultMessage: 'None of your favorites match “{query}”.',
	},
})

const query = ref('')
const matches = computed(() => searchFavorites(favorites.value, query.value).map(fromFavorite))
</script>

<template>
	<section class="flex flex-col gap-3" data-skin-favorites>
		<div>
			<h2 class="m-0 text-lg font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
			<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.saved) }}</p>
		</div>
		<Input
			v-if="favorites.length > 0"
			v-model="query"
			:icon="SearchIcon"
			type="text"
			wrapper-class="w-full"
			autocomplete="off"
			clearable
			:spellcheck="false"
			:placeholder="formatMessage(messages.search)"
		/>
		<p v-if="favorites.length === 0" class="m-0 text-secondary">{{ formatMessage(messages.empty) }}</p>
		<p v-else-if="matches.length === 0" class="m-0 text-secondary">
			{{ formatMessage(messages.noMatches, { query }) }}
		</p>
		<div v-else class="grid grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] gap-3">
			<SkinCard
				v-for="skin in matches"
				:key="skin.key"
				:skin="skin"
				:previewing="previewingKey === skin.key"
				:read-only="readOnly"
				@preview="(found) => emit('preview', found)"
				@open="(found, event) => emit('open', found, event)"
				@import="(found) => emit('import', found)"
			/>
		</div>
	</section>
</template>
