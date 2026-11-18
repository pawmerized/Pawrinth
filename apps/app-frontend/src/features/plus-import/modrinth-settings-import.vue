<script setup lang="ts">
import { ArrowBigRightDashIcon, CheckIcon, SpinnerIcon } from '@modrinth/assets'
import { Button, Checkbox, defineMessages, injectNotificationManager, useVIntl } from '@modrinth/ui'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { computed, onMounted, ref } from 'vue'

import { useAppSettings } from '@/composables/use-app-settings'
import { DARK_THEMES, type DarkTheme, useTheme } from '@/composables/use-theme'
import {
	plus_modrinth_settings_import,
	plus_modrinth_settings_preview,
	type SettingsImportPreview,
} from '@/helpers/plus'
import { get as getSettings } from '@/helpers/settings.ts'
import { setLocale } from '@/i18n.config'

const props = withDefaults(defineProps<{ compact?: boolean }>(), { compact: false })

const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const appTheme = useTheme()
const appSettings = useAppSettings()

const messages = defineMessages({
	title: { id: 'plus.settings-import.title', defaultMessage: 'Settings from the Modrinth App' },
	description: {
		id: 'plus.settings-import.description',
		defaultMessage:
			'Pawrinth found the Modrinth App on this computer. Choose which of its settings to use here. Nothing changes until you import, and the Modrinth App keeps its own settings.',
	},
	notFound: {
		id: 'plus.settings-import.not-found',
		defaultMessage: 'The Modrinth App isn’t on this computer, so there’s nothing to import.',
	},
	allMatch: {
		id: 'plus.settings-import.all-match',
		defaultMessage: 'Your settings already match the Modrinth App’s.',
	},
	notImported: {
		id: 'plus.settings-import.not-imported',
		defaultMessage:
			'Its data folder, telemetry and ad choices aren’t imported. Pawrinth keeps its own.',
	},
	importChosen: {
		id: 'plus.settings-import.import',
		defaultMessage: 'Import {count, plural, one {# setting} other {# settings}}',
	},
	imported: {
		id: 'plus.settings-import.imported',
		defaultMessage: 'Imported {count, plural, one {# setting} other {# settings}}',
	},
	empty: { id: 'plus.settings-import.empty', defaultMessage: 'None' },
	on: { id: 'plus.settings-import.on', defaultMessage: 'On' },
	off: { id: 'plus.settings-import.off', defaultMessage: 'Off' },
	theme: { id: 'plus.settings-import.key.theme', defaultMessage: 'Theme' },
	locale: { id: 'plus.settings-import.key.locale', defaultMessage: 'Language' },
	default_page: { id: 'plus.settings-import.key.default-page', defaultMessage: 'Start page' },
	collapsed_navigation: {
		id: 'plus.settings-import.key.collapsed-navigation',
		defaultMessage: 'Collapsed navigation',
	},
	toggle_sidebar: {
		id: 'plus.settings-import.key.toggle-sidebar',
		defaultMessage: 'Hide the right sidebar',
	},
	advanced_rendering: {
		id: 'plus.settings-import.key.advanced-rendering',
		defaultMessage: 'Advanced rendering',
	},
	native_decorations: {
		id: 'plus.settings-import.key.native-decorations',
		defaultMessage: 'Native window frame',
	},
	discord_rpc: { id: 'plus.settings-import.key.discord-rpc', defaultMessage: 'Discord status' },
	hide_on_process_start: {
		id: 'plus.settings-import.key.hide-on-start',
		defaultMessage: 'Hide the launcher while playing',
	},
	max_concurrent_downloads: {
		id: 'plus.settings-import.key.downloads',
		defaultMessage: 'Downloads at once',
	},
	max_concurrent_writes: {
		id: 'plus.settings-import.key.writes',
		defaultMessage: 'Files written at once',
	},
	memory: { id: 'plus.settings-import.key.memory', defaultMessage: 'Default memory' },
	force_fullscreen: {
		id: 'plus.settings-import.key.fullscreen',
		defaultMessage: 'Start the game in fullscreen',
	},
	game_resolution: {
		id: 'plus.settings-import.key.resolution',
		defaultMessage: 'Game window size',
	},
	extra_launch_args: {
		id: 'plus.settings-import.key.java-args',
		defaultMessage: 'Default Java arguments',
	},
	custom_env_vars: {
		id: 'plus.settings-import.key.env',
		defaultMessage: 'Default environment variables',
	},
	hook_pre_launch: {
		id: 'plus.settings-import.key.pre-launch',
		defaultMessage: 'Command before launch',
	},
	hook_wrapper: { id: 'plus.settings-import.key.wrapper', defaultMessage: 'Wrapper command' },
	hook_post_exit: {
		id: 'plus.settings-import.key.post-exit',
		defaultMessage: 'Command after the game closes',
	},
})

const preview = ref<SettingsImportPreview | null>(null)
const chosen = ref(new Set<string>())
const importing = ref(false)
const loaded = ref(false)

async function load(): Promise<void> {
	try {
		preview.value = await plus_modrinth_settings_preview()
		chosen.value = new Set(preview.value.changes.map((change) => change.key))
	} catch (error) {
		handleError(error as Error)
	} finally {
		loaded.value = true
	}
}

onMounted(load)

function label(key: string): string {
	const message = messages[key as keyof typeof messages]
	return message ? formatMessage(message) : key
}

function show(key: string, value: unknown): string {
	if (typeof value === 'boolean') return formatMessage(value ? messages.on : messages.off)
	if (key === 'memory' && typeof value === 'number') return `${value} MB`
	if (key === 'game_resolution' && Array.isArray(value)) return value.join(' × ')
	if (key === 'custom_env_vars' && Array.isArray(value)) {
		return value.length
			? value.map((pair) => (Array.isArray(pair) ? pair.join('=') : String(pair))).join(' ')
			: formatMessage(messages.empty)
	}
	if (Array.isArray(value)) return value.length ? value.join(' ') : formatMessage(messages.empty)
	if (value === '' || value === null || value === undefined) return formatMessage(messages.empty)
	return String(value)
}

function toggle(key: string, value: boolean): void {
	const next = new Set(chosen.value)
	if (value) next.add(key)
	else next.delete(key)
	chosen.value = next
}

/** Settings the window already shows are applied now, the rest when they're next used. */
async function applyLive(keys: string[]): Promise<void> {
	const settings = await getSettings()
	if (keys.includes('theme')) {
		appTheme.preferred = settings.theme
		if (DARK_THEMES.includes(settings.theme as DarkTheme)) {
			appTheme.preferredDark = settings.theme as DarkTheme
		}
	}
	if (keys.includes('advanced_rendering')) appTheme.advancedRendering = settings.advanced_rendering
	if (keys.includes('toggle_sidebar')) appSettings.toggleSidebar = settings.toggle_sidebar
	if (keys.includes('locale') && settings.locale) await setLocale(settings.locale)
	if (keys.includes('native_decorations')) {
		appSettings.nativeDecorations = settings.native_decorations
		await getCurrentWindow()
			.setDecorations(settings.native_decorations)
			.catch(() => {})
	}
}

async function importChosen(): Promise<void> {
	const keys = [...chosen.value]
	if (keys.length === 0) return
	importing.value = true
	try {
		const count = await plus_modrinth_settings_import(keys)
		await applyLive(keys)
		addNotification({ type: 'success', title: formatMessage(messages.imported, { count }) })
		await load()
	} catch (error) {
		handleError(error as Error)
	} finally {
		importing.value = false
	}
}

const visible = computed(() => !props.compact || !!preview.value?.found)
</script>

<template>
	<section v-if="loaded && visible" class="flex flex-col gap-3" data-settings-import>
		<h3 class="m-0 text-lg font-semibold text-contrast">{{ formatMessage(messages.title) }}</h3>
		<p v-if="!preview?.found" class="m-0 text-secondary">{{ formatMessage(messages.notFound) }}</p>
		<template v-else>
			<p class="m-0 text-secondary">{{ formatMessage(messages.description) }}</p>
			<p v-if="preview.changes.length === 0" class="m-0 flex items-center gap-2 text-primary">
				<CheckIcon class="text-brand" aria-hidden="true" />
				{{ formatMessage(messages.allMatch) }}
			</p>
			<ul v-else class="m-0 flex list-none flex-col gap-1 p-0">
				<li
					v-for="change in preview.changes"
					:key="change.key"
					class="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-xl bg-surface-2 px-3 py-2"
				>
					<Checkbox
						:model-value="chosen.has(change.key)"
						:label="label(change.key)"
						class="min-w-[12rem] flex-1"
						@update:model-value="(value: boolean) => toggle(change.key, value)"
					/>
					<span class="flex min-w-0 items-center gap-2 text-sm">
						<span class="break-all text-secondary">{{ show(change.key, change.current) }}</span>
						<ArrowBigRightDashIcon class="size-4 shrink-0 text-secondary" aria-hidden="true" />
						<span class="break-all font-semibold text-contrast">
							{{ show(change.key, change.incoming) }}
						</span>
					</span>
				</li>
			</ul>
			<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.notImported) }}</p>
			<div v-if="preview.changes.length > 0">
				<Button
					color="brand"
					type="colored"
					:disabled="importing || chosen.size === 0"
					@click="importChosen"
				>
					<SpinnerIcon v-if="importing" class="animate-spin" aria-hidden="true" />
					{{ formatMessage(messages.importChosen, { count: chosen.size }) }}
				</Button>
			</div>
		</template>
	</section>
</template>
