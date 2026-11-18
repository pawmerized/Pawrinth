<script setup lang="ts">
import { CheckIcon, PlusIcon, SearchIcon, SpinnerIcon, TrashIcon } from '@modrinth/assets'
import {
	Avatar,
	Button,
	defineMessages,
	injectNotificationManager,
	Input,
	SegmentedControl,
	useVIntl,
} from '@modrinth/ui'
import { useDebounceFn } from '@vueuse/core'
import { computed, onMounted, ref, watch } from 'vue'

import { LOADER_NAMES as CURSEFORGE_LOADERS } from '@/features/plus-curseforge/provider-mapping'
import { loaderName } from '@/features/plus-install/loader-names'
import { usePlusSettings } from '@/features/plus-settings/use-plus-settings'
import { get_search_results_v3 } from '@/helpers/cache.js'
import { type LibraryMod, plus_cf_data_mode, plus_cf_search } from '@/helpers/plus'

import { findSame } from './library-match'
import { addToLibrary, saveLibrary } from './mod-library'

type Source = LibraryMod['source']

interface Result {
	mod: LibraryMod
	author: string
	summary: string
	loaders: string[]
	downloads: number
}

const PAGE_SIZE = 20
/** CurseForge's class for Minecraft mods. */
const CURSEFORGE_MODS_CLASS = 6
/** CurseForge has no relevance ranking, so matches come most popular first. */
const CURSEFORGE_POPULARITY = 2

const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const { settings } = usePlusSettings()

const messages = defineMessages({
	title: { id: 'plus.mod-library.browse.title', defaultMessage: 'Find mods' },
	description: {
		id: 'plus.mod-library.browse.description',
		defaultMessage:
			'Saving a mod only adds it to your library. Nothing is installed until you create an instance.',
	},
	search: { id: 'plus.mod-library.browse.search', defaultMessage: 'Search mods…' },
	add: { id: 'plus.mod-library.browse.add', defaultMessage: 'Add' },
	saved: { id: 'plus.mod-library.browse.saved', defaultMessage: 'Saved' },
	savedFrom: {
		id: 'plus.mod-library.browse.saved-from',
		defaultMessage: 'Saved from {source}',
	},
	remove: { id: 'plus.mod-library.browse.remove', defaultMessage: 'Remove {name}' },
	by: { id: 'plus.mod-library.browse.by', defaultMessage: 'by {author}' },
	downloads: {
		id: 'plus.mod-library.browse.downloads',
		defaultMessage: '{count} downloads',
	},
	more: { id: 'plus.mod-library.browse.more', defaultMessage: 'Show more' },
	empty: { id: 'plus.mod-library.browse.empty', defaultMessage: 'No mods match your search.' },
	failed: {
		id: 'plus.mod-library.browse.failed',
		defaultMessage: 'Couldn’t search {source}. Check your connection and try again.',
	},
	retry: { id: 'plus.mod-library.browse.retry', defaultMessage: 'Try again' },
	curseforgeOff: {
		id: 'plus.mod-library.browse.curseforge-off',
		defaultMessage:
			'CurseForge needs to be set up first: add a key or turn on curse.tools in API configuration.',
	},
	added: { id: 'plus.mod-library.added', defaultMessage: 'Added {name} to your mod library' },
	already: {
		id: 'plus.mod-library.already',
		defaultMessage: '{name} is already in your mod library',
	},
	modrinth: { id: 'plus.mod-library.source.modrinth', defaultMessage: 'Modrinth' },
	curseforge: { id: 'plus.mod-library.source.curseforge', defaultMessage: 'CurseForge' },
	sourceLabel: { id: 'plus.mod-library.source', defaultMessage: 'Site to search' },
})

const SOURCES: Source[] = ['modrinth', 'curseforge']
const source = ref<Source>('modrinth')
const query = ref('')
const results = ref<Result[]>([])
const total = ref(0)
const loading = ref(false)
const failed = ref(false)
const curseforgeReady = ref(true)
let searchRun = 0

const library = computed(() => settings.value?.mod_library.mods ?? [])
const compactNumber = new Intl.NumberFormat(undefined, { notation: 'compact' })

function modrinthUrl(offset: number): string {
	const params = [
		`limit=${PAGE_SIZE}`,
		`index=${query.value.trim() ? 'relevance' : 'downloads'}`,
		`new_filters=${encodeURIComponent('project_types = `mod`')}`,
	]
	if (query.value.trim()) params.push(`query=${encodeURIComponent(query.value.trim())}`)
	if (offset > 0) params.push(`offset=${offset}`)
	return `?${params.join('&')}`
}

async function fetchPage(offset: number): Promise<{ items: Result[]; total: number }> {
	if (source.value === 'modrinth') {
		const raw = (await get_search_results_v3(modrinthUrl(offset), 'must_revalidate')) as {
			result?: { hits: ModrinthHit[]; total_hits: number }
			hits?: ModrinthHit[]
			total_hits?: number
		}
		const page = raw.result ?? raw
		return {
			total: page.total_hits ?? 0,
			items: (page.hits ?? []).map((hit) => ({
				mod: {
					source: 'modrinth',
					project_id: hit.project_id,
					name: hit.name,
					slug: hit.slug ?? null,
					icon_url: hit.icon_url ?? null,
					auto_install: true,
				},
				author: hit.author,
				summary: hit.summary,
				loaders: hit.loaders ?? [],
				downloads: hit.downloads,
			})),
		}
	}
	const page = await plus_cf_search({
		classId: CURSEFORGE_MODS_CLASS,
		searchFilter: query.value.trim() || null,
		sortField: CURSEFORGE_POPULARITY,
		sortOrder: 'desc',
		index: offset,
		pageSize: PAGE_SIZE,
	})
	return {
		total: page.pagination.totalCount,
		items: page.data.map((project) => ({
			mod: {
				source: 'curseforge',
				project_id: String(project.id),
				name: project.name,
				slug: project.slug,
				icon_url: project.logo?.thumbnailUrl ?? null,
				auto_install: true,
			},
			author: project.authors[0]?.name ?? '',
			summary: project.summary,
			loaders: [
				...new Set(
					project.latestFilesIndexes
						.map((index) => (index.modLoader === null ? '' : CURSEFORGE_LOADERS[index.modLoader]))
						.filter((loader): loader is string => !!loader),
				),
			],
			downloads: project.downloadCount,
		})),
	}
}

interface ModrinthHit {
	project_id: string
	name: string
	slug: string | null
	icon_url: string | null
	author: string
	summary: string
	loaders?: string[]
	downloads: number
}

async function search(more = false): Promise<void> {
	const run = ++searchRun
	failed.value = false
	if (source.value === 'curseforge') {
		curseforgeReady.value = (await plus_cf_data_mode().catch(() => 'needs_key')) === 'live'
		if (!curseforgeReady.value) {
			results.value = []
			return
		}
	}
	loading.value = true
	try {
		const page = await fetchPage(more ? results.value.length : 0)
		if (run !== searchRun) return
		results.value = more ? [...results.value, ...page.items] : page.items
		total.value = page.total
	} catch {
		if (run === searchRun) failed.value = true
	} finally {
		if (run === searchRun) loading.value = false
	}
}

const searchSoon = useDebounceFn(() => search(), 400)
watch(query, () => void searchSoon())
watch(source, () => void search())
onMounted(() => void search())

/** The library entry for this result, or for the same mod saved from the other site. */
function savedAs(result: Result): LibraryMod | null {
	return findSame(library.value, result.mod)
}

async function add(result: Result): Promise<void> {
	try {
		const outcome = await addToLibrary(result.mod)
		addNotification({
			type: outcome.added ? 'success' : 'info',
			title: outcome.added
				? formatMessage(messages.added, { name: result.mod.name })
				: formatMessage(messages.already, { name: outcome.existing.name }),
		})
	} catch (error) {
		handleError(error as Error)
	}
}

async function remove(saved: LibraryMod): Promise<void> {
	try {
		await saveLibrary(
			library.value.filter(
				(item) => !(item.source === saved.source && item.project_id === saved.project_id),
			),
		)
	} catch (error) {
		handleError(error as Error)
	}
}
</script>

<template>
	<div class="flex flex-col gap-3" data-library-browse>
		<div>
			<h3 class="m-0 text-lg font-semibold text-contrast">{{ formatMessage(messages.title) }}</h3>
			<p class="m-0 mt-1 text-sm text-secondary">{{ formatMessage(messages.description) }}</p>
		</div>
		<div class="flex flex-wrap items-center gap-2">
			<SegmentedControl
				v-model="source"
				:items="SOURCES"
				:format-label="(option) => formatMessage(messages[option])"
				:aria-label="formatMessage(messages.sourceLabel)"
			/>
			<Input
				v-model="query"
				:icon="SearchIcon"
				:placeholder="formatMessage(messages.search)"
				wrapper-class="min-w-[12rem] flex-1"
				clearable
			/>
		</div>

		<p v-if="source === 'curseforge' && !curseforgeReady" class="m-0 text-secondary">
			{{ formatMessage(messages.curseforgeOff) }}
		</p>
		<div v-else-if="failed" class="flex flex-wrap items-center gap-3">
			<p class="m-0 text-red">
				{{ formatMessage(messages.failed, { source: formatMessage(messages[source]) }) }}
			</p>
			<Button @click="search()">{{ formatMessage(messages.retry) }}</Button>
		</div>
		<p v-else-if="!loading && results.length === 0" class="m-0 text-secondary">
			{{ formatMessage(messages.empty) }}
		</p>

		<ul v-if="results.length > 0" class="m-0 flex list-none flex-col gap-2 p-0">
			<li
				v-for="result in results"
				:key="`${result.mod.source}:${result.mod.project_id}`"
				class="flex items-start gap-3 rounded-xl bg-surface-2 p-3"
			>
				<Avatar
					:src="result.mod.icon_url ?? undefined"
					size="3rem"
					:tint-by="result.mod.project_id"
					alt=""
					pad-transparent-corners
				/>
				<div class="flex min-w-0 flex-1 flex-col gap-1">
					<div class="flex flex-wrap items-baseline gap-x-2">
						<span class="font-semibold text-contrast">{{ result.mod.name }}</span>
						<span v-if="result.author" class="text-sm text-secondary">
							{{ formatMessage(messages.by, { author: result.author }) }}
						</span>
					</div>
					<p class="m-0 line-clamp-2 text-sm text-primary">{{ result.summary }}</p>
					<div class="flex flex-wrap items-center gap-1.5 text-xs text-secondary">
						<span
							v-for="loader in result.loaders"
							:key="loader"
							class="rounded-full bg-button-bg px-2 py-0.5 font-semibold"
						>
							{{ loaderName(loader) }}
						</span>
						<span>
							{{
								formatMessage(messages.downloads, { count: compactNumber.format(result.downloads) })
							}}
						</span>
					</div>
				</div>
				<div class="flex shrink-0 flex-col items-end gap-1">
					<template v-if="savedAs(result)">
						<span class="flex items-center gap-1 text-sm font-semibold text-brand">
							<CheckIcon aria-hidden="true" />
							{{
								savedAs(result)!.source === result.mod.source
									? formatMessage(messages.saved)
									: formatMessage(messages.savedFrom, {
											source: formatMessage(messages[savedAs(result)!.source]),
										})
							}}
						</span>
						<Button
							type="quiet"
							color="red"
							:aria-label="formatMessage(messages.remove, { name: savedAs(result)!.name })"
							@click="remove(savedAs(result)!)"
						>
							<TrashIcon aria-hidden="true" />
						</Button>
					</template>
					<Button v-else color="brand" type="outlined" @click="add(result)">
						<PlusIcon aria-hidden="true" />
						{{ formatMessage(messages.add) }}
					</Button>
				</div>
			</li>
		</ul>

		<div class="flex justify-center">
			<SpinnerIcon v-if="loading" class="size-6 animate-spin text-secondary" aria-hidden="true" />
			<Button v-else-if="results.length > 0 && results.length < total" @click="search(true)">
				{{ formatMessage(messages.more) }}
			</Button>
		</div>
	</div>
</template>
