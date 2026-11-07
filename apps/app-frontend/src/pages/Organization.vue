<template>
	<div class="flex w-full flex-col gap-4 p-6">
		<template v-if="organization">
			<div class="flex flex-wrap items-center gap-4">
				<IconButton
					v-tooltip="formatMessage(messages.back)"
					size="xl"
					:label="formatMessage(messages.back)"
					native-type="button"
					@click="router.back()"
				>
					<LeftArrowIcon />
				</IconButton>
				<Avatar :src="organization.icon_url" :alt="organization.name" size="64px" />
				<div class="flex min-w-0 grow flex-col gap-1">
					<h1 class="m-0 text-2xl font-semibold text-contrast">{{ organization.name }}</h1>
					<p v-if="organization.description" class="m-0 text-secondary">
						{{ organization.description }}
					</p>
					<span class="text-sm text-secondary">
						{{ formatMessage(messages.projectCount, { count: projects.length }) }}
					</span>
				</div>
				<ButtonLink
					:href="`https://modrinth.com/organization/${organization.slug ?? organization.id}`"
					target="_blank"
				>
					{{ formatMessage(messages.openOnModrinth) }}
					<ExternalIcon aria-hidden="true" />
				</ButtonLink>
			</div>

			<ProjectToolsBar v-if="projects.length > 1" v-model="tools" :projects="projects" />

			<ProjectList
				v-if="visibleProjects.length > 0"
				:projects="visibleProjects"
				layout="list"
				link-mode="app"
			>
				<template #actions="{ project }">
					<Button
						v-if="project.minecraft_server == null"
						type="outlined"
						class="!text-brand [&>svg]:!text-brand !shadow-[inset_0_0_0_1px_var(--color-brand)]"
						:disabled="installingProjectIds.has(project.id)"
						@click.stop="installProject(project)"
					>
						<SpinnerIcon v-if="installingProjectIds.has(project.id)" class="animate-spin" />
						<PlusIcon v-else />
						{{
							formatMessage(
								installingProjectIds.has(project.id)
									? commonMessages.installingLabel
									: commonMessages.installButton,
							)
						}}
					</Button>
				</template>
			</ProjectList>

			<Pagination
				v-if="pageCount > 1"
				class="ml-auto"
				:page="tools.page"
				:count="pageCount"
				@switch-page="tools = { ...tools, page: $event }"
			/>

			<EmptyState
				v-if="visibleProjects.length === 0"
				type="empty"
				:heading="
					formatMessage(projects.length === 0 ? messages.noProjects : messages.noProjectsMatch)
				"
			/>
		</template>

		<Admonition v-else-if="failed" type="critical" :header="formatMessage(messages.loadFailed)">
			{{ formatMessage(messages.loadFailedText) }}
		</Admonition>
		<LoadingIndicator v-else />
	</div>
</template>

<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { ExternalIcon, LeftArrowIcon, PlusIcon, SpinnerIcon } from '@modrinth/assets'
import {
	Admonition,
	Avatar,
	Button,
	ButtonLink,
	commonMessages,
	defineMessages,
	EmptyState,
	filterProjects,
	IconButton,
	injectModrinthClient,
	injectNotificationManager,
	LoadingIndicator,
	pageCountOf,
	pageOf,
	Pagination,
	ProjectList,
	ProjectToolsBar,
	rememberedProjectTools,
	rememberProjectTools,
	useVIntl,
} from '@modrinth/ui'
import { computed, ref, shallowRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { injectContentInstall } from '@/providers/content-install'
import { useBreadcrumb } from '@/shared/breadcrumbs'

const route = useRoute()
const router = useRouter()
const client = injectModrinthClient()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { install: installVersion } = injectContentInstall()

const messages = defineMessages({
	back: { id: 'plus.organization.back', defaultMessage: 'Back' },
	projectCount: {
		id: 'plus.organization.project-count',
		defaultMessage: '{count, plural, one {# project} other {# projects}}',
	},
	openOnModrinth: { id: 'plus.organization.open', defaultMessage: 'Open on Modrinth' },
	noProjects: {
		id: 'plus.organization.no-projects',
		defaultMessage: "This organization hasn't published any projects",
	},
	noProjectsMatch: {
		id: 'plus.organization.no-match',
		defaultMessage: 'No projects match these filters',
	},
	loadFailed: { id: 'plus.organization.failed', defaultMessage: "Couldn't load this organization" },
	loadFailedText: {
		id: 'plus.organization.failed.text',
		defaultMessage: 'It may have been removed, or Modrinth may be unreachable right now.',
	},
})

const organizationId = computed(() => String(route.params.id ?? ''))
const organization = shallowRef<Labrinth.Organizations.v3.Organization | null>(null)
const projects = shallowRef<Labrinth.Projects.v3.Project[]>([])
const failed = ref(false)
const installingProjectIds = ref(new Set<string>())

const stateKey = computed(() => `organization:${organizationId.value}`)
const tools = ref(rememberedProjectTools(stateKey.value))
watch(tools, (value) => rememberProjectTools(stateKey.value, value), { deep: true })

const matching = computed(() => filterProjects(projects.value, tools.value))
const pageCount = computed(() => pageCountOf(matching.value.length))
const visibleProjects = computed(() => pageOf(matching.value, tools.value.page))

useBreadcrumb({
	slot: 'user',
	id: () => `organization:${organizationId.value}`,
	label: () => organization.value?.name ?? formatMessage(commonMessages.loadingLabel),
	to: () => route.fullPath,
	visual: () => ({
		type: 'image',
		src: organization.value?.icon_url,
		alt: organization.value?.name,
		tintBy: organizationId.value,
	}),
})

async function load(id: string): Promise<void> {
	failed.value = false
	organization.value = null
	projects.value = []
	try {
		const [loadedOrganization, loadedProjects] = await Promise.all([
			client.labrinth.organizations_v3.get(id),
			client.labrinth.organizations_v3.getProjects(id),
		])
		organization.value = loadedOrganization
		projects.value = loadedProjects
	} catch {
		failed.value = true
	}
}

watch(
	organizationId,
	(id) => {
		if (id) void load(id)
	},
	{ immediate: true },
)

function setInstalling(projectId: string, installing: boolean): void {
	const next = new Set(installingProjectIds.value)
	if (installing) next.add(projectId)
	else next.delete(projectId)
	installingProjectIds.value = next
}

async function installProject(project: Labrinth.Projects.v3.Project): Promise<void> {
	setInstalling(project.id, true)
	try {
		await installVersion(
			project.id,
			null,
			null,
			'OrganizationPage',
			() => setInstalling(project.id, false),
			(instanceId) => router.push(`/instance/${encodeURIComponent(instanceId)}`),
		)
	} catch (error) {
		setInstalling(project.id, false)
		handleError(error as Error)
	}
}
</script>
