<script setup lang="ts">
import { SearchIcon } from '@modrinth/assets'
import { computed } from 'vue'

import Combobox from '#ui/components/base/Combobox.vue'
import Input from '#ui/components/base/inputs/Input.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { getTagMessage } from '#ui/utils/tag-messages'

import {
	type ProjectSort,
	projectFilterOptions,
	type ProjectToolsState,
	type ToolProject,
} from '../project-tools'

const props = defineProps<{
	projects: readonly ToolProject[]
}>()

const state = defineModel<ProjectToolsState>({ required: true })

const { formatMessage } = useVIntl()

const messages = defineMessages({
	search: { id: 'user-profile.project-tools.search', defaultMessage: 'Search projects...' },
	sortDownloads: { id: 'user-profile.project-tools.sort.downloads', defaultMessage: 'Most downloads' },
	sortFollowers: { id: 'user-profile.project-tools.sort.followers', defaultMessage: 'Most popular' },
	sortUpdated: { id: 'user-profile.project-tools.sort.updated', defaultMessage: 'Recently updated' },
	sortName: { id: 'user-profile.project-tools.sort.name', defaultMessage: 'Name' },
	allCategories: { id: 'user-profile.project-tools.all-categories', defaultMessage: 'All categories' },
	allLoaders: { id: 'user-profile.project-tools.all-loaders', defaultMessage: 'All loaders' },
	allVersions: { id: 'user-profile.project-tools.all-versions', defaultMessage: 'All versions' },
})

const options = computed(() => projectFilterOptions(props.projects))

const sortOptions = computed<{ value: ProjectSort; label: string }[]>(() => [
	{ value: 'downloads', label: formatMessage(messages.sortDownloads) },
	{ value: 'followers', label: formatMessage(messages.sortFollowers) },
	{ value: 'updated', label: formatMessage(messages.sortUpdated) },
	{ value: 'name', label: formatMessage(messages.sortName) },
])

function tagLabel(tag: string): string {
	const message = getTagMessage(tag)
	return message ? formatMessage(message) : tag
}

function withAll(values: string[], allLabel: string, label: (value: string) => string = (v) => v) {
	return [
		{ value: null as string | null, label: allLabel },
		...values.map((value) => ({ value: value as string | null, label: label(value) })),
	]
}

const categoryOptions = computed(() =>
	withAll(options.value.categories, formatMessage(messages.allCategories), tagLabel),
)
const loaderOptions = computed(() =>
	withAll(options.value.loaders, formatMessage(messages.allLoaders), tagLabel),
)
const versionOptions = computed(() =>
	withAll(options.value.gameVersions, formatMessage(messages.allVersions)),
)

function update(change: Partial<ProjectToolsState>): void {
	state.value = { ...state.value, ...change, page: 1 }
}
</script>

<template>
	<div class="flex flex-wrap items-center gap-2">
		<Input
			:model-value="state.search"
			:icon="SearchIcon"
			type="text"
			autocomplete="off"
			:placeholder="formatMessage(messages.search)"
			clearable
			wrapper-class="min-w-48 grow basis-64"
			@update:model-value="update({ search: String($event ?? '') })"
			@clear="update({ search: '' })"
		/>
		<Combobox
			:model-value="state.sort"
			:options="sortOptions"
			trigger-type="base"
			class="!w-44"
			@update:model-value="update({ sort: $event })"
		/>
		<Combobox
			v-if="options.categories.length > 1"
			:model-value="state.category"
			:options="categoryOptions"
			trigger-type="base"
			class="!w-44"
			@update:model-value="update({ category: $event })"
		/>
		<Combobox
			v-if="options.loaders.length > 1"
			:model-value="state.loader"
			:options="loaderOptions"
			trigger-type="base"
			class="!w-36"
			@update:model-value="update({ loader: $event })"
		/>
		<Combobox
			v-if="options.gameVersions.length > 1"
			:model-value="state.gameVersion"
			:options="versionOptions"
			trigger-type="base"
			class="!w-36"
			@update:model-value="update({ gameVersion: $event })"
		/>
	</div>
</template>
