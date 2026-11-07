<script setup lang="ts">
import {
	ChevronLeftIcon,
	ChevronRightIcon,
	ExternalIcon,
	GlobeIcon,
	ImageIcon,
	LayersIcon,
	PlusIcon,
	SearchIcon,
	ShirtIcon,
	SpinnerIcon,
	UploadIcon,
	UserSearchIcon,
} from '@modrinth/assets'
import {
	Admonition,
	Button,
	ButtonLink,
	defineMessages,
	Input,
	NavTabs,
	useVIntl,
} from '@modrinth/ui'
import { type Component, computed, onMounted } from 'vue'

import { usePrivacy } from '@/features/plus-privacy/use-privacy'
import { injectSettingsActions } from '@/features/settings/settings-actions'

import { type DiscoveredSkin, fromPlayerSkin } from './discovered-skin'
import SkinCard from './skin-card.vue'
import { useSkinGallery } from './skin-gallery'
import {
	GALLERY_SOURCE_ID,
	isLinkedSiteId,
	type LinkedSiteId,
	playerPageLinks,
	type SkinSearchKind,
	skinSourceBrowseUrl,
	type SkinSourceId,
	skinSourceSearchLinks,
} from './skin-sources'

const PLACEHOLDER_CARDS = 12

const props = defineProps<{
	source: SkinSourceId
	previewingKey?: string
	readOnly?: boolean
}>()

const emit = defineEmits<{
	preview: [skin: DiscoveredSkin]
	open: [skin: DiscoveredSkin, event: MouseEvent]
	import: [skin: DiscoveredSkin]
	'add-file': []
}>()

const { formatMessage } = useVIntl()
const { streamerMode } = usePrivacy()
const settingsActions = injectSettingsActions()
const gallery = useSkinGallery()

const messages = defineMessages({
	searchPlaceholder: {
		id: 'plus.skins.find.placeholder',
		defaultMessage: 'Search skins like cat, hoodie or creeper, or a Minecraft username',
	},
	search: { id: 'plus.skins.find.search', defaultMessage: 'Search' },
	recent: { id: 'plus.skins.find.recent', defaultMessage: 'Recent' },
	latest: { id: 'plus.skins.gallery.latest', defaultMessage: 'Latest skins' },
	resultsFor: { id: 'plus.skins.gallery.results-for', defaultMessage: 'Skins matching “{query}”' },
	live: { id: 'plus.skins.gallery.live', defaultMessage: 'Live results from MineSkin' },
	liveSearch: {
		id: 'plus.skins.gallery.live-search',
		defaultMessage: "Live results from MineSkin. These are its closest matches, so some may be loose.",
	},
	empty: {
		id: 'plus.skins.gallery.empty',
		defaultMessage: 'MineSkin found no skins for “{query}”. Try another word, or search another site.',
	},
	emptyLatest: {
		id: 'plus.skins.gallery.empty-latest',
		defaultMessage: "MineSkin didn't return any skins.",
	},
	rateLimited: {
		id: 'plus.skins.gallery.rate-limited',
		defaultMessage: 'MineSkin is limiting requests right now. Try again in a minute.',
	},
	keyRejected: {
		id: 'plus.skins.gallery.key-rejected',
		defaultMessage: 'MineSkin rejected your API key. Check or remove it in Settings.',
	},
	openApiSettings: {
		id: 'plus.curseforge.browse.open-api-settings',
		defaultMessage: 'Open API configuration',
	},
	unavailable: {
		id: 'plus.skins.gallery.unavailable',
		defaultMessage: "Couldn't reach MineSkin. Check your connection and try again.",
	},
	retry: { id: 'plus.skins.find.player.retry', defaultMessage: 'Try again' },
	page: { id: 'plus.skins.gallery.page', defaultMessage: 'Page {page}' },
	pages: { id: 'plus.skins.gallery.pages', defaultMessage: 'Pages {first} to {last}' },
	previous: { id: 'plus.skins.gallery.previous', defaultMessage: 'Previous' },
	next: { id: 'plus.skins.gallery.next', defaultMessage: 'Next' },
	loadMore: { id: 'plus.skins.gallery.load-more', defaultMessage: 'Load more' },
	endOfResults: { id: 'plus.skins.gallery.end', defaultMessage: "That's everything MineSkin returned." },
	moreRateLimited: {
		id: 'plus.skins.gallery.more-rate-limited',
		defaultMessage: "MineSkin is limiting requests, so more couldn't be loaded. Try again in a minute.",
	},
	moreUnavailable: {
		id: 'plus.skins.gallery.more-unavailable',
		defaultMessage: "Couldn't load more from MineSkin. Try again.",
	},
	playerRateLimited: {
		id: 'plus.skins.find.player.rate-limited',
		defaultMessage: 'Mojang is limiting lookups right now. Try again in a minute.',
	},
	playerUnavailable: {
		id: 'plus.skins.find.player.unavailable',
		defaultMessage: "Couldn't reach Mojang's profile service. Check your connection and try again.",
	},
	noCustomSkin: {
		id: 'plus.skins.find.player.default-skin',
		defaultMessage: '{name} has no custom skin.',
	},
	viewOn: { id: 'plus.skins.find.view-on', defaultMessage: 'View on {site}' },
	namemcText: {
		id: 'plus.skins.find.sites.namemc',
		defaultMessage:
			"Find names, UUIDs and tagged skins. NameMC's terms don't allow apps to read its pages, so results open on its own site.",
	},
	skinmcText: {
		id: 'plus.skins.find.sites.skinmc',
		defaultMessage:
			"Find player profiles, skins and community creations. SkinMC doesn't publish a way for apps to search it, so results open on its own site.",
	},
	minecraftskinsText: {
		id: 'plus.skins.find.sites.minecraftskins',
		defaultMessage:
			"Find skins by name or by creator on The Skindex. Its terms don't allow apps to read its pages, so results open on its own site.",
	},
	enterQuery: {
		id: 'plus.skins.find.sites.enter-query',
		defaultMessage: 'Enter something above to search {site}.',
	},
	everything: { id: 'plus.skins.find.search-kind.everything', defaultMessage: 'Search {site}' },
	players: { id: 'plus.skins.find.search-kind.players', defaultMessage: 'Search players' },
	skins: { id: 'plus.skins.find.search-kind.skins', defaultMessage: 'Search skins' },
	creations: { id: 'plus.skins.find.search-kind.creations', defaultMessage: 'Search creations' },
	creators: { id: 'plus.skins.find.search-kind.creators', defaultMessage: 'Search creators' },
	browse: { id: 'plus.skins.find.browse', defaultMessage: 'Browse {site}' },
	addHint: {
		id: 'plus.skins.find.add-hint',
		defaultMessage: 'Found one you like? Download its PNG from the site, then add it here.',
	},
	addFile: { id: 'plus.skins.find.add-file', defaultMessage: 'Add skin from file' },
	readOnly: {
		id: 'plus.skins.find.read-only',
		defaultMessage: "Skins can't be added while Minecraft's servers can't be reached.",
	},
	attribution: {
		id: 'plus.skins.find.attribution',
		defaultMessage:
			"Links open in your browser. These sites aren't connected to Pawrinth, and skins belong to their creators.",
	},
})

const SOURCE_NAMES: Record<SkinSourceId, string> = {
	mineskin: 'MineSkin',
	namemc: 'NameMC',
	skinmc: 'SkinMC',
	minecraftskins: 'MinecraftSkins',
}

const SOURCE_ICONS: Record<SkinSourceId, Component> = {
	mineskin: LayersIcon,
	namemc: UserSearchIcon,
	skinmc: ShirtIcon,
	minecraftskins: ImageIcon,
}

const SOURCE_TEXT: Record<LinkedSiteId, (typeof messages)[keyof typeof messages]> = {
	namemc: messages.namemcText,
	skinmc: messages.skinmcText,
	minecraftskins: messages.minecraftskinsText,
}

const SEARCH_LABELS: Record<SkinSearchKind, (typeof messages)[keyof typeof messages]> = {
	everything: messages.everything,
	players: messages.players,
	skins: messages.skins,
	creations: messages.creations,
	creators: messages.creators,
}

const sourceLinks = computed(() =>
	(Object.keys(SOURCE_NAMES) as SkinSourceId[]).map((id) => ({
		label: SOURCE_NAMES[id],
		href: id,
		icon: SOURCE_ICONS[id],
	})),
)
const linkedSite = computed(() => (isLinkedSiteId(props.source) ? props.source : null))
const linkedSiteName = computed(() => SOURCE_NAMES[props.source])
const searchLinks = computed(() =>
	linkedSite.value ? skinSourceSearchLinks(linkedSite.value, gallery.typedQuery.value) : [],
)
const browseUrl = computed(() => (linkedSite.value ? skinSourceBrowseUrl(linkedSite.value) : ''))
const emptySearchLinks = computed(() =>
	(['namemc', 'skinmc', 'minecraftskins'] as const).map((id) => ({
		site: SOURCE_NAMES[id],
		url: skinSourceSearchLinks(id, gallery.activeQuery.value)[0]?.url ?? skinSourceBrowseUrl(id),
	})),
)
const playerSkin = computed(() =>
	gallery.player.value ? fromPlayerSkin(gallery.player.value) : null,
)
const playerLinks = computed(() =>
	gallery.player.value ? playerPageLinks(gallery.player.value) : [],
)
const playerNote = computed(() => {
	if (gallery.playerStatus.value === 'rate_limited') return formatMessage(messages.playerRateLimited)
	if (gallery.playerStatus.value === 'unavailable') return formatMessage(messages.playerUnavailable)
	return undefined
})
const pageLabel = computed(() => {
	const first = gallery.firstShownPage.value + 1
	const last = gallery.lastShownPage.value + 1
	return first === last
		? formatMessage(messages.page, { page: first })
		: formatMessage(messages.pages, { first, last })
})
const moreFailedText = computed(() => {
	if (gallery.moreFailed.value === 'rate_limited') return formatMessage(messages.moreRateLimited)
	if (gallery.moreFailed.value === 'key_rejected') return formatMessage(messages.keyRejected)
	return gallery.moreFailed.value ? formatMessage(messages.moreUnavailable) : undefined
})

onMounted(gallery.loadOnce)

function submit(): void {
	void gallery.searchSkins(gallery.typedQuery.value)
}
</script>

<template>
	<div class="flex flex-col gap-5">
		<div class="flex flex-col gap-2">
			<form class="flex items-center gap-2" role="search" @submit.prevent="submit">
				<Input
					v-model="gallery.typedQuery.value"
					:icon="SearchIcon"
					type="text"
					size="large"
					wrapper-class="grow"
					autocomplete="off"
					clearable
					:spellcheck="false"
					:placeholder="formatMessage(messages.searchPlaceholder)"
					@clear="gallery.searchSkins('')"
				/>
				<Button
					color="brand"
					type="colored"
					size="lg"
					native-type="submit"
					:disabled="gallery.status.value === 'loading'"
				>
					<SpinnerIcon
						v-if="gallery.status.value === 'loading'"
						class="animate-spin"
						aria-hidden="true"
					/>
					<SearchIcon v-else aria-hidden="true" />
					{{ formatMessage(messages.search) }}
				</Button>
			</form>
			<div
				v-if="gallery.recentSearches.value.length > 0 && !streamerMode"
				class="flex flex-wrap items-center gap-1 text-sm text-secondary"
			>
				<span class="mr-1">{{ formatMessage(messages.recent) }}</span>
				<Button
					v-for="item in gallery.recentSearches.value"
					:key="item"
					type="quiet"
					size="sm"
					@click="gallery.searchSkins(item)"
				>
					{{ item }}
				</Button>
			</div>
		</div>

		<NavTabs :links="sourceLinks" query="source" replace class="max-w-full flex-wrap !rounded-[1.75rem]" />

		<template v-if="source === GALLERY_SOURCE_ID">
			<section class="flex flex-col gap-3" aria-live="polite" data-skin-gallery>
				<div>
					<h2 class="m-0 text-lg font-semibold text-contrast">
						{{
							gallery.activeQuery.value
								? formatMessage(messages.resultsFor, { query: gallery.activeQuery.value })
								: formatMessage(messages.latest)
						}}
					</h2>
					<p class="m-0 text-sm text-secondary">
						{{ formatMessage(gallery.activeQuery.value ? messages.liveSearch : messages.live) }}
					</p>
				</div>

				<p v-if="playerNote" class="m-0 text-sm text-secondary" data-player-note>{{ playerNote }}</p>
				<div
					v-if="gallery.player.value && !playerSkin"
					class="flex flex-wrap items-center gap-2"
					data-player-result
				>
					<span class="text-secondary">
						{{ formatMessage(messages.noCustomSkin, { name: gallery.player.value.name }) }}
					</span>
					<ButtonLink v-for="link in playerLinks" :key="link.source" :href="link.url" target="_blank">
						{{ formatMessage(messages.viewOn, { site: SOURCE_NAMES[link.source] }) }}
						<ExternalIcon aria-hidden="true" />
					</ButtonLink>
				</div>

				<Admonition
					v-if="
						gallery.status.value === 'rate_limited' ||
						gallery.status.value === 'unavailable' ||
						gallery.status.value === 'key_rejected'
					"
					:type="gallery.status.value === 'rate_limited' ? 'warning' : 'critical'"
					inline-actions
				>
					{{
						formatMessage(
							gallery.status.value === 'rate_limited'
								? messages.rateLimited
								: gallery.status.value === 'key_rejected'
									? messages.keyRejected
									: messages.unavailable,
						)
					}}
					<template #actions>
						<Button
							v-if="gallery.status.value === 'key_rejected'"
							@click="settingsActions.openApiConfiguration()"
						>
							{{ formatMessage(messages.openApiSettings) }}
						</Button>
						<Button @click="gallery.retry()">{{ formatMessage(messages.retry) }}</Button>
					</template>
				</Admonition>

				<div
					v-if="gallery.status.value === 'ready' && gallery.shownSkins.value.length === 0"
					class="flex flex-col gap-3"
					data-skin-empty
				>
					<p class="m-0 text-secondary">
						{{
							gallery.activeQuery.value
								? formatMessage(messages.empty, { query: gallery.activeQuery.value })
								: formatMessage(messages.emptyLatest)
						}}
					</p>
					<div v-if="gallery.activeQuery.value" class="flex flex-wrap gap-2">
						<ButtonLink v-for="link in emptySearchLinks" :key="link.site" :href="link.url" target="_blank">
							{{ formatMessage(messages.everything, { site: link.site }) }}
							<ExternalIcon aria-hidden="true" />
						</ButtonLink>
					</div>
				</div>

				<div
					v-if="playerSkin || gallery.status.value === 'loading' || gallery.shownSkins.value.length > 0"
					class="grid grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] gap-3"
				>
					<SkinCard
						v-if="playerSkin"
						:key="`player-${playerSkin.key}`"
						:skin="playerSkin"
						:previewing="previewingKey === playerSkin.key"
						:read-only="readOnly"
						data-player-result
						@preview="(found) => emit('preview', found)"
						@open="(found, event) => emit('open', found, event)"
						@import="(found) => emit('import', found)"
					/>
					<template v-if="gallery.status.value === 'loading'">
						<div
							v-for="index in PLACEHOLDER_CARDS"
							:key="index"
							class="flex animate-pulse flex-col gap-2 rounded-2xl border border-solid border-surface-5 bg-surface-3 p-2"
							data-skin-placeholders
						>
							<div class="aspect-[31/40] w-full rounded-xl bg-surface-4" />
							<div class="h-4 w-3/4 rounded bg-surface-4" />
							<div class="h-8 w-full rounded-xl bg-surface-4" />
						</div>
					</template>
					<SkinCard
						v-for="skin in gallery.shownSkins.value"
						:key="skin.key"
						:skin="skin"
						:previewing="previewingKey === skin.key"
						:read-only="readOnly"
						@preview="(found) => emit('preview', found)"
						@open="(found, event) => emit('open', found, event)"
						@import="(found) => emit('import', found)"
					/>
				</div>

				<template v-if="gallery.shownSkins.value.length > 0">
					<Admonition v-if="moreFailedText" type="warning">{{ moreFailedText }}</Admonition>

					<div class="flex flex-wrap items-center justify-between gap-2" data-skin-pagination>
						<div class="flex items-center gap-2">
							<Button :disabled="!gallery.hasPrevious.value" @click="gallery.showPreviousPage()">
								<ChevronLeftIcon aria-hidden="true" />
								{{ formatMessage(messages.previous) }}
							</Button>
							<span class="text-sm font-semibold text-primary">{{ pageLabel }}</span>
							<Button
								:disabled="!gallery.hasNext.value || gallery.fetchingMore.value"
								@click="gallery.showNextPage()"
							>
								{{ formatMessage(messages.next) }}
								<ChevronRightIcon aria-hidden="true" />
							</Button>
						</div>
						<Button
							v-if="gallery.hasNext.value"
							:disabled="gallery.fetchingMore.value"
							@click="gallery.loadMore()"
						>
							<SpinnerIcon v-if="gallery.fetchingMore.value" class="animate-spin" aria-hidden="true" />
							<PlusIcon v-else aria-hidden="true" />
							{{ formatMessage(messages.loadMore) }}
						</Button>
						<span v-else class="text-sm text-secondary">{{ formatMessage(messages.endOfResults) }}</span>
					</div>
				</template>
			</section>
		</template>

		<section v-else-if="linkedSite" class="flex flex-col gap-3" data-linked-site>
			<div
				v-if="playerSkin"
				class="grid grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] gap-3"
			>
				<SkinCard
					:skin="playerSkin"
					:previewing="previewingKey === playerSkin.key"
					:read-only="readOnly"
					data-player-result
					@preview="(found) => emit('preview', found)"
					@open="(found, event) => emit('open', found, event)"
					@import="(found) => emit('import', found)"
				/>
			</div>
			<div
				class="flex flex-col gap-3 rounded-2xl border border-solid border-surface-5 bg-surface-3 p-4"
			>
				<p class="m-0 text-primary">{{ formatMessage(SOURCE_TEXT[linkedSite]) }}</p>
				<p v-if="searchLinks.length === 0" class="m-0 text-sm text-secondary">
					{{ formatMessage(messages.enterQuery, { site: linkedSiteName }) }}
				</p>
				<div class="flex flex-wrap gap-2">
					<ButtonLink
						v-for="link in searchLinks"
						:key="link.kind"
						:href="link.url"
						target="_blank"
						color="brand"
						type="outlined"
					>
						<SearchIcon aria-hidden="true" />
						{{ formatMessage(SEARCH_LABELS[link.kind], { site: linkedSiteName }) }}
						<ExternalIcon aria-hidden="true" />
					</ButtonLink>
					<ButtonLink :href="browseUrl" target="_blank">
						<GlobeIcon aria-hidden="true" />
						{{ formatMessage(messages.browse, { site: linkedSiteName }) }}
						<ExternalIcon aria-hidden="true" />
					</ButtonLink>
				</div>
				<div
					class="flex flex-wrap items-center justify-between gap-2 border-0 border-t border-solid border-surface-5 pt-3"
				>
					<span class="text-sm text-secondary">{{ formatMessage(messages.addHint) }}</span>
					<Button
						v-tooltip="readOnly ? formatMessage(messages.readOnly) : undefined"
						:disabled="readOnly"
						@click="emit('add-file')"
					>
						<UploadIcon aria-hidden="true" />
						{{ formatMessage(messages.addFile) }}
					</Button>
				</div>
			</div>
			<p class="m-0 text-xs text-secondary">{{ formatMessage(messages.attribution) }}</p>
		</section>
	</div>
</template>
