import { invoke } from '@tauri-apps/api/core'

/** The only Minecraft version Horizon plays. */
export const HORIZON_GAME_VERSION = '26.3'

/** The Create instance window's loaders: upstream's, then Horizon. */
export const CREATION_LOADERS = ['fabric', 'neoforge', 'forge', 'quilt', 'horizon']

/** Horizon makes a vanilla instance of the one version it plays. */
export const CREATION_FIXED_VERSION_LOADERS = { horizon: HORIZON_GAME_VERSION }

export interface HorizonSetupProgress {
	message: string
	fraction: number | null
}

export interface HorizonStatus {
	supported: boolean
	version: string | null
	ready: boolean
	setting_up: HorizonSetupProgress | null
	enabled: boolean
	eligible: boolean
}

export type HorizonNote = 'unsupported' | 'ineligible' | 'not-set-up' | 'needs-setup' | null

export interface HorizonView {
	note: HorizonNote
	canToggle: boolean
	canSetUp: boolean
	canRemove: boolean
	setUpIsUpdate: boolean
}

/** What the Horizon tab shows for a status. */
export function horizonView(status: HorizonStatus): HorizonView {
	const busy = status.setting_up !== null
	const installed = status.version !== null
	let note: HorizonNote = null
	if (!status.supported) note = 'unsupported'
	else if (!status.eligible) note = 'ineligible'
	else if (!busy && !installed) note = 'not-set-up'
	else if (!busy && !status.ready) note = 'needs-setup'
	return {
		note,
		canToggle: !busy && status.eligible,
		canSetUp: !busy && status.supported && status.eligible,
		canRemove: !busy && installed,
		setUpIsUpdate: installed && status.ready,
	}
}

export function isHorizonCandidate(instance: { loader: string; game_version: string }): boolean {
	return instance.loader === 'vanilla' && instance.game_version === HORIZON_GAME_VERSION
}

/** Horizon's name as a loader, which Create instance and the library show and filter by. */
export const HORIZON_LOADER = 'horizon'

/**
 * The loader an instance plays with. A vanilla 26.3 instance that has Horizon turned on
 * starts Horizon instead of Minecraft, so it's a Horizon instance; Minecraft's files and
 * Modrinth still see vanilla 26.3, which is what its content has to fit.
 */
export function playedLoader(
	instance: { id: string; loader: string; game_version: string },
	horizonInstances: Readonly<Record<string, boolean>> | undefined,
): string {
	return isHorizonCandidate(instance) && horizonInstances?.[instance.id] === true
		? HORIZON_LOADER
		: instance.loader
}

export async function plus_horizon_status(instanceId: string): Promise<HorizonStatus> {
	return await invoke('plugin:plus|plus_horizon_status', { instanceId })
}

/** Downloads or updates Horizon and prepares it. Resolves when it's done. */
export async function plus_horizon_set_up(instanceId: string): Promise<HorizonStatus> {
	return await invoke('plugin:plus|plus_horizon_set_up', { instanceId })
}

/** Works before Horizon is set up: Play then asks for setup first. */
export async function plus_horizon_set_enabled(
	instanceId: string,
	enabled: boolean,
): Promise<void> {
	return await invoke('plugin:plus|plus_horizon_set_enabled', { instanceId, enabled })
}

export async function plus_horizon_remove(): Promise<void> {
	return await invoke('plugin:plus|plus_horizon_remove')
}
