<script setup lang="ts">
import { DownloadIcon, ExternalIcon, LeftArrowIcon, SpinnerIcon } from '@modrinth/assets'
import {
	Admonition,
	Button,
	ButtonLink,
	commonMessages,
	defineMessages,
	IconButton,
	LoadingIndicator,
	ProjectPageHeader,
	useVIntl,
} from '@modrinth/ui'
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { errorText } from '@/features/plus-curseforge/project-adapter'
import { plus_technic_pack, type TechnicPackDetails } from '@/helpers/plus'
import { useBreadcrumb } from '@/shared/breadcrumbs'

import TechnicInstall from './technic-install.vue'
import { technicPreparing } from './technic-state'

const { formatMessage } = useVIntl()
const route = useRoute()
const router = useRouter()

const messages = defineMessages({
	back: { id: 'plus.technic.back', defaultMessage: 'Back' },
	provider: { id: 'plus.technic.provider', defaultMessage: 'Technic' },
	openOnTechnic: { id: 'plus.technic.open', defaultMessage: 'Open on Technic' },
	failedHeader: { id: 'plus.technic.pack.failed', defaultMessage: "Couldn't load this modpack" },
	retry: { id: 'plus.technic.retry', defaultMessage: 'Try again' },
	minecraft: { id: 'plus.technic.minecraft', defaultMessage: 'Minecraft {version}' },
	packVersion: { id: 'plus.technic.pack.version', defaultMessage: 'Version {version}' },
	insecureHeader: {
		id: 'plus.technic.pack.insecure.header',
		defaultMessage: 'Downloads over an insecure connection',
	},
	insecureText: {
		id: 'plus.technic.pack.insecure.text',
		defaultMessage:
			'This pack is downloaded over plain HTTP, so its files can’t be checked on the way. Pawrinth asks before installing it.',
	},
	nothingHeader: {
		id: 'plus.technic.pack.nothing.header',
		defaultMessage: 'Nothing to download',
	},
	nothingText: {
		id: 'plus.technic.pack.nothing.text',
		defaultMessage: 'Technic doesn’t list a download for this pack, so it can’t be installed.',
	},
	about: { id: 'plus.technic.pack.about', defaultMessage: 'About' },
	noDescription: {
		id: 'plus.technic.pack.no-description',
		defaultMessage: 'The creator hasn’t written a description.',
	},
	details: { id: 'plus.technic.pack.details', defaultMessage: 'Details' },
	gameVersion: { id: 'plus.technic.pack.game-version', defaultMessage: 'Minecraft' },
	latestVersion: { id: 'plus.technic.pack.latest', defaultMessage: 'Latest version' },
	installs: { id: 'plus.technic.pack.installs', defaultMessage: 'Installs' },
	runs: { id: 'plus.technic.pack.runs', defaultMessage: 'Times played' },
	official: { id: 'plus.technic.pack.official', defaultMessage: 'Official Technic pack' },
	tags: { id: 'plus.technic.pack.tags', defaultMessage: 'Tags' },
	updates: { id: 'plus.technic.pack.updates', defaultMessage: 'Updates from the creator' },
	likes: {
		id: 'plus.technic.pack.likes',
		defaultMessage: '{count, plural, one {like} other {likes}}',
	},
})

const slug = computed(() => String(route.params.slug ?? ''))
const pack = ref<TechnicPackDetails | null>(null)
const loading = ref(true)
const error = ref<string | null>(null)
const installer = ref<InstanceType<typeof TechnicInstall> | null>(null)
const numbers = new Intl.NumberFormat()
const dates = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' })

async function load(): Promise<void> {
	const current = slug.value
	loading.value = true
	error.value = null
	try {
		const details = await plus_technic_pack(current)
		if (current === slug.value) pack.value = details
	} catch (problem) {
		if (current === slug.value) {
			pack.value = null
			error.value = errorText(problem)
		}
	} finally {
		if (current === slug.value) loading.value = false
	}
}

watch(slug, load, { immediate: true })

const busy = computed(() => technicPreparing.value === pack.value?.slug)

const view = computed(() => {
	const current = pack.value
	if (!current) return null
	return {
		id: `technic-${current.slug}`,
		title: current.name,
		description: [
			current.game_version
				? formatMessage(messages.minecraft, { version: current.game_version })
				: null,
			current.version ? formatMessage(messages.packVersion, { version: current.version }) : null,
		]
			.filter(Boolean)
			.join(' · '),
		status: 'approved',
		downloads: current.installs ?? 0,
		followers: current.ratings ?? 0,
		categories: [],
		icon_url: current.icon_url,
	} as never
})

const facts = computed(() => {
	const current = pack.value
	if (!current) return []
	return [
		{ label: messages.gameVersion, value: current.game_version },
		{ label: messages.latestVersion, value: current.version },
		{
			label: messages.installs,
			value: current.installs === null ? null : numbers.format(current.installs),
		},
		{ label: messages.runs, value: current.runs === null ? null : numbers.format(current.runs) },
	].filter((fact): fact is { label: (typeof messages)['installs']; value: string } =>
		Boolean(fact.value),
	)
})

function dateOf(seconds: number | null): string | null {
	return seconds ? dates.format(new Date(seconds * 1000)) : null
}

useBreadcrumb({
	slot: 'project',
	id: () => `technic-pack:${slug.value}`,
	label: () => pack.value?.name ?? formatMessage(commonMessages.loadingLabel),
	to: () => route.fullPath,
	visual: () => ({
		type: 'image',
		src: pack.value?.icon_url ?? undefined,
		alt: pack.value?.name,
		tintBy: slug.value,
	}),
})
</script>

<template>
	<div class="flex flex-col gap-4 p-6">
		<div>
			<IconButton
				v-tooltip="formatMessage(messages.back)"
				:label="formatMessage(messages.back)"
				native-type="button"
				@click="router.back()"
			>
				<LeftArrowIcon />
			</IconButton>
		</div>

		<Admonition
			v-if="error"
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

		<template v-else-if="pack && view">
			<img
				v-if="pack.background_url"
				:src="pack.background_url"
				alt=""
				class="aspect-[4/1] w-full rounded-2xl object-cover"
			/>

			<ProjectPageHeader
				:project="view"
				:followers-label="formatMessage(messages.likes, { count: pack.ratings ?? 0 })"
			>
				<template #actions>
					<span
						class="rounded-full border border-solid border-surface-5 px-3 py-1 text-sm font-semibold text-secondary"
					>
						{{ formatMessage(messages.provider) }}
					</span>
					<ButtonLink
						v-if="pack.page_url"
						v-tooltip="formatMessage(messages.openOnTechnic)"
						size="xl"
						:href="pack.page_url"
						target="_blank"
					>
						<ExternalIcon aria-hidden="true" />
					</ButtonLink>
					<Button
						v-if="pack.downloadable"
						type="colored"
						color="brand"
						size="xl"
						:disabled="technicPreparing !== null"
						@click="installer?.install(pack.slug, pack.name)"
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
				v-if="!pack.downloadable"
				type="info"
				:header="formatMessage(messages.nothingHeader)"
			>
				{{ formatMessage(messages.nothingText) }}
			</Admonition>
			<Admonition
				v-else-if="pack.insecure"
				type="warning"
				:header="formatMessage(messages.insecureHeader)"
			>
				{{ formatMessage(messages.insecureText) }}
			</Admonition>

			<div class="grid gap-4 lg:grid-cols-[1fr_18rem]">
				<section class="rounded-2xl border border-solid border-surface-5 bg-surface-2 p-5">
					<h2 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.about) }}
					</h2>
					<p class="m-0 mt-2 whitespace-pre-line break-words">
						{{ pack.description ?? formatMessage(messages.noDescription) }}
					</p>
				</section>

				<section
					class="flex flex-col gap-3 rounded-2xl border border-solid border-surface-5 bg-surface-2 p-5"
				>
					<h2 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.details) }}
					</h2>
					<dl class="m-0 flex flex-col gap-2">
						<div v-for="fact in facts" :key="fact.label.id" class="flex justify-between gap-3">
							<dt class="text-secondary">{{ formatMessage(fact.label) }}</dt>
							<dd class="m-0 text-right font-semibold text-contrast">{{ fact.value }}</dd>
						</div>
					</dl>
					<p v-if="pack.official" class="m-0 font-semibold text-brand">
						{{ formatMessage(messages.official) }}
					</p>
					<div v-if="pack.tags.length > 0">
						<h3 class="m-0 text-sm font-semibold text-secondary">
							{{ formatMessage(messages.tags) }}
						</h3>
						<div class="mt-2 flex flex-wrap gap-1">
							<span
								v-for="tag in pack.tags"
								:key="tag"
								class="rounded-full bg-surface-4 px-2 py-0.5 text-sm text-primary"
							>
								{{ tag }}
							</span>
						</div>
					</div>
				</section>
			</div>

			<section
				v-if="pack.updates.length > 0"
				class="rounded-2xl border border-solid border-surface-5 bg-surface-2 p-5"
			>
				<h2 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.updates) }}
				</h2>
				<ul class="m-0 mt-3 flex list-none flex-col gap-3 p-0">
					<li
						v-for="(update, position) in pack.updates"
						:key="position"
						class="flex flex-col gap-1"
					>
						<span class="text-sm text-secondary">
							{{ [update.author, dateOf(update.date)].filter(Boolean).join(' · ') }}
						</span>
						<span class="whitespace-pre-line break-words">{{ update.text }}</span>
					</li>
				</ul>
			</section>
		</template>

		<TechnicInstall ref="installer" />
	</div>
</template>
