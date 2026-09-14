<script setup lang="ts">
import { ExternalIcon, loaderIconMap, PlugIcon, UndoIcon } from '@modrinth/assets'
import {
	Admonition,
	Button,
	Chips,
	defineMessages,
	injectNotificationManager,
	Input,
	Toggle,
	useVIntl,
} from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { useDebounceFn, useIntervalFn } from '@vueuse/core'
import { computed, onMounted, ref, watch } from 'vue'

import pawrinthLogo from '@/assets/welcome/modrinth-social-icon.png'
import {
	type DiscordCheck,
	type DiscordPreview,
	type DiscordStatus,
	plus_discord_check,
	plus_discord_preview,
	plus_discord_refresh,
	plus_discord_status,
} from '@/helpers/plus'
import { get, set } from '@/helpers/settings.ts'

import { patchPlusSettings, usePlusSettings } from './use-plus-settings'

const PORTAL_URL = 'https://discord.com/developers/applications'
const APPLICATION_ID = /^\d{17,20}$/
const SAVE_DELAY_MS = 500
const STATUS_REFRESH_MS = 3000
const MODES = ['default', 'custom'] as const
type Mode = (typeof MODES)[number]

const CLEARED_SETTINGS = {
	use_custom: false,
	application_id: null,
	show_instance: true,
	show_version: true,
	show_elapsed: true,
	show_loader_badge: false,
	idle_text: null,
	idle_state_text: null,
	playing_text: null,
	playing_state_text: null,
	logo_image: null,
	logo_text: null,
	button_label: null,
	button_url: null,
	second_button_label: null,
	second_button_url: null,
}

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const { settings: plusSettings } = usePlusSettings()

const messages = defineMessages({
	title: { id: 'plus.discord.title', defaultMessage: 'Discord Rich Presence' },
	description: {
		id: 'plus.discord.description',
		defaultMessage:
			"Choose what your Discord profile shows while Pawrinth is open. This doesn't affect Rich Presence added to instances by mods.",
	},
	enable: { id: 'plus.discord.enable', defaultMessage: 'Show Pawrinth on Discord' },
	enableText: {
		id: 'plus.discord.enable.text',
		defaultMessage:
			'Turn this off to keep Pawrinth off your profile, or to use another presence app.',
	},
	connected: {
		id: 'plus.discord.connected',
		defaultMessage: 'Connected. Discord is showing this now.',
	},
	notRunning: {
		id: 'plus.discord.not-running',
		defaultMessage: "Discord isn't open. Pawrinth connects by itself as soon as it starts.",
	},
	blocked: {
		id: 'plus.discord.blocked',
		defaultMessage:
			'Windows is blocking Pawrinth from reaching Discord, which usually means Discord is running as administrator. Close Discord completely and open it normally.',
	},
	applicationRefused: {
		id: 'plus.discord.application-refused',
		defaultMessage: 'Discord refused this Application ID. Check it, or go back to the default.',
	},
	activityRefused: {
		id: 'plus.discord.activity-refused',
		defaultMessage: 'Discord refused this status: {reason}',
	},
	noReply: {
		id: 'plus.discord.no-reply',
		defaultMessage: "Discord didn't answer. Pawrinth will keep trying.",
	},
	off: { id: 'plus.discord.off', defaultMessage: "Off. Discord isn't showing Pawrinth." },
	testConnection: { id: 'plus.discord.test', defaultMessage: 'Test connection' },
	testApplication: {
		id: 'plus.discord.test.application',
		defaultMessage: 'Discord knows this application as {name}.',
	},
	testApplicationUnknown: {
		id: 'plus.discord.test.application-unknown',
		defaultMessage: "Discord doesn't know this Application ID.",
	},
	testLogoFound: {
		id: 'plus.discord.test.logo-found',
		defaultMessage: 'The picture named {name} is uploaded.',
	},
	testLogoMissing: {
		id: 'plus.discord.test.logo-missing',
		defaultMessage:
			"There's no picture named {name} on this application, so the status shows without a logo.",
	},
	testOffline: {
		id: 'plus.discord.test.offline',
		defaultMessage: "Couldn't ask Discord's website about the application and its pictures.",
	},
	mode: { id: 'plus.discord.mode', defaultMessage: 'Presence style' },
	modeDefault: { id: 'plus.discord.mode.default', defaultMessage: 'Default' },
	modeCustom: { id: 'plus.discord.mode.custom', defaultMessage: 'Custom' },
	modeDefaultText: {
		id: 'plus.discord.mode.default.text',
		defaultMessage:
			"Uses Pawrinth's name and logo, with buttons to install Pawrinth and join its Discord server. There's nothing to set up.",
	},
	modeCustomText: {
		id: 'plus.discord.mode.custom.text',
		defaultMessage:
			'Choose your own text, picture and buttons, and optionally your own Discord application.',
	},
	preview: { id: 'plus.discord.preview', defaultMessage: 'Preview' },
	previewIdle: { id: 'plus.discord.preview.idle', defaultMessage: 'While browsing' },
	previewButtonsNote: {
		id: 'plus.discord.preview.buttons-note',
		defaultMessage:
			"Other people see the buttons when they look at your profile. Discord doesn't show your own status's buttons to you.",
	},
	previewPlaying: { id: 'plus.discord.preview.playing', defaultMessage: 'While playing' },
	elapsed: { id: 'plus.discord.preview.elapsed', defaultMessage: '12:34 elapsed' },
	showInstance: { id: 'plus.discord.show-instance', defaultMessage: 'Show the instance name' },
	showVersion: { id: 'plus.discord.show-version', defaultMessage: 'Show the Minecraft version' },
	showHiddenText: {
		id: 'plus.discord.show-hidden.text',
		defaultMessage: 'With both off, or in streamer mode, Discord only says "Playing Minecraft".',
	},
	showElapsed: { id: 'plus.discord.show-elapsed', defaultMessage: 'Show time played' },
	idleText: { id: 'plus.discord.idle-text', defaultMessage: 'Text while browsing' },
	playingText: { id: 'plus.discord.playing-text', defaultMessage: 'Text while playing' },
	playingTextHelp: {
		id: 'plus.discord.playing-text.help',
		defaultMessage: 'You can use {tokens}. Leave a box empty to use the default.',
	},
	secondLine: { id: 'plus.discord.second-line', defaultMessage: 'Second line (optional)' },
	logoImage: { id: 'plus.discord.logo-image', defaultMessage: 'Logo picture' },
	logoImageText: {
		id: 'plus.discord.logo-image.text',
		defaultMessage:
			"The name of a picture uploaded to the Discord application, or a link starting with https://. Leave it empty to use Pawrinth's logo.",
	},
	logoImagePlaceholder: {
		id: 'plus.discord.logo-image.placeholder',
		defaultMessage: 'Picture name or link',
	},
	logoText: { id: 'plus.discord.logo-text', defaultMessage: 'Text when hovering over the logo' },
	showBadge: {
		id: 'plus.discord.show-badge',
		defaultMessage: 'Show the mod loader as a small picture',
	},
	showBadgeText: {
		id: 'plus.discord.show-badge.text',
		defaultMessage:
			"Adds the loader to the corner of the logo while playing. Upload its pictures to your Discord application first, or it won't show.",
	},
	buttonTitle: { id: 'plus.discord.button', defaultMessage: 'Profile buttons' },
	buttonText: {
		id: 'plus.discord.button.text',
		defaultMessage:
			"Adds up to two links other people can click on your profile. Discord doesn't show them to you. The address must start with https://.",
	},
	buttonLabel: { id: 'plus.discord.button.label', defaultMessage: 'Button text' },
	buttonUrl: { id: 'plus.discord.button.url', defaultMessage: 'https://example.com' },
	buttonUrlInvalid: {
		id: 'plus.discord.button.url.invalid',
		defaultMessage: "That address won't be used, because it doesn't start with https://.",
	},
	applicationTitle: {
		id: 'plus.discord.application',
		defaultMessage: 'Your own Discord application (optional)',
	},
	applicationText: {
		id: 'plus.discord.application.text',
		defaultMessage:
			"Only needed to show a different name than Pawrinth. It's the Application ID alone, never a bot token or secret. Leave it empty to use Pawrinth's.",
	},
	stepsTitle: { id: 'plus.discord.steps', defaultMessage: 'Setting one up' },
	stepOne: {
		id: 'plus.discord.steps.one',
		defaultMessage:
			'Create an application in the Discord Developer Portal. Its name is what Discord shows.',
	},
	stepTwo: {
		id: 'plus.discord.steps.two',
		defaultMessage: 'Copy its Application ID from General Information, and paste it here.',
	},
	stepThree: {
		id: 'plus.discord.steps.three',
		defaultMessage: 'Under Rich Presence, then Art Assets, upload your logo and name it pawrinth.',
	},
	stepFour: {
		id: 'plus.discord.steps.four',
		defaultMessage:
			'For the small mod loader picture, also upload pictures named fabric, forge, neoforge and quilt.',
	},
	openPortal: { id: 'plus.discord.open-portal', defaultMessage: 'Open Discord Developer Portal' },
	applicationPlaceholder: {
		id: 'plus.discord.application.placeholder',
		defaultMessage: 'Application ID',
	},
	applicationInvalid: {
		id: 'plus.discord.application.invalid',
		defaultMessage: "An Application ID is 17 to 20 digits, so this one isn't used.",
	},
	restore: { id: 'plus.discord.restore', defaultMessage: 'Restore defaults' },
})

const status = ref<DiscordStatus | null>(null)
const preview = ref<DiscordPreview | null>(null)
const check = ref<DiscordCheck | null>(null)
const checking = ref(false)
const enabled = ref(true)
const discord = computed(() => plusSettings.value?.discord)
const mode = computed<Mode>(() => (discord.value?.use_custom ? 'custom' : 'default'))

const idleText = ref('')
const idleStateText = ref('')
const playingText = ref('')
const playingStateText = ref('')
const logoImage = ref('')
const logoText = ref('')
const buttonLabel = ref('')
const buttonUrl = ref('')
const secondButtonLabel = ref('')
const secondButtonUrl = ref('')
const applicationId = ref('')

function loadTexts(): void {
	const saved = discord.value
	idleText.value = saved?.idle_text ?? ''
	idleStateText.value = saved?.idle_state_text ?? ''
	playingText.value = saved?.playing_text ?? ''
	playingStateText.value = saved?.playing_state_text ?? ''
	logoImage.value = saved?.logo_image ?? ''
	logoText.value = saved?.logo_text ?? ''
	buttonLabel.value = saved?.button_label ?? ''
	buttonUrl.value = saved?.button_url ?? ''
	secondButtonLabel.value = saved?.second_button_label ?? ''
	secondButtonUrl.value = saved?.second_button_url ?? ''
	applicationId.value = saved?.application_id ?? ''
}
loadTexts()

const applicationInvalid = computed(() => {
	const id = applicationId.value.trim()
	return id.length > 0 && !APPLICATION_ID.test(id)
})
const anyButtonUrlInvalid = computed(() =>
	[buttonUrl.value, secondButtonUrl.value].some((url) => {
		const trimmed = url.trim()
		return trimmed.length > 0 && !trimmed.startsWith('https://')
	}),
)
const statusText = computed(() => {
	const current = status.value
	if (!current) return undefined
	if (!current.enabled) return formatMessage(messages.off)
	switch (current.problem?.kind) {
		case 'not_running':
			return formatMessage(messages.notRunning)
		case 'blocked':
			return formatMessage(messages.blocked)
		case 'application_refused':
			return formatMessage(messages.applicationRefused)
		case 'activity_refused':
			return formatMessage(messages.activityRefused, { reason: current.problem.message })
		case 'no_reply':
			return formatMessage(messages.noReply)
	}
	return current.connected ? formatMessage(messages.connected) : undefined
})
const statusIsProblem = computed(() => !!status.value?.enabled && !!status.value?.problem)
const checkLines = computed(() => {
	const result = check.value
	if (!result) return []
	const lines: string[] = []
	if (result.application_known === true && result.application_name) {
		lines.push(formatMessage(messages.testApplication, { name: result.application_name }))
	} else if (result.application_known === false) {
		lines.push(formatMessage(messages.testApplicationUnknown))
	}
	if (result.logo_uploaded === true) {
		lines.push(formatMessage(messages.testLogoFound, { name: result.logo_image }))
	} else if (result.logo_uploaded === false && result.application_known !== false) {
		lines.push(formatMessage(messages.testLogoMissing, { name: result.logo_image }))
	}
	if (result.application_known === null) lines.push(formatMessage(messages.testOffline))
	return lines
})

function pictureFor(image: string): string {
	return image.startsWith('https://') ? image : pawrinthLogo
}

async function refresh(): Promise<void> {
	const [nextStatus, nextPreview] = await Promise.all([
		plus_discord_status(),
		plus_discord_preview(),
	])
	status.value = nextStatus
	preview.value = nextPreview
}

useIntervalFn(() => {
	plus_discord_status()
		.then((nextStatus) => {
			status.value = nextStatus
		})
		.catch(() => {})
}, STATUS_REFRESH_MS)

async function save(change: Record<string, unknown>): Promise<void> {
	try {
		await patchPlusSettings({ discord: change })
		check.value = null
		await refresh()
	} catch (error) {
		handleError(error as Error)
	}
}

const saveTexts = useDebounceFn(() => {
	void save({
		idle_text: idleText.value.trim() || null,
		idle_state_text: idleStateText.value.trim() || null,
		playing_text: playingText.value.trim() || null,
		playing_state_text: playingStateText.value.trim() || null,
		logo_image: logoImage.value.trim() || null,
		logo_text: logoText.value.trim() || null,
		button_label: buttonLabel.value.trim() || null,
		button_url: buttonUrl.value.trim() || null,
		second_button_label: secondButtonLabel.value.trim() || null,
		second_button_url: secondButtonUrl.value.trim() || null,
	})
}, SAVE_DELAY_MS)

const saveApplication = useDebounceFn(() => {
	if (applicationInvalid.value) return
	void save({ application_id: applicationId.value.trim() || null })
}, SAVE_DELAY_MS)

watch(
	[
		idleText,
		idleStateText,
		playingText,
		playingStateText,
		logoImage,
		logoText,
		buttonLabel,
		buttonUrl,
		secondButtonLabel,
		secondButtonUrl,
	],
	() => void saveTexts(),
)
watch(applicationId, () => void saveApplication())

async function setEnabled(value: boolean): Promise<void> {
	enabled.value = value
	try {
		const current = await get()
		current.discord_rpc = value
		await set(current)
		check.value = null
		await plus_discord_refresh()
		await refresh()
	} catch (error) {
		handleError(error as Error)
	}
}

function setMode(next: Mode): void {
	void save({ use_custom: next === 'custom' })
}

async function testConnection(): Promise<void> {
	checking.value = true
	try {
		const result = await plus_discord_check()
		check.value = result
		status.value = result.status
	} catch (error) {
		handleError(error as Error)
	} finally {
		checking.value = false
	}
}

async function restoreDefaults(): Promise<void> {
	await save({ ...CLEARED_SETTINGS })
	loadTexts()
}

onMounted(async () => {
	try {
		enabled.value = (await get()).discord_rpc
		await refresh()
	} catch (error) {
		handleError(error as Error)
	}
})
</script>

<template>
	<section data-discord-settings>
		<h2 class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
		<p class="m-0 mt-1">{{ formatMessage(messages.description) }}</p>

		<div class="mt-4 flex flex-col gap-6">
			<div class="flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.enable) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.enableText) }}</p>
				</div>
				<Toggle
					id="plus-discord-enabled"
					:model-value="enabled"
					class="shrink-0"
					@update:model-value="setEnabled"
				/>
			</div>

			<div>
				<Admonition
					v-if="statusText && statusIsProblem"
					type="warning"
					:body="statusText"
					data-discord-status
					data-discord-problem
				/>
				<p v-else-if="statusText" class="m-0 text-secondary" data-discord-status>
					{{ statusText }}
				</p>
				<Button
					id="plus-discord-test"
					class="mt-2"
					native-type="button"
					:disabled="checking || !enabled"
					@click="testConnection"
				>
					<PlugIcon aria-hidden="true" />
					{{ formatMessage(messages.testConnection) }}
				</Button>
				<ul v-if="checkLines.length > 0" class="m-0 mt-2 pl-5 text-sm" data-discord-check>
					<li v-for="line in checkLines" :key="line">{{ line }}</li>
				</ul>
			</div>

			<div>
				<h3 class="m-0 text-lg font-semibold text-contrast">{{ formatMessage(messages.mode) }}</h3>
				<Chips
					:model-value="mode"
					class="mt-2"
					:items="[...MODES]"
					:format-label="
						(item: string) =>
							formatMessage(item === 'custom' ? messages.modeCustom : messages.modeDefault)
					"
					:never-empty="true"
					data-discord-mode
					@update:model-value="setMode"
				/>
				<p class="m-0 mt-2 text-secondary">
					{{
						formatMessage(mode === 'custom' ? messages.modeCustomText : messages.modeDefaultText)
					}}
				</p>
			</div>

			<div v-if="preview">
				<h3 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.preview) }}
				</h3>
				<div class="mt-2 grid grid-cols-1 gap-3 lg:grid-cols-2">
					<div
						v-for="kind in ['idle', 'playing'] as const"
						:key="kind"
						class="rounded-2xl border border-solid border-surface-5 bg-surface-2 p-3"
						:data-discord-preview="kind"
					>
						<p class="m-0 mb-2 text-xs font-semibold text-secondary">
							{{ formatMessage(kind === 'idle' ? messages.previewIdle : messages.previewPlaying) }}
						</p>
						<div class="flex min-w-0 gap-3">
							<div
								class="relative size-16 shrink-0"
								:title="preview[kind].logo_text"
								data-discord-logo
							>
								<img
									:src="pictureFor(preview[kind].logo_image)"
									alt=""
									class="size-16 rounded-xl"
								/>
								<div
									v-if="preview[kind].badge"
									class="absolute -bottom-1 -right-1 flex size-6 items-center justify-center rounded-full border-2 border-solid border-surface-2 bg-surface-1"
									:title="preview[kind].badge?.text"
									data-discord-badge
								>
									<component
										:is="loaderIconMap[preview[kind].badge?.image ?? '']"
										class="size-4 text-contrast"
										aria-hidden="true"
									/>
								</div>
							</div>
							<div class="min-w-0">
								<p class="m-0 truncate font-semibold text-contrast">Pawrinth</p>
								<p class="m-0 truncate text-sm text-primary" :title="preview[kind].details">
									{{ preview[kind].details }}
								</p>
								<p v-if="preview[kind].state" class="m-0 truncate text-sm text-primary">
									{{ preview[kind].state }}
								</p>
								<p v-if="preview[kind].started_at" class="m-0 truncate text-sm text-secondary">
									{{ formatMessage(messages.elapsed) }}
								</p>
							</div>
						</div>
						<div
							v-for="button in preview[kind].buttons"
							:key="button.url"
							class="mt-2 truncate rounded-lg bg-surface-4 px-3 py-1.5 text-center text-sm font-semibold text-contrast"
							data-discord-button
						>
							{{ button.label }}
						</div>
					</div>
				</div>
				<p
					v-if="preview.idle.buttons.length > 0 || preview.playing.buttons.length > 0"
					class="m-0 mt-2 text-sm text-secondary"
				>
					{{ formatMessage(messages.previewButtonsNote) }}
				</p>
			</div>

			<div class="flex flex-col gap-3">
				<div class="flex items-center justify-between gap-4">
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.showInstance) }}
					</h3>
					<Toggle
						id="plus-discord-show-instance"
						:model-value="discord?.show_instance ?? true"
						class="shrink-0"
						@update:model-value="save({ show_instance: $event })"
					/>
				</div>
				<div class="flex items-center justify-between gap-4">
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.showVersion) }}
					</h3>
					<Toggle
						id="plus-discord-show-version"
						:model-value="discord?.show_version ?? true"
						class="shrink-0"
						@update:model-value="save({ show_version: $event })"
					/>
				</div>
				<p class="m-0 text-secondary">{{ formatMessage(messages.showHiddenText) }}</p>
				<div class="flex items-center justify-between gap-4">
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.showElapsed) }}
					</h3>
					<Toggle
						id="plus-discord-show-elapsed"
						:model-value="discord?.show_elapsed ?? true"
						class="shrink-0"
						@update:model-value="save({ show_elapsed: $event })"
					/>
				</div>
			</div>

			<template v-if="mode === 'custom'">
				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.idleText) }}
					</h3>
					<Input
						id="plus-discord-idle-text"
						v-model="idleText"
						class="mt-2"
						wrapper-class="w-full"
						:maxlength="128"
						autocomplete="off"
						placeholder="Browsing Pawrinth"
					/>
					<Input
						id="plus-discord-idle-state-text"
						v-model="idleStateText"
						class="mt-2"
						wrapper-class="w-full"
						:maxlength="128"
						autocomplete="off"
						:placeholder="formatMessage(messages.secondLine)"
					/>
				</div>

				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.playingText) }}
					</h3>
					<p class="m-0 mt-1">
						{{
							formatMessage(messages.playingTextHelp, { tokens: '{instance}, {version}, {loader}' })
						}}
					</p>
					<Input
						id="plus-discord-playing-text"
						v-model="playingText"
						class="mt-2"
						wrapper-class="w-full"
						:maxlength="128"
						autocomplete="off"
						placeholder="Playing {instance}"
					/>
					<Input
						id="plus-discord-playing-state-text"
						v-model="playingStateText"
						class="mt-2"
						wrapper-class="w-full"
						:maxlength="128"
						autocomplete="off"
						placeholder="Minecraft {version} · {loader}"
					/>
				</div>

				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.logoImage) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.logoImageText) }}</p>
					<Input
						id="plus-discord-logo-image"
						v-model="logoImage"
						class="mt-2"
						wrapper-class="w-full"
						autocomplete="off"
						:spellcheck="false"
						:placeholder="formatMessage(messages.logoImagePlaceholder)"
					/>
				</div>

				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.logoText) }}
					</h3>
					<Input
						id="plus-discord-logo-text"
						v-model="logoText"
						class="mt-2"
						wrapper-class="w-full"
						:maxlength="128"
						autocomplete="off"
						placeholder="Pawrinth"
					/>
				</div>

				<div class="flex items-center justify-between gap-4">
					<div>
						<h3 class="m-0 text-lg font-semibold text-contrast">
							{{ formatMessage(messages.showBadge) }}
						</h3>
						<p class="m-0 mt-1">{{ formatMessage(messages.showBadgeText) }}</p>
					</div>
					<Toggle
						id="plus-discord-show-badge"
						:model-value="discord?.show_loader_badge ?? false"
						class="shrink-0"
						@update:model-value="save({ show_loader_badge: $event })"
					/>
				</div>

				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.buttonTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.buttonText) }}</p>
					<div class="mt-2 grid grid-cols-1 gap-2 lg:grid-cols-[1fr_2fr]">
						<Input
							id="plus-discord-button-label"
							v-model="buttonLabel"
							wrapper-class="w-full"
							:maxlength="32"
							autocomplete="off"
							:placeholder="formatMessage(messages.buttonLabel)"
						/>
						<Input
							id="plus-discord-button-url"
							v-model="buttonUrl"
							wrapper-class="w-full"
							autocomplete="off"
							:spellcheck="false"
							:placeholder="formatMessage(messages.buttonUrl)"
						/>
						<Input
							id="plus-discord-second-button-label"
							v-model="secondButtonLabel"
							wrapper-class="w-full"
							:maxlength="32"
							autocomplete="off"
							:placeholder="formatMessage(messages.buttonLabel)"
						/>
						<Input
							id="plus-discord-second-button-url"
							v-model="secondButtonUrl"
							wrapper-class="w-full"
							autocomplete="off"
							:spellcheck="false"
							:placeholder="formatMessage(messages.buttonUrl)"
						/>
					</div>
					<p v-if="anyButtonUrlInvalid" class="m-0 mt-1 text-sm text-red" role="alert">
						{{ formatMessage(messages.buttonUrlInvalid) }}
					</p>
				</div>

				<div>
					<h3 class="m-0 text-lg font-semibold text-contrast">
						{{ formatMessage(messages.applicationTitle) }}
					</h3>
					<p class="m-0 mt-1">{{ formatMessage(messages.applicationText) }}</p>
					<Input
						id="plus-discord-application-id"
						v-model="applicationId"
						class="mt-2"
						wrapper-class="w-full"
						autocomplete="off"
						:spellcheck="false"
						:placeholder="formatMessage(messages.applicationPlaceholder)"
					/>
					<p v-if="applicationInvalid" class="m-0 mt-1 text-sm text-red" role="alert">
						{{ formatMessage(messages.applicationInvalid) }}
					</p>
					<h4 class="m-0 mt-4 text-base font-semibold text-contrast">
						{{ formatMessage(messages.stepsTitle) }}
					</h4>
					<ol class="m-0 mt-1 flex list-decimal flex-col gap-1 pl-5">
						<li>{{ formatMessage(messages.stepOne) }}</li>
						<li>{{ formatMessage(messages.stepTwo) }}</li>
						<li>{{ formatMessage(messages.stepThree) }}</li>
						<li>{{ formatMessage(messages.stepFour) }}</li>
					</ol>
					<Button class="mt-3" native-type="button" @click="openUrl(PORTAL_URL)">
						{{ formatMessage(messages.openPortal) }}
						<ExternalIcon aria-hidden="true" />
					</Button>
				</div>
			</template>

			<div>
				<Button id="plus-discord-restore" native-type="button" @click="restoreDefaults">
					<UndoIcon aria-hidden="true" />
					{{ formatMessage(messages.restore) }}
				</Button>
			</div>
		</div>
	</section>
</template>
