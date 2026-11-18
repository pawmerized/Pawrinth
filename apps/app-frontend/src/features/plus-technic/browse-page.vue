<script setup lang="ts">
import { CompassIcon, PlusIcon, SpinnerIcon } from '@modrinth/assets'
import type { CardAction } from '@modrinth/ui'
import {
	Admonition,
	BrowsePageLayout,
	Button,
	commonMessages,
	defineMessages,
	getProjectTypeIcon,
	provideBrowseManager,
	useVIntl,
} from '@modrinth/ui'
import { useDebounceFn } from '@vueuse/core'
import { computed, onMounted, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { errorText } from '@/features/plus-curseforge/project-adapter'
import ProviderSwitch from '@/features/plus-curseforge/provider-switch.vue'
import { plus_technic_browse } from '@/helpers/plus'
import { useRootBreadcrumb } from '@/shared/breadcrumbs'

import { type TechnicSearchHit, toTechnicHit } from './technic-hit'
import TechnicInstall from './technic-install.vue'
import { technicPackPath, technicPreparing } from './technic-state'

const { formatMessage } = useVIntl()
const route = useRoute()
const router = useRouter()

const messages = defineMessages({
	title: { id: 'plus.technic.discover', defaultMessage: 'Discover on Technic' },
	modpacks: { id: 'plus.technic.modpacks', defaultMessage: 'Modpacks' },
	trending: { id: 'plus.technic.sort.trending', defaultMessage: 'Trending' },
	relevance: { id: 'plus.technic.sort.relevance', defaultMessage: 'Relevance' },
	newest: { id: 'plus.technic.sort.newest', defaultMessage: 'Newest' },
	name: { id: 'plus.technic.sort.name', defaultMessage: 'Name' },
	gathered: {
		id: 'plus.technic.gathered',
		defaultMessage:
			"Technic has no list of all its modpacks, so these are its trending packs followed by the ones Pawrinth found through Technic's search. Search to find any pack that isn't here.",
	},
	searchLimit: {
		id: 'plus.technic.search-limit',
		defaultMessage:
			'Technic sends at most 50 results for a search, so try a more exact name if a pack is missing.',
	},
	failedHeader: { id: 'plus.technic.failed', defaultMessage: "Couldn't load Technic" },
	retry: { id: 'plus.technic.retry', defaultMessage: 'Try again' },
})

/** Searches keep Technic's own order. The gathered list can also be reordered. */
const TRENDING = { name: 'trending', display: formatMessage(messages.trending) }
const NEWEST = { name: 'newest', display: formatMessage(messages.newest) }
const NAME = { name: 'name', display: formatMessage(messages.name) }
const RELEVANCE = { name: 'relevance', display: formatMessage(messages.relevance) }
const BROWSE_SORTS = [TRENDING, NEWEST, NAME]

const query = ref(typeof route.query.q === 'string' ? route.query.q : '')
const searching = computed(() => query.value.trim().length > 0)
const sortType = shallowRef(searching.value ? RELEVANCE : TRENDING)
const maxResults = ref(10)
const currentPage = ref(1)
const totalHits = ref(0)
const projectHits = shallowRef<TechnicSearchHit[]>([])
const loading = ref(true)
const refreshing = ref(false)
const loadError = ref<string | null>(null)
const installer = ref<InstanceType<typeof TechnicInstall> | null>(null)
let request = 0

async function load(): Promise<void> {
	const id = ++request
	if (projectHits.value.length === 0) loading.value = true
	else refreshing.value = true
	loadError.value = null
	try {
		const page = await plus_technic_browse(
			query.value.trim() || null,
			(currentPage.value - 1) * maxResults.value,
			maxResults.value,
			searching.value ? null : sortType.value.name,
		)
		if (id !== request) return
		projectHits.value = page.packs.map(toTechnicHit)
		totalHits.value = page.total
	} catch (error) {
		if (id !== request) return
		projectHits.value = []
		totalHits.value = 0
		loadError.value = errorText(error)
	} finally {
		if (id === request) {
			loading.value = false
			refreshing.value = false
		}
	}
}

const search = useDebounceFn(() => {
	const text = query.value.trim()
	void router.replace({ query: text ? { q: text } : {} })
	currentPage.value = 1
	void load()
}, 400)

watch(query, () => void search())
watch(searching, (value) => (sortType.value = value ? RELEVANCE : TRENDING))
watch(sortType, (value, previous) => {
	if (searching.value || value.name === previous.name) return
	currentPage.value = 1
	void load()
})
watch(maxResults, () => {
	currentPage.value = 1
	void load()
})

function getCardActions(result: TechnicSearchHit): CardAction[] {
	const preparing = technicPreparing.value === result.slug
	return [
		{
			key: 'install',
			label: formatMessage(
				preparing ? commonMessages.installingLabel : commonMessages.installButton,
			),
			icon: preparing ? SpinnerIcon : PlusIcon,
			iconClass: preparing ? 'animate-spin' : undefined,
			disabled: technicPreparing.value !== null,
			color: 'brand',
			type: 'outlined',
			onClick: () => installer.value?.install(result.slug, result.name),
		},
	]
}

provideBrowseManager({
	tags: ref({ gameVersions: [], loaders: [], categories: [] }),
	projectType: computed(() => 'modpack'),
	query,
	filters: computed(() => []),
	currentFilters: ref([]),
	toggledGroups: ref([]),
	overriddenProvidedFilterTypes: ref([]),
	providedFilters: computed(() => []),
	installContext: computed(() => null),
	serverFilterTypes: computed(() => []),
	serverCurrentFilters: ref([]),
	serverToggledGroups: ref([]),
	effectiveSortTypes: computed(() => (searching.value ? [RELEVANCE] : BROWSE_SORTS)),
	effectiveCurrentSortType: sortType,
	loading,
	refreshing,
	projectHits,
	serverHits: shallowRef([]),
	totalHits,
	pageCount: computed(() => Math.max(1, Math.ceil(totalHits.value / maxResults.value))),
	maxResults,
	currentPage,
	isServerType: computed(() => false),
	effectiveLayout: computed(() => 'list' as const),
	deprioritizedTags: computed(() => []),
	excludeLoaders: computed(() => false),
	loadFailed: computed(() => loadError.value !== null),
	refreshSearch: load,
	setPage: async (page: number) => {
		currentPage.value = page
		await load()
	},
	clearSearch: () => {
		query.value = ''
	},
	onFilterChange: () => {},
	linkOverridesAdvancedPrefs: ref(false),
	applySavedAdvancedPrefs: () => {},
	getProjectLink: (result: TechnicSearchHit) => technicPackPath(result.slug),
	getServerProjectLink: () => '',
	getAuthorLink: (result: TechnicSearchHit) => technicPackPath(result.slug),
	selectableProjectTypes: computed(() => [
		{
			label: formatMessage(messages.modpacks),
			href: '/technic',
			icon: getProjectTypeIcon('modpack'),
		},
	]),
	showProjectTypeTabs: computed(() => true),
	variant: 'app',
	getCardActions: (result: TechnicSearchHit) => getCardActions(result),
	maxResultsOptions: computed(() => [5, 10, 20]),
} as never)

useRootBreadcrumb({
	slot: 'browse',
	id: () => 'technic-browse',
	label: () => formatMessage(messages.title),
	to: () => route.fullPath,
	visual: { type: 'icon', component: CompassIcon },
})

onMounted(load)
</script>

<template>
	<div class="flex flex-col gap-2 p-6">
		<BrowsePageLayout>
			<template #before-tabs>
				<ProviderSwitch />
				<p v-if="!searching" class="m-0 text-sm text-secondary">
					{{ formatMessage(messages.gathered) }}
				</p>
				<p v-else-if="totalHits >= 50" class="m-0 text-sm text-secondary">
					{{ formatMessage(messages.searchLimit) }}
				</p>
				<Admonition
					v-if="loadError"
					type="critical"
					:header="formatMessage(messages.failedHeader)"
					inline-actions
				>
					{{ loadError }}
					<template #actions>
						<Button @click="load">{{ formatMessage(messages.retry) }}</Button>
					</template>
				</Admonition>
			</template>
		</BrowsePageLayout>
		<TechnicInstall ref="installer" />
	</div>
</template>
