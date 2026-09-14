<script setup lang="ts">
import { ExternalIcon, NewspaperIcon } from '@modrinth/assets'
import { Button, ButtonLink, defineMessages, NewsArticleCard, useVIntl } from '@modrinth/ui'
import { useQuery } from '@tanstack/vue-query'
import { computed } from 'vue'

import { newsArticlesQueryOptions } from './queries'

const MAX_SHOWN = 4

const { formatMessage } = useVIntl()
const messages = defineMessages({
	news: { id: 'plus.news.title', defaultMessage: 'Pawrinth News' },
	viewAllNews: { id: 'plus.news.view-all', defaultMessage: 'View all news' },
	empty: { id: 'plus.news.empty', defaultMessage: 'No announcements yet.' },
	emptyText: {
		id: 'plus.news.empty.text',
		defaultMessage: 'Pawrinth release, update and maintenance news will show up here.',
	},
	failed: { id: 'plus.news.failed', defaultMessage: "Couldn't load Pawrinth News." },
	retry: { id: 'plus.news.retry', defaultMessage: 'Try again' },
	modrinthNews: { id: 'plus.news.modrinth', defaultMessage: "Modrinth's own news" },
	release: { id: 'plus.news.category.release', defaultMessage: 'Release' },
	update: { id: 'plus.news.category.update', defaultMessage: 'Update' },
	feature: { id: 'plus.news.category.feature', defaultMessage: 'New feature' },
	maintenance: { id: 'plus.news.category.maintenance', defaultMessage: 'Maintenance' },
	compatibility: { id: 'plus.news.category.compatibility', defaultMessage: 'Compatibility' },
	community: { id: 'plus.news.category.community', defaultMessage: 'Community' },
	announcement: { id: 'plus.news.category.announcement', defaultMessage: 'Announcement' },
})

const { data: feed, isError, isFetching, refetch } = useQuery(newsArticlesQueryOptions())

const articles = computed(() => feed.value?.articles.slice(0, MAX_SHOWN) ?? [])
</script>

<template>
	<div class="flex flex-col p-4">
		<h3 class="m-0 mb-4 w-full text-left text-base font-medium text-primary">
			{{ formatMessage(messages.news) }}
		</h3>

		<div v-if="articles.length > 0" class="flex w-full flex-col items-center space-y-4">
			<div v-for="item in articles" :key="item.path" class="flex w-full flex-col gap-2">
				<span class="text-xs font-semibold text-secondary">
					{{ formatMessage(messages[item.category]) }}
				</span>
				<NewsArticleCard :article="item" />
			</div>
			<ButtonLink
				v-if="feed?.allNewsUrl"
				type="colored"
				color="brand"
				size="xl"
				:href="feed.allNewsUrl"
				target="_blank"
				class="my-4"
			>
				<NewspaperIcon />
				{{ formatMessage(messages.viewAllNews) }}
			</ButtonLink>
		</div>

		<div v-else-if="isError" class="flex flex-col items-start gap-2">
			<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.failed) }}</p>
			<Button :disabled="isFetching" @click="refetch()">
				{{ formatMessage(messages.retry) }}
			</Button>
		</div>

		<div v-else-if="feed" class="flex flex-col gap-1">
			<p class="m-0 text-sm font-semibold text-contrast">{{ formatMessage(messages.empty) }}</p>
			<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.emptyText) }}</p>
		</div>

		<a
			class="mt-4 inline-flex items-center gap-1 text-sm text-secondary hover:underline"
			href="https://modrinth.com/news"
			target="_blank"
		>
			{{ formatMessage(messages.modrinthNews) }}
			<ExternalIcon aria-hidden="true" />
		</a>
	</div>
</template>
