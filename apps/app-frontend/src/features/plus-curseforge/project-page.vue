<script setup lang="ts">
import { CheckIcon, DownloadIcon, ExternalIcon, KeyIcon, SpinnerIcon } from '@modrinth/assets'
import {
	Admonition,
	BrowseInstallHeader,
	Button,
	ButtonLink,
	Card,
	commonMessages,
	defineMessages,
	IconButton,
	LoadingIndicator,
	NavTabs,
	NewModal,
	ProjectPageHeader,
	ProjectPageVersions,
	ProjectSidebarCompatibility,
	ProjectSidebarCreators,
	ProjectSidebarDetails,
	ProjectSidebarLinks,
	ProjectSidebarTags,
	useVIntl,
} from '@modrinth/ui'
import { configuredXss } from '@modrinth/utils'
import { useQuery } from '@tanstack/vue-query'
import { computed, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { injectSettingsActions } from '@/features/settings/settings-actions'
import { getInstanceIconUrl } from '@/helpers/instance'
import {
	type CfCategory,
	type CfDataMode,
	type CfFile,
	type CfProject,
	plus_cf_changelog,
	plus_cf_classes,
	plus_cf_data_mode,
	plus_cf_description,
	plus_cf_files,
	plus_cf_install_options,
	plus_cf_installed_projects,
	plus_cf_project,
} from '@/helpers/plus'
import { get_game_versions, get_loaders } from '@/helpers/tags'
import { instanceDetailQueryOptions } from '@/pages/instance/query-options'
import Gallery from '@/pages/project/Gallery.vue'
import { useBreadcrumb } from '@/shared/breadcrumbs'

import { useCurseforgeInstall } from './install-actions'
import { curseforgeKeyRevision } from './key-state'
import { injectModpackInstall } from './modpack-install-context'
import {
	authorPath,
	classSlugOf,
	errorText,
	toCreators,
	toProjectView,
	toVersionView,
} from './project-adapter'
import { classInfo } from './provider-mapping'

const route = useRoute()
const router = useRouter()
const settingsActions = injectSettingsActions()
const { formatMessage } = useVIntl()

const messages = defineMessages({
	provider: { id: 'plus.curseforge.provider', defaultMessage: 'CurseForge' },
	openOnCurseforge: { id: 'plus.curseforge.project.website', defaultMessage: 'Open on CurseForge' },
	description: { id: 'plus.curseforge.page.tab.description', defaultMessage: 'Description' },
	versions: { id: 'plus.curseforge.page.tab.versions', defaultMessage: 'Versions' },
	gallery: { id: 'plus.curseforge.page.tab.gallery', defaultMessage: 'Gallery' },
	installed: { id: 'plus.curseforge.installed', defaultMessage: 'Installed' },
	notDistributableHeader: {
		id: 'plus.curseforge.project.not-distributable.header',
		defaultMessage: 'Download on CurseForge',
	},
	notDistributable: {
		id: 'plus.curseforge.project.not-distributable',
		defaultMessage:
			"This project's author doesn't allow downloads through other apps. You can still get it from the CurseForge website.",
	},
	unsupportedHeader: {
		id: 'plus.curseforge.project.unsupported.header',
		defaultMessage: "Pawrinth can't install this yet",
	},
	unsupported: {
		id: 'plus.curseforge.card.unsupported',
		defaultMessage: "Installing this kind of CurseForge project isn't supported yet.",
	},
	noCompatibleHeader: {
		id: 'plus.project.no-compatible.header',
		defaultMessage: 'No compatible version',
	},
	noCompatibleText: {
		id: 'plus.project.no-compatible.text',
		defaultMessage: 'This project has no version for {target}.',
	},
	changelog: { id: 'plus.curseforge.project.changelog', defaultMessage: 'Changelog' },
	noChangelog: {
		id: 'plus.curseforge.install.no-changelog',
		defaultMessage: "This version doesn't have a changelog.",
	},
	noDescription: {
		id: 'plus.curseforge.page.no-description',
		defaultMessage: "CurseForge doesn't have a description for this project.",
	},
	sampleHeader: { id: 'plus.curseforge.sample', defaultMessage: 'Showing sample data' },
	sampleText: {
		id: 'plus.curseforge.page.sample',
		defaultMessage: "This isn't a real CurseForge project.",
	},
	needsKeyHeader: {
		id: 'plus.curseforge.browse.needs-key',
		defaultMessage: 'CurseForge needs an API key',
	},
	needsKeyText: {
		id: 'plus.curseforge.page.needs-key',
		defaultMessage: 'CurseForge only shows project details to apps with an API key.',
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
	failedHeader: { id: 'plus.curseforge.page.failed', defaultMessage: "Couldn't load this project" },
	retry: { id: 'plus.curseforge.browse.retry', defaultMessage: 'Try again' },
	backToBrowse: { id: 'plus.curseforge.page.back-to-browse', defaultMessage: 'Back to discover' },
	likes: {
		id: 'plus.curseforge.page.likes',
		defaultMessage: '{count, plural, one {like} other {likes}}',
	},
})

const projectId = computed(() => Number(route.params.id))
const tab = computed(() => String(route.params.tab ?? ''))
const instanceId = computed(() => (typeof route.query.i === 'string' ? route.query.i : ''))
const querySuffix = computed(() => {
	const params = new URLSearchParams()
	if (instanceId.value) params.set('i', instanceId.value)
	if (typeof route.query.b === 'string') params.set('b', route.query.b)
	const text = params.toString()
	return text ? `?${text}` : ''
})

const { data: instance } = useQuery(
	computed(() => ({
		...instanceDetailQueryOptions(instanceId.value),
		enabled: !!instanceId.value,
	})),
)

const mode = ref<CfDataMode | null>(null)
const project = shallowRef<CfProject | null>(null)
const classes = shallowRef<CfCategory[]>([])
const description = ref('')
const files = shallowRef<CfFile[]>([])
const loaders = shallowRef<unknown[]>([])
const gameVersions = shallowRef<unknown[]>([])
const plannedFile = shallowRef<CfFile | null>(null)
const plannedChecked = ref(false)
const installedHere = ref(false)
const error = ref<string | null>(null)
const loading = ref(true)
const modpackInstall = injectModpackInstall()
const installing = ref(false)
const changelogModal = ref<InstanceType<typeof NewModal> | null>(null)
const changelogTitle = ref('')
const changelogHtml = ref('')

const blocked = computed(() => mode.value === 'needs_key' || mode.value === 'key_rejected')
const classSlug = computed(() => (project.value ? classSlugOf(project.value, classes.value) : ''))
const view = computed(() => (project.value ? toProjectView(project.value, classSlug.value) : null))
const creators = computed(() => (project.value ? toCreators(project.value) : []))
const versionViews = computed(() => files.value.map(toVersionView))
const safeDescription = computed(() => configuredXss.process(description.value))
const canInstall = computed(
	() =>
		project.value?.allowModDistribution === true &&
		classInfo(classSlug.value).installable &&
		mode.value !== 'sample',
)
const isModpack = computed(() => classSlug.value === 'modpacks')
const busy = computed(
	() => installing.value || modpackInstall.value?.preparing === project.value?.id,
)
const { install: installContent } = useCurseforgeInstall(classes)
const browsePath = computed(() => `/curseforge/${classSlug.value || 'mc-mods'}`)
const backUrl = computed(() => {
	const back = route.query.b
	return typeof back === 'string' && back.startsWith('/curseforge/')
		? back
		: `${browsePath.value}${querySuffix.value}`
})
const tabLinks = computed(() => {
	const base = `/curseforge/project/${projectId.value}`
	return [
		{ label: formatMessage(messages.description), href: `${base}${querySuffix.value}` },
		{ label: formatMessage(messages.versions), href: `${base}/versions${querySuffix.value}` },
		{
			label: formatMessage(messages.gallery),
			href: `${base}/gallery${querySuffix.value}`,
			shown: (view.value?.gallery.length ?? 0) > 0,
		},
	]
})
const installContext = computed(() =>
	instance.value
		? {
				name: instance.value.name,
				loader: instance.value.loader,
				gameVersion: instance.value.game_version,
				iconSrc: getInstanceIconUrl(instance.value.icon_path),
				backUrl: backUrl.value,
				backLabel: formatMessage(messages.backToBrowse),
				heading: formatMessage(commonMessages.installingContentLabel),
			}
		: null,
)
const plannedTarget = computed(() => {
	if (!instance.value) return ''
	const needsLoader = classInfo(classSlug.value).projectType === 'mod'
	const loader = instance.value.loader
	return [
		needsLoader && loader !== 'vanilla' ? loader.charAt(0).toUpperCase() + loader.slice(1) : '',
		`Minecraft ${instance.value.game_version}`,
	]
		.filter(Boolean)
		.join(' ')
})

useBreadcrumb({
	slot: 'project',
	id: () => `curseforge-project:${projectId.value}`,
	label: () => project.value?.name ?? formatMessage(commonMessages.loadingLabel),
	to: () => route.fullPath,
	visual: () => ({
		type: 'image',
		src: view.value?.icon_url,
		alt: project.value?.name,
		tintBy: String(projectId.value),
	}),
})

async function loadPlannedVersion(): Promise<void> {
	plannedFile.value = null
	plannedChecked.value = false
	if (!instanceId.value || !project.value || !canInstall.value || isModpack.value) return
	try {
		installedHere.value = (await plus_cf_installed_projects(instanceId.value)).includes(
			project.value.id,
		)
		const options = await plus_cf_install_options(instanceId.value, project.value.id, false, null)
		plannedFile.value =
			options.files.find((option) => option.file.id === options.recommended_file_id)?.file ?? null
		plannedChecked.value = true
	} catch {
		plannedChecked.value = false
	}
}

async function load(): Promise<void> {
	loading.value = true
	error.value = null
	project.value = null
	installedHere.value = false
	try {
		mode.value = await plus_cf_data_mode()
		if (blocked.value) return
		const [loadedProject, loadedDescription, loadedClasses, loadedLoaders, loadedGameVersions] =
			await Promise.all([
				plus_cf_project(projectId.value),
				plus_cf_description(projectId.value).catch(() => ''),
				plus_cf_classes(),
				get_loaders().catch(() => []),
				get_game_versions().catch(() => []),
			])
		project.value = loadedProject
		description.value = loadedDescription
		classes.value = loadedClasses
		loaders.value = loadedLoaders
		gameVersions.value = loadedGameVersions
		files.value = await plus_cf_files(projectId.value, null, null).catch(() => [])
		void loadPlannedVersion()
	} catch (cause) {
		error.value = errorText(cause)
	} finally {
		loading.value = false
	}
}

watch(projectId, () => void load(), { immediate: true })
watch(instanceId, () => void loadPlannedVersion())
watch(curseforgeKeyRevision, () => void load())

function goToCategory(name: string): void {
	const category = project.value?.categories.find((item) => item.name === name)
	if (!category) return
	void router.push(
		`${browsePath.value}?cat=${category.id}${instanceId.value ? `&i=${encodeURIComponent(instanceId.value)}` : ''}`,
	)
}

function goToLoader(loader: string): void {
	void router.push(
		`${browsePath.value}?loader=${encodeURIComponent(loader)}${instanceId.value ? `&i=${encodeURIComponent(instanceId.value)}` : ''}`,
	)
}

function creatorLink(username: string): string {
	const author = project.value?.authors.find((item) => item.name === username)
	return (author && authorPath(author, instanceId.value)) ?? ''
}

async function install(fileId: number | null = null): Promise<void> {
	const current = project.value
	if (!current || busy.value) return
	if (isModpack.value) {
		await modpackInstall.value?.install(current, fileId)
		return
	}
	installing.value = true
	try {
		if (await installContent(current, { instanceId: instanceId.value || null, fileId })) {
			installedHere.value = true
		}
	} finally {
		installing.value = false
	}
}

async function openChangelog(file: CfFile): Promise<void> {
	changelogTitle.value = file.displayName
	changelogHtml.value = configuredXss.process(
		await plus_cf_changelog(file.modId, file.id).catch(() => ''),
	)
	changelogModal.value?.show()
}
</script>

<template>
	<div class="flex flex-col gap-4 p-6">
		<div
			v-if="installContext"
			class="sticky top-0 z-20 -mx-6 -mt-6 rounded-tl-[--radius-xl] border-0 border-b border-solid bg-surface-1 px-3 py-4 border-surface-5"
		>
			<BrowseInstallHeader :install-context="installContext" />
		</div>

		<Admonition
			v-if="blocked"
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

		<Admonition
			v-else-if="error"
			type="critical"
			:header="formatMessage(messages.failedHeader)"
			inline-actions
		>
			{{ error }}
			<template #actions>
				<Button @click="load">{{ formatMessage(messages.retry) }}</Button>
			</template>
		</Admonition>

		<LoadingIndicator v-else-if="loading" />

		<template v-else-if="project && view">
			<Teleport to="#sidebar-teleport-target">
				<ProjectSidebarCompatibility
					:project="view"
					:tags="{ loaders, gameVersions }"
					:loader-action="goToLoader"
					class="project-sidebar-section"
				/>
				<ProjectSidebarLinks link-target="_blank" :project="view" class="project-sidebar-section" />
				<ProjectSidebarTags
					:project="view"
					:tag-action="goToCategory"
					class="project-sidebar-section"
				/>
				<ProjectSidebarCreators
					:members="creators"
					:org-link="() => ''"
					:user-link="creatorLink"
					:user-link-target="null"
					class="project-sidebar-section"
				/>
				<ProjectSidebarDetails
					:project="view"
					:has-versions="files.length > 0"
					link-target="_blank"
					hide-license
					class="project-sidebar-section"
				/>
			</Teleport>

			<Admonition
				v-if="mode === 'sample'"
				type="info"
				:header="formatMessage(messages.sampleHeader)"
			>
				{{ formatMessage(messages.sampleText) }}
			</Admonition>

			<ProjectPageHeader
				:project="view"
				:followers-label="formatMessage(messages.likes, { count: view.followers })"
				@category="goToCategory"
			>
				<template #actions>
					<span
						class="rounded-full border border-solid border-surface-5 px-3 py-1 text-sm font-semibold text-secondary"
					>
						{{ formatMessage(messages.provider) }}
					</span>
					<ButtonLink
						v-if="project.links.websiteUrl && mode !== 'sample'"
						v-tooltip="formatMessage(messages.openOnCurseforge)"
						size="xl"
						:href="project.links.websiteUrl"
						target="_blank"
					>
						<ExternalIcon aria-hidden="true" />
					</ButtonLink>
					<Button v-if="installedHere" size="xl" disabled>
						<CheckIcon aria-hidden="true" />
						{{ formatMessage(messages.installed) }}
					</Button>
					<Button
						v-else-if="canInstall"
						type="colored"
						color="brand"
						size="xl"
						:disabled="busy"
						@click="install()"
					>
						<SpinnerIcon v-if="busy" class="animate-spin" aria-hidden="true" />
						<DownloadIcon v-else aria-hidden="true" />
						{{
							formatMessage(busy ? commonMessages.installingLabel : commonMessages.installButton)
						}}
					</Button>
				</template>
			</ProjectPageHeader>

			<Admonition
				v-if="mode !== 'sample' && project.allowModDistribution !== true"
				type="info"
				:header="formatMessage(messages.notDistributableHeader)"
			>
				{{ formatMessage(messages.notDistributable) }}
			</Admonition>
			<Admonition
				v-else-if="mode !== 'sample' && !classInfo(classSlug).installable"
				type="info"
				:header="formatMessage(messages.unsupportedHeader)"
			>
				{{ formatMessage(messages.unsupported) }}
			</Admonition>

			<Admonition
				v-if="instanceId && plannedChecked && !plannedFile && !installedHere"
				type="warning"
				:header="formatMessage(messages.noCompatibleHeader)"
			>
				{{ formatMessage(messages.noCompatibleText, { target: plannedTarget }) }}
			</Admonition>

			<NavTabs :links="tabLinks" />

			<template v-if="tab === 'versions'">
				<ProjectPageVersions
					:loaders="loaders"
					:game-versions="gameVersions"
					:versions="versionViews"
					:project="{ id: String(project.id), slug: project.slug, project_type: view.project_type }"
				>
					<template #actions="{ version }">
						<Button type="quiet" @click.stop="openChangelog(version.curseforge)">
							{{ formatMessage(messages.changelog) }}
						</Button>
						<IconButton
							v-if="canInstall"
							v-tooltip="formatMessage(commonMessages.installButton)"
							type="quiet"
							color="green"
							:disabled="busy"
							:label="formatMessage(commonMessages.installButton)"
							@click.stop="install(version.curseforge.id)"
						>
							<DownloadIcon />
						</IconButton>
					</template>
				</ProjectPageVersions>
			</template>
			<Gallery v-else-if="tab === 'gallery'" :project="view" />
			<Card v-else>
				<!-- eslint-disable-next-line vue/no-v-html -- sanitized with configuredXss -->
				<div v-if="safeDescription" class="markdown-body" v-html="safeDescription" />
				<p v-else class="m-0 text-secondary">{{ formatMessage(messages.noDescription) }}</p>
			</Card>

			<NewModal ref="changelogModal" :header="changelogTitle" max-width="640px">
				<!-- eslint-disable-next-line vue/no-v-html -- sanitized with configuredXss -->
				<div v-if="changelogHtml" class="markdown-body" v-html="changelogHtml" />
				<p v-else class="m-0 text-secondary">{{ formatMessage(messages.noChangelog) }}</p>
			</NewModal>
		</template>
	</div>
</template>

<style scoped lang="scss">
:global(.project-sidebar-section) {
	padding: 1rem;
	border-bottom: 1px solid var(--brand-gradient-border);
}
</style>
