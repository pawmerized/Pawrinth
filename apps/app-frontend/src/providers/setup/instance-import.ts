import { type AbstractWebNotificationManager, provideInstanceImport } from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'

import {
	get_default_launcher_path,
	get_importable_instances,
	import_instance,
} from '@/helpers/import.js'
import { type FtbAppInstance, plus_ftb_app_instances } from '@/helpers/plus'

const FTB_APP = 'FTBApp'
const FTB_APP_LABEL = 'FTB App'
const LOADER_NAMES: Record<string, string> = {
	forge: 'Forge',
	neoforge: 'NeoForge',
	fabric: 'Fabric',
	quilt: 'Quilt',
}

/** FTB App instance folders are named by id, so the list shows each pack's own name. */
const ftbFolders = new Map<string, Map<string, string>>()

function ftbLabel(instance: FtbAppInstance): string {
	const loader = LOADER_NAMES[instance.loader]
	const details = [instance.game_version, loader].filter(Boolean).join(' ')
	return details ? `${instance.name} (${details})` : instance.name
}

async function ftbInstances(path: string): Promise<string[]> {
	const labels = new Map<string, string>()
	for (const instance of await plus_ftb_app_instances(path)) {
		const base = ftbLabel(instance)
		let label = base
		for (let count = 2; labels.has(label); count++) label = `${base} ${count}`
		labels.set(label, instance.folder)
	}
	if (labels.size > 0) ftbFolders.set(path, labels)
	return [...labels.keys()]
}

export function setupInstanceImportProvider(notificationManager: AbstractWebNotificationManager) {
	const { handleError } = notificationManager

	provideInstanceImport({
		async getDetectedLaunchers() {
			const launcherNames = ['MultiMC', 'GDLauncher', 'ATLauncher', 'Curseforge', 'PrismLauncher']
			const launchers = []
			for (const name of launcherNames) {
				try {
					const path = await get_default_launcher_path(name)
					if (!path) continue
					const instances = await get_importable_instances(name, path)
					if (instances?.length > 0) {
						launchers.push({ name, path, instances })
					}
				} catch {
					// Skip launchers that fail detection
				}
			}
			try {
				const path = await get_default_launcher_path(FTB_APP)
				const instances = path ? await ftbInstances(path) : []
				if (path && instances.length > 0) {
					launchers.push({ name: FTB_APP_LABEL, path, instances })
				}
			} catch {
				// Skip the FTB App when its folder can't be read
			}
			return launchers
		},
		async getImportableInstances(launcherName: string, path: string) {
			if (launcherName === FTB_APP_LABEL) return await ftbInstances(path)
			const ftb = await ftbInstances(path).catch(() => [])
			if (ftb.length > 0) return ftb
			return (await get_importable_instances(launcherName, path)) ?? []
		},
		async importInstances(selections) {
			for (const sel of selections) {
				const ftb = ftbFolders.get(sel.path)
				for (const instanceName of sel.instanceNames) {
					if (ftb) {
						await import_instance(FTB_APP, sel.path, ftb.get(instanceName) ?? instanceName).catch(
							handleError,
						)
						continue
					}
					await import_instance(sel.launcher, sel.path, instanceName).catch(handleError)
				}
			}
		},
		async selectDirectory() {
			const result = await open({ multiple: false, directory: true })
			return result?.toString() ?? null
		},
	})
}
