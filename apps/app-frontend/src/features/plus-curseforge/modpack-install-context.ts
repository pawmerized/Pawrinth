import { createContext } from '@modrinth/ui'
import type { Ref } from 'vue'

import type ModpackInstall from './modpack-install.vue'

/** The one CurseForge modpack installer, mounted at the app's root so it outlives pages and modals. */
export const [injectModpackInstall, provideModpackInstall] = createContext<
	Ref<InstanceType<typeof ModpackInstall> | null>
>('root', 'curseforgeModpackInstall')
