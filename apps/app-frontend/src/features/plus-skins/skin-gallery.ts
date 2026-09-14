import { computed, ref, shallowRef } from 'vue'

import { type PlayerSkin, plus_skin_gallery, plus_skin_lookup } from '@/helpers/plus'

import { type DiscoveredSkin, fromGallerySkin, newSkinsOnly } from './discovered-skin'
import { couldBePlayer, withRecentSearch } from './skin-sources'

type GalleryStatus = 'idle' | 'loading' | 'ready' | 'rate_limited' | 'key_rejected' | 'unavailable'
type PlayerStatus = 'none' | 'searching' | 'found' | 'rate_limited' | 'unavailable'

/** A page can repeat everything already shown, so a few more are tried before stopping. */
const MAX_REPEATED_PAGES = 3

const typedQuery = ref('')
const activeQuery = ref('')
const status = ref<GalleryStatus>('idle')
const pages = shallowRef<DiscoveredSkin[][]>([])
const firstShownPage = ref(0)
const lastShownPage = ref(0)
const nextCursor = ref<string | null>(null)
const fetchingMore = ref(false)
const moreFailed = ref<Exclude<GalleryStatus, 'idle' | 'loading' | 'ready'> | null>(null)
const player = shallowRef<PlayerSkin | null>(null)
const playerStatus = ref<PlayerStatus>('none')
const recentSearches = ref<string[]>([])

let seen = new Set<string>()
let latestSearch = 0

const shownSkins = computed(() =>
	pages.value.slice(firstShownPage.value, lastShownPage.value + 1).flat(),
)
const hasPrevious = computed(() => firstShownPage.value > 0)
const hasNext = computed(
	() => lastShownPage.value < pages.value.length - 1 || nextCursor.value !== null,
)

async function fetchPage(request: number): Promise<boolean> {
	const query = activeQuery.value || null
	for (let attempt = 0; attempt < MAX_REPEATED_PAGES; attempt++) {
		const page = await plus_skin_gallery(query, nextCursor.value).catch(() => ({
			status: 'unavailable' as const,
		}))
		if (request !== latestSearch) return false
		if (page.status !== 'ok') {
			if (pages.value.length === 0) status.value = page.status
			else moreFailed.value = page.status
			return false
		}
		const fresh = newSkinsOnly(page.skins.map(fromGallerySkin), seen)
		nextCursor.value = page.next
		if (fresh.length > 0) {
			pages.value = [...pages.value, fresh]
			return true
		}
		if (page.next === null) return false
	}
	return false
}

async function loadFirstPage(): Promise<void> {
	const request = ++latestSearch
	seen = new Set()
	pages.value = []
	firstShownPage.value = 0
	lastShownPage.value = 0
	nextCursor.value = null
	moreFailed.value = null
	status.value = 'loading'
	await fetchPage(request)
	if (request === latestSearch && status.value === 'loading') status.value = 'ready'
}

async function lookUpPlayer(text: string, request: number): Promise<void> {
	player.value = null
	if (!couldBePlayer(text)) {
		playerStatus.value = 'none'
		return
	}
	playerStatus.value = 'searching'
	const result = await plus_skin_lookup(text).catch(() => ({ status: 'unavailable' as const }))
	if (request !== latestSearch) return
	if (result.status === 'found') {
		player.value = result.player
		playerStatus.value = 'found'
	} else {
		playerStatus.value = result.status === 'not_found' ? 'none' : result.status
	}
}

async function searchSkins(text: string): Promise<void> {
	const trimmed = text.trim()
	typedQuery.value = trimmed
	activeQuery.value = trimmed
	if (trimmed) recentSearches.value = withRecentSearch(recentSearches.value, trimmed)
	const gallery = loadFirstPage()
	const request = latestSearch
	if (trimmed) {
		await Promise.all([gallery, lookUpPlayer(trimmed, request)])
	} else {
		player.value = null
		playerStatus.value = 'none'
		await gallery
	}
}

async function fetchNextIfNeeded(): Promise<boolean> {
	if (lastShownPage.value < pages.value.length - 1) return true
	if (nextCursor.value === null || fetchingMore.value) return false
	const request = latestSearch
	fetchingMore.value = true
	moreFailed.value = null
	const fetched = await fetchPage(request)
	if (request === latestSearch) fetchingMore.value = false
	return fetched
}

async function showNextPage(): Promise<void> {
	if (await fetchNextIfNeeded()) {
		lastShownPage.value += 1
		firstShownPage.value = lastShownPage.value
	}
}

function showPreviousPage(): void {
	if (!hasPrevious.value) return
	firstShownPage.value -= 1
	lastShownPage.value = firstShownPage.value
}

async function loadMore(): Promise<void> {
	if (await fetchNextIfNeeded()) lastShownPage.value += 1
}

/** Results stay while moving around the app, and only load the first time. */
function loadOnce(): void {
	if (status.value === 'idle') void loadFirstPage()
}

export function useSkinGallery() {
	return {
		typedQuery,
		activeQuery,
		status,
		shownSkins,
		firstShownPage,
		lastShownPage,
		hasPrevious,
		hasNext,
		fetchingMore,
		moreFailed,
		player,
		playerStatus,
		recentSearches,
		loadOnce,
		retry: () => searchSkins(activeQuery.value),
		searchSkins,
		showNextPage,
		showPreviousPage,
		loadMore,
	}
}
