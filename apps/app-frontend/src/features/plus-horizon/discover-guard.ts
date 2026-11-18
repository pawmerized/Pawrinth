import type { Router } from 'vue-router'

import { usePlusSettings } from '@/features/plus-settings/use-plus-settings'
import { get as getInstance } from '@/helpers/instance'

import { HORIZON_LOADER, playedLoader } from './horizon'
import { discoverLocation } from './horizon-packs'

/** Other sites' Discover pages, which a Horizon instance never opens. */
const OTHER_DISCOVER_ROUTES = new Set(['Discover content', 'Discover on CurseForge'])

interface DiscoverTarget {
	name?: unknown
	params: Record<string, unknown>
	query: Record<string, unknown>
}

/** Where a Discover navigation goes instead, when its instance plays with Horizon. */
export function horizonDiscoverRedirect(
	to: DiscoverTarget,
	instance: { id: string; loader: string; game_version: string } | null,
	horizonInstances: Readonly<Record<string, boolean>> | undefined,
): { path: string; query: { i: string } } | null {
	if (!instance || !OTHER_DISCOVER_ROUTES.has(String(to.name))) return null
	if (playedLoader(instance, horizonInstances) !== HORIZON_LOADER) return null
	const type = String(to.params.projectType ?? to.params.classSlug ?? '')
	return discoverLocation(HORIZON_LOADER, instance.id, type)
}

/**
 * A Horizon instance only has Horizon's resource packs and shaders. This decides before another
 * site's Discover page opens, so that page never shows, even for a moment.
 */
export function guardHorizonDiscover(router: Router): void {
	const { settings } = usePlusSettings()
	router.beforeEach(async (to) => {
		const instanceId = to.query.i
		if (!OTHER_DISCOVER_ROUTES.has(String(to.name)) || typeof instanceId !== 'string') return true
		const instance = await getInstance(instanceId).catch(() => null)
		return horizonDiscoverRedirect(to, instance, settings.value?.horizon?.instances) ?? true
	})
}
