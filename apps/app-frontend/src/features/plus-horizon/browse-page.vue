<script setup lang="ts">
import {
	CheckIcon,
	CompassIcon,
	DownloadIcon,
	getCategoryIcon,
	PlusIcon,
	SpinnerIcon,
} from '@modrinth/assets'
import type { CardAction } from '@modrinth/ui'
import {
	Admonition,
	BrowsePageLayout,
	BrowseSidebar,
	Button,
	commonMessages,
	defineMessages,
	formatCategory,
	formatCategoryHeader,
	formatProjectTypeSentence,
	getProjectTypeIcon,
	injectNotificationManager,
	provideBrowseManager,
	useVIntl,
} from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref, shallowRef, watch } from 'vue'
import { useRoute } from 'vue-router'

import { errorText } from '@/features/plus-curseforge/project-adapter'
import { getInstanceIconUrl } from '@/helpers/instance'
import { get_categories } from '@/helpers/tags'
import { instanceDetailQueryOptions } from '@/pages/instance/query-options'
import { type BreadcrumbDefinition, injectBreadcrumbManager } from '@/shared/breadcrumbs'

import { HORIZON_LOADER } from './horizon'
import {
	categoryGroups,
	type CategoryTag,
	findPacks,
	type HorizonPack,
	horizonPacksPath,
	type HorizonSearchHit,
	type HorizonSort,
	kindFromPath,
	matchesFilters,
	plus_horizon_pack_install,
	plus_horizon_packs,
	toHorizonHit,
} from './horizon-packs'
import { useInstanceLoader } from './instance-loader'

const BROWSE_ROUTE = 'Discover on Horizon'

const { formatMessage } = useVIntl()
const route = useRoute()
const loaderOf = useInstanceLoader()
const { addNotification, handleError } = injectNotificationManager()

const messages = defineMessages({
	discoverProjectType: {
		id: 'app.browse.discover-project-type',
		defaultMessage: 'Discover {projectType}',
	},
	resourcePacks: {
		id: 'app.browse.project-type.resource-packs',
		defaultMessage: 'Resource Packs',
	},
	shaders: { id: 'app.browse.project-type.shaders', defaultMessage: 'Shaders' },
	relevance: { id: 'plus.horizon.browse.sort.relevance', defaultMessage: 'Relevance' },
	downloads: { id: 'plus.horizon.browse.sort.downloads', defaultMessage: 'Downloads' },
	newest: { id: 'plus.horizon.browse.sort.newest', defaultMessage: 'Date published' },
	updated: { id: 'plus.horizon.browse.sort.updated', defaultMessage: 'Date updated' },
	gameVersion: { id: 'plus.horizon.browse.game-version', defaultMessage: 'Game version' },
	gameVersionProvidedByInstance: {
		id: 'search.filter.locked.instance-game-version.title',
		defaultMessage: 'Game version is provided by the instance',
	},
	providedByInstance: {
		id: 'search.filter.locked.instance',
		defaultMessage: 'Provided by the instance',
	},
	syncFilterButton: {
		id: 'search.filter.locked.instance.sync',
		defaultMessage: 'Sync with instance',
	},
	back: { id: 'plus.horizon.packs.back', defaultMessage: 'Back to instance' },
	update: { id: 'plus.horizon.packs.update', defaultMessage: 'Update' },
	installedTitle: { id: 'plus.horizon.packs.installed-title', defaultMessage: 'Installed {name}' },
	failedHeader: {
		id: 'plus.horizon.browse.failed',
		defaultMessage: 'Horizon’s registry couldn’t be reached',
	},
	retry: { id: 'plus.horizon.packs.retry', defaultMessage: 'Try again' },
	registryEmpty: {
		id: 'plus.horizon.browse.registry-empty',
		defaultMessage:
			'Horizon’s registry has no {kind, select, shader {shader packs} other {resource packs}} for Minecraft {version} right now. New ones show here as soon as Horizon lists them.',
	},
	noMatch: {
		id: 'plus.horizon.packs.no-match',
		defaultMessage: 'No packs match your search or filters.',
	},
	notHorizon: {
		id: 'plus.horizon.packs.not-horizon',
		defaultMessage:
			'These packs are for instances that play with Horizon. Turn Horizon on in this instance’s settings to use them.',
	},
})

const SORTS = (['relevance', 'downloads', 'newest', 'updated'] as const).map((name) => ({
	name,
	display: formatMessage(messages[name]),
}))

const pageActive = computed(() => route.name === BROWSE_ROUTE)
const instanceId = ref(typeof route.query.i === 'string' ? route.query.i : '')
const kind = ref(kindFromPath(route.params.kind))
const { data: instance } = useQuery(
	computed(() => ({
		...instanceDetailQueryOptions(instanceId.value),
		enabled: !!instanceId.value,
	})),
)
const isHorizon = computed(() => !!instance.value && loaderOf(instance.value) === HORIZON_LOADER)
const projectType = computed(() => (kind.value === 'shader' ? 'shader' : 'resourcepack'))

const query = ref(typeof route.query.q === 'string' ? route.query.q : '')
const sortType = shallowRef(SORTS[0])
const maxResults = ref(20)
const currentPage = ref(1)
const packs = shallowRef<HorizonPack[]>([])
const loading = ref(true)
const refreshing = ref(false)
const loadError = ref<string | null>(null)
const installing = ref(new Set<string>())
let request = 0

const categoryTags = shallowRef<CategoryTag[]>([])
void (get_categories() as Promise<CategoryTag[]>)
	.then((tags) => (categoryTags.value = tags))
	.catch(() => {})
const currentFilters = ref<{ type: string; option: string; negative?: boolean }[]>([])
const toggledGroups = ref<string[]>([])
const groups = computed(() => categoryGroups(packs.value, categoryTags.value, projectType.value))

/** Horizon's registry only offers packs made for the instance's version, as the chip says. */
const providedFilters = computed(() =>
	instance.value ? [{ type: 'game_version', option: instance.value.game_version }] : [],
)

/** The sidebar's filters, built the way Modrinth's are, for the categories Horizon's packs use. */
const filters = computed(() => [
	...(instance.value
		? [
				{
					id: 'game_version',
					formatted_name: formatMessage(messages.gameVersion),
					supported_project_types: [projectType.value],
					display: 'all',
					query_param: 'v',
					supports: ['include'],
					searchable: false,
					options: [
						{
							id: instance.value.game_version,
							formatted_name: instance.value.game_version,
							value: instance.value.game_version,
							method: 'or',
						},
					],
				},
			]
		: []),
	...groups.value.map((group) => ({
		id: group.id,
		formatted_name: formatCategoryHeader(formatMessage, group.header),
		supported_project_types: [projectType.value],
		display: 'all',
		query_param: 'f',
		supports: ['include', 'exclude'],
		searchable: false,
		options: group.names.map((name) => ({
			id: name,
			formatted_name: formatCategory(formatMessage, name),
			icon: getCategoryIcon(name),
			value: name,
			method: group.method,
		})),
	})),
])

const matching = computed(() =>
	findPacks(
		packs.value.filter((pack) => matchesFilters(pack, currentFilters.value, groups.value)),
		query.value,
		sortType.value.name as HorizonSort,
	),
)
const projectHits = computed(() =>
	matching.value
		.slice((currentPage.value - 1) * maxResults.value, currentPage.value * maxResults.value)
		.map(toHorizonHit),
)
const totalHits = computed(() => matching.value.length)

/** Says why the list is empty: Horizon has nothing of this kind yet, or nothing matches. */
const noResultsMessage = computed(() => {
	if (!instance.value) return null
	if (!isHorizon.value) return formatMessage(messages.notHorizon)
	return packs.value.length === 0
		? formatMessage(messages.registryEmpty, {
				kind: kind.value,
				version: instance.value.game_version,
			})
		: formatMessage(messages.noMatch)
})

async function load(): Promise<void> {
	const id = ++request
	if (!instanceId.value || !isHorizon.value) {
		packs.value = []
		loading.value = !instance.value && !!instanceId.value
		return
	}
	if (packs.value.length === 0) loading.value = true
	else refreshing.value = true
	loadError.value = null
	try {
		const found = await plus_horizon_packs(instanceId.value, kind.value)
		if (id !== request) return
		packs.value = found
	} catch (error) {
		if (id !== request) return
		packs.value = []
		loadError.value = errorText(error)
	} finally {
		if (id === request) {
			loading.value = false
			refreshing.value = false
		}
	}
}

watch(
	() => [route.params.kind, route.query.i] as const,
	([nextKind, nextInstance]) => {
		if (!pageActive.value) return
		const kindChanged = kindFromPath(nextKind) !== kind.value
		kind.value = kindFromPath(nextKind)
		instanceId.value = typeof nextInstance === 'string' ? nextInstance : ''
		if (kindChanged) {
			packs.value = []
			currentFilters.value = []
			currentPage.value = 1
		}
		syncBreadcrumbs()
	},
)
watch(
	() => route.query.q,
	(value) => {
		if (pageActive.value && typeof value === 'string') query.value = value
	},
)
watch([kind, instanceId, instance, isHorizon], () => void load(), { immediate: true })
watch([query, sortType, maxResults, currentFilters], () => (currentPage.value = 1), {
	deep: true,
})

async function install(pack: HorizonPack): Promise<void> {
	installing.value = new Set(installing.value).add(pack.id)
	try {
		await plus_horizon_pack_install(instanceId.value, kind.value, pack.id)
		addNotification({
			type: 'success',
			title: formatMessage(messages.installedTitle, { name: pack.name }),
		})
		await load()
	} catch (error) {
		handleError(error as Error)
	} finally {
		const next = new Set(installing.value)
		next.delete(pack.id)
		installing.value = next
	}
}

function getCardActions(result: HorizonSearchHit): CardAction[] {
	const pack = result.horizon
	const busy = installing.value.has(pack.id)
	const installed = !!pack.installed && !pack.update_available
	return [
		{
			key: 'install',
			label: busy
				? formatMessage(commonMessages.installingLabel)
				: installed
					? formatMessage(commonMessages.installedLabel)
					: pack.update_available
						? formatMessage(messages.update)
						: formatMessage(commonMessages.installButton),
			icon: busy
				? SpinnerIcon
				: installed
					? CheckIcon
					: pack.update_available
						? DownloadIcon
						: PlusIcon,
			iconClass: busy ? 'animate-spin' : undefined,
			disabled: busy || installed,
			color: 'brand',
			type: 'outlined',
			onClick: () => install(pack),
		},
	]
}

const breadcrumbs = injectBreadcrumbManager()
const browseBreadcrumb = {
	slot: 'browse',
	id: () => `browse:horizon:${instanceId.value}:${kind.value}`,
	label: () =>
		formatMessage(messages.discoverProjectType, {
			projectType: formatProjectTypeSentence(formatMessage, projectType.value, 2),
		}),
	to: () => route.fullPath,
	visual: { type: 'icon', component: CompassIcon },
} satisfies BreadcrumbDefinition
const instanceBreadcrumb = {
	slot: 'instance',
	id: () => `instance:${instanceId.value}`,
	label: () => instance.value?.name ?? formatMessage(commonMessages.loadingLabel),
	visual: () => ({
		type: 'image' as const,
		src: getInstanceIconUrl(instance.value?.icon_path),
		alt: instance.value?.name,
		tintBy: instanceId.value,
	}),
	to: () => `/instance/${encodeURIComponent(instanceId.value)}`,
} satisfies BreadcrumbDefinition

function syncBreadcrumbs(): void {
	if (instanceId.value) {
		breadcrumbs.push(browseBreadcrumb, { parent: breadcrumbs.reset(instanceBreadcrumb) })
	} else {
		breadcrumbs.reset(browseBreadcrumb)
	}
}
syncBreadcrumbs()

const installContext = computed(() =>
	instance.value
		? {
				name: instance.value.name,
				loader: loaderOf(instance.value),
				gameVersion: instance.value.game_version,
				iconSrc: getInstanceIconUrl(instance.value.icon_path),
				backUrl: `/instance/${encodeURIComponent(instance.value.id)}`,
				backLabel: formatMessage(messages.back),
				heading: formatMessage(commonMessages.installingContentLabel),
			}
		: null,
)

/** Horizon has resource packs and shaders, and nothing from other sites. */
const selectableProjectTypes = computed(() => [
	{
		label: formatMessage(messages.resourcePacks),
		href: horizonPacksPath('resource_pack', instanceId.value),
		icon: getProjectTypeIcon('resourcepack'),
	},
	{
		label: formatMessage(messages.shaders),
		href: horizonPacksPath('shader', instanceId.value),
		icon: getProjectTypeIcon('shader'),
	},
])

provideBrowseManager({
	tags: ref({ gameVersions: [], loaders: [], categories: [] }),
	projectType,
	query,
	filters,
	currentFilters,
	toggledGroups,
	overriddenProvidedFilterTypes: ref([]),
	providedFilters,
	lockedFilterMessages: computed(() => ({
		gameVersion: formatMessage(messages.gameVersionProvidedByInstance),
		modLoader: '',
		environment: '',
		syncButton: formatMessage(messages.syncFilterButton),
		providedBy: formatMessage(messages.providedByInstance),
	})),
	installContext,
	serverFilterTypes: computed(() => []),
	serverCurrentFilters: ref([]),
	serverToggledGroups: ref([]),
	effectiveSortTypes: computed(() => SORTS),
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
	noResultsMessage,
	refreshSearch: load,
	setPage: async (page: number) => {
		currentPage.value = page
	},
	clearSearch: () => {
		query.value = ''
	},
	onFilterChange: () => {
		currentPage.value = 1
	},
	linkOverridesAdvancedPrefs: ref(false),
	applySavedAdvancedPrefs: () => {},
	getProjectLink: (result: HorizonSearchHit) => result.horizon.page_url,
	getServerProjectLink: () => '',
	getAuthorLink: (result: HorizonSearchHit) =>
		`${horizonPacksPath(kind.value, instanceId.value)}&q=${encodeURIComponent(result.horizon.authors[0] ?? '')}`,
	selectableProjectTypes,
	showProjectTypeTabs: computed(() => !!instanceId.value),
	variant: 'app',
	getCardActions: (result: HorizonSearchHit) => getCardActions(result),
	maxResultsOptions: computed(() => [5, 10, 20, 50]),
} as never)
</script>

<template>
	<div class="flex flex-col gap-2 p-6">
		<BrowsePageLayout>
			<template #before-tabs>
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
		<Teleport v-if="pageActive" to="#sidebar-teleport-target">
			<BrowseSidebar />
		</Teleport>
	</div>
</template>
