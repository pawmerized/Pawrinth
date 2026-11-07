<script setup lang="ts">
import {
	DownloadIcon,
	ExternalIcon,
	HeartIcon,
	PlusIcon,
	ZoomInIcon,
	ZoomOutIcon,
} from '@modrinth/assets'
import {
	Admonition,
	Button,
	ButtonLink,
	defineMessages,
	IconButton,
	injectNotificationManager,
	NewModal,
	RadioButtons,
	SkinPreviewRenderer,
	TagItem,
	useVIntl,
} from '@modrinth/ui'
import { save } from '@tauri-apps/plugin-dialog'
import { computed, ref } from 'vue'

import {
	type GallerySkinDetails,
	plus_skin_file_save,
	plus_skin_gallery_details,
} from '@/helpers/plus'
import {
	determineModelType,
	get_normalized_skin_texture,
	type SkinModel,
} from '@/helpers/skins'

import {
	type DiscoveredSkin,
	skinBytes,
	toSelectableSkin,
} from './discovered-skin'
import { useSkinFavorites } from './skin-favorites'
import { useSkinGallery } from './skin-gallery'
import { mineskinPageUrl, playerPageLinks } from './skin-sources'

const FRONT_ROTATION = Math.PI / 8
const BACK_ROTATION = FRONT_ROTATION + Math.PI
const ZOOM_STEP = 0.2
const MIN_ZOOM = 0.8
const MAX_ZOOM = 1.8

defineProps<{
	readOnly?: boolean
}>()

const emit = defineEmits<{
	import: [skin: DiscoveredSkin]
	'search-tag': [tag: string]
}>()

const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const { isFavorite, addFavorite, removeFavorite } = useSkinFavorites()
const { searchSkins } = useSkinGallery()

const messages = defineMessages({
	untitled: { id: 'plus.skins.card.untitled', defaultMessage: 'Untitled skin' },
	loadFailed: { id: 'plus.skins.detail.load-failed', defaultMessage: "Couldn't load this skin." },
	front: { id: 'plus.skins.detail.front', defaultMessage: 'Front' },
	back: { id: 'plus.skins.detail.back', defaultMessage: 'Back' },
	zoomIn: { id: 'plus.skins.detail.zoom-in', defaultMessage: 'Zoom in' },
	zoomOut: { id: 'plus.skins.detail.zoom-out', defaultMessage: 'Zoom out' },
	source: { id: 'plus.skins.find.player.source', defaultMessage: 'Source' },
	sourceMineskin: { id: 'plus.skins.source.mineskin', defaultMessage: 'MineSkin' },
	sourceMojang: { id: 'plus.skins.source.mojang', defaultMessage: 'Minecraft player' },
	model: { id: 'plus.skins.find.player.model', defaultMessage: 'Model' },
	classic: { id: 'plus.skins.find.player.model.classic', defaultMessage: 'Classic' },
	slim: { id: 'plus.skins.find.player.model.slim', defaultMessage: 'Slim' },
	madeFor: { id: 'plus.skins.detail.made-for', defaultMessage: 'This skin is made for {model} arms.' },
	views: { id: 'plus.skins.detail.views', defaultMessage: 'Views on MineSkin' },
	tags: { id: 'plus.skins.detail.tags', defaultMessage: 'Tags' },
	noCreatorMineskin: {
		id: 'plus.skins.detail.no-creator.mineskin',
		defaultMessage: "MineSkin doesn't say who made its skins.",
	},
	noCreatorPlayer: {
		id: 'plus.skins.find.player.no-creator',
		defaultMessage: "Player profiles don't say who made a skin or what it's called.",
	},
	favorite: { id: 'plus.skins.card.favorite', defaultMessage: 'Add to favorites' },
	unfavorite: { id: 'plus.skins.card.unfavorite', defaultMessage: 'Remove from favorites' },
	addToMySkins: { id: 'plus.skins.find.import', defaultMessage: 'Add to my skins' },
	savePng: { id: 'plus.skins.find.save-png', defaultMessage: 'Save PNG' },
	pngFilter: { id: 'plus.skins.find.png-filter', defaultMessage: 'PNG image' },
	savedPng: { id: 'plus.skins.find.saved-png', defaultMessage: 'Skin saved.' },
	viewOn: { id: 'plus.skins.find.view-on', defaultMessage: 'View on {site}' },
	readOnly: {
		id: 'plus.skins.find.read-only',
		defaultMessage: "Skins can't be added while Minecraft's servers can't be reached.",
	},
})

const SITE_NAMES = { namemc: 'NameMC', skinmc: 'SkinMC', minecraftskins: 'MinecraftSkins' }

const modal = ref<InstanceType<typeof NewModal>>()
const skin = ref<DiscoveredSkin | null>(null)
const textureSrc = ref('')
const details = ref<GallerySkinDetails | null>(null)
const detectedModel = ref<SkinModel>('UNKNOWN')
const previewModel = ref<SkinModel>('CLASSIC')
const loadFailed = ref(false)
const savingPng = ref(false)
const rotation = ref(FRONT_ROTATION)
const zoom = ref(1)
let latestOpen = 0

const title = computed(() => skin.value?.name ?? formatMessage(messages.untitled))
const favorited = computed(() => !!skin.value && isFavorite(skin.value.key))
const favoriteLabel = computed(() =>
	formatMessage(favorited.value ? messages.unfavorite : messages.favorite),
)
const links = computed(() => {
	const current = skin.value
	if (!current) return []
	if (current.source === 'mineskin') {
		return [{ site: 'MineSkin', url: mineskinPageUrl(current.sourceId) }]
	}
	return playerPageLinks({ id: current.sourceId, name: current.name ?? '' }).map((link) => ({
		site: SITE_NAMES[link.source],
		url: link.url,
	}))
})

/** A tiny difference makes pressing the same view again undo a drag. */
function faceView(target: number): void {
	rotation.value = rotation.value === target ? target + 1e-6 : target
}

function changeZoom(step: number): void {
	zoom.value = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom.value + step))
}

async function show(event: MouseEvent, target: DiscoveredSkin): Promise<void> {
	const open = ++latestOpen
	skin.value = target
	details.value = null
	textureSrc.value = ''
	loadFailed.value = false
	rotation.value = FRONT_ROTATION
	zoom.value = 1
	detectedModel.value = target.model
	previewModel.value = target.model === 'SLIM' ? 'SLIM' : 'CLASSIC'
	modal.value?.show(event)

	try {
		const texture = await get_normalized_skin_texture(toSelectableSkin(target))
		if (open !== latestOpen) return
		textureSrc.value = texture
		if (target.model === 'UNKNOWN') {
			const model = await determineModelType(texture)
			if (open !== latestOpen) return
			detectedModel.value = model
			previewModel.value = model
		}
	} catch {
		if (open === latestOpen) loadFailed.value = true
		return
	}

	if (target.source === 'mineskin') {
		const found = await plus_skin_gallery_details(target.sourceId).catch(() => null)
		if (open !== latestOpen || !found) return
		details.value = found
		if (found.model) {
			detectedModel.value = found.model
			previewModel.value = found.model
		}
	}
}

function hide(): void {
	modal.value?.hide()
}

async function toggleFavorite(): Promise<void> {
	if (!skin.value) return
	try {
		if (favorited.value) await removeFavorite(skin.value.key)
		else await addFavorite(skin.value, details.value?.tags ?? [])
	} catch (error) {
		handleError(error as Error)
	}
}

async function savePng(): Promise<void> {
	if (!skin.value) return
	savingPng.value = true
	try {
		const path = await save({
			defaultPath: `${skin.value.name ?? skin.value.key.slice(0, 12)}.png`,
			filters: [{ name: formatMessage(messages.pngFilter), extensions: ['png'] }],
		})
		if (!path) return
		await plus_skin_file_save(path, await skinBytes(skin.value))
		addNotification({ type: 'success', title: formatMessage(messages.savedPng) })
	} catch (error) {
		handleError(error as Error)
	} finally {
		savingPng.value = false
	}
}

function addToMySkins(): void {
	if (!skin.value) return
	const target = skin.value
	hide()
	emit('import', target)
}

function searchTag(tag: string): void {
	hide()
	emit('search-tag', tag)
	void searchSkins(tag)
}

defineExpose({ show, hide })
</script>

<template>
	<NewModal ref="modal">
		<template #title>
			<span class="truncate text-lg font-extrabold text-contrast">{{ title }}</span>
		</template>

		<div v-if="skin" class="flex flex-col gap-6 md:flex-row">
			<div class="flex shrink-0 flex-col items-center gap-3 md:self-center">
				<div class="h-[25rem] w-[16rem] overflow-hidden" data-skin-detail-preview>
					<Admonition v-if="loadFailed" type="critical">
						{{ formatMessage(messages.loadFailed) }}
					</Admonition>
					<SkinPreviewRenderer
						v-else-if="textureSrc"
						:variant="previewModel"
						:texture-src="textureSrc"
						framing="modal"
						:initial-rotation="rotation"
						:fit-zoom="zoom"
						:lock-fit="false"
						class="h-full w-full"
					/>
				</div>
				<div class="flex items-center gap-2">
					<Button size="sm" @click="faceView(FRONT_ROTATION)">
						{{ formatMessage(messages.front) }}
					</Button>
					<Button size="sm" @click="faceView(BACK_ROTATION)">
						{{ formatMessage(messages.back) }}
					</Button>
					<IconButton
						v-tooltip="formatMessage(messages.zoomOut)"
						size="sm"
						:label="formatMessage(messages.zoomOut)"
						:disabled="zoom <= MIN_ZOOM"
						@click="changeZoom(-ZOOM_STEP)"
					>
						<ZoomOutIcon aria-hidden="true" />
					</IconButton>
					<IconButton
						v-tooltip="formatMessage(messages.zoomIn)"
						size="sm"
						:label="formatMessage(messages.zoomIn)"
						:disabled="zoom >= MAX_ZOOM"
						@click="changeZoom(ZOOM_STEP)"
					>
						<ZoomInIcon aria-hidden="true" />
					</IconButton>
				</div>
			</div>

			<div class="flex min-h-[20rem] w-full min-w-52 max-w-96 flex-col gap-4">
				<section>
					<h2 class="m-0 mb-1 text-base font-semibold text-contrast">
						{{ formatMessage(messages.source) }}
					</h2>
					<p class="m-0 text-primary">
						{{
							formatMessage(skin.source === 'mineskin' ? messages.sourceMineskin : messages.sourceMojang)
						}}
					</p>
					<p class="m-0 mt-1 text-sm text-secondary">
						{{
							formatMessage(
								skin.source === 'mineskin' ? messages.noCreatorMineskin : messages.noCreatorPlayer,
							)
						}}
					</p>
				</section>

				<section>
					<h2 class="m-0 mb-2 text-base font-semibold text-contrast">
						{{ formatMessage(messages.model) }}
					</h2>
					<RadioButtons v-model="previewModel" :items="['CLASSIC', 'SLIM']" class="!flex-row flex-wrap">
						<template #default="{ item }">
							{{ formatMessage(item === 'CLASSIC' ? messages.classic : messages.slim) }}
						</template>
					</RadioButtons>
					<p v-if="detectedModel !== 'UNKNOWN'" class="m-0 mt-1 text-sm text-secondary">
						{{
							formatMessage(messages.madeFor, {
								model: formatMessage(detectedModel === 'SLIM' ? messages.slim : messages.classic),
							})
						}}
					</p>
				</section>

				<section v-if="details?.tags.length">
					<h2 class="m-0 mb-2 text-base font-semibold text-contrast">
						{{ formatMessage(messages.tags) }}
					</h2>
					<div class="flex flex-wrap gap-1">
						<TagItem v-for="tag in details.tags" :key="tag" :action="() => searchTag(tag)">
							{{ tag }}
						</TagItem>
					</div>
				</section>

				<section v-if="details?.views">
					<h2 class="m-0 mb-1 text-base font-semibold text-contrast">
						{{ formatMessage(messages.views) }}
					</h2>
					<p class="m-0 text-primary">{{ details.views.toLocaleString() }}</p>
				</section>
			</div>
		</div>

		<template #actions>
			<div class="flex flex-wrap justify-end gap-2">
				<ButtonLink v-for="link in links" :key="link.site" :href="link.url" target="_blank">
					{{ formatMessage(messages.viewOn, { site: link.site }) }}
					<ExternalIcon aria-hidden="true" />
				</ButtonLink>
				<Button :aria-pressed="favorited" @click="toggleFavorite">
					<HeartIcon :class="{ 'fill-current text-brand-red': favorited }" aria-hidden="true" />
					{{ favoriteLabel }}
				</Button>
				<Button :disabled="savingPng || loadFailed" @click="savePng">
					<DownloadIcon aria-hidden="true" />
					{{ formatMessage(messages.savePng) }}
				</Button>
				<Button
					v-tooltip="readOnly ? formatMessage(messages.readOnly) : undefined"
					type="colored"
					color="brand"
					:disabled="readOnly || loadFailed"
					@click="addToMySkins"
				>
					<PlusIcon aria-hidden="true" />
					{{ formatMessage(messages.addToMySkins) }}
				</Button>
			</div>
		</template>
	</NewModal>
</template>
