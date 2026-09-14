<script setup lang="ts">
import { UnknownIcon } from '@modrinth/assets'
import { defineMessages, useVIntl } from '@modrinth/ui'
import { useIntersectionObserver } from '@vueuse/core'
import { computed, ref, useTemplateRef, watch } from 'vue'

import { acquireSkinPreview } from '@/helpers/rendering/skin-previews'

import { type DiscoveredSkin, toSelectableSkin } from './discovered-skin'
import { drawFrontView, THUMBNAIL_HEIGHT, THUMBNAIL_WIDTH } from './skin-front-view'

const props = defineProps<{
	skin: DiscoveredSkin
	name: string
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
	preview: { id: 'plus.skins.thumbnail.preview', defaultMessage: 'Preview of {name}' },
	failed: { id: 'plus.skins.thumbnail.failed', defaultMessage: "This picture couldn't be loaded." },
})

const root = useTemplateRef<HTMLElement>('root')
const canvas = useTemplateRef<HTMLCanvasElement>('canvas')
const visible = ref(false)
const inView = ref(false)
const state = ref<'loading' | 'ready' | 'failed'>('loading')
const renderedUrl = ref<string>()
const renderReady = ref(false)
const selectable = computed(() => toSelectableSkin(props.skin))

/** Pictures come from Mojang's image host, and only load as they come into view. */
useIntersectionObserver(
	root,
	([entry]) => {
		inView.value = entry?.isIntersecting ?? false
		if (inView.value) visible.value = true
	},
	{ rootMargin: '200px' },
)

/** The flat view shows straight away, then the 3D render replaces it once it's drawn. */
watch(
	[inView, () => props.skin.key],
	([isInView], _, onCleanup) => {
		renderedUrl.value = undefined
		renderReady.value = false
		if (!isInView) return
		const preview = acquireSkinPreview(selectable.value, [])
		let active = true
		onCleanup(() => {
			active = false
			preview.release()
		})
		void preview.promise
			.then((result) => {
				if (active) renderedUrl.value = result?.forwards
			})
			.catch((error) => console.warn('Could not render skin preview', error))
	},
	{ immediate: true },
)

watch(
	[visible, () => props.skin.textureUrl],
	([isVisible, src], _, onCleanup) => {
		if (!isVisible) return
		state.value = 'loading'
		let active = true
		onCleanup(() => {
			active = false
		})
		const image = new Image()
		image.crossOrigin = 'anonymous'
		image.onload = () => {
			const context = canvas.value?.getContext('2d')
			if (!active || !context) return
			try {
				drawFrontView(context, image)
				state.value = 'ready'
			} catch {
				state.value = 'failed'
			}
		}
		image.onerror = () => {
			if (active) state.value = 'failed'
		}
		image.src = src
	},
	{ immediate: true },
)
</script>

<template>
	<div
		ref="root"
		class="relative flex h-full w-full items-center justify-center overflow-hidden rounded-xl bg-surface-2"
		:class="{ 'animate-pulse': state === 'loading' }"
	>
		<canvas
			v-show="state === 'ready' && !renderReady"
			ref="canvas"
			:width="THUMBNAIL_WIDTH"
			:height="THUMBNAIL_HEIGHT"
			class="h-[88%] [image-rendering:pixelated]"
			role="img"
			:aria-label="formatMessage(messages.preview, { name })"
			:data-skin-thumbnail="state"
		/>
		<img
			v-if="renderedUrl"
			v-show="renderReady"
			:src="renderedUrl"
			alt=""
			class="h-full w-full object-contain"
			data-skin-render
			@load="renderReady = true"
		/>
		<UnknownIcon
			v-if="state === 'failed'"
			v-tooltip="formatMessage(messages.failed)"
			class="size-8 text-secondary"
			:aria-label="formatMessage(messages.failed)"
		/>
	</div>
</template>
