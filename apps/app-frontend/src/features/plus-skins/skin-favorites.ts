import { computed } from 'vue'

import { patchPlusSettings, usePlusSettings } from '@/features/plus-settings/use-plus-settings'

import {
	type DiscoveredSkin,
	parseFavorites,
	toFavorite,
	withFavorite,
	withoutFavorite,
} from './discovered-skin'

const { settings } = usePlusSettings()
const favorites = computed(() => parseFavorites(settings.value?.skins?.favorites))

async function save(list: ReturnType<typeof parseFavorites>): Promise<void> {
	await patchPlusSettings({ skins: { favorites: list } })
}

export function useSkinFavorites() {
	return {
		favorites,
		isFavorite: (key: string) => favorites.value.some((item) => item.key === key),
		addFavorite: (skin: DiscoveredSkin, tags: readonly string[] = []) =>
			save(withFavorite(favorites.value, toFavorite(skin, tags, new Date().toISOString()))),
		removeFavorite: (key: string) => save(withoutFavorite(favorites.value, key)),
	}
}
