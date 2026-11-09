<script setup lang="ts">
import { computed } from 'vue'

import { currentLogo, logoFrame, type LogoId, logoLayers } from './logos'

const props = defineProps<{
	/** Shows this logo instead of the chosen one. */
	logo?: LogoId
	/** Leaves out the Pawrinth wordmark. */
	markOnly?: boolean
}>()

const shown = computed(() => props.logo ?? currentLogo.value)
const layers = computed(() => logoLayers(shown.value))
const transform = computed(() => {
	const frame = logoFrame(shown.value)
	return `translate(${frame.x} ${frame.y}) scale(${frame.scale})`
})
</script>

<template>
	<svg
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
