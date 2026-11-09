<script setup lang="ts">
import { DownloadIcon, SearchIcon, SpinnerIcon } from '@modrinth/assets'
import {
	Admonition,
	Button,
	commonMessages,
	defineMessages,
	Input,
	LoadingIndicator,
	ProjectCard,
	ProjectCardList,
	useVIntl,
} from '@modrinth/ui'
import { useDebounceFn } from '@vueuse/core'
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { errorText } from '@/features/plus-curseforge/project-adapter'
import ProviderSwitch from '@/features/plus-curseforge/provider-switch.vue'
import { plus_technic_browse, type TechnicPackSummary } from '@/helpers/plus'
import { useBreadcrumb } from '@/shared/breadcrumbs'

import TechnicInstall from './technic-install.vue'
import { technicPackPath, technicPreparing } from './technic-state'

const { formatMessage } = useVIntl()
const route = useRoute()
const router = useRouter()

const messages = defineMessages({
	title: { id: 'plus.technic.title', defaultMessage: 'Technic modpacks' },
	description: {
		id: 'plus.technic.description',
		defaultMessage:
			'Modpacks from the Technic Platform. Each one is made and hosted by its creator, and installs as a new instance.',
	},
	searchPlaceholder: { id: 'plus.technic.search', defaultMessage: 'Search Technic modpacks' },
	trending: { id: 'plus.technic.trending', defaultMessage: 'Trending on Technic' },
	results: { id: 'plus.technic.results', defaultMessage: 'Search results' },
	noResults: { id: 'plus.technic.no-results', defaultMessage: 'No Technic modpacks match that.' },
	failedHeader: { id: 'plus.technic.failed', defaultMessage: "Couldn't load Technic" },
	retry: { id: 'plus.technic.retry', defaultMessage: 'Try again' },
	minecraft: { id: 'plus.technic.minecraft', defaultMessage: 'Minecraft {version}' },
})

const query = ref(typeof route.query.q === 'string' ? route.query.q : '')
const results = ref<TechnicPackSummary[]>([])
const loading = ref(true)
const loadError = ref<string | null>(null)
const installer = ref<InstanceType<typeof TechnicInstall> | null>(null)
let request = 0

const searching = computed(() => query.value.trim().length > 0)

async function load(): Promise<void> {
	const id = ++request
	loading.value = true
	loadError.value = null
	try {
		const packs = await plus_technic_browse(query.value.trim() || null)
		if (id === request) results.value = packs
	} catch (error) {
		if (id === request) {
			results.value = []
			loadError.value = errorText(error)
		}
	} finally {
		if (id === request) loading.value = false
	}
}

const search = useDebounceFn(() => {
	const text = query.value.trim()
	void router.replace({ query: text ? { q: text } : {} })
	void load()
}, 400)

function summaryOf(pack: TechnicPackSummary): string {
	return [
		pack.game_version ? formatMessage(messages.minecraft, { version: pack.game_version }) : null,
		pack.summary,
	]
		.filter(Boolean)
		.join(' · ')
}

useBreadcrumb({
	slot: 'page',
	id: () => 'technic-browse',
	label: () => formatMessage(messages.title),
	to: () => route.fullPath,
})

onMounted(load)
</script>

<template>
	<div class="flex flex-col gap-4 p-6">
		<ProviderSwitch />
		<div>
			<h1 class="m-0 text-2xl font-semibold text-contrast">{{ formatMessage(messages.title) }}</h1>
			<p class="m-0 mt-1 text-secondary">{{ formatMessage(messages.description) }}</p>
		</div>
		<Input
			v-model="query"
			:icon="SearchIcon"
			clearable
			:placeholder="formatMessage(messages.searchPlaceholder)"
			:spellcheck="false"
			autocomplete="off"
			wrapper-class="w-full"
			@update:model-value="search"
		/>
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
		<LoadingIndicator v-else-if="loading" />
		<p v-else-if="results.length === 0" class="m-0 text-secondary">
			{{ formatMessage(messages.noResults) }}
		</p>
		<template v-else>
			<h2 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(searching ? messages.results : messages.trending) }}
			</h2>
			<ProjectCardList layout="list">
				<ProjectCard
					v-for="pack in results"
					:key="pack.slug"
					layout="list"
					:link="technicPackPath(pack.slug)"
					:icon-url="pack.icon_url ?? undefined"
					:title="pack.name"
					:author="pack.author ? { name: pack.author } : undefined"
					:summary="summaryOf(pack)"
					:downloads="pack.installs ?? undefined"
				>
					<template #actions>
						<Button
							type="colored"
							color="brand"
							:disabled="technicPreparing !== null"
							@click="installer?.install(pack.slug, pack.name)"
						>
							<SpinnerIcon
								v-if="technicPreparing === pack.slug"
								class="animate-spin"
								aria-hidden="true"
							/>
							<DownloadIcon v-else aria-hidden="true" />
							{{
								formatMessage(
									technicPreparing === pack.slug
										? commonMessages.installingLabel
										: commonMessages.installButton,
								)
							}}
						</Button>
					</template>
				</ProjectCard>
			</ProjectCardList>
		</template>
		<TechnicInstall ref="installer" />
	</div>
</template>
