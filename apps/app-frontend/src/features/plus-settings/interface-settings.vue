<script setup lang="ts">
import {
	Button,
	defineMessages,
	injectNotificationManager,
	Slider,
	Toggle,
	useVIntl,
} from '@modrinth/ui'
import { useDebounceFn } from '@vueuse/core'
import { computed } from 'vue'

import { openSetupWizard } from '@/features/plus-wizard/wizard-state'
import { injectAppSettingsModal } from '@/features/settings/app-settings-modal/context'
import type { PlusSettingsPatch } from '@/helpers/plus'

import { patchPlusSettings, usePlusSettings } from './use-plus-settings'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { settings } = usePlusSettings()

const messages = defineMessages({
	title: { id: 'plus.interface.title', defaultMessage: 'Interface' },
	scaleTitle: { id: 'plus.interface.scale', defaultMessage: 'Interface scale' },
	scaleDescription: {
		id: 'plus.interface.scale.description',
		defaultMessage:
			'Makes the whole app larger or smaller, on top of your Windows display scaling. The smallest window size grows with it.',
	},
	resetScale: { id: 'plus.interface.scale.reset', defaultMessage: 'Reset to 100%' },
	compactTitle: { id: 'plus.interface.compact', defaultMessage: 'Compact layout' },
	compactDescription: {
		id: 'plus.interface.compact.description',
		defaultMessage: 'Tightens text and spacing throughout the app so more fits on screen.',
	},
	motionTitle: { id: 'plus.interface.motion', defaultMessage: 'Reduce motion' },
	motionDescription: {
		id: 'plus.interface.motion.description',
		defaultMessage: 'Turns off interface animations and transitions.',
	},
	lastPageTitle: { id: 'plus.interface.last-page', defaultMessage: 'Reopen the last page' },
	lastPageDescription: {
		id: 'plus.interface.last-page.description',
		defaultMessage: 'Start on the page you had open instead of your default page.',
	},
	setupTitle: { id: 'plus.interface.setup', defaultMessage: 'First-launch setup' },
	setupDescription: {
		id: 'plus.interface.setup.description',
		defaultMessage: 'Run the setup steps again. Your instances and settings are kept.',
	},
	setupAction: { id: 'plus.interface.setup.action', defaultMessage: 'Run setup' },
	resetAll: { id: 'plus.interface.reset-all', defaultMessage: 'Reset interface settings' },
})

const settingsModal = injectAppSettingsModal()

function runSetup(): void {
	settingsModal.close()
	openSetupWizard()
}

const scalePercent = computed(() => Math.round((settings.value?.interface.ui_scale ?? 1) * 100))

async function update(value: NonNullable<PlusSettingsPatch['interface']>): Promise<void> {
	try {
		await patchPlusSettings({ interface: value })
	} catch (error) {
		handleError(error as Error)
	}
}

const updateScale = useDebounceFn((percent: number) => update({ ui_scale: percent / 100 }), 400)

function resetAll(): Promise<void> {
	return update({
		ui_scale: 1,
		density: 'comfortable',
		reduce_motion: false,
		remember_last_page: false,
	})
}
</script>

<template>
	<section v-if="settings">
		<h2 class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
		<div class="mt-4 flex flex-col gap-6">
			<div>
				<div class="flex items-center justify-between gap-4">
					<div>
						<h3 class="m-0 text-lg font-semibold text-contrast">
							{{ formatMessage(messages.scaleTitle) }}
						</h3>
						<p class="m-0 mt-1">{{ formatMessage(messages.scaleDescription) }}</p>
					</div>
					<Button
						:disabled="scalePercent === 100"
						class="shrink-0"
						@click="update({ ui_scale: 1 })"
					>
						{{ formatMessage(messages.resetScale) }}
					</Button>
				</div>
				<Slider
					class="mt-3"
					:model-value="scalePercent"
					:min="80"
					:max="150"
					:step="5"
					:snap-points="[100, 125]"
					unit="%"
					force-step
					@update:model-value="updateScale($event)"
				/>
			</div>

			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.compactTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.compactDescription) }}</p>
				</div>
				<Toggle
					id="plus-compact-layout"
					:model-value="settings.interface.density === 'compact'"
					@update:model-value="update({ density: $event ? 'compact' : 'comfortable' })"
				/>
			</div>

			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.motionTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.motionDescription) }}</p>
				</div>
				<Toggle
					id="plus-reduce-motion"
					:model-value="settings.interface.reduce_motion"
					@update:model-value="update({ reduce_motion: $event })"
				/>
			</div>

			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.lastPageTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.lastPageDescription) }}</p>
				</div>
				<Toggle
					id="plus-remember-last-page"
					:model-value="settings.interface.remember_last_page"
					@update:model-value="update({ remember_last_page: $event })"
				/>
			</div>

			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.setupTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.setupDescription) }}</p>
				</div>
				<Button class="shrink-0" @click="runSetup">{{
					formatMessage(messages.setupAction)
				}}</Button>
			</div>

			<div>
				<Button @click="resetAll">{{ formatMessage(messages.resetAll) }}</Button>
			</div>
		</div>
	</section>
</template>
