<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, ref, watch } from 'vue'

import { cornerRadius, currentCustomLogo } from './custom-logo'
import { currentLogo, logoFrame, type LogoId, logoLayers } from './logos'

const props = defineProps<{
	/** Shows this logo instead of the chosen one. */
	logo?: LogoId
	/** Leaves out the Pawrinth wordmark. */
	markOnly?: boolean
}>()

const shown = computed(() => props.logo ?? currentLogo.value)
const failed = ref(false)
/** A chosen picture, unless a built-in logo was asked for or the picture is gone. */
const custom = computed(() => (props.logo || failed.value ? null : currentCustomLogo.value))
watch(
	() => currentCustomLogo.value?.path,
	() => (failed.value = false),
)
const layers = computed(() => logoLayers(shown.value))
const transform = computed(() => {
	const frame = logoFrame(shown.value)
	return `translate(${frame.x} ${frame.y}) scale(${frame.scale})`
})
</script>

<template>
	<img
		v-if="custom"
		:src="convertFileSrc(custom.path)"
		alt="Pawrinth"
		class="shrink-0 object-contain"
		:style="{
			width: `${custom.width}px`,
			height: `${custom.height}px`,
			borderRadius: cornerRadius(custom),
		}"
		draggable="false"
		@error="failed = true"
	/>
	<svg
		v-else
		xmlns="http://www.w3.org/2000/svg"
		:viewBox="markOnly ? '0 0 200 200' : '0 0 880 200'"
		role="img"
		aria-label="Pawrinth"
	>
		<g :transform="transform">
			<path
				v-for="(layer, index) in layers"
				:key="index"
				:d="layer.d"
				:fill="layer.fill"
				:fill-opacity="layer.opacity"
			/>
		</g>
		<text
			v-if="!markOnly"
			x="222"
			y="146"
			fill="currentColor"
			font-family="inherit"
			font-size="136"
			font-weight="700"
			textLength="650"
			lengthAdjust="spacingAndGlyphs"
		>
			Pawrinth
		</text>
	</svg>
</template>
