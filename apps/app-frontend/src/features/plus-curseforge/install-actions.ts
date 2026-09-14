import { defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import type { Ref } from 'vue'

import { readInstanceTarget } from '@/features/plus-install/instance-target'
import { get } from '@/helpers/instance'
import {
	type CfCategory,
	type CfProject,
	plus_cf_apply_updates,
	plus_cf_files,
	plus_cf_install,
	plus_cf_install_options,
	plus_cf_installed,
	plus_cf_installed_in,
	plus_cf_installed_projects,
	plus_cf_plan_install,
} from '@/helpers/plus'
import { type ExternalInstallInstance, injectContentInstall } from '@/providers/content-install'

import { classSlugOf, errorText, installTargets, projectPath } from './project-adapter'
import { classInfo } from './provider-mapping'

const messages = defineMessages({
	installedTitle: { id: 'plus.curseforge.direct.installed', defaultMessage: 'Installed {name}' },
	installedText: {
		id: 'plus.curseforge.direct.installed.text',
		defaultMessage: 'Added to {instance}.',
	},
	installedWithDependencies: {
		id: 'plus.curseforge.direct.installed.dependencies',
		defaultMessage:
			'Added to {instance}, with {count, plural, one {# required project} other {# required projects}}.',
	},
	missingDependencies: {
		id: 'plus.curseforge.direct.missing-dependencies',
		defaultMessage:
			"It also needs {names}, which can't be downloaded through Pawrinth. Get {count, plural, one {it} other {them}} from CurseForge's website.",
	},
	conflicts: {
		id: 'plus.curseforge.direct.conflicts',
		defaultMessage:
			"This version says it doesn't work with {count, plural, one {# project} other {# projects}} already in the instance.",
	},
	switchedTitle: { id: 'plus.curseforge.direct.switched', defaultMessage: 'Switched {name}' },
	switchedText: {
		id: 'plus.curseforge.direct.switched.text',
		defaultMessage: 'It now uses {version}.',
	},
	failedTitle: { id: 'plus.curseforge.direct.failed', defaultMessage: "Couldn't install {name}" },
	noVersion: {
		id: 'plus.curseforge.direct.no-version',
		defaultMessage: 'CurseForge has no version of it for {target}.',
	},
	incompatible: {
		id: 'plus.curseforge.direct.incompatible',
		defaultMessage: "That version doesn't work with {instance}: {reason}.",
	},
	notDistributable: {
		id: 'plus.curseforge.direct.not-distributable',
		defaultMessage: "Its author only allows downloads from CurseForge's website.",
	},
	unknownVersion: {
		id: 'plus.curseforge.direct.unknown-version',
		defaultMessage:
			"Pawrinth couldn't tell which Minecraft version {instance} uses. Check it in the instance's settings.",
	},
	unknownLoader: {
		id: 'plus.curseforge.direct.unknown-loader',
		defaultMessage:
			"Pawrinth couldn't tell which mod loader {instance} uses. Check it in the instance's settings.",
	},
	noLoader: {
		id: 'plus.curseforge.direct.no-loader',
		defaultMessage: "{instance} has no mod loader, so mods can't run in it.",
	},
})

const LOADER_NAMES: Record<string, string> = {
	fabric: 'Fabric',
	forge: 'Forge',
	neoforge: 'NeoForge',
	quilt: 'Quilt',
}

/**
 * Installs CurseForge content the way Modrinth content is installed: straight into the
 * current instance with the newest version that fits, or through the instance picker
 * when there's no instance yet.
 */
export function useCurseforgeInstall(classes: Ref<readonly CfCategory[]>) {
	const contentInstall = injectContentInstall()
	const { addNotification, handleError } = injectNotificationManager()
	const { formatMessage } = useVIntl()

	async function installToInstance(
		project: CfProject,
		instance: ExternalInstallInstance,
		fileId: number | null = null,
	): Promise<boolean> {
		const info = classInfo(classSlugOf(project, classes.value))
		const isMod = info.projectType === 'mod'
		const target = readInstanceTarget(instance)
		const fail = (text: string) => {
			addNotification({
				type: 'error',
				title: formatMessage(messages.failedTitle, { name: project.name }),
				text,
			})
			return false
		}
		if (target.status === 'unknown-version') {
			return fail(formatMessage(messages.unknownVersion, { instance: instance.name }))
		}
		if (isMod && target.status === 'unknown-loader') {
			return fail(formatMessage(messages.unknownLoader, { instance: instance.name }))
		}
		if (isMod && target.status === 'no-loader') {
			return fail(formatMessage(messages.noLoader, { instance: instance.name }))
		}

		let projectIds: string[] = []
		try {
			let chosen = fileId
			if (chosen === null) {
				const options = await plus_cf_install_options(instance.id, project.id, false, null)
				chosen = options.recommended_file_id
				if (chosen === null) {
					const targetName =
						isMod && target.loader
							? `${LOADER_NAMES[target.loader] ?? target.loader} ${target.gameVersion}`
							: `Minecraft ${target.gameVersion}`
					return fail(formatMessage(messages.noVersion, { target: targetName }))
				}
			}
			const plan = await plus_cf_plan_install(instance.id, project.id, chosen, null)
			if (!plan.can_download) return fail(formatMessage(messages.notDistributable))
			if (plan.compatibility.status === 'incompatible') {
				return fail(
					formatMessage(messages.incompatible, {
						instance: instance.name,
						reason: plan.compatibility.reason,
					}),
				)
			}

			if ((await plus_cf_installed_projects(instance.id)).includes(project.id)) {
				const current = (await plus_cf_installed(instance.id)).find(
					(file) => file.project_id === project.id,
				)
				if (current && current.file_id !== chosen) {
					await plus_cf_apply_updates(instance.id, [
						{ relative_path: current.relative_path, project_id: project.id, file_id: chosen },
					])
					contentInstall.endExternalInstall(instance.id, [], true)
					addNotification({
						type: 'success',
						title: formatMessage(messages.switchedTitle, { name: project.name }),
						text: formatMessage(messages.switchedText, { version: plan.file.displayName }),
					})
				}
				return true
			}

			const dependencies = plan.required.filter((item) => item.status === 'will_install')
			const unavailable = plan.required.filter((item) => item.status === 'unavailable')
			projectIds = [project.id, ...dependencies.map((item) => item.project_id)].map(
				(id) => `curseforge:${id}`,
			)
			contentInstall.beginExternalInstall(
				instance.id,
				{
					id: projectIds[0],
					title: project.name,
					icon_url: project.logo?.thumbnailUrl ?? null,
					project_type: info.projectType,
				},
				plan.file.displayName,
			)
			for (const dependency of dependencies) {
				contentInstall.beginExternalInstall(
					instance.id,
					{
						id: `curseforge:${dependency.project_id}`,
						title: dependency.name,
						project_type: 'mod',
					},
					dependency.file?.displayName,
				)
			}

			await plus_cf_install(instance.id, project.id, chosen, false, true, null)
			contentInstall.endExternalInstall(instance.id, projectIds, true)

			const text = [
				dependencies.length > 0
					? formatMessage(messages.installedWithDependencies, {
							instance: instance.name,
							count: dependencies.length,
						})
					: formatMessage(messages.installedText, { instance: instance.name }),
				unavailable.length > 0
					? formatMessage(messages.missingDependencies, {
							names: unavailable.map((item) => item.name).join(', '),
							count: unavailable.length,
						})
					: '',
				plan.conflicts.length > 0
					? formatMessage(messages.conflicts, { count: plan.conflicts.length })
					: '',
			]
				.filter(Boolean)
				.join(' ')
			addNotification({
				type: unavailable.length > 0 || plan.conflicts.length > 0 ? 'warning' : 'success',
				title: formatMessage(messages.installedTitle, { name: project.name }),
				text,
			})
			return true
		} catch (error) {
			if (projectIds.length > 0) contentInstall.endExternalInstall(instance.id, projectIds, false)
			return fail(errorText(error))
		}
	}

	/**
	 * Installs into `instanceId` without asking anything. Without an instance, Modrinth's
	 * instance picker opens, where an instance can be chosen or created.
	 */
	async function install(
		project: CfProject,
		options: { instanceId?: string | null; fileId?: number | null } = {},
	): Promise<boolean> {
		const fileId = options.fileId ?? null
		try {
			if (options.instanceId) {
				const instance = await get(options.instanceId)
				if (!instance) return false
				return await installToInstance(project, instance, fileId)
			}
			const info = classInfo(classSlugOf(project, classes.value))
			const [files, installedIn] = await Promise.all([
				plus_cf_files(project.id, null, null),
				plus_cf_installed_in(project.id).catch(() => []),
			])
			await contentInstall.showExternalInstallModal({
				projectId: `curseforge:${project.id}`,
				projectType: info.projectType,
				projectInfo: {
					title: project.name,
					iconUrl: project.logo?.thumbnailUrl ?? null,
					link: projectPath(project),
				},
				targets: installTargets(
					files.filter((file) => fileId === null || file.id === fileId),
					info.projectType,
				),
				installedInstanceIds: installedIn,
				installToInstance: (instance) => installToInstance(project, instance, fileId),
			})
			return false
		} catch (error) {
			handleError(error as Error)
			return false
		}
	}

	return { install, installToInstance }
}
