<script setup lang="ts">
import { ImageIcon, TrashIcon } from '@modrinth/assets'
import {
	Admonition,
	Button,
	ColorPicker,
	Combobox,
	defineMessages,
	injectNotificationManager,
	Slider,
	Toggle,
	useVIntl,
} from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { useDebounceFn } from '@vueuse/core'
import { computed } from 'vue'

import { patchPlusSettings, usePlusSettings } from '@/features/plus-settings/use-plus-settings'
import { plus_background_import } from '@/helpers/plus'

import {
	type BackgroundSettings,
	DEFAULT_BACKGROUND,
	isGif,
	MAX_OVERLAY,
	MIN_OVERLAY,
	normalizeBackground,
} from './background-model'
import { backgroundLoadAttempt, backgroundLoadFailed } from './background-state'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { settings } = usePlusSettings()

const messages = defineMessages({
	title: { id: 'plus.background.title', defaultMessage: 'Background image' },
	description: {
		id: 'plus.background.description',
		defaultMessage:
			'Show your own PNG, JPEG, WebP or GIF behind the app. Cards stay solid so text stays readable.',
	},
	failedHeader: { id: 'plus.background.failed', defaultMessage: "Couldn't load this image" },
	failedText: {
		id: 'plus.background.failed.text',
		defaultMessage:
			'It may have been deleted or damaged, so the app is showing your theme without it. Choose it again or pick another image.',
	},
	choose: { id: 'plus.background.choose', defaultMessage: 'Choose image' },
	replace: { id: 'plus.background.replace', defaultMessage: 'Replace image' },
	remove: { id: 'plus.background.remove', defaultMessage: 'Remove' },
	reset: { id: 'plus.background.reset', defaultMessage: 'Reset adjustments' },
	fit: { id: 'plus.background.fit', defaultMessage: 'Fit' },
	fitCover: { id: 'plus.background.fit.cover', defaultMessage: 'Fill window' },
	fitContain: { id: 'plus.background.fit.contain', defaultMessage: 'Fit inside window' },
	fitTile: { id: 'plus.background.fit.tile', defaultMessage: 'Tile' },
	position: { id: 'plus.background.position', defaultMessage: 'Position' },
	center: { id: 'plus.background.position.center', defaultMessage: 'Center' },
	top: { id: 'plus.background.position.top', defaultMessage: 'Top' },
	bottom: { id: 'plus.background.position.bottom', defaultMessage: 'Bottom' },
	left: { id: 'plus.background.position.left', defaultMessage: 'Left' },
	right: { id: 'plus.background.position.right', defaultMessage: 'Right' },
	opacity: { id: 'plus.background.opacity', defaultMessage: 'Image opacity' },
	blur: { id: 'plus.background.blur', defaultMessage: 'Blur' },
	brightness: { id: 'plus.background.brightness', defaultMessage: 'Brightness' },
	overlay: { id: 'plus.background.overlay', defaultMessage: 'Theme overlay' },
	overlayNote: {
		id: 'plus.background.overlay.note',
		defaultMessage: "How strongly your theme's background covers the image.",
	},
	tint: { id: 'plus.background.tint', defaultMessage: 'Tint color' },
	tintOn: { id: 'plus.background.tint-on', defaultMessage: 'Use a tint' },
	tintStrength: { id: 'plus.background.tint-strength', defaultMessage: 'Tint strength' },
	animate: { id: 'plus.background.animate', defaultMessage: 'Animate GIF' },
	ignoreSystem: {
		id: 'plus.background.ignore-system',
		defaultMessage: 'Play even with Windows animations off',
	},
	ignoreSystemNote: {
		id: 'plus.background.ignore-system.note',
		defaultMessage:
			'Windows is set to reduce animations, so GIFs stay still unless you allow this.',
	},
	animateNote: {
		id: 'plus.background.animate.note',
		defaultMessage: 'Also pauses while the window is hidden or when reduced motion is on.',
	},
})

const systemReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches

const background = computed(() => normalizeBackground(settings.value?.appearance.background))

async function save(value: BackgroundSettings | null): Promise<void> {
	try {
		await patchPlusSettings({ appearance: { background: value } })
	} catch (error) {
		handleError(error as Error)
	}
}

function update(change: Partial<BackgroundSettings>): void {
	if (background.value) void save({ ...background.value, ...change })
}

const updateSoon = useDebounceFn(update, 250)

async function choose(): Promise<void> {
	try {
		const selected = await open({
			multiple: false,
			directory: false,
			filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
		})
		if (typeof selected !== 'string') return
		const path = await plus_background_import(selected)
		await save({ ...DEFAULT_BACKGROUND, ...background.value, path })
		backgroundLoadAttempt.value++
	} catch (error) {
		handleError(error as Error)
	}
}

const fitOptions = computed(() => [
	{ value: 'cover', label: formatMessage(messages.fitCover) },
	{ value: 'contain', label: formatMessage(messages.fitContain) },
	{ value: 'tile', label: formatMessage(messages.fitTile) },
])

const positionOptions = computed(() =>
	(['center', 'top', 'bottom', 'left', 'right'] as const).map((value) => ({
		value,
		label: formatMessage(messages[value]),
	})),
)
</script>

<template>
	<section class="mt-6 border-0 border-b border-solid border-divider pb-6">
		<h2 class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
		<p class="m-0 mt-1 text-secondary">{{ formatMessage(messages.description) }}</p>
		<Admonition
			v-if="background && backgroundLoadFailed"
			class="mt-4"
			type="critical"
			:header="formatMessage(messages.failedHeader)"
		>
			{{ formatMessage(messages.failedText) }}
		</Admonition>
		<div class="mt-4 flex flex-wrap gap-2">
			<Button @click="choose">
				<ImageIcon aria-hidden="true" />
				{{ formatMessage(background ? messages.replace : messages.choose) }}
			</Button>
			<template v-if="background">
				<Button type="quiet" @click="save({ ...DEFAULT_BACKGROUND, path: background.path })">
					{{ formatMessage(messages.reset) }}
				</Button>
				<Button type="quiet" color="red" @click="save(null)">
					<TrashIcon aria-hidden="true" />
					{{ formatMessage(messages.remove) }}
				</Button>
			</template>
		</div>

		<div v-if="background" class="mt-4 flex flex-col gap-4">
			<div class="grid grid-cols-2 gap-3">
				<label class="flex flex-col gap-1">
					<span class="font-semibold text-contrast">{{ formatMessage(messages.fit) }}</span>
					<Combobox
						:model-value="background.fit"
						:options="fitOptions"
						@update:model-value="update({ fit: $event })"
					/>
				</label>
				<label class="flex flex-col gap-1">
					<span class="font-semibold text-contrast">{{ formatMessage(messages.position) }}</span>
					<Combobox
						:model-value="background.position"
						:options="positionOptions"
						@update:model-value="update({ position: $event })"
					/>
				</label>
			</div>
			<div>
				<span class="font-semibold text-contrast">{{ formatMessage(messages.opacity) }}</span>
				<Slider
					:model-value="Math.round(background.opacity * 100)"
					:min="10"
					:max="100"
					:step="5"
					unit="%"
					@update:model-value="updateSoon({ opacity: $event / 100 })"
				/>
			</div>
			<div>
				<span class="font-semibold text-contrast">{{ formatMessage(messages.overlay) }}</span>
				<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.overlayNote) }}</p>
				<Slider
					:model-value="Math.round(background.overlay * 100)"
					:min="MIN_OVERLAY * 100"
					:max="MAX_OVERLAY * 100"
					:step="5"
					unit="%"
					@update:model-value="updateSoon({ overlay: $event / 100 })"
				/>
			</div>
			<div>
				<span class="font-semibold text-contrast">{{ formatMessage(messages.blur) }}</span>
				<Slider
					:model-value="background.blur"
					:min="0"
					:max="40"
					:step="1"
					unit="px"
					@update:model-value="updateSoon({ blur: $event })"
				/>
			</div>
			<div>
				<span class="font-semibold text-contrast">{{ formatMessage(messages.brightness) }}</span>
				<Slider
					:model-value="Math.round(background.brightness * 100)"
					:min="20"
					:max="150"
					:step="5"
					unit="%"
					@update:model-value="updateSoon({ brightness: $event / 100 })"
				/>
			</div>
			<div class="flex items-center justify-between gap-4">
				<span class="font-semibold text-contrast">{{ formatMessage(messages.tintOn) }}</span>
				<Toggle
					id="plus-background-tint"
					:model-value="!!background.tint"
					@update:model-value="update({ tint: $event ? '#000000' : null })"
				/>
			</div>
			<template v-if="background.tint">
				<div class="flex items-center justify-between gap-4">
					<span class="font-semibold text-contrast">{{ formatMessage(messages.tint) }}</span>
					<ColorPicker
						:model-value="background.tint"
						:label="formatMessage(messages.tint)"
						@update:model-value="update({ tint: $event.toLowerCase() })"
					/>
				</div>
				<div>
					<span class="font-semibold text-contrast">{{
						formatMessage(messages.tintStrength)
					}}</span>
					<Slider
						:model-value="Math.round(background.tintStrength * 100)"
						:min="0"
						:max="80"
						:step="5"
						unit="%"
						@update:model-value="updateSoon({ tintStrength: $event / 100 })"
					/>
				</div>
			</template>
			<div v-if="isGif(background.path)" class="flex items-center justify-between gap-4">
				<div>
					<span class="font-semibold text-contrast">{{ formatMessage(messages.animate) }}</span>
					<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.animateNote) }}</p>
				</div>
				<Toggle
					id="plus-background-animate"
					:model-value="background.animate"
					@update:model-value="update({ animate: $event })"
				/>
			</div>
			<div
				v-if="isGif(background.path) && background.animate && systemReducedMotion"
				class="flex items-center justify-between gap-4"
			>
				<div>
					<span class="font-semibold text-contrast">{{
						formatMessage(messages.ignoreSystem)
					}}</span>
					<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.ignoreSystemNote) }}</p>
				</div>
				<Toggle
					id="plus-background-ignore-system"
					:model-value="background.ignoreSystemMotion"
					@update:model-value="update({ ignoreSystemMotion: $event })"
				/>
			</div>
		</div>
	</section>
</template>
