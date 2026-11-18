import { type AbstractWebNotificationManager, defineMessages, useVIntl } from '@modrinth/ui'

import { useAppEvent } from '@/composables/use-app-event'
import {
	type LibraryReport,
	type LibrarySkipReason,
	plus_mod_library_install,
} from '@/helpers/plus'

const messages = defineMessages({
	installedTitle: {
		id: 'plus.mod-library.installed.title',
		defaultMessage: 'Added {count, plural, one {# mod} other {# mods}} from your mod library',
	},
	skippedTitle: {
		id: 'plus.mod-library.skipped.title',
		defaultMessage: 'Some mods from your mod library weren’t added',
	},
	already_installed: {
		id: 'plus.mod-library.skip.already-installed',
		defaultMessage: '{name}: already in this instance',
	},
	no_mod_loader: {
		id: 'plus.mod-library.skip.no-mod-loader',
		defaultMessage: '{name}: this instance has no mod loader',
	},
	no_matching_version: {
		id: 'plus.mod-library.skip.no-matching-version',
		defaultMessage: '{name}: no version for this Minecraft version and loader',
	},
	curseforge_unavailable: {
		id: 'plus.mod-library.skip.curseforge-unavailable',
		defaultMessage: '{name}: CurseForge isn’t set up in API configuration',
	},
	failed: { id: 'plus.mod-library.skip.failed', defaultMessage: '{name}: {detail}' },
})

const SKIPPED_QUIETLY = new Set<LibrarySkipReason>(['already_installed'])

const held = new Set<string>()
const waiting = new Set<string>()
let installLibrary: ((instanceId: string) => Promise<void>) | null = null

/**
 * Keeps the mod library out of an instance while it's created with a mod of its own, so the
 * two installs can't add the same mod at once. Call the returned function when that install
 * is done, whether it worked or not.
 */
export function holdModLibrary(instanceId: string): () => void {
	// shortcut: a creation job that finishes before its id gets here isn't held; creating checks or downloads the whole game first, so it doesn't.
	held.add(instanceId)
	return () => {
		held.delete(instanceId)
		if (waiting.delete(instanceId)) void installLibrary?.(instanceId)
	}
}

/**
 * Puts the mod library into each instance created from Create instance. App.vue provides
 * the notifications itself, so it passes them in.
 */
export function useModLibraryAutoInstall(
	events: Parameters<typeof useAppEvent>[2],
	addNotification: AbstractWebNotificationManager['addNotification'],
): void {
	const { formatMessage } = useVIntl()
	const handled = new Set<string>()

	function report(result: LibraryReport): void {
		const skipped = result.skipped.filter((item) => !SKIPPED_QUIETLY.has(item.reason))
		const reasons = skipped
			.map((item) =>
				formatMessage(messages[item.reason], { name: item.name, detail: item.detail ?? '' }),
			)
			.join('\n')
		if (result.installed.length > 0) {
			addNotification({
				type: skipped.length > 0 ? 'warning' : 'success',
				title: formatMessage(messages.installedTitle, { count: result.installed.length }),
				text: [result.installed.join(', '), reasons].filter(Boolean).join('\n'),
			})
		} else if (skipped.length > 0) {
			addNotification({
				type: 'warning',
				title: formatMessage(messages.skippedTitle),
				text: reasons,
			})
		}
	}

	async function install(instanceId: string): Promise<void> {
		try {
			report(await plus_mod_library_install(instanceId))
		} catch (error) {
			addNotification({
				type: 'error',
				title: formatMessage(messages.skippedTitle),
				text: String(error),
			})
		}
	}
	installLibrary = install

	useAppEvent(
		'install_job',
		async (job) => {
			if (job.kind !== 'create_instance' || job.status !== 'succeeded') return
			const instanceId = job.instance_id ?? job.target.instance_id
			if (!instanceId || handled.has(job.job_id)) return
			handled.add(job.job_id)
			if (held.has(instanceId)) waiting.add(instanceId)
			else await install(instanceId)
		},
		events,
	)
}
