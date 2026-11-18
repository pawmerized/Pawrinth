<script setup lang="ts">
import { ImageIcon, UndoIcon } from '@modrinth/assets'
import {
	Button,
	defineMessages,
	injectNotificationManager,
	Slider,
	Toggle,
	useVIntl,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'

import { patchPlusSettings, usePlusSettings } from '@/features/plus-settings/use-plus-settings'
import { plus_background_import } from '@/helpers/plus'

import {
	cornerRadius,
	type CustomLogo,
	LOGO_HEIGHT,
	LOGO_WIDTH,
	readCustomLogo,
	resize,
	startingSize,
} from './custom-logo'

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { settings } = usePlusSettings()

const messages = defineMessages({
	title: { id: 'plus.interface.logo.custom.title', defaultMessage: 'Your own logo' },
	preview: {
		id: 'plus.interface.logo.custom.preview',
		defaultMessage: 'Preview in the title bar. Nothing changes until you apply it.',
	},
	width: { id: 'plus.interface.logo.custom.width', defaultMessage: 'Width' },
	height: { id: 'plus.interface.logo.custom.height', defaultMessage: 'Height' },
	keepRatio: {
		id: 'plus.interface.logo.custom.keep-ratio',
		defaultMessage: 'Keep the picture’s proportions',
	},
	keepRatioNote: {
		id: 'plus.interface.logo.custom.keep-ratio.note',
		defaultMessage:
			'When this is off, the width and height set the space the logo gets, and the picture still fits inside it without stretching.',
	},
	rounded: { id: 'plus.interface.logo.custom.rounded', defaultMessage: 'Round the corners' },
	apply: { id: 'plus.interface.logo.custom.apply', defaultMessage: 'Apply' },
	another: {
		id: 'plus.interface.logo.custom.another',
		defaultMessage: 'Choose another picture',
	},
	reset: { id: 'plus.interface.logo.custom.reset', defaultMessage: 'Reset to default' },
	fileFilter: {
		id: 'plus.interface.logo.custom.file-filter',
		defaultMessage: 'Images',
	},
	unreadable: {
		id: 'plus.interface.logo.custom.unreadable',
		defaultMessage: 'That picture couldn’t be shown. Choose a PNG, JPEG or WebP image.',
	},
})

const saved = computed(() => readCustomLogo(settings.value?.appearance.custom_logo))
const inUse = computed(() => settings.value?.appearance.logo === 'custom' && !!saved.value)
const draft = ref<CustomLogo | null>(saved.value)
const error = ref<string | null>(null)

watch(saved, (value) => {
	if (!draft.value || draft.value.path === value?.path) draft.value = value
})

const changed = computed(
	() => !inUse.value || JSON.stringify(draft.value) !== JSON.stringify(saved.value),
)

function measure(path: string): Promise<number> {
	return new Promise((resolve, reject) => {
		const picture = new window.Image()
		picture.onload = () =>
			picture.naturalWidth > 0 && picture.naturalHeight > 0
				? resolve(picture.naturalWidth / picture.naturalHeight)
				: reject(new Error('empty image'))
		picture.onerror = () => reject(new Error('unreadable image'))
		picture.src = convertFileSrc(path)
	})
}

/** Copies a picture into Pawrinth's data and starts a draft with it. */
async function choose(): Promise<void> {
	const picked = await open({
		multiple: false,
		directory: false,
		filters: [
			{ name: formatMessage(messages.fileFilter), extensions: ['png', 'jpg', 'jpeg', 'webp'] },
		],
	})
	if (typeof picked !== 'string') return
	error.value = null
	try {
		const path = await plus_background_import(picked)
		const ratio = await measure(path).catch(() => {
			throw new Error(formatMessage(messages.unreadable))
		})
		draft.value = {
			path,
			ratio,
			keep_ratio: true,
			rounded: draft.value?.rounded ?? false,
			...startingSize(ratio),
		}
	} catch (failure) {
		error.value = failure instanceof Error ? failure.message : String(failure)
	}
}

function setSide(side: 'width' | 'height', value: number): void {
	if (draft.value) draft.value = resize(draft.value, side, value)
}

function setKeepRatio(value: boolean): void {
	if (!draft.value) return
	draft.value = { ...draft.value, keep_ratio: value }
	if (value) draft.value = resize(draft.value, 'height', draft.value.height)
}

async function apply(): Promise<void> {
	if (!draft.value) return
	try {
		await patchPlusSettings({ appearance: { logo: 'custom', custom_logo: { ...draft.value } } })
	} catch (failure) {
		handleError(failure as Error)
	}
}

async function reset(): Promise<void> {
	try {
		await patchPlusSettings({ appearance: { logo: null, custom_logo: null } })
		draft.value = null
		error.value = null
	} catch (failure) {
		handleError(failure as Error)
	}
}

defineExpose({ choose })
</script>

<template>
	<div
		v-if="draft || error"
		class="flex flex-col gap-4 rounded-2xl bg-surface-2 p-4"
		data-custom-logo
	>
		<h4 class="m-0 text-base font-semibold text-contrast">{{ formatMessage(messages.title) }}</h4>
		<p v-if="error" class="m-0 text-red">{{ error }}</p>
		<template v-if="draft">
			<div class="flex flex-col gap-2">
				<div
					class="flex h-12 items-center gap-3 overflow-hidden rounded-xl px-3"
					:style="{ backgroundColor: 'var(--plus-frame-color, var(--color-raised-bg))' }"
				>
					<img
						:src="convertFileSrc(draft.path)"
						alt=""
						class="shrink-0 object-contain"
						:style="{
							width: `${draft.width}px`,
							height: `${draft.height}px`,
							borderRadius: cornerRadius(draft),
						}"
						draggable="false"
					/>
					<span class="h-2 w-24 rounded-full bg-button-bg" aria-hidden="true" />
				</div>
				<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.preview) }}</p>
			</div>
			<label class="flex flex-col gap-1">
				<span class="font-semibold text-contrast">{{ formatMessage(messages.width) }}</span>
				<Slider
					:model-value="draft.width"
					:min="LOGO_WIDTH.min"
					:max="LOGO_WIDTH.max"
					:step="1"
					unit="px"
					@update:model-value="setSide('width', Number($event))"
				/>
			</label>
			<label class="flex flex-col gap-1">
				<span class="font-semibold text-contrast">{{ formatMessage(messages.height) }}</span>
				<Slider
					:model-value="draft.height"
					:min="LOGO_HEIGHT.min"
					:max="LOGO_HEIGHT.max"
					:step="1"
					unit="px"
					@update:model-value="setSide('height', Number($event))"
				/>
			</label>
			<label class="flex cursor-pointer items-center justify-between gap-4">
				<span>
					<span class="block font-semibold text-contrast">{{
						formatMessage(messages.keepRatio)
					}}</span>
					<span class="text-sm text-secondary">{{ formatMessage(messages.keepRatioNote) }}</span>
				</span>
				<Toggle :model-value="draft.keep_ratio" @update:model-value="setKeepRatio" />
			</label>
			<label class="flex cursor-pointer items-center justify-between gap-4">
				<span class="font-semibold text-contrast">{{ formatMessage(messages.rounded) }}</span>
				<Toggle
					:model-value="draft.rounded"
					@update:model-value="(value: boolean) => draft && (draft = { ...draft, rounded: value })"
				/>
			</label>
		</template>
		<div class="flex flex-wrap gap-2">
			<Button v-if="draft" color="brand" type="colored" :disabled="!changed" @click="apply">
				{{ formatMessage(messages.apply) }}
			</Button>
			<Button @click="choose">
				<ImageIcon aria-hidden="true" />
				{{ formatMessage(messages.another) }}
			</Button>
			<Button type="quiet" @click="reset">
				<UndoIcon aria-hidden="true" />
				{{ formatMessage(messages.reset) }}
			</Button>
		</div>
	</div>
</template>
