import type { NewsArticle } from '@modrinth/ui'
import { queryOptions } from '@tanstack/vue-query'

import { type NewsCategory, plus_news_feed, type PlusNewsArticle } from '@/helpers/plus'

const REFRESH_AFTER_MS = 60 * 60 * 1000

export const newsKeys = {
	articles: ['pawrinth-news', 'articles'] as const,
}

export type PawrinthNewsArticle = NewsArticle & { category: NewsCategory }

/** The fields the news card expects. An article with no image shows without one. */
export function toNewsCard(article: PlusNewsArticle): PawrinthNewsArticle {
	return {
		path: article.link,
		title: article.title,
		summary: article.summary,
		date: article.date,
		thumbnail: article.thumbnail ?? undefined,
		category: article.category,
	}
}

export function newsArticlesQueryOptions() {
	return queryOptions({
		queryKey: newsKeys.articles,
		queryFn: async () => {
			const feed = await plus_news_feed()
			return {
				configured: feed.configured,
				articles: feed.articles.map(toNewsCard),
				allNewsUrl: feed.all_news_url,
			}
		},
		staleTime: REFRESH_AFTER_MS,
		retry: false,
		refetchOnWindowFocus: false,
	})
}
