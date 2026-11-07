import type { Router } from 'vue-router'

import { usePlusSettings } from './use-plus-settings'

const LAST_PAGE_KEY = 'plus-last-page'

/** Reopens the remembered page once, and keeps remembering pages from then on. */
export function restoreLastPage(router: Router): void {
	const { settings } = usePlusSettings()

	router.afterEach((to) => {
		try {
			window.localStorage.setItem(LAST_PAGE_KEY, to.fullPath)
		} catch {
			// storage blocked or full
		}
	})

	if (!settings.value?.interface.remember_last_page) return

	try {
		const lastPage = window.localStorage.getItem(LAST_PAGE_KEY)
		if (lastPage?.startsWith('/') && router.resolve(lastPage).matched.length > 0) {
			void router.replace(lastPage)
		}
	} catch {
		// storage blocked or full
	}
}
