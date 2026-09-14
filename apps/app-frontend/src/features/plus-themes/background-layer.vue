<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { useTheme } from '@/composables/use-theme'
import { usePlusSettings } from '@/features/plus-settings/use-plus-settings'

import {
	type BackgroundSettings,
	isGif,
	normalizeBackground,
	overlayStylesheet,
	shouldAnimate,
} from './background-model'
import { backgroundLoadAttempt, backgroundLoadFailed } from './background-state'
import { useThemeLibrary } from './theme-engine'

const STYLE_ELEMENT_ID = 'plus-background-overlay'
const HAS_BACKGROUND_CLASS = 'plus-has-background'
const TRANSLUCENT_TOKENS = ['--surface-1', '--surface-1-5', '--surface-2', '--color-bg']

const { settings: plusSettings } = usePlusSettings()
const appTheme = useTheme()
const { selectedTheme } = useThemeLibrary()

const background = computed<BackgroundSettings | null>(() =>
	normalizeBackground(plusSettings.value?.appearance.background),
)
const source = computed(() => (background.value ? convertFileSrc(background.value.path) : ''))
const shownBackground = computed(() => (backgroundLoadFailed.value ? null : background.value))

/**
 * Checked separately for every fit, because a tiled image is a CSS background and
 * reports nothing. Until it's known to fail, the image is treated as fine.
 */
watch(
	[source, backgroundLoadAttempt],
	([value]) => {
		backgroundLoadFailed.value = false
		if (!value) return
		const probe = new Image()
		probe.onerror = () => {
			if (source.value === value) backgroundLoadFailed.value = true
		}
		probe.src = value
	},
	{ immediate: true },
)

const windowVisible = ref(!document.hidden)
const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
const osReducedMotion = ref(motionQuery.matches)

const image = ref<HTMLImageElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
const paused = computed(
	() =>
		!!background.value &&
		isGif(background.value.path) &&
		!shouldAnimate(
			background.value,
			{
				appReduced: plusSettings.value?.interface.reduce_motion === true,
				systemReduced: osReducedMotion.value,
			},
			windowVisible.value,
		),
)

/** A browser can't pause a GIF, so a paused one shows a still copy of a frame instead. */
function drawStillFrame(): void {
	const img = image.value
	const target = canvas.value
	if (!img || !target || !img.naturalWidth) return
	target.width = img.naturalWidth
	target.height = img.naturalHeight
	target.getContext('2d')?.drawImage(img, 0, 0)
}

watch(paused, (isPaused) => {
	if (isPaused) drawStillFrame()
})

const layerStyle = computed(() => {
	const value = background.value
	if (!value) return {}
	return {
		opacity: value.opacity,
		filter: `blur(${value.blur}px) brightness(${value.brightness})`,
		transform: value.blur > 0 ? 'scale(1.05)' : undefined,
	}
})

const imageStyle = computed(() => {
	const value = background.value
	if (!value) return {}
	if (value.fit === 'tile') {
		return {
			backgroundImage: `url("${source.value}")`,
			backgroundRepeat: 'repeat',
			backgroundPosition: value.position,
		}
	}
	return { objectFit: value.fit, objectPosition: value.position }
})

function styleElement(): HTMLStyleElement {
	const existing = document.getElementById(STYLE_ELEMENT_ID)
	if (existing) return existing as HTMLStyleElement
	const element = document.createElement('style')
	element.id = STYLE_ELEMENT_ID
	document.head.appendChild(element)
	return element
}

/** Reads the active theme's real colors, with this overlay removed, before fading them. */
async function applyOverlay(): Promise<void> {
	const html = document.documentElement
	const value = shownBackground.value
	if (!value) {
		html.classList.remove(HAS_BACKGROUND_CLASS)
		styleElement().textContent = ''
		return
	}
	html.classList.remove(HAS_BACKGROUND_CLASS)
	await nextTick()
	const computedStyle = getComputedStyle(html)
	const surfaces = Object.fromEntries(
		TRANSLUCENT_TOKENS.map((token) => [token, computedStyle.getPropertyValue(token)]),
	)
	surfaces['--color-bg'] = computedStyle.getPropertyValue('--surface-1')
	styleElement().textContent = overlayStylesheet(surfaces, value.overlay)
	html.classList.add(HAS_BACKGROUND_CLASS)
}

watch(
	[
		() => background.value?.overlay,
		() => !!shownBackground.value,
		() => appTheme.active,
		selectedTheme,
	],
	() => void applyOverlay(),
	{ immediate: true },
)

function onVisibility(): void {
	windowVisible.value = !document.hidden
}

function onMotionChange(event: MediaQueryListEvent): void {
	osReducedMotion.value = event.matches
}

onMounted(() => {
	document.addEventListener('visibilitychange', onVisibility)
	motionQuery.addEventListener('change', onMotionChange)
})

onBeforeUnmount(() => {
	document.removeEventListener('visibilitychange', onVisibility)
	motionQuery.removeEventListener('change', onMotionChange)
	document.documentElement.classList.remove(HAS_BACKGROUND_CLASS)
})
</script>

<template>
	<div
		v-if="shownBackground && background"
		aria-hidden="true"
		class="plus-background-layer pointer-events-none fixed inset-0 overflow-hidden"
		:style="layerStyle"
	>
		<div v-if="background.fit === 'tile'" class="absolute inset-0" :style="imageStyle" />
		<template v-else>
			<img
				ref="image"
				:key="source"
				:src="source"
				alt=""
				crossorigin="anonymous"
				class="absolute inset-0 size-full"
				:class="{ invisible: paused }"
				:style="imageStyle"
				@load="paused && drawStillFrame()"
			/>
			<canvas v-show="paused" ref="canvas" class="absolute inset-0 size-full" :style="imageStyle" />
		</template>
		<div
			v-if="background.tint"
			class="absolute inset-0"
			:style="{ backgroundColor: background.tint, opacity: background.tintStrength }"
		/>
	</div>
</template>

<style scoped>
.plus-background-layer {
	z-index: -1;
	contain: strict;
}
</style>
