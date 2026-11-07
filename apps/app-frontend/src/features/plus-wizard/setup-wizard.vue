<script setup lang="ts">
import { CheckIcon, ImportIcon, LeftArrowIcon, PlusIcon, RightArrowIcon } from '@modrinth/assets'
import {
	Admonition,
	Button,
	Checkbox,
	ColorPicker,
	defineMessages,
	injectInstanceImport,
	injectNotificationManager,
	LoadingIndicator,
	NewModal,
	Slider,
	Toggle,
	useVIntl,
} from '@modrinth/ui'
import { useDebounceFn } from '@vueuse/core'
import { computed, inject, nextTick, ref, watch } from 'vue'

import { useTheme } from '@/composables/use-theme'
import { patchPlusSettings, usePlusSettings } from '@/features/plus-settings/use-plus-settings'
import {
	applyCustomTheme,
	applyStockTheme,
	persistThemes,
	type StockTheme,
} from '@/features/plus-themes/theme-actions'
import { useThemeLibrary } from '@/features/plus-themes/theme-engine'
import type { CustomTheme } from '@/features/plus-themes/theme-model'
import { THEME_PRESETS } from '@/features/plus-themes/theme-presets'
import { list } from '@/helpers/instance'
import { type MinecraftFolder, plus_detect_minecraft_folder } from '@/helpers/plus'

import {
	ACCENT_THEME_ID,
	accentTheme,
	DEFAULT_ACCENT,
	type DetectedLauncher,
	importSelections,
	nextStep,
	previousStep,
	withAccentTheme,
	WIZARD_STEPS,
	wizardDecision,
	type WizardStep,
} from './wizard-model'
import { wizardOpenRequests } from './wizard-state'

const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const { settings } = usePlusSettings()
const appTheme = useTheme()
const { savedThemes, activeThemeId } = useThemeLibrary()
const instanceImport = injectInstanceImport(null)
const showCreationModal = inject<(() => void) | undefined>('showCreationModal', undefined)

const messages = defineMessages({
	title: { id: 'plus.wizard.title', defaultMessage: 'Set up Pawrinth' },
	stepOf: { id: 'plus.wizard.step-of', defaultMessage: 'Step {current} of {total}' },
	welcomeHeading: { id: 'plus.wizard.welcome', defaultMessage: 'Welcome to Pawrinth' },
	welcomeText: {
		id: 'plus.wizard.welcome.text',
		defaultMessage:
			"A few quick choices and you're ready to play. Every step is optional, and you can change anything later in Settings.",
	},
	welcomeFork: {
		id: 'plus.wizard.welcome.fork',
		defaultMessage:
			'Pawrinth is an unofficial, community-developed fork of Modrinth App, maintained by pawmerized.',
	},
	appearanceHeading: { id: 'plus.wizard.appearance', defaultMessage: 'Choose a look' },
	stockThemes: { id: 'plus.wizard.stock', defaultMessage: 'Standard themes' },
	stockDark: { id: 'plus.themes.stock.original', defaultMessage: 'Original dark' },
	stockLight: { id: 'plus.themes.stock.light', defaultMessage: 'Light' },
	stockOled: { id: 'plus.themes.stock.amoled', defaultMessage: 'AMOLED' },
	presets: { id: 'plus.themes.presets', defaultMessage: 'Presets' },
	accent: { id: 'plus.wizard.accent', defaultMessage: 'Accent color' },
	accentText: {
		id: 'plus.wizard.accent.text',
		defaultMessage: 'Colors buttons and highlights, using your current dark or light base.',
	},
	accentThemeName: { id: 'plus.wizard.accent.theme-name', defaultMessage: 'My accent' },
	accentUnreadable: {
		id: 'plus.wizard.accent.unreadable',
		defaultMessage: "That color would be too hard to read, so it wasn't applied.",
	},
	layoutHeading: { id: 'plus.wizard.layout', defaultMessage: 'Set your layout' },
	scale: { id: 'plus.interface.scale', defaultMessage: 'Interface scale' },
	scaleText: {
		id: 'plus.wizard.scale.text',
		defaultMessage:
			'Makes the whole app larger or smaller, on top of your Windows display scaling.',
	},
	compact: { id: 'plus.interface.compact', defaultMessage: 'Compact layout' },
	compactText: {
		id: 'plus.interface.compact.description',
		defaultMessage: 'Tightens text and spacing throughout the app so more fits on screen.',
	},
	contentHeading: { id: 'plus.wizard.content', defaultMessage: 'Bring your existing game' },
	minecraftFound: { id: 'plus.wizard.minecraft.found', defaultMessage: 'Minecraft folder found' },
	minecraftFoundText: {
		id: 'plus.wizard.minecraft.found.text',
		defaultMessage:
			'{path} has {saves, plural, one {# world} other {# worlds}} and {versions, plural, one {# game version} other {# game versions}}. Pawrinth keeps its own instances and leaves this folder as it is.',
	},
	minecraftMissing: {
		id: 'plus.wizard.minecraft.missing',
		defaultMessage: 'No Minecraft launcher folder was found in the usual place.',
	},
	launchersHeading: { id: 'plus.wizard.launchers', defaultMessage: 'Instances in other launchers' },
	launchersText: {
		id: 'plus.wizard.launchers.text',
		defaultMessage: 'Choose any to copy into Pawrinth. The originals stay where they are.',
	},
	noLaunchers: {
		id: 'plus.wizard.launchers.none',
		defaultMessage: 'No instances were found in other launchers.',
	},
	alreadyHere: { id: 'plus.wizard.launchers.already', defaultMessage: 'Already in Pawrinth' },
	importAction: {
		id: 'plus.wizard.import',
		defaultMessage: '{count, plural, one {Copy # instance} other {Copy # instances}}',
	},
	importStarted: {
		id: 'plus.wizard.import.started',
		defaultMessage: 'Copying started. You can follow it in the download manager.',
	},
	featuresHeading: { id: 'plus.wizard.features', defaultMessage: 'Pawrinth features' },
	recovery: { id: 'plus.recovery.enabled', defaultMessage: 'Save recovery data before updating' },
	recoveryText: {
		id: 'plus.wizard.recovery.text',
		defaultMessage: 'Lets you restore earlier mod versions after an update. Uses some disk space.',
	},
	mclogs: { id: 'plus.diagnostics.mclogs', defaultMessage: 'Also check crashes with mclo.gs' },
	mclogsText: {
		id: 'plus.diagnostics.mclogs.description',
		defaultMessage:
			"When Pawrinth's own checks find nothing, sends the game's log to the mclo.gs service for analysis. Logs can contain personal details.",
	},
	finishHeading: { id: 'plus.wizard.finish', defaultMessage: "You're all set" },
	finishText: {
		id: 'plus.wizard.finish.text',
		defaultMessage: 'You can run this setup again from Settings, under Pawrinth → Interface.',
	},
	createInstance: { id: 'plus.wizard.create-instance', defaultMessage: 'Create an instance' },
	skip: { id: 'plus.wizard.skip', defaultMessage: 'Skip setup' },
	back: { id: 'plus.wizard.back', defaultMessage: 'Back' },
	next: { id: 'plus.wizard.next', defaultMessage: 'Next' },
	done: { id: 'plus.wizard.done', defaultMessage: 'Done' },
})

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const step = ref<WizardStep>('welcome')
const accent = ref(DEFAULT_ACCENT)
const accentRejected = ref(false)
const minecraftFolder = ref<MinecraftFolder | null>(null)
const launchers = ref<DetectedLauncher[]>([])
const existingNames = ref<Set<string>>(new Set())
const selected = ref<Set<string>>(new Set())
const detecting = ref(false)
const detected = ref(false)
const importing = ref(false)
const scalePercent = computed(() => Math.round((settings.value?.interface.ui_scale ?? 1) * 100))
const stepNumber = computed(() => WIZARD_STEPS.indexOf(step.value) + 1)
const canImport = computed(() => selected.value.size > 0 && !importing.value)

const stockThemes: { value: StockTheme; label: (typeof messages)[keyof typeof messages] }[] = [
	{ value: 'dark', label: messages.stockDark },
	{ value: 'light', label: messages.stockLight },
	{ value: 'oled', label: messages.stockOled },
]

function isStockActive(value: StockTheme): boolean {
	return activeThemeId.value === null && appTheme.preferred === value
}

async function chooseStock(value: StockTheme): Promise<void> {
	try {
		await applyStockTheme(value, savedThemes.value)
	} catch (error) {
		handleError(error as Error)
	}
}

async function choosePreset(theme: CustomTheme): Promise<void> {
	try {
		await applyCustomTheme(theme, savedThemes.value)
	} catch (error) {
		handleError(error as Error)
	}
}

const chooseAccent = useDebounceFn(async (color: string) => {
	const base = appTheme.active === 'light' ? 'light' : 'dark'
	const theme = accentTheme(base, color.toLowerCase(), formatMessage(messages.accentThemeName))
	accentRejected.value = theme === null
	if (!theme) return
	try {
		const themes = withAccentTheme(savedThemes.value, theme)
		await persistThemes(themes, ACCENT_THEME_ID)
		await applyCustomTheme(theme, themes)
	} catch (error) {
		handleError(error as Error)
	}
}, 200)

const updateScale = useDebounceFn(
	(percent: number) => updateInterface({ ui_scale: percent / 100 }),
	400,
)

async function updateInterface(
	value: Parameters<typeof patchPlusSettings>[0]['interface'],
): Promise<void> {
	try {
		await patchPlusSettings({ interface: value })
	} catch (error) {
		handleError(error as Error)
	}
}

async function setRecovery(enabled: boolean): Promise<void> {
	try {
		await patchPlusSettings({ rollback: { enabled } })
	} catch (error) {
		handleError(error as Error)
	}
}

async function setMclogs(send_logs_to_mclogs: boolean): Promise<void> {
	try {
		await patchPlusSettings({ diagnostics: { send_logs_to_mclogs } })
	} catch (error) {
		handleError(error as Error)
	}
}

/** Only reads other launchers' folders. Nothing is copied until the user asks. */
async function detectContent(): Promise<void> {
	if (detected.value || detecting.value) return
	detecting.value = true
	try {
		const [folder, found, instances] = await Promise.all([
			plus_detect_minecraft_folder(),
			instanceImport?.getDetectedLaunchers() ?? Promise.resolve([]),
			list(),
		])
		minecraftFolder.value = folder
		launchers.value = found as DetectedLauncher[]
		existingNames.value = new Set(instances.map((instance) => instance.name))
		detected.value = true
	} catch (error) {
		handleError(error as Error)
	} finally {
		detecting.value = false
	}
}

/** Starts the picker on the color the app is showing, so it never looks out of date. */
function syncAccentFromTheme(): void {
	const brand = getComputedStyle(document.documentElement).getPropertyValue('--color-brand').trim()
	if (/^#[0-9a-f]{6}$/i.test(brand)) accent.value = brand
}

watch(step, (value) => {
	if (value === 'content') void detectContent()
	if (value === 'appearance') syncAccentFromTheme()
})

watch(activeThemeId, () => {
	if (step.value === 'appearance') void nextTick(syncAccentFromTheme)
})

function instanceKey(launcher: DetectedLauncher, name: string): string {
	return `${launcher.name}/${name}`
}

function toggleInstance(key: string, value: boolean): void {
	const next = new Set(selected.value)
	if (value) next.add(key)
	else next.delete(key)
	selected.value = next
}

async function importChosen(): Promise<void> {
	if (!instanceImport) return
	importing.value = true
	try {
		await instanceImport.importInstances(importSelections(launchers.value, selected.value))
		addNotification({ type: 'success', title: formatMessage(messages.importStarted) })
		for (const key of selected.value) existingNames.value.add(key.split('/').slice(1).join('/'))
		selected.value = new Set()
	} catch (error) {
		handleError(error as Error)
	} finally {
		importing.value = false
	}
}

async function markComplete(): Promise<void> {
	if (settings.value?.onboarding.wizard_completed) return
	try {
		await patchPlusSettings({ onboarding: { wizard_completed: true } })
	} catch (error) {
		handleError(error as Error)
	}
}

function close(): void {
	modal.value?.hide()
}

function createInstance(): void {
	close()
	showCreationModal?.()
}

function open(): void {
	step.value = 'welcome'
	modal.value?.show()
}

watch(wizardOpenRequests, open)

let decided = false

watch(
	settings,
	async (value) => {
		if (!value || decided) return
		decided = true
		const instances = await list().catch(() => null)
		if (instances === null) return
		const decision = wizardDecision({
			completed: value.onboarding.wizard_completed,
			instanceCount: instances.length,
		})
		if (decision === 'show') open()
		if (decision === 'mark-complete') void markComplete()
	},
	{ immediate: true },
)
</script>

<template>
	<NewModal
		ref="modal"
		:header="formatMessage(messages.title)"
		max-width="640px"
		:close-on-click-outside="false"
		:on-hide="markComplete"
	>
		<div v-if="settings" class="flex w-[36rem] max-w-full flex-col gap-4">
			<p class="m-0 text-sm font-semibold text-secondary">
				{{ formatMessage(messages.stepOf, { current: stepNumber, total: WIZARD_STEPS.length }) }}
			</p>

			<section v-if="step === 'welcome'" class="flex flex-col gap-2">
				<h3 class="m-0 text-xl font-semibold text-contrast">
					{{ formatMessage(messages.welcomeHeading) }}
				</h3>
				<p class="m-0">{{ formatMessage(messages.welcomeText) }}</p>
				<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.welcomeFork) }}</p>
			</section>

			<section v-else-if="step === 'appearance'" class="flex flex-col gap-4">
				<h3 class="m-0 text-xl font-semibold text-contrast">
					{{ formatMessage(messages.appearanceHeading) }}
				</h3>
				<div class="flex flex-col gap-2">
					<span class="font-semibold text-contrast">{{ formatMessage(messages.stockThemes) }}</span>
					<div class="flex flex-wrap gap-2">
						<Button
							v-for="stock in stockThemes"
							:key="stock.value"
							:type="isStockActive(stock.value) ? 'colored' : 'base'"
							:color="isStockActive(stock.value) ? 'brand' : undefined"
							@click="chooseStock(stock.value)"
						>
							<CheckIcon v-if="isStockActive(stock.value)" aria-hidden="true" />
							{{ formatMessage(stock.label) }}
						</Button>
					</div>
				</div>
				<div class="flex flex-col gap-2">
					<span class="font-semibold text-contrast">{{ formatMessage(messages.presets) }}</span>
					<div class="flex flex-wrap gap-2">
						<Button
							v-for="theme in THEME_PRESETS"
							:key="theme.id"
							:type="activeThemeId === theme.id ? 'colored' : 'base'"
							:color="activeThemeId === theme.id ? 'brand' : undefined"
							@click="choosePreset(theme)"
						>
							<CheckIcon v-if="activeThemeId === theme.id" aria-hidden="true" />
							<span
								v-else
								class="size-3 rounded-full"
								:style="{ backgroundColor: theme.palette.accent }"
								aria-hidden="true"
							/>
							{{ theme.name }}
						</Button>
					</div>
				</div>
				<div class="flex items-center justify-between gap-4">
					<div>
						<span class="font-semibold text-contrast">{{ formatMessage(messages.accent) }}</span>
						<p class="m-0 mt-1">{{ formatMessage(messages.accentText) }}</p>
					</div>
					<ColorPicker
						v-model="accent"
						:label="formatMessage(messages.accent)"
						@update:model-value="chooseAccent($event)"
					/>
				</div>
				<Admonition v-if="accentRejected" type="warning">
					{{ formatMessage(messages.accentUnreadable) }}
				</Admonition>
			</section>

			<section v-else-if="step === 'layout'" class="flex flex-col gap-4">
				<h3 class="m-0 text-xl font-semibold text-contrast">
					{{ formatMessage(messages.layoutHeading) }}
				</h3>
				<div>
					<span class="font-semibold text-contrast">{{ formatMessage(messages.scale) }}</span>
					<p class="m-0 mt-1">{{ formatMessage(messages.scaleText) }}</p>
					<Slider
						class="mt-3"
						:model-value="scalePercent"
						:min="80"
						:max="150"
						:step="5"
						:snap-points="[100, 125]"
						unit="%"
						force-step
						@update:model-value="updateScale($event)"
					/>
				</div>
				<div class="flex items-center justify-between gap-4">
					<div>
						<span class="font-semibold text-contrast">{{ formatMessage(messages.compact) }}</span>
						<p class="m-0 mt-1">{{ formatMessage(messages.compactText) }}</p>
					</div>
					<Toggle
						id="plus-wizard-compact"
						:model-value="settings.interface.density === 'compact'"
						@update:model-value="updateInterface({ density: $event ? 'compact' : 'comfortable' })"
					/>
				</div>
			</section>

			<section v-else-if="step === 'content'" class="flex flex-col gap-4">
				<h3 class="m-0 text-xl font-semibold text-contrast">
					{{ formatMessage(messages.contentHeading) }}
				</h3>
				<LoadingIndicator v-if="detecting" />
				<template v-else>
					<Admonition
						v-if="minecraftFolder"
						type="info"
						:header="formatMessage(messages.minecraftFound)"
					>
						{{
							formatMessage(messages.minecraftFoundText, {
								path: minecraftFolder.path,
								saves: minecraftFolder.saves,
								versions: minecraftFolder.versions,
							})
						}}
					</Admonition>
					<p v-else class="m-0 text-secondary">{{ formatMessage(messages.minecraftMissing) }}</p>

					<div class="flex flex-col gap-2">
						<span class="font-semibold text-contrast">
							{{ formatMessage(messages.launchersHeading) }}
						</span>
						<p v-if="launchers.length === 0" class="m-0 text-secondary">
							{{ formatMessage(messages.noLaunchers) }}
						</p>
						<template v-else>
							<p class="m-0">{{ formatMessage(messages.launchersText) }}</p>
							<div class="flex max-h-56 flex-col gap-3 overflow-y-auto">
								<div
									v-for="launcher in launchers"
									:key="launcher.name"
									class="rounded-xl bg-surface-2 p-3"
								>
									<p class="m-0 mb-2 text-sm font-semibold text-secondary">{{ launcher.name }}</p>
									<label
										v-for="name in launcher.instances"
										:key="name"
										class="flex items-center gap-2 py-1"
									>
										<Checkbox
											:model-value="selected.has(instanceKey(launcher, name))"
											:disabled="existingNames.has(name)"
											@update:model-value="toggleInstance(instanceKey(launcher, name), $event)"
										/>
										<span class="min-w-0 break-all">{{ name }}</span>
										<span v-if="existingNames.has(name)" class="shrink-0 text-sm text-secondary">
											{{ formatMessage(messages.alreadyHere) }}
										</span>
									</label>
								</div>
							</div>
							<div>
								<Button :disabled="!canImport" @click="importChosen">
									<ImportIcon aria-hidden="true" />
									{{ formatMessage(messages.importAction, { count: selected.size }) }}
								</Button>
							</div>
						</template>
					</div>
				</template>
			</section>

			<section v-else-if="step === 'features'" class="flex flex-col gap-4">
				<h3 class="m-0 text-xl font-semibold text-contrast">
					{{ formatMessage(messages.featuresHeading) }}
				</h3>
				<div class="flex items-center justify-between gap-4">
					<div>
						<span class="font-semibold text-contrast">{{ formatMessage(messages.recovery) }}</span>
						<p class="m-0 mt-1">{{ formatMessage(messages.recoveryText) }}</p>
					</div>
					<Toggle
						id="plus-wizard-recovery"
						:model-value="settings.rollback.enabled"
						@update:model-value="setRecovery($event)"
					/>
				</div>
				<div class="flex items-center justify-between gap-4">
					<div>
						<span class="font-semibold text-contrast">{{ formatMessage(messages.mclogs) }}</span>
						<p class="m-0 mt-1">{{ formatMessage(messages.mclogsText) }}</p>
					</div>
					<Toggle
						id="plus-wizard-mclogs"
						:model-value="settings.diagnostics.send_logs_to_mclogs"
						@update:model-value="setMclogs($event)"
					/>
				</div>
			</section>

			<section v-else class="flex flex-col gap-2">
				<h3 class="m-0 text-xl font-semibold text-contrast">
					{{ formatMessage(messages.finishHeading) }}
				</h3>
				<p class="m-0">{{ formatMessage(messages.finishText) }}</p>
			</section>

			<div class="flex flex-wrap items-center justify-between gap-2">
				<Button v-if="step !== 'finish'" type="quiet" @click="close">
					{{ formatMessage(messages.skip) }}
				</Button>
				<span v-else />
				<div class="flex gap-2">
					<Button v-if="step !== 'welcome'" @click="step = previousStep(step)">
						<LeftArrowIcon aria-hidden="true" />
						{{ formatMessage(messages.back) }}
					</Button>
					<Button
						v-if="step !== 'finish'"
						type="colored"
						color="brand"
						@click="step = nextStep(step)"
					>
						{{ formatMessage(messages.next) }}
						<RightArrowIcon aria-hidden="true" />
					</Button>
					<template v-else>
						<Button v-if="showCreationModal" @click="createInstance">
							<PlusIcon aria-hidden="true" />
							{{ formatMessage(messages.createInstance) }}
						</Button>
						<Button type="colored" color="brand" @click="close">
							<CheckIcon aria-hidden="true" />
							{{ formatMessage(messages.done) }}
						</Button>
					</template>
				</div>
			</div>
		</div>
	</NewModal>
</template>
