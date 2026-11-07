<template>
	<Transition name="splash-fade" @after-leave="onAfterLeave">
		<div v-if="!doneLoading" class="splash-screen" :class="`${theme.active}-mode`">
			<div class="app-logo-wrapper" data-tauri-drag-region>
				<svg
					class="app-logo"
					viewBox="0 0 880 200"
					role="img"
					aria-label="Pawrinth"
					color="var(--color-contrast)"
					xmlns="http://www.w3.org/2000/svg"
				>
					<g fill="var(--color-brand)" transform="translate(4 4) scale(.375)">
						<path d="M32 160H96V192H128V288H64V256H32ZM96 320H128V288H160V256H192V224H320V256H352V288H384V192H416V160H480V256H448V288H384V320H416V416H384V448H288V416H224V448H128V416H96ZM128 96H160V64H224V192H160V160H128ZM288 64H352V96H384V160H352V192H288Z"/><path d="M32 160H96V192H128V224H96V192H32ZM96 320H128V352H96ZM128 96H160V64H224V96H160V128H128ZM128 288H160V256H192V288H160V320H128ZM192 224H320V256H352V288H384V320H352V288H320V256H192ZM288 64H352V96H384V128H352V96H288ZM384 192H416V224H384ZM384 320H416V352H384ZM416 160H480V192H416Z" fill="#fff" fill-opacity=".3"/><path d="M32 224H64V256H32ZM64 256H128V288H64ZM96 384H128V416H224V384H288V416H384V448H288V416H224V448H128V416H96ZM128 128H160V160H224V192H160V160H128ZM288 160H352V192H288ZM352 128H384V160H352ZM384 256H448V288H384ZM384 384H416V416H384ZM448 224H480V256H448Z" fill="#000" fill-opacity=".2"/>
					</g>
					<text
						x="222"
						y="146"
						fill="currentColor"
						font-size="136"
						font-weight="700"
						textLength="650"
						lengthAdjust="spacingAndGlyphs"
					>
						Pawrinth
					</text>
				</svg>
				<ProgressBar class="loading-bar" :progress="Math.min(loadingProgress, 100)" />
				<span v-if="message">{{ message }}</span>
			</div>
			<div class="gradient-bg" data-tauri-drag-region></div>
			<div class="base-bg"></div>
		</div>
	</Transition>
</template>

<script setup>
import { injectLoadingState } from '@modrinth/ui'
import { onMounted, ref, watch } from 'vue'

import ProgressBar from '@/components/ui/ProgressBar.vue'
import { useAppEvent } from '@/composables/use-app-event'
import { useTheme } from '@/composables/use-theme.ts'
import { debugStartup } from '@/helpers/startup-debug'

const theme = useTheme()

const doneLoading = ref(false)
const loadingProgress = ref(0)
const message = ref()

const MIN_DISPLAY_MS = 500
const mountedAt = Date.now()

const loading = injectLoadingState()
onMounted(() => debugStartup('Splash mounted'))

function onAfterLeave() {
	debugStartup('Splash fade completed', { displayedMs: Date.now() - mountedAt })
	loading.setEnabled(true)
}

watch(
	[loading.barEnabled, loading.pending],
	([barEnabled, pending]) => {
		debugStartup('Splash loading state changed', { barEnabled, pending })
		if (barEnabled) {
			return
		}

		if (pending) {
			loadingProgress.value = 0
			fakeLoadingIncrease()
			return
		}

		const elapsed = Date.now() - mountedAt
		const delay = Math.max(0, MIN_DISPLAY_MS - elapsed)
		debugStartup('Splash dismissal scheduled', { delayMs: delay, displayedMs: elapsed })

		setTimeout(() => {
			if (loading.pending.value) {
				debugStartup('Splash dismissal deferred: new loading work')
				return
			}
			doneLoading.value = true
			debugStartup('Splash fade started', { displayedMs: Date.now() - mountedAt })
		}, delay)
	},
	{ immediate: true },
)

function fakeLoadingIncrease() {
	if (loadingProgress.value < 95) {
		setTimeout(() => {
			loadingProgress.value += 2
			fakeLoadingIncrease()
		}, 5)
	}
}

useAppEvent('loading', (e) => {
	if (e.event.type === 'directory_move') {
		loadingProgress.value = 100 * (e.fraction ?? 1)
		message.value = 'Updating app directory...'
	}
})
</script>

<style scoped lang="scss">
.splash-screen {
	position: fixed;
	inset: 0;
	z-index: 10000;
}

.splash-fade-leave-active {
	transition: opacity 0.3s ease-in-out;
}

.splash-fade-leave-to {
	opacity: 0;
}

.app-logo-wrapper {
	position: absolute;
	height: 100vh;
	width: 100%;

	display: flex;
	flex-direction: column;
	justify-content: center;
	align-items: center;

	gap: 1rem;
	color: var(--color-contrast);

	z-index: 9998;
}

.app-logo {
	height: 2.25rem;
	width: fit-content;
}

.loading-bar {
	max-width: 20rem;
}

.gradient-bg {
	position: absolute;
	height: 100vh;
	width: 100vw;
	background:
		linear-gradient(180deg, var(--splash-tint-top) 0%, var(--splash-tint-bottom) 97.29%),
		linear-gradient(0deg, var(--splash-overlay), var(--splash-overlay));
	z-index: 9997;
}

.base-bg {
	position: absolute;
	top: 0;
	left: 0;
	width: 100%;
	height: 100%;
	background: var(--color-bg);
	z-index: 9995;
}
</style>
