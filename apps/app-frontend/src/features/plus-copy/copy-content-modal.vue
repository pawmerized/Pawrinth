<script setup lang="ts">
import { CopyIcon, LeftArrowIcon, SpinnerIcon } from '@modrinth/assets'
import {
	Avatar,
	Button,
	defineMessages,
	injectNotificationManager,
	NewModal,
	useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import { loaderName } from '@/features/plus-install/loader-names'
import { getInstanceIconUrl, list } from '@/helpers/instance'
import {
	type CopyItem,
	type CopyStatus,
	plus_copy_content,
	plus_copy_content_plan,
} from '@/helpers/plus'
import type { GameInstance } from '@/helpers/types'

const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()

const messages = defineMessages({
	title: { id: 'plus.copy.title', defaultMessage: 'Copy to another instance' },
	choose: {
		id: 'plus.copy.choose',
		defaultMessage:
			'Choose where to copy {count, plural, one {# item} other {# items}}. Pawrinth checks each one against that instance first.',
	},
	noOthers: {
		id: 'plus.copy.no-others',
		defaultMessage: 'There’s no other instance to copy to.',
	},
	checking: { id: 'plus.copy.checking', defaultMessage: 'Checking what fits…' },
	back: { id: 'plus.copy.back', defaultMessage: 'Choose another instance' },
	copy: {
		id: 'plus.copy.action',
		defaultMessage: 'Copy {count, plural, one {# item} other {# items}}',
	},
	nothing: {
		id: 'plus.copy.nothing',
		defaultMessage: 'Nothing here can be copied to {name}.',
	},
	copied: {
		id: 'plus.copy.copied',
		defaultMessage: 'Copied {count, plural, one {# item} other {# items}} to {name}',
	},
	failed: { id: 'plus.copy.failed', defaultMessage: 'Some items couldn’t be copied' },
	same_version: { id: 'plus.copy.status.same-version', defaultMessage: 'Same version fits' },
	other_version: {
		id: 'plus.copy.status.other-version',
		defaultMessage: 'Gets the version made for {target}',
	},
	copy_file: {
		id: 'plus.copy.status.copy-file',
		defaultMessage: 'Copied as it is',
	},
	already_installed: {
		id: 'plus.copy.status.already-installed',
		defaultMessage: 'Already there',
	},
	no_version: {
		id: 'plus.copy.status.no-version',
		defaultMessage: 'No version for {target}',
	},
	curseforge_unavailable: {
		id: 'plus.copy.status.curseforge-unavailable',
		defaultMessage: 'Needs CurseForge set up in API configuration',
	},
	older_game: {
		id: 'plus.copy.status.older-game',
		defaultMessage: 'Not copied: {target} is older, which can damage a world',
	},
	game_running: {
		id: 'plus.copy.status.game-running',
		defaultMessage: 'Close the game first, so the copy is complete',
	},
})

const COPYABLE = new Set<CopyStatus>(['same_version', 'other_version', 'copy_file'])

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const sourceId = ref('')
const paths = ref<string[]>([])
const instances = ref<GameInstance[]>([])
const target = ref<GameInstance | null>(null)
const planned = ref<CopyItem[] | null>(null)
const busy = ref(false)

const others = computed(() =>
	instances.value.filter((instance) => instance.id !== sourceId.value && !instance.quarantined),
)
const copyable = computed(() => planned.value?.filter((item) => COPYABLE.has(item.status)) ?? [])
const targetLabel = computed(() =>
	target.value ? `${loaderName(target.value.loader)} ${target.value.game_version}` : '',
)

async function show(instanceId: string, filePaths: string[]): Promise<void> {
	sourceId.value = instanceId
	paths.value = filePaths
	target.value = null
	planned.value = null
	modal.value?.show()
	instances.value = await list().catch(() => [])
}

async function choose(instance: GameInstance): Promise<void> {
	target.value = instance
	planned.value = null
	busy.value = true
	try {
		planned.value = await plus_copy_content_plan(sourceId.value, instance.id, paths.value)
	} catch (error) {
		handleError(error as Error)
		target.value = null
	} finally {
		busy.value = false
	}
}

async function copy(): Promise<void> {
	const destination = target.value
	if (!destination || copyable.value.length === 0) return
	busy.value = true
	try {
		const report = await plus_copy_content(
			sourceId.value,
			destination.id,
			copyable.value.map((item) => item.path),
		)
		if (report.copied.length > 0) {
			addNotification({
				type: 'success',
				title: formatMessage(messages.copied, {
					count: report.copied.length,
					name: destination.name,
				}),
			})
		}
		if (report.failed.length > 0) {
			addNotification({
				type: 'error',
				title: formatMessage(messages.failed),
				text: report.failed.map(([name, error]) => `${name}: ${error}`).join('\n'),
			})
		}
		modal.value?.hide()
	} catch (error) {
		handleError(error as Error)
	} finally {
		busy.value = false
	}
}

function statusText(item: CopyItem): string {
	return formatMessage(messages[item.status], { target: targetLabel.value })
}

defineExpose({ show })
</script>

<template>
	<NewModal ref="modal" :header="formatMessage(messages.title)" max-width="560px">
		<div class="flex flex-col gap-3">
			<template v-if="!target">
				<p class="m-0">{{ formatMessage(messages.choose, { count: paths.length }) }}</p>
				<p v-if="others.length === 0" class="m-0 text-secondary">
					{{ formatMessage(messages.noOthers) }}
				</p>
				<div v-else class="flex max-h-[50vh] flex-col gap-1 overflow-y-auto">
					<button
						v-for="instance in others"
						:key="instance.id"
						type="button"
						class="flex items-center gap-3 rounded-xl border-0 bg-surface-2 p-2 text-left hover:bg-surface-3 focus-visible:bg-surface-3"
						@click="choose(instance)"
					>
						<Avatar
							:src="getInstanceIconUrl(instance.icon_path)"
							size="2.25rem"
							:tint-by="instance.id"
							alt=""
							pad-transparent-corners
						/>
						<span class="flex min-w-0 flex-col">
							<span class="truncate font-semibold text-contrast">{{ instance.name }}</span>
							<span class="text-sm text-secondary">
								{{ loaderName(instance.loader) }} {{ instance.game_version }}
							</span>
						</span>
					</button>
				</div>
			</template>
			<template v-else>
				<p class="m-0 font-semibold text-contrast">{{ target.name }} · {{ targetLabel }}</p>
				<p v-if="!planned" class="m-0 flex items-center gap-2 text-secondary">
					<SpinnerIcon class="animate-spin" aria-hidden="true" />
					{{ formatMessage(messages.checking) }}
				</p>
				<ul v-else class="m-0 flex max-h-[50vh] list-none flex-col gap-1 overflow-y-auto p-0">
					<li
						v-for="item in planned"
						:key="item.path"
						class="flex items-center justify-between gap-3 rounded-xl bg-surface-2 px-3 py-2"
					>
						<span class="min-w-0 truncate text-contrast">{{ item.name }}</span>
						<span
							class="shrink-0 text-sm"
							:class="COPYABLE.has(item.status) ? 'text-brand' : 'text-secondary'"
						>
							{{ statusText(item) }}
						</span>
					</li>
				</ul>
				<p v-if="planned && copyable.length === 0" class="m-0 text-secondary">
					{{ formatMessage(messages.nothing, { name: target.name }) }}
				</p>
				<div class="flex flex-wrap justify-end gap-2">
					<Button :disabled="busy" @click="target = null">
						<LeftArrowIcon aria-hidden="true" />
						{{ formatMessage(messages.back) }}
					</Button>
					<Button
						v-if="copyable.length > 0"
						color="brand"
						type="colored"
						:disabled="busy"
						@click="copy"
					>
						<SpinnerIcon v-if="busy" class="animate-spin" aria-hidden="true" />
						<CopyIcon v-else aria-hidden="true" />
						{{ formatMessage(messages.copy, { count: copyable.length }) }}
					</Button>
				</div>
			</template>
		</div>
	</NewModal>
</template>
