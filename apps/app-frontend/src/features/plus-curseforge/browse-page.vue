<script setup lang="ts">
import {
	BlocksIcon,
	BoxIcon,
	BracesIcon,
	CheckIcon,
	CompassIcon,
	ExternalIcon,
	EyeIcon,
	getCategoryIcon,
	getLoaderIcon,
	GlassesIcon,
	GlobeIcon,
	KeyIcon,
	LayersIcon,
	LeftArrowIcon,
	PackageOpenIcon,
	PaintbrushIcon,
	PaletteIcon,
	PlugIcon,
	PlusIcon,
	SpinnerIcon,
} from '@modrinth/assets'
import type { CardAction } from '@modrinth/ui'
import {
	Admonition,
	Avatar,
	BrowseInstallHeader,
	BrowsePageLayout,
	BrowseSidebar,
	Button,
	ButtonLink,
	commonMessages,
	defineMessages,
	formatLoaderLabel,
	IconButton,
	provideBrowseManager,
	useVIntl,
} from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { useDebounceFn } from '@vueuse/core'
import { type Component, computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import {
	hasSyncedFilter,
	useInstanceFilterSync,
} from '@/features/plus-install/instance-filter-sync'
import { readInstanceTarget } from '@/features/plus-install/instance-target'
import { injectSettingsActions } from '@/features/settings/settings-actions'
import { getInstanceIconUrl } from '@/helpers/instance'
import {
	type CfCategory,
	type CfDataMode,
	type CfProject,
	plus_cf_categories,
	plus_cf_classes,
	plus_cf_data_mode,
	plus_cf_game_versions,
	plus_cf_installed_projects,
	plus_cf_search,
} from '@/helpers/plus'
import { instanceDetailQueryOptions } from '@/pages/instance/query-options'
import { type BreadcrumbDefinition, injectBreadcrumbManager } from '@/shared/breadcrumbs'

import { curseforgeCategoryIcon } from './category-icons'
import { useCurseforgeInstall } from './install-actions'
import { curseforgeKeyRevision } from './key-state'
import { injectModpackInstall } from './modpack-install-context'
import {
	authorPath,
	classSlugOf,
	errorText,
	projectPath,
	rememberBrowse,
	rememberedBrowse,
} from './project-adapter'
import {
	type CfSearchHit,
	classesForInstance,
	classInfo,
	LOADER_TYPE_IDS,
	orderClasses,
	toSearchHit,
} from './provider-mapping'
import ProviderSwitch from './provider-switch.vue'

/** The API rejects any page past index + pageSize = 10,000. */
const MAX_INDEX = 10_000
const ALL_CLASSES = 'all'
const BROWSE_ROUTE = 'Discover on CurseForge'
const AUTHOR_ROUTE = 'CurseForge author'
const CLASS_ICONS: Record<string, Component> = {
	modpacks: PackageOpenIcon,
	'mc-mods': BoxIcon,
	'texture-packs': PaintbrushIcon,
	'data-packs': BracesIcon,
	shaders: GlassesIcon,
	worlds: GlobeIcon,
	'bukkit-plugins': PlugIcon,
	customization: PaletteIcon,
	'mc-addons': BlocksIcon,
}

const { formatMessage } = useVIntl()
const route = useRoute()
const router = useRouter()
const settingsActions = injectSettingsActions()

const messages = defineMessages({
	breadcrumb: { id: 'plus.curseforge.browse.breadcrumb', defaultMessage: 'Discover on CurseForge' },
	sampleHeader: { id: 'plus.curseforge.sample', defaultMessage: 'Showing sample data' },
	sampleText: {
		id: 'plus.curseforge.browse.sample.text',
		defaultMessage:
			"These aren't real CurseForge projects. This development build has no API key, so it shows samples. The tabs, categories and game versions are real.",
	},
	needsKeyHeader: {
		id: 'plus.curseforge.browse.needs-key',
		defaultMessage: 'CurseForge needs an API key',
	},
	needsKeyText: {
		id: 'plus.curseforge.browse.needs-key.text',
		defaultMessage:
			'CurseForge only allows searching and downloading with an API key, so no projects can be shown yet. Add your own key in Settings to browse here.',
	},
	keyRejectedHeader: {
		id: 'plus.curseforge.browse.key-rejected',
		defaultMessage: 'CurseForge rejected your API key',
	},
	keyRejectedText: {
		id: 'plus.curseforge.browse.key-rejected.text',
		defaultMessage:
			'No projects can be shown until this is fixed. The key may have been revoked, so check it in Settings.',
	},
	openApiSettings: {
		id: 'plus.curseforge.browse.open-api-settings',
		defaultMessage: 'Open API configuration',
	},
	failedHeader: { id: 'plus.curseforge.browse.failed', defaultMessage: "Couldn't load projects" },
	retry: { id: 'plus.curseforge.browse.retry', defaultMessage: 'Try again' },
	popularity: { id: 'plus.curseforge.sort.popularity', defaultMessage: 'Popularity' },
	updated: { id: 'plus.curseforge.sort.updated', defaultMessage: 'Recently updated' },
	downloads: { id: 'plus.curseforge.sort.downloads', defaultMessage: 'Downloads' },
	released: { id: 'plus.curseforge.sort.released', defaultMessage: 'Newest' },
	rating: { id: 'plus.curseforge.sort.rating', defaultMessage: 'Rating' },
	name: { id: 'plus.curseforge.sort.name', defaultMessage: 'Name' },
	filterLoader: { id: 'plus.curseforge.filter.loader', defaultMessage: 'Loader' },
	filterVersion: { id: 'plus.curseforge.filter.version', defaultMessage: 'Game version' },
	filterCategory: { id: 'plus.curseforge.filter.category', defaultMessage: 'Category' },
	backToInstance: { id: 'plus.curseforge.back-to-instance', defaultMessage: 'Back to instance' },
	view: { id: 'plus.curseforge.card.view', defaultMessage: 'View' },
	unsupported: {
		id: 'plus.curseforge.card.unsupported',
		defaultMessage: "Installing this kind of CurseForge project isn't supported yet.",
	},
	restricted: {
		id: 'plus.curseforge.card.restricted',
		defaultMessage: "This project's author doesn't allow downloads through other apps.",
	},
	syncOnTitle: {
		id: 'plus.browse.sync.on.title',
		defaultMessage: 'Filters are provided by the instance',
	},
	syncOffTitle: { id: 'plus.browse.sync.off.title', defaultMessage: 'Filters are unlocked' },
	syncOn: { id: 'plus.browse.sync.on.status', defaultMessage: 'Showing {target}' },
	syncOff: {
		id: 'plus.browse.sync.off.status',
		defaultMessage: 'Browsing every version and loader',
	},
	syncNoLoader: {
		id: 'plus.browse.sync.on.unknown-loader',
		defaultMessage: "Showing {target}. Couldn't tell this instance's loader, so it isn't filtered.",
	},
	syncOnText: {
		id: 'plus.browse.sync.on.text',
		defaultMessage: 'Unlocking these filters may allow you to install incompatible content.',
	},
	syncOffText: {
		id: 'plus.browse.sync.off.text',
		defaultMessage: "Results can include content that doesn't work with this instance.",
	},
	syncUnlock: { id: 'plus.browse.sync.unlock', defaultMessage: 'Unlock filters' },
	syncRelock: { id: 'plus.browse.sync.relock', defaultMessage: 'Sync with instance' },
	gameVersionProvided: {
		id: 'search.filter.locked.instance-game-version.title',
		defaultMessage: 'Game version is provided by the instance',
	},
	loaderProvided: {
		id: 'search.filter.locked.instance-loader.title',
		defaultMessage: 'Loader is provided by the instance',
	},
	unlockFilter: { id: 'search.filter.locked.instance.sync', defaultMessage: 'Sync with instance' },
	providedByInstance: {
		id: 'search.filter.locked.instance',
		defaultMessage: 'Provided by the instance',
	},
	allClasses: { id: 'plus.curseforge.author.all', defaultMessage: 'All' },
	authorProjects: {
		id: 'plus.curseforge.author.projects',
		defaultMessage: '{count, plural, one {# project} other {# projects}}',
	},
	back: { id: 'plus.curseforge.author.back', defaultMessage: 'Back' },
	provider: { id: 'plus.curseforge.provider', defaultMessage: 'CurseForge' },
	openOnCurseforge: { id: 'plus.curseforge.project.website', defaultMessage: 'Open on CurseForge' },
})

/**
 * `sortField` values from the API docs. CurseForge has no relevance ranking, and leaving the
 * field out returns matches in no useful order, so popularity comes first.
 */
const sortTypes = [
	{ name: 'popularity', display: formatMessage(messages.popularity), field: 2 },
	{ name: 'downloads', display: formatMessage(messages.downloads), field: 6 },
	{ name: 'updated', display: formatMessage(messages.updated), field: 3 },
	{ name: 'newest', display: formatMessage(messages.released), field: 11 },
	{ name: 'rating', display: formatMessage(messages.rating), field: 12 },
	{ name: 'name', display: formatMessage(messages.name), field: 4 },
]

const pageActive = computed(() => route.name === BROWSE_ROUTE || route.name === AUTHOR_ROUTE)
const instanceId = ref(typeof route.query.i === 'string' ? route.query.i : '')
const authorId = ref<number | null>(
	route.name === AUTHOR_ROUTE ? Number(route.params.authorId) : null,
)
const { data: instance } = useQuery(
	computed(() => ({
		...instanceDetailQueryOptions(instanceId.value),
		enabled: !!instanceId.value,
	})),
)

const breadcrumbs = injectBreadcrumbManager()
const browseBreadcrumb = {
	slot: 'browse',
	id: () => `browse:curseforge:${instanceId.value}:${authorId.value ?? ''}`,
	label: () => authorInfo.value.name ?? formatMessage(messages.breadcrumb),
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

watch(
	() => route.query.i,
	(value) => {
		if (!pageActive.value) return
		instanceId.value = typeof value === 'string' ? value : ''
		syncBreadcrumbs()
	},
)

const mode = ref<CfDataMode | null>(null)
const browsingBlocked = computed(() => mode.value === 'needs_key' || mode.value === 'key_rejected')
const classes = ref<CfCategory[]>([])
const categories = ref<CfCategory[]>([])
const gameVersions = ref<string[]>([])
const loadError = ref<string | null>(null)

const classSlug = ref(
	String(route.params.classSlug ?? (authorId.value === null ? 'mc-mods' : ALL_CLASSES)),
)
const currentClass = computed(() => classes.value.find((item) => item.slug === classSlug.value))
const projectType = computed(() => classInfo(classSlug.value).projectType)

const snapshotKey = computed(() => `${authorId.value ?? ''}|${classSlug.value}|${instanceId.value}`)
const restored = rememberedBrowse(snapshotKey.value)

function filtersFromRoute(): { type: string; option: string }[] {
	const filters = []
	if (typeof route.query.cat === 'string') {
		filters.push({ type: 'category_curseforge', option: route.query.cat })
	}
	if (typeof route.query.loader === 'string') {
		filters.push({ type: 'mod_loader', option: route.query.loader })
	}
	return filters
}

const fromRoute = filtersFromRoute()
const query = ref(restored?.query ?? '')
const currentFilters = ref<{ type: string; option: string; negative?: boolean }[]>(
	fromRoute.length > 0 ? fromRoute : (restored?.filters ?? []),
)
const toggledGroups = ref<string[]>([])
const overriddenProvidedFilterTypes = ref<string[]>([])
const effectiveCurrentSortType = shallowRef(
	sortTypes.find((sort) => sort.name === restored?.sort) ?? sortTypes[0],
)
const maxResults = ref(restored?.maxResults ?? 20)
const currentPage = ref(restored?.page ?? 1)
const loading = ref(true)
const refreshing = ref(false)
const projectHits = shallowRef<CfSearchHit[]>([])
const totalHits = ref(0)

onBeforeUnmount(() => {
	rememberBrowse(snapshotKey.value, {
		query: query.value,
		filters: currentFilters.value,
		sort: effectiveCurrentSortType.value.name,
		maxResults: maxResults.value,
		page: currentPage.value,
	})
})

const authorInfo = computed(() => {
	if (authorId.value === null) return { name: undefined, url: undefined, avatar: undefined }
	const listed = projectHits.value
		.flatMap((hit) => hit.curseforge.authors)
		.find((author) => author.id === authorId.value)
	const fromQuery = (key: string) =>
		typeof route.query[key] === 'string' ? (route.query[key] as string) : undefined
	return {
		name: listed?.name ?? fromQuery('name'),
		url: listed?.url ?? fromQuery('url'),
		avatar: listed?.avatarUrl ?? fromQuery('avatar'),
	}
})

syncBreadcrumbs()

const instanceTarget = computed(() => readInstanceTarget(instance.value))
const providedFilters = computed(() => {
	if (!instance.value) return []
	const filters: { type: string; option: string }[] = []
	if (instanceTarget.value.gameVersion) {
		filters.push({ type: 'game_version', option: instanceTarget.value.gameVersion })
	}
	if (instanceTarget.value.loader && projectType.value === 'mod') {
		filters.push({ type: 'mod_loader', option: instanceTarget.value.loader })
	}
	return filters
})
const filterSync = useInstanceFilterSync(
	instanceId,
	overriddenProvidedFilterTypes,
	computed(() => providedFilters.value.map((filter) => filter.type)),
)
const lockedFilterMessages = computed(() => ({
	gameVersion: formatMessage(messages.gameVersionProvided),
	modLoader: formatMessage(messages.loaderProvided),
	syncButton: formatMessage(messages.unlockFilter),
	providedBy: formatMessage(messages.providedByInstance),
}))
const instanceSync = computed(() => {
	if (!instance.value) return null
	if (!hasSyncedFilter(providedFilters.value.map((filter) => filter.type))) return null
	const { loader, gameVersion, status } = instanceTarget.value
	const loaderName = loader && projectType.value === 'mod' ? formatLoaderLabel(loader) : ''
	const target = [loaderName, gameVersion ? `Minecraft ${gameVersion}` : '']
		.filter(Boolean)
		.join(' ')
	const onMessage = status === 'unknown-loader' ? messages.syncNoLoader : messages.syncOn
	const enabled = filterSync.enabled.value
	return {
		enabled,
		title: formatMessage(enabled ? messages.syncOnTitle : messages.syncOffTitle),
		status: enabled ? formatMessage(onMessage, { target }) : formatMessage(messages.syncOff),
		description: formatMessage(enabled ? messages.syncOnText : messages.syncOffText),
		actionLabel: formatMessage(enabled ? messages.syncUnlock : messages.syncRelock),
		set: filterSync.set,
	}
})

const pageCount = computed(() =>
	Math.max(1, Math.ceil(Math.min(totalHits.value, MAX_INDEX) / maxResults.value)),
)

const filters = computed(() => {
	const list = []
	if (categories.value.length > 0) {
		list.push({
			id: 'category_curseforge',
			formatted_name: formatMessage(messages.filterCategory),
			supported_project_types: [projectType.value],
			query_param: 'cf',
			supports: ['include'],
			display: 'all',
			searchable: false,
			options: categories.value.map((category) => ({
				id: String(category.id),
				formatted_name: category.name,
				icon: getCategoryIcon(curseforgeCategoryIcon(category.name)),
				method: 'or',
				value: String(category.id),
			})),
		})
	}
	if (projectType.value === 'mod' || classSlug.value === ALL_CLASSES) {
		list.push({
			id: 'mod_loader',
			formatted_name: formatMessage(messages.filterLoader),
			supported_project_types: [projectType.value],
			query_param: 'loader',
			supports: ['include'],
			display: 'all',
			searchable: false,
			options: Object.keys(LOADER_TYPE_IDS).map((loader) => ({
				id: loader,
				formatted_name: formatLoaderLabel(loader),
				icon: getLoaderIcon(loader),
				method: 'or',
				value: loader,
			})),
		})
	}
	if (gameVersions.value.length > 0) {
		list.push({
			id: 'game_version',
			formatted_name: formatMessage(messages.filterVersion),
			supported_project_types: [projectType.value],
			query_param: 'gv',
			supports: ['include'],
			display: 'scrollable',
			searchable: true,
			options: gameVersions.value.map((version) => ({
				id: version,
				method: 'or',
				value: version,
			})),
		})
	}
	return list
})

/** A filter the instance provides wins, unless it was unlocked. */
function selectedOption(type: string): string | undefined {
	const provided = providedFilters.value.find((filter) => filter.type === type)
	if (provided && !overriddenProvidedFilterTypes.value.includes(type)) return provided.option
	const matches = currentFilters.value.filter((filter) => filter.type === type)
	return matches[matches.length - 1]?.option
}

/** The API takes one value per filter, so choosing another replaces the first. */
function keepOneValuePerFilter(): void {
	const seen = new Set<string>()
	const single = [...currentFilters.value].reverse().filter((filter) => {
		if (seen.has(filter.type)) return false
		seen.add(filter.type)
		return true
	})
	if (single.length !== currentFilters.value.length) {
		currentFilters.value = single.reverse()
	}
}

let latestRequest = 0
let started = false

async function runSearch(): Promise<void> {
	const request = ++latestRequest
	const classId = currentClass.value?.id
	if (classId === undefined && authorId.value === null) return
	if (browsingBlocked.value) {
		projectHits.value = []
		totalHits.value = 0
		loading.value = false
		refreshing.value = false
		return
	}
	if (projectHits.value.length === 0) loading.value = true
	else refreshing.value = true
	loadError.value = null
	try {
		const loader = selectedOption('mod_loader')
		const category = selectedOption('category_curseforge')
		const sortField =
			sortTypes.find((sort) => sort.name === effectiveCurrentSortType.value.name)?.field ?? 2
		const response = await plus_cf_search({
			classId: classId ?? null,
			searchFilter: query.value,
			gameVersion: selectedOption('game_version') ?? null,
			modLoaderType: loader ? LOADER_TYPE_IDS[loader] : null,
			categoryId: category ? Number(category) : null,
			authorId: authorId.value,
			sortField,
			sortOrder: sortField === 4 ? 'asc' : 'desc',
			index: (currentPage.value - 1) * maxResults.value,
			pageSize: maxResults.value,
		})
		if (request !== latestRequest) return
		projectHits.value = response.data.map(toSearchHit)
		totalHits.value = response.pagination.totalCount
	} catch (error) {
		if (request !== latestRequest) return
		projectHits.value = []
		totalHits.value = 0
		loadError.value = errorText(error)
	} finally {
		if (request === latestRequest) {
			loading.value = false
			refreshing.value = false
		}
	}
}

const debouncedSearch = useDebounceFn(() => {
	currentPage.value = 1
	void runSearch()
}, 300)

function searchFromFirstPage(): void {
	if (!started) return
	currentPage.value = 1
	void runSearch()
}

watch(query, () => {
	if (started) void debouncedSearch()
})
watch([effectiveCurrentSortType, maxResults], searchFromFirstPage)
watch(
	currentFilters,
	() => {
		keepOneValuePerFilter()
		searchFromFirstPage()
	},
	{ deep: true },
)
watch([providedFilters, overriddenProvidedFilterTypes], searchFromFirstPage, { deep: true })

async function loadClassData(): Promise<void> {
	const classId = currentClass.value?.id
	categories.value = classId === undefined ? [] : await plus_cf_categories(classId)
}

watch(
	() => route.params.classSlug,
	async (slug) => {
		if (!pageActive.value) return
		const next = String(slug ?? (authorId.value === null ? 'mc-mods' : ALL_CLASSES))
		if (next === classSlug.value) return
		classSlug.value = next
		currentFilters.value = []
		currentPage.value = 1
		projectHits.value = []
		loading.value = true
		try {
			await loadClassData()
			await runSearch()
		} catch (error) {
			loading.value = false
			loadError.value = errorText(error)
		}
	},
)

async function loadPage(): Promise<void> {
	loadError.value = null
	try {
		const [loadedMode, loadedClasses, loadedVersions] = await Promise.all([
			plus_cf_data_mode(),
			plus_cf_classes(),
			plus_cf_game_versions(),
		])
		mode.value = loadedMode
		classes.value = orderClasses(loadedClasses)
		gameVersions.value = loadedVersions.map((version) => version.versionString)
		await loadClassData()
		await runSearch()
	} catch (error) {
		loading.value = false
		loadError.value = errorText(error)
	} finally {
		started = true
	}
}

onMounted(loadPage)
watch(curseforgeKeyRevision, () => {
	projectHits.value = []
	loading.value = true
	void loadPage()
})

function retry(): void {
	if (classes.value.length === 0) void loadPage()
	else void runSearch()
}

const modpackInstall = injectModpackInstall()
const { install: installContent } = useCurseforgeInstall(classes)
const installingIds = ref(new Set<number>())
const installedIds = ref(new Set<number>())

async function loadInstalled(): Promise<void> {
	installedIds.value = new Set(
		instanceId.value ? await plus_cf_installed_projects(instanceId.value).catch(() => []) : [],
	)
}
watch(instanceId, () => void loadInstalled(), { immediate: true })

function markInstalling(projectId: number, installing: boolean): void {
	const next = new Set(installingIds.value)
	if (installing) next.add(projectId)
	else next.delete(projectId)
	installingIds.value = next
}

async function install(project: CfProject): Promise<void> {
	if (classSlugOf(project, classes.value) === 'modpacks') {
		await modpackInstall.value?.install(project)
		return
	}
	if (!instanceId.value) {
		await installContent(project)
		return
	}
	markInstalling(project.id, true)
	try {
		if (await installContent(project, { instanceId: instanceId.value })) {
			installedIds.value = new Set([...installedIds.value, project.id])
		}
	} finally {
		markInstalling(project.id, false)
	}
}

const installContext = computed(() =>
	instance.value
		? {
				name: instance.value.name,
				loader: instance.value.loader,
				gameVersion: instance.value.game_version,
				iconSrc: getInstanceIconUrl(instance.value.icon_path),
				backUrl: `/instance/${encodeURIComponent(instance.value.id)}`,
				backLabel: formatMessage(messages.backToInstance),
				heading: formatMessage(commonMessages.installingContentLabel),
			}
		: null,
)

const visibleClasses = computed(() =>
	instance.value
		? classesForInstance(classes.value, instance.value, gameVersions.value)
		: classes.value,
)

function authorQuery(): string {
	const params = new URLSearchParams()
	for (const key of ['name', 'url', 'avatar', 'i']) {
		const value = route.query[key]
		if (typeof value === 'string') params.set(key, value)
	}
	const text = params.toString()
	return text ? `?${text}` : ''
}

const selectableProjectTypes = computed(() => {
	if (authorId.value !== null) {
		const base = `/curseforge/author/${authorId.value}`
		return [
			{
				label: formatMessage(messages.allClasses),
				href: `${base}${authorQuery()}`,
				icon: LayersIcon,
			},
			...visibleClasses.value.map((item) => ({
				label: item.name,
				href: `${base}/${item.slug}${authorQuery()}`,
				icon: CLASS_ICONS[item.slug] ?? BoxIcon,
			})),
		]
	}
	const suffix = instanceId.value ? `?i=${encodeURIComponent(instanceId.value)}` : ''
	return visibleClasses.value.map((item) => ({
		label: item.name,
		href: `/curseforge/${item.slug}${suffix}`,
		icon: CLASS_ICONS[item.slug] ?? BoxIcon,
	}))
})

function getCardActions(result: CfSearchHit): CardAction[] {
	const project = result.curseforge
	const restricted = project.allowModDistribution !== true
	const installable = classInfo(classSlugOf(project, classes.value)).installable
	if (mode.value === 'sample' || !installable || restricted) {
		return [
			{
				key: 'view',
				label: formatMessage(messages.view),
				icon: EyeIcon,
				type: 'outlined',
				tooltip:
					mode.value === 'sample'
						? undefined
						: formatMessage(restricted ? messages.restricted : messages.unsupported),
				onClick: () => router.push(projectPath(project, instanceId.value)),
			},
		]
	}
	const installing =
		installingIds.value.has(project.id) || modpackInstall.value?.preparing === project.id
	const installed = installedIds.value.has(project.id)
	return [
		{
			key: 'install',
			label: formatMessage(
				installing
					? commonMessages.installingLabel
					: installed
						? commonMessages.installedLabel
						: commonMessages.installButton,
			),
			icon: installing ? SpinnerIcon : installed ? CheckIcon : PlusIcon,
			iconClass: installing ? 'animate-spin' : undefined,
			disabled: installing || installed,
			color: 'brand',
			type: 'outlined',
			onClick: () => install(project),
		},
	]
}

provideBrowseManager({
	tags: ref({ gameVersions: [], loaders: [], categories: [] }),
	projectType,
	query,
	filters,
	currentFilters,
	toggledGroups,
	overriddenProvidedFilterTypes,
	providedFilters,
	lockedFilterMessages,
	instanceSync,
	installContext,
	serverFilterTypes: computed(() => []),
	serverCurrentFilters: ref([]),
	serverToggledGroups: ref([]),
	effectiveSortTypes: computed(() => sortTypes),
	effectiveCurrentSortType,
	loading,
	refreshing,
	projectHits,
	serverHits: shallowRef([]),
	totalHits,
	pageCount,
	maxResults,
	currentPage,
	isServerType: computed(() => false),
	effectiveLayout: computed(() => 'list' as const),
	deprioritizedTags: computed(() => []),
	excludeLoaders: computed(() => false),
	loadFailed: computed(() => loadError.value !== null),
	refreshSearch: runSearch,
	setPage: async (page: number) => {
		currentPage.value = page
		await runSearch()
	},
	clearSearch: () => {
		query.value = ''
	},
	onFilterChange: () => {},
	linkOverridesAdvancedPrefs: ref(false),
	applySavedAdvancedPrefs: () => {},
	getProjectLink: (result: CfSearchHit) =>
		`${projectPath(result.curseforge, instanceId.value)}${instanceId.value ? '&' : '?'}b=${encodeURIComponent(route.fullPath)}`,
	getServerProjectLink: () => '',
	getAuthorLink: (result: CfSearchHit) => {
		const author = result.curseforge.authors[0]
		return author ? authorPath(author, instanceId.value) : undefined
	},
	selectableProjectTypes,
	showProjectTypeTabs: computed(() => selectableProjectTypes.value.length > 0),
	variant: 'app',
	getCardActions: (result: CfSearchHit) => getCardActions(result),
	maxResultsOptions: computed(() => [5, 10, 20, 50]),
} as never)
</script>

<template>
	<div class="flex flex-col gap-2 p-6">
		<template v-if="browsingBlocked">
			<div
				v-if="installContext"
				class="-mx-6 -mt-6 mb-4 rounded-tl-[--radius-xl] border-0 border-b border-solid bg-surface-1 px-6 py-4 border-surface-5"
			>
				<BrowseInstallHeader />
			</div>
			<ProviderSwitch v-if="authorId === null" />
			<Admonition
				class="mt-2"
				:type="mode === 'key_rejected' ? 'critical' : 'info'"
				:header="
					formatMessage(
						mode === 'key_rejected' ? messages.keyRejectedHeader : messages.needsKeyHeader,
					)
				"
			>
				{{
					formatMessage(mode === 'key_rejected' ? messages.keyRejectedText : messages.needsKeyText)
				}}
				<template #actions>
					<Button @click="settingsActions.openApiConfiguration()">
						<KeyIcon aria-hidden="true" />
						{{ formatMessage(messages.openApiSettings) }}
					</Button>
				</template>
			</Admonition>
		</template>
		<BrowsePageLayout v-else>
			<template #before-tabs>
				<ProviderSwitch v-if="authorId === null" />
				<div v-else class="flex flex-wrap items-center gap-4">
					<IconButton
						v-tooltip="formatMessage(messages.back)"
						size="xl"
						:label="formatMessage(messages.back)"
						native-type="button"
						@click="router.back()"
					>
						<LeftArrowIcon />
					</IconButton>
					<Avatar :src="authorInfo.avatar" :alt="authorInfo.name" size="64px" circle />
					<div class="flex min-w-0 grow flex-col gap-1">
						<h1 class="m-0 text-2xl font-semibold text-contrast">{{ authorInfo.name }}</h1>
						<span class="text-sm text-secondary">
							{{ formatMessage(messages.provider) }}
							<template v-if="!loading">
								· {{ formatMessage(messages.authorProjects, { count: totalHits }) }}
							</template>
						</span>
					</div>
					<ButtonLink
						v-if="authorInfo.url && mode !== 'sample'"
						:href="authorInfo.url"
						target="_blank"
					>
						{{ formatMessage(messages.openOnCurseforge) }}
						<ExternalIcon aria-hidden="true" />
					</ButtonLink>
				</div>
				<Admonition
					v-if="mode === 'sample'"
					type="info"
					:header="formatMessage(messages.sampleHeader)"
				>
					{{ formatMessage(messages.sampleText) }}
				</Admonition>
				<Admonition
					v-if="loadError"
					type="critical"
					:header="formatMessage(messages.failedHeader)"
					inline-actions
				>
					{{ loadError }}
					<template #actions>
						<Button @click="retry">{{ formatMessage(messages.retry) }}</Button>
					</template>
				</Admonition>
			</template>
		</BrowsePageLayout>
		<Teleport v-if="pageActive && !browsingBlocked" to="#sidebar-teleport-target">
			<BrowseSidebar />
		</Teleport>
	</div>
</template>
