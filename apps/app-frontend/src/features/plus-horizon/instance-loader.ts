import { usePlusSettings } from '@/features/plus-settings/use-plus-settings'

import { playedLoader } from './horizon'

/** Reads an instance's loader the way it plays: Horizon where Horizon is turned on. */
export function useInstanceLoader() {
	const { settings } = usePlusSettings()
	return (instance: { id: string; loader: string; game_version: string }) =>
		playedLoader(instance, settings.value?.horizon?.instances)
}
