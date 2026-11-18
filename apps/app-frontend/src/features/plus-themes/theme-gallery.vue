<script setup lang="ts">
import { PaletteIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'

import { bundledGallery } from './theme-gallery'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	title: { id: 'plus.themes.gallery', defaultMessage: 'Theme gallery' },
	comingSoon: { id: 'plus.themes.gallery.coming-soon', defaultMessage: 'Coming soon' },
	heading: {
		id: 'plus.themes.gallery.coming-soon.heading',
		defaultMessage: 'Themes made by other players are on their way',
	},
	text: {
		id: 'plus.themes.gallery.coming-soon.text',
		defaultMessage:
			'Browsing and sharing themes inside Pawrinth isn’t available yet. Until it is, share a theme as a theme file with Import and export below.',
	},
})

/** A few themes made for the gallery, shown as a sample of what it will hold. */
const samples = bundledGallery.slice(0, 4)
</script>

<template>
	<section class="mt-6 border-0 border-b border-solid border-divider pb-6" data-theme-gallery>
		<div class="flex flex-wrap items-center gap-2">
			<h2 class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
			<span
				class="rounded-full border border-solid border-brand px-2 py-0.5 text-xs font-semibold text-brand"
				data-coming-soon
			>
				{{ formatMessage(messages.comingSoon) }}
			</span>
		</div>
		<div class="mt-4 flex flex-col gap-4 rounded-2xl bg-surface-2 p-5">
			<div class="flex items-start gap-3">
				<PaletteIcon class="mt-0.5 size-7 shrink-0 text-brand" aria-hidden="true" />
				<div class="flex flex-col gap-1">
					<p class="m-0 font-semibold text-contrast">{{ formatMessage(messages.heading) }}</p>
					<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.text) }}</p>
				</div>
			</div>
			<div class="grid grid-cols-2 gap-3 opacity-50 sm:grid-cols-4" aria-hidden="true">
				<div
					v-for="item in samples"
					:key="item.id"
					class="flex flex-col overflow-hidden rounded-xl border border-solid border-surface-5"
					:style="{ backgroundColor: item.theme.palette.background }"
				>
					<div class="flex h-14 items-end gap-1.5 p-2">
						<span
							class="h-6 flex-1 rounded-md"
							:style="{ backgroundColor: item.theme.palette.surface }"
						/>
						<span
							class="size-6 rounded-md"
							:style="{ backgroundColor: item.theme.palette.accent }"
						/>
					</div>
					<span
						class="truncate px-2 pb-2 text-sm font-semibold"
						:style="{ color: item.theme.palette.text }"
					>
						{{ item.theme.name }}
					</span>
				</div>
			</div>
		</div>
	</section>
</template>
