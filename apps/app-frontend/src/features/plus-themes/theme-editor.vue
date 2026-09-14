<script setup lang="ts">
import {
	CheckIcon,
	ClipboardCopyIcon,
	CopyIcon,
	DownloadIcon,
	EditIcon,
	PlusIcon,
	SaveIcon,
	TrashIcon,
	UndoIcon,
	UploadIcon,
	XIcon,
} from '@modrinth/assets'
import {
	Admonition,
	Button,
	ColorPicker,
	ConfirmModal,
	defineMessages,
	IconButton,
	injectNotificationManager,
	Input,
	Textarea,
	Toggle,
	useVIntl,
} from '@modrinth/ui'
import { open, save } from '@tauri-apps/plugin-dialog'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

import { useTheme } from '@/composables/use-theme'
import { patchPlusSettings, usePlusSettings } from '@/features/plus-settings/use-plus-settings'
import { plus_theme_file_read, plus_theme_file_save } from '@/helpers/plus'

import { normalizeBackground } from './background-model'
import BackgroundSettings from './background-settings.vue'
import { applyStockTheme, matchAppTheme, persistThemes, type StockTheme } from './theme-actions'
import { setThemePreview, useThemeLibrary } from './theme-engine'
import {
	type BackgroundAdjustments,
	contrastIssues,
	type CustomTheme,
	exportTheme,
	isUsable,
	PALETTE_KEYS,
	type PaletteKey,
	paletteFromTokens,
	parseThemeImport,
	themeFileName,
	type ThemeBase,
	type ThemePalette,
	uniqueThemeName,
} from './theme-model'
import { DARK_STARTER, LIGHT_STARTER, THEME_PRESETS } from './theme-presets'

const { formatMessage } = useVIntl()
const { handleError, addNotification } = injectNotificationManager()
const appTheme = useTheme()
const { savedThemes, activeThemeId } = useThemeLibrary()

const messages = defineMessages({
	themesTitle: { id: 'plus.themes.title', defaultMessage: 'Themes' },
	themesDescription: {
		id: 'plus.themes.description',
		defaultMessage:
			'Custom themes recolor the whole app. Choose a stock theme in Appearance to switch back at any time.',
	},
	stockTheme: { id: 'plus.themes.stock', defaultMessage: 'Stock theme' },
	stockThemeDescription: {
		id: 'plus.themes.stock.description',
		defaultMessage: 'Use the theme selected in Appearance.',
	},
	presets: { id: 'plus.themes.presets', defaultMessage: 'Presets' },
	stockOriginal: { id: 'plus.themes.stock.original', defaultMessage: 'Original dark' },
	stockLight: { id: 'plus.themes.stock.light', defaultMessage: 'Light' },
	stockAmoled: { id: 'plus.themes.stock.amoled', defaultMessage: 'AMOLED' },
	yourThemes: { id: 'plus.themes.your-themes', defaultMessage: 'Your themes' },
	noThemes: {
		id: 'plus.themes.none',
		defaultMessage: 'You have no saved themes yet. Customize a preset or start from scratch.',
	},
	apply: { id: 'plus.themes.apply', defaultMessage: 'Apply' },
	applied: { id: 'plus.themes.applied', defaultMessage: 'Applied' },
	customize: { id: 'plus.themes.customize', defaultMessage: 'Customize' },
	edit: { id: 'plus.themes.edit', defaultMessage: 'Edit' },
	duplicate: { id: 'plus.themes.duplicate', defaultMessage: 'Duplicate' },
	delete: { id: 'plus.themes.delete', defaultMessage: 'Delete' },
	copySuffix: { id: 'plus.themes.copy-suffix', defaultMessage: '{name} copy' },
	newTheme: { id: 'plus.themes.new', defaultMessage: 'New theme' },
	newDark: { id: 'plus.themes.new-dark', defaultMessage: 'New dark theme' },
	newLight: { id: 'plus.themes.new-light', defaultMessage: 'New light theme' },
	editorTitle: { id: 'plus.themes.editor', defaultMessage: 'Theme editor' },
	nameLabel: { id: 'plus.themes.name', defaultMessage: 'Name' },
	undo: { id: 'plus.themes.undo', defaultMessage: 'Undo' },
	resetColors: { id: 'plus.themes.reset', defaultMessage: 'Reset colors' },
	saveTheme: { id: 'plus.themes.save', defaultMessage: 'Save and apply' },
	cancel: { id: 'plus.themes.cancel', defaultMessage: 'Cancel' },
	export: { id: 'plus.themes.export', defaultMessage: 'Copy theme to clipboard' },
	exported: { id: 'plus.themes.exported', defaultMessage: 'Theme copied to clipboard.' },
	exportFile: { id: 'plus.themes.export-file', defaultMessage: 'Export to a file' },
	exportedFile: { id: 'plus.themes.exported-file', defaultMessage: 'Theme exported.' },
	exportCurrent: { id: 'plus.themes.export-current', defaultMessage: 'Export current theme' },
	rename: { id: 'plus.themes.rename', defaultMessage: 'Rename' },
	renameSave: { id: 'plus.themes.rename.save', defaultMessage: 'Save name' },
	restoreDefault: { id: 'plus.themes.restore-default', defaultMessage: 'Restore default theme' },
	restoreDefaultDescription: {
		id: 'plus.themes.restore-default.description',
		defaultMessage: 'Goes back to the original dark theme. Your saved themes are kept.',
	},
	deleteTitle: { id: 'plus.themes.delete.title', defaultMessage: 'Delete {name}?' },
	deleteDescription: {
		id: 'plus.themes.delete.description',
		defaultMessage: "This can't be undone, unless you've exported it.",
	},
	importTitle: { id: 'plus.themes.import', defaultMessage: 'Import and export' },
	importDescription: {
		id: 'plus.themes.import.description',
		defaultMessage:
			'Theme files hold colors and background adjustments. A background image itself is not included.',
	},
	importFile: { id: 'plus.themes.import-file', defaultMessage: 'Import from a file' },
	pasteLabel: { id: 'plus.themes.import.paste', defaultMessage: 'Or paste a copied theme' },
	importAction: { id: 'plus.themes.import.action', defaultMessage: 'Preview' },
	previewTitle: { id: 'plus.themes.import.preview', defaultMessage: 'Previewing {name}' },
	previewDescription: {
		id: 'plus.themes.import.preview.description',
		defaultMessage: "This is how it looks. Nothing is saved until you choose below.",
	},
	duplicateHeader: {
		id: 'plus.themes.import.duplicate',
		defaultMessage: 'You already have a theme called {name}',
	},
	duplicateText: {
		id: 'plus.themes.import.duplicate.text',
		defaultMessage: 'Keep both, or replace the one you have.',
	},
	keepBoth: { id: 'plus.themes.import.keep-both', defaultMessage: 'Keep both' },
	replaceExisting: { id: 'plus.themes.import.replace', defaultMessage: 'Replace existing' },
	saveOnly: { id: 'plus.themes.import.save', defaultMessage: 'Save for later' },
	saveAndApply: { id: 'plus.themes.import.save-apply', defaultMessage: 'Save and apply' },
	useAdjustments: {
		id: 'plus.themes.import.adjustments',
		defaultMessage: 'Also use its background adjustments',
	},
	useAdjustmentsNote: {
		id: 'plus.themes.import.adjustments.note',
		defaultMessage: 'Applies to your current background image, if you have one.',
	},
	importUnsupported: {
		id: 'plus.themes.import.unsupported',
		defaultMessage: 'That theme was made by a newer version of Pawrinth. Update to use it.',
	},
	importReadFailed: {
		id: 'plus.themes.import.read-failed',
		defaultMessage: "That file couldn't be read as a theme.",
	},
	fileFilter: { id: 'plus.themes.file-filter', defaultMessage: 'Pawrinth theme' },
	importNotJson: {
		id: 'plus.themes.import.not-json',
		defaultMessage: "That text isn't valid JSON.",
	},
	importWrongFormat: {
		id: 'plus.themes.import.wrong-format',
		defaultMessage: "That isn't a Pawrinth theme.",
	},
	importInvalidColors: {
		id: 'plus.themes.import.invalid-colors',
		defaultMessage: 'That theme has missing or invalid colors.',
	},
	importUnreadable: {
		id: 'plus.themes.import.unreadable',
		defaultMessage: 'That theme was rejected because its text would be too hard to read.',
	},
	contrastBlocking: {
		id: 'plus.themes.contrast.blocking',
		defaultMessage:
			'{foreground} on {background} is too hard to read ({ratio}:1). Raise it to at least 3:1 to save.',
	},
	contrastWarning: {
		id: 'plus.themes.contrast.warning',
		defaultMessage:
			'{foreground} on {background} has low contrast ({ratio}:1). 4.5:1 is recommended.',
	},
	contrastTitle: { id: 'plus.themes.contrast.title', defaultMessage: 'Contrast' },
	background: { id: 'plus.themes.color.background', defaultMessage: 'Background' },
	panel: { id: 'plus.themes.color.panel', defaultMessage: 'Panels' },
	surface: { id: 'plus.themes.color.surface', defaultMessage: 'Cards' },
	raised: { id: 'plus.themes.color.raised', defaultMessage: 'Raised surfaces' },
	sidebar: { id: 'plus.themes.color.sidebar', defaultMessage: 'Navigation' },
	button: { id: 'plus.themes.color.button', defaultMessage: 'Buttons' },
	border: { id: 'plus.themes.color.border', defaultMessage: 'Borders' },
	text: { id: 'plus.themes.color.text', defaultMessage: 'Text' },
	textMuted: { id: 'plus.themes.color.text-muted', defaultMessage: 'Secondary text' },
	accent: { id: 'plus.themes.color.accent', defaultMessage: 'Accent' },
	selection: { id: 'plus.themes.color.selection', defaultMessage: 'Text selection' },
})

interface Draft {
	id: string | null
	name: string
	base: ThemeBase
	palette: ThemePalette
}

interface PendingImport {
	theme: Omit<CustomTheme, 'id'>
	background: BackgroundAdjustments | null
	useBackground: boolean
	replaceId: string | null
}

const { settings: plusSettings } = usePlusSettings()
const pendingImport = ref<PendingImport | null>(null)
const renamingId = ref<string | null>(null)
const renameText = ref('')
const pendingDelete = ref<CustomTheme | null>(null)
const deleteModal = ref<InstanceType<typeof ConfirmModal> | null>(null)
const draft = ref<Draft | null>(null)
const draftOriginal = ref<ThemePalette | null>(null)
const undoStack = ref<ThemePalette[]>([])
const importText = ref('')
const importError = ref<string | null>(null)

const issues = computed(() => (draft.value ? contrastIssues(draft.value.palette) : []))
const canSave = computed(
	() => !!draft.value && draft.value.name.trim().length > 0 && isUsable(draft.value.palette),
)

function colorLabel(key: PaletteKey): string {
	return formatMessage(messages[key])
}

function openDraft(id: string | null, name: string, base: ThemeBase, palette: ThemePalette): void {
	draft.value = { id, name, base, palette: { ...palette } }
	draftOriginal.value = { ...palette }
	undoStack.value = []
}

function setColor(key: PaletteKey, value: string): void {
	if (!draft.value || draft.value.palette[key] === value) return
	undoStack.value.push({ ...draft.value.palette })
	draft.value.palette[key] = value.toLowerCase()
}

function undo(): void {
	const previous = undoStack.value.pop()
	if (previous && draft.value) draft.value.palette = previous
}

function resetColors(): void {
	if (!draft.value || !draftOriginal.value) return
	undoStack.value.push({ ...draft.value.palette })
	draft.value.palette = { ...draftOriginal.value }
}

function cancelDraft(): void {
	draft.value = null
}

watch(
	draft,
	(value) => {
		if (!value) {
			setThemePreview(null)
			appTheme.preview = null
			return
		}
		appTheme.preview = value.base
		setThemePreview({
			id: value.id ?? 'draft',
			name: value.name || 'draft',
			base: value.base,
			palette: { ...value.palette },
		})
	},
	{ deep: true },
)

onBeforeUnmount(() => {
	setThemePreview(null)
	appTheme.preview = null
})

async function apply(theme: CustomTheme): Promise<void> {
	try {
		await matchAppTheme(theme.base)
		await persistThemes(savedThemes.value, theme.id)
	} catch (error) {
		handleError(error as Error)
	}
}

const stockThemes = [
	{ value: 'dark', label: messages.stockOriginal },
	{ value: 'light', label: messages.stockLight },
	{ value: 'oled', label: messages.stockAmoled },
] as const

async function useStock(value: StockTheme): Promise<void> {
	try {
		await applyStockTheme(value, savedThemes.value)
	} catch (error) {
		handleError(error as Error)
	}
}

async function useStockTheme(): Promise<void> {
	try {
		await persistThemes(savedThemes.value, null)
	} catch (error) {
		handleError(error as Error)
	}
}

async function saveDraft(): Promise<void> {
	if (!draft.value || !canSave.value) return
	const theme: CustomTheme = {
		id: draft.value.id ?? crypto.randomUUID(),
		name: draft.value.name.trim(),
		base: draft.value.base,
		palette: { ...draft.value.palette },
	}
	const themes = savedThemes.value.some((saved) => saved.id === theme.id)
		? savedThemes.value.map((saved) => (saved.id === theme.id ? theme : saved))
		: [...savedThemes.value, theme]
	try {
		await matchAppTheme(theme.base)
		await persistThemes(themes, theme.id)
		draft.value = null
	} catch (error) {
		handleError(error as Error)
	}
}

async function duplicate(theme: CustomTheme): Promise<void> {
	const copy: CustomTheme = {
		...theme,
		id: crypto.randomUUID(),
		name: formatMessage(messages.copySuffix, { name: theme.name }).slice(0, 48),
		palette: { ...theme.palette },
	}
	try {
		await persistThemes([...savedThemes.value, copy], activeThemeId.value)
	} catch (error) {
		handleError(error as Error)
	}
}

function askToRemove(theme: CustomTheme): void {
	pendingDelete.value = theme
	deleteModal.value?.show()
}

async function remove(): Promise<void> {
	const theme = pendingDelete.value
	if (!theme) return
	pendingDelete.value = null
	try {
		await persistThemes(
			savedThemes.value.filter((saved) => saved.id !== theme.id),
			activeThemeId.value === theme.id ? null : activeThemeId.value,
		)
		if (draft.value?.id === theme.id) draft.value = null
	} catch (error) {
		handleError(error as Error)
	}
}

function startRename(theme: CustomTheme): void {
	renamingId.value = theme.id
	renameText.value = theme.name
}

async function saveRename(theme: CustomTheme): Promise<void> {
	const name = renameText.value.trim().slice(0, 48)
	if (!name) return
	const others = savedThemes.value.filter((saved) => saved.id !== theme.id).map((saved) => saved.name)
	const finalName = uniqueThemeName(name, others)
	renamingId.value = null
	if (finalName === theme.name) return
	try {
		await persistThemes(
			savedThemes.value.map((saved) => (saved.id === theme.id ? { ...saved, name: finalName } : saved)),
			activeThemeId.value,
		)
	} catch (error) {
		handleError(error as Error)
	}
}

async function restoreDefault(): Promise<void> {
	await useStock('dark')
}

const currentTheme = computed<Pick<CustomTheme, 'name' | 'base' | 'palette'> | null>(() => {
	const custom = [...THEME_PRESETS, ...savedThemes.value].find(
		(theme) => theme.id === activeThemeId.value,
	)
	if (custom) return custom
	const style = getComputedStyle(document.documentElement)
	const palette = paletteFromTokens((token) => style.getPropertyValue(token))
	if (!palette) return null
	const base: ThemeBase = appTheme.active === 'light' ? 'light' : 'dark'
	const stock = stockThemes.find((item) => item.value === appTheme.preferred) ?? stockThemes[0]
	return { name: formatMessage(stock.label), base, palette }
})

async function exportToFile(theme: Pick<CustomTheme, 'name' | 'base' | 'palette'>): Promise<void> {
	try {
		const path = await save({
			defaultPath: themeFileName(theme.name),
			filters: [{ name: formatMessage(messages.fileFilter), extensions: ['json'] }],
		})
		if (!path) return
		await plus_theme_file_save(
			path,
			exportTheme(theme, normalizeBackground(plusSettings.value?.appearance.background)),
		)
		addNotification({ type: 'success', title: formatMessage(messages.exportedFile) })
	} catch (error) {
		handleError(error as Error)
	}
}

async function importFromFile(): Promise<void> {
	try {
		const path = await open({
			multiple: false,
			directory: false,
			filters: [{ name: formatMessage(messages.fileFilter), extensions: ['json'] }],
		})
		if (typeof path !== 'string') return
		startImport(await plus_theme_file_read(path))
	} catch {
		importError.value = formatMessage(messages.importReadFailed)
	}
}

function startImport(text: string): void {
	const result = parseThemeImport(text)
	if (!result.ok) {
		const errors = {
			'not-json': messages.importNotJson,
			'wrong-format': messages.importWrongFormat,
			'unsupported-version': messages.importUnsupported,
			'invalid-colors': messages.importInvalidColors,
			unreadable: messages.importUnreadable,
		}
		importError.value = formatMessage(errors[result.error])
		return
	}
	importError.value = null
	importText.value = ''
	draft.value = null
	pendingImport.value = {
		theme: result.theme,
		background: result.background,
		useBackground: !!result.background,
		replaceId: null,
	}
}

const duplicateOf = computed(() => {
	const name = pendingImport.value?.theme.name.toLowerCase()
	return savedThemes.value.find((theme) => theme.name.toLowerCase() === name) ?? null
})

watch(pendingImport, (value) => {
	if (draft.value) return
	if (!value) {
		setThemePreview(null)
		appTheme.preview = null
		return
	}
	appTheme.preview = value.theme.base
	setThemePreview({ id: 'import-preview', ...value.theme })
})

function cancelImport(): void {
	pendingImport.value = null
}

async function commitImport(apply: boolean, replace: boolean): Promise<void> {
	const pending = pendingImport.value
	if (!pending) return
	const existing = replace ? duplicateOf.value : null
	const names = savedThemes.value.filter((theme) => theme.id !== existing?.id).map((theme) => theme.name)
	const saved: CustomTheme = {
		...pending.theme,
		id: existing?.id ?? crypto.randomUUID(),
		name: uniqueThemeName(pending.theme.name, names),
	}
	const themes = existing
		? savedThemes.value.map((theme) => (theme.id === existing.id ? saved : theme))
		: [...savedThemes.value, saved]
	try {
		pendingImport.value = null
		if (apply) await matchAppTheme(saved.base)
		await persistThemes(themes, apply ? saved.id : activeThemeId.value)
		const current = normalizeBackground(plusSettings.value?.appearance.background)
		if (pending.useBackground && pending.background && current) {
			await patchPlusSettings({ appearance: { background: { ...pending.background } } })
		}
	} catch (error) {
		handleError(error as Error)
	}
}

async function copyExport(theme: CustomTheme): Promise<void> {
	await navigator.clipboard.writeText(exportTheme(theme))
	addNotification({ type: 'success', title: formatMessage(messages.exported) })
}

function importPasted(): void {
	startImport(importText.value)
}
</script>

<template>
	<section class="border-0 border-b border-solid border-divider pb-6">
		<h2 class="m-0 text-xl font-semibold text-contrast">
			{{ formatMessage(messages.themesTitle) }}
		</h2>
		<p class="m-0 mt-1 text-secondary">{{ formatMessage(messages.themesDescription) }}</p>

		<div class="mt-4 flex flex-wrap gap-2">
			<Button
				v-for="stock in stockThemes"
				:key="stock.value"
				:type="activeThemeId === null && appTheme.preferred === stock.value ? 'colored' : 'base'"
				:color="activeThemeId === null && appTheme.preferred === stock.value ? 'brand' : undefined"
				@click="useStock(stock.value)"
			>
				{{ formatMessage(stock.label) }}
			</Button>
		</div>
		<div class="mt-4 flex items-center justify-between gap-4">
			<div>
				<h3 class="m-0 text-lg font-semibold text-contrast">
					{{ formatMessage(messages.stockTheme) }}
				</h3>
				<p class="m-0 mt-1">{{ formatMessage(messages.stockThemeDescription) }}</p>
			</div>
			<Button
				:type="activeThemeId === null ? 'colored' : 'base'"
				color="brand"
				:disabled="activeThemeId === null"
				@click="useStockTheme"
			>
				<CheckIcon v-if="activeThemeId === null" aria-hidden="true" />
				{{ formatMessage(activeThemeId === null ? messages.applied : messages.apply) }}
			</Button>
		</div>
		<div class="mt-4 flex items-center justify-between gap-4">
			<p class="m-0">{{ formatMessage(messages.restoreDefaultDescription) }}</p>
			<Button class="shrink-0" @click="restoreDefault">
				<UndoIcon aria-hidden="true" />
				{{ formatMessage(messages.restoreDefault) }}
			</Button>
		</div>
	</section>

	<BackgroundSettings />

	<section v-if="draft" class="mt-6 border-0 border-b border-solid border-divider pb-6">
		<h2 class="m-0 text-xl font-semibold text-contrast">
			{{ formatMessage(messages.editorTitle) }}
		</h2>
		<label class="mt-4 flex flex-col gap-2">
			<span class="font-semibold text-contrast">{{ formatMessage(messages.nameLabel) }}</span>
			<Input v-model="draft.name" :maxlength="48" class="max-w-80" />
		</label>

		<div class="mt-4 grid grid-cols-2 gap-x-6 gap-y-3">
			<div v-for="key in PALETTE_KEYS" :key="key" class="flex items-center justify-between gap-3">
				<span class="min-w-0 truncate text-contrast">{{ colorLabel(key) }}</span>
				<ColorPicker
					:model-value="draft.palette[key]"
					:label="colorLabel(key)"
					@update:model-value="setColor(key, $event)"
				/>
			</div>
		</div>

		<div v-if="issues.length > 0" class="mt-4 flex flex-col gap-2">
			<Admonition
				v-for="issue in issues"
				:key="`${issue.label}-${issue.on}`"
				:type="issue.severity === 'blocking' ? 'critical' : 'warning'"
				:header="formatMessage(messages.contrastTitle)"
			>
				{{
					formatMessage(
						issue.severity === 'blocking' ? messages.contrastBlocking : messages.contrastWarning,
						{
							foreground: colorLabel(issue.label),
							background: colorLabel(issue.on),
							ratio: issue.ratio.toFixed(1),
						},
					)
				}}
			</Admonition>
		</div>

		<div class="mt-4 flex flex-wrap gap-2">
			<Button type="colored" color="brand" :disabled="!canSave" @click="saveDraft">
				<SaveIcon aria-hidden="true" />
				{{ formatMessage(messages.saveTheme) }}
			</Button>
			<Button :disabled="undoStack.length === 0" @click="undo">
				<UndoIcon aria-hidden="true" />
				{{ formatMessage(messages.undo) }}
			</Button>
			<Button :disabled="undoStack.length === 0" @click="resetColors">
				{{ formatMessage(messages.resetColors) }}
			</Button>
			<Button type="quiet" @click="cancelDraft">
				<XIcon aria-hidden="true" />
				{{ formatMessage(messages.cancel) }}
			</Button>
		</div>
	</section>

	<section class="mt-6 border-0 border-b border-solid border-divider pb-6">
		<h2 class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(messages.presets) }}</h2>
		<div class="mt-4 flex flex-col gap-3">
			<div
				v-for="theme in THEME_PRESETS"
				:key="theme.id"
				class="flex items-center justify-between gap-4"
			>
				<div class="flex min-w-0 items-center gap-3">
					<div
						class="flex shrink-0 overflow-hidden rounded-lg border border-solid border-surface-5"
					>
						<span
							v-for="color in [
								theme.palette.background,
								theme.palette.surface,
								theme.palette.accent,
								theme.palette.text,
							]"
							:key="color"
							class="size-6"
							:style="{ backgroundColor: color }"
						/>
					</div>
					<span class="truncate font-semibold text-contrast">{{ theme.name }}</span>
				</div>
				<div class="flex shrink-0 gap-2">
					<IconButton
						v-tooltip="formatMessage(messages.exportFile)"
						type="quiet"
						:label="formatMessage(messages.exportFile)"
						@click="exportToFile(theme)"
					>
						<DownloadIcon aria-hidden="true" />
					</IconButton>
					<Button @click="openDraft(null, theme.name, theme.base, theme.palette)">
						{{ formatMessage(messages.customize) }}
					</Button>
					<Button
						:type="activeThemeId === theme.id ? 'colored' : 'base'"
						color="brand"
						:disabled="activeThemeId === theme.id"
						@click="apply(theme)"
					>
						{{ formatMessage(activeThemeId === theme.id ? messages.applied : messages.apply) }}
					</Button>
				</div>
			</div>
		</div>
	</section>

	<section class="mt-6 border-0 border-b border-solid border-divider pb-6">
		<div class="flex flex-wrap items-center justify-between gap-2">
			<h2 class="m-0 text-xl font-semibold text-contrast">
				{{ formatMessage(messages.yourThemes) }}
			</h2>
			<div class="flex gap-2">
				<Button @click="openDraft(null, formatMessage(messages.newTheme), 'dark', DARK_STARTER)">
					<PlusIcon aria-hidden="true" />
					{{ formatMessage(messages.newDark) }}
				</Button>
				<Button @click="openDraft(null, formatMessage(messages.newTheme), 'light', LIGHT_STARTER)">
					<PlusIcon aria-hidden="true" />
					{{ formatMessage(messages.newLight) }}
				</Button>
			</div>
		</div>
		<p v-if="savedThemes.length === 0" class="m-0 mt-3 text-secondary">
			{{ formatMessage(messages.noThemes) }}
		</p>
		<div v-else class="mt-4 flex flex-col gap-3">
			<div
				v-for="theme in savedThemes"
				:key="theme.id"
				class="flex flex-wrap items-center justify-between gap-3"
			>
				<div class="flex min-w-0 items-center gap-3">
					<div
						class="flex shrink-0 overflow-hidden rounded-lg border border-solid border-surface-5"
					>
						<span
							v-for="color in [
								theme.palette.background,
								theme.palette.surface,
								theme.palette.accent,
								theme.palette.text,
							]"
							:key="color"
							class="size-6"
							:style="{ backgroundColor: color }"
						/>
					</div>
					<form
						v-if="renamingId === theme.id"
						class="flex items-center gap-2"
						@submit.prevent="saveRename(theme)"
					>
						<Input v-model="renameText" :maxlength="48" autofocus />
						<Button native-type="submit" :disabled="renameText.trim().length === 0">
							{{ formatMessage(messages.renameSave) }}
						</Button>
					</form>
					<span v-else class="truncate font-semibold text-contrast">{{ theme.name }}</span>
				</div>
				<div class="flex shrink-0 flex-wrap gap-2">
					<IconButton
						v-tooltip="formatMessage(messages.rename)"
						type="quiet"
						:label="formatMessage(messages.rename)"
						@click="startRename(theme)"
					>
						<EditIcon aria-hidden="true" />
					</IconButton>
					<IconButton
						v-tooltip="formatMessage(messages.exportFile)"
						type="quiet"
						:label="formatMessage(messages.exportFile)"
						@click="exportToFile(theme)"
					>
						<DownloadIcon aria-hidden="true" />
					</IconButton>
					<IconButton
						v-tooltip="formatMessage(messages.export)"
						type="quiet"
						:label="formatMessage(messages.export)"
						@click="copyExport(theme)"
					>
						<ClipboardCopyIcon aria-hidden="true" />
					</IconButton>
					<IconButton
						v-tooltip="formatMessage(messages.duplicate)"
						type="quiet"
						:label="formatMessage(messages.duplicate)"
						@click="duplicate(theme)"
					>
						<CopyIcon aria-hidden="true" />
					</IconButton>
					<IconButton
						v-tooltip="formatMessage(messages.delete)"
						type="quiet"
						color="red"
						:label="formatMessage(messages.delete)"
						@click="askToRemove(theme)"
					>
						<TrashIcon aria-hidden="true" />
					</IconButton>
					<Button @click="openDraft(theme.id, theme.name, theme.base, theme.palette)">
						{{ formatMessage(messages.edit) }}
					</Button>
					<Button
						:type="activeThemeId === theme.id ? 'colored' : 'base'"
						color="brand"
						:disabled="activeThemeId === theme.id"
						@click="apply(theme)"
					>
						{{ formatMessage(activeThemeId === theme.id ? messages.applied : messages.apply) }}
					</Button>
				</div>
			</div>
		</div>
	</section>

	<section class="mt-6">
		<h2 class="m-0 text-xl font-semibold text-contrast">
			{{ formatMessage(messages.importTitle) }}
		</h2>
		<p class="m-0 mt-1 text-secondary">{{ formatMessage(messages.importDescription) }}</p>
		<div class="mt-3 flex flex-wrap gap-2">
			<Button @click="importFromFile">
				<UploadIcon aria-hidden="true" />
				{{ formatMessage(messages.importFile) }}
			</Button>
			<Button :disabled="!currentTheme" @click="currentTheme && exportToFile(currentTheme)">
				<DownloadIcon aria-hidden="true" />
				{{ formatMessage(messages.exportCurrent) }}
			</Button>
		</div>
		<p v-if="importError" class="m-0 mt-2 text-red">{{ importError }}</p>

		<div v-if="pendingImport" class="mt-4 flex flex-col gap-3 rounded-2xl bg-surface-2 p-4">
			<h3 class="m-0 text-lg font-semibold text-contrast">
				{{ formatMessage(messages.previewTitle, { name: pendingImport.theme.name }) }}
			</h3>
			<p class="m-0">{{ formatMessage(messages.previewDescription) }}</p>
			<div class="flex w-fit overflow-hidden rounded-lg border border-solid border-surface-5">
				<span
					v-for="color in [
						pendingImport.theme.palette.background,
						pendingImport.theme.palette.surface,
						pendingImport.theme.palette.accent,
						pendingImport.theme.palette.text,
					]"
					:key="color"
					class="size-6"
					:style="{ backgroundColor: color }"
				/>
			</div>
			<label
				v-if="pendingImport.background"
				class="flex cursor-pointer items-center justify-between gap-3"
			>
				<span>
					<span class="block font-semibold text-contrast">
						{{ formatMessage(messages.useAdjustments) }}
					</span>
					<span class="text-sm text-secondary">
						{{ formatMessage(messages.useAdjustmentsNote) }}
					</span>
				</span>
				<Toggle v-model="pendingImport.useBackground" class="shrink-0" />
			</label>
			<Admonition
				v-if="duplicateOf"
				type="warning"
				:header="formatMessage(messages.duplicateHeader, { name: pendingImport.theme.name })"
			>
				{{ formatMessage(messages.duplicateText) }}
			</Admonition>
			<div class="flex flex-wrap gap-2">
				<template v-if="duplicateOf">
					<Button type="colored" color="brand" @click="commitImport(true, false)">
						{{ formatMessage(messages.keepBoth) }}
					</Button>
					<Button type="colored" color="red" @click="commitImport(true, true)">
						{{ formatMessage(messages.replaceExisting) }}
					</Button>
				</template>
				<template v-else>
					<Button type="colored" color="brand" @click="commitImport(true, false)">
						{{ formatMessage(messages.saveAndApply) }}
					</Button>
					<Button @click="commitImport(false, false)">
						{{ formatMessage(messages.saveOnly) }}
					</Button>
				</template>
				<Button type="quiet" @click="cancelImport">
					<XIcon aria-hidden="true" />
					{{ formatMessage(messages.cancel) }}
				</Button>
			</div>
		</div>

		<h3 class="m-0 mt-4 font-semibold text-contrast">{{ formatMessage(messages.pasteLabel) }}</h3>
		<Textarea v-model="importText" class="mt-2" :rows="3" />
		<div class="mt-3 flex">
			<Button :disabled="importText.trim().length === 0" @click="importPasted">
				{{ formatMessage(messages.importAction) }}
			</Button>
		</div>
	</section>

	<ConfirmModal
		ref="deleteModal"
		:title="formatMessage(messages.deleteTitle, { name: pendingDelete?.name ?? '' })"
		:description="formatMessage(messages.deleteDescription)"
		:proceed-label="formatMessage(messages.delete)"
		@proceed="remove"
	/>
</template>
