<script setup lang="ts">
import { InfoIcon, TriangleAlertIcon, XCircleIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'

import { loaderName } from '@/features/plus-install/loader-names'

import {
	applyPreset,
	type ArgContext,
	type ArgIssue,
	type ArgIssueKind,
	checkArgs,
	isBlocked,
	optionName,
	type PresetId,
	splitArgs,
	suggestedPresets,
	usesPreset,
} from './java-args'

const props = defineProps<{
	args: string
	context: ArgContext
	/** Whether the arguments shown are this instance's own, which are the ones saved. */
	own: boolean
}>()

const emit = defineEmits<{ update: [args: string] }>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
	notSaved: {
		id: 'plus.java-args.not-saved',
		defaultMessage:
			'These arguments aren’t saved until the problems marked in red are fixed, so the game keeps starting with the last ones that work.',
	},
	suggestions: { id: 'plus.java-args.suggestions', defaultMessage: 'Suggestions' },
	suggestionsFor: {
		id: 'plus.java-args.suggestions-for',
		defaultMessage: 'Suggestions for Java {version}',
	},
	suggestionsNote: {
		id: 'plus.java-args.suggestions-note',
		defaultMessage:
			'Only settings that suit this instance’s Java and memory are suggested. Using one replaces the garbage collector settings above and keeps everything else.',
	},
	useThese: { id: 'plus.java-args.use', defaultMessage: 'Use these' },
	inUse: { id: 'plus.java-args.in-use', defaultMessage: 'In use' },
	whatTheyDo: { id: 'plus.java-args.what-they-do', defaultMessage: 'What these do' },
	remove: { id: 'plus.java-args.remove', defaultMessage: 'Remove' },
	removeEarlier: { id: 'plus.java-args.remove-earlier', defaultMessage: 'Remove the earlier one' },
	fix: { id: 'plus.java-args.fix', defaultMessage: 'Fix' },
	unknownJava: { id: 'plus.java-args.unknown-java', defaultMessage: 'this Java' },
})

const issueMessages = defineMessages({
	'not-an-option': {
		id: 'plus.java-args.issue.not-an-option',
		defaultMessage:
			'This isn’t a Java option. Everything here goes to Java before the game, so the game won’t start with it.',
	},
	quoted: {
		id: 'plus.java-args.issue.quoted',
		defaultMessage:
			'Quotes are passed to Java as they are. Pawrinth splits arguments at spaces, so leave quotes out.',
	},
	'memory-flag': {
		id: 'plus.java-args.issue.memory-flag',
		defaultMessage:
			'This overrides the memory set above. Use the memory setting instead, so Pawrinth knows how much the game has.',
	},
	'heap-too-large': {
		id: 'plus.java-args.issue.heap-too-large',
		defaultMessage:
			'The starting memory is more than the most the game may use, so Java won’t start.',
	},
	duplicate: {
		id: 'plus.java-args.issue.duplicate',
		defaultMessage: 'This option is set more than once. Only the last one counts.',
	},
	'many-collectors': {
		id: 'plus.java-args.issue.many-collectors',
		defaultMessage: 'More than one garbage collector is chosen. Java refuses to start with that.',
	},
	'needs-java': {
		id: 'plus.java-args.issue.needs-java',
		defaultMessage:
			'This needs Java {detail} or newer. This instance uses Java {java}, which won’t start with it.',
	},
	experimental: {
		id: 'plus.java-args.issue.experimental',
		defaultMessage:
			'Java only accepts this after -XX:+UnlockExperimentalVMOptions. Without it, Java won’t start.',
	},
	removed: {
		id: 'plus.java-args.issue.removed',
		defaultMessage:
			'Java {detail} removed this option. Java {java} ignores it at best and may refuse to start.',
	},
	'default-already': {
		id: 'plus.java-args.issue.default-already',
		defaultMessage: 'Java {detail} and newer already do this, so it can go.',
	},
	'other-collector': {
		id: 'plus.java-args.issue.other-collector',
		defaultMessage:
			'This only tunes the G1 garbage collector, and {detail} is chosen instead, so it does nothing.',
	},
	'some-builds': {
		id: 'plus.java-args.issue.some-builds',
		defaultMessage:
			'Only some Java builds include Shenandoah, and Oracle’s don’t. If the game doesn’t start, remove this.',
	},
	'wrong-loader': {
		id: 'plus.java-args.issue.wrong-loader',
		defaultMessage:
			'Only {detail} reads this, and this instance uses {loader}, so it does nothing.',
	},
	debug: {
		id: 'plus.java-args.issue.debug',
		defaultMessage:
			'Mixin debugging makes the game start slower and writes debug files. It’s meant for mod developers.',
	},
	agent: {
		id: 'plus.java-args.issue.agent',
		defaultMessage: 'A Java agent runs its own code inside the game. Only use agents you trust.',
	},
	'large-pages': {
		id: 'plus.java-args.issue.large-pages',
		defaultMessage:
			'On Windows this needs the “Lock pages in memory” permission, and Java ignores it otherwise.',
	},
	'pre-touch': {
		id: 'plus.java-args.issue.pre-touch',
		defaultMessage:
			'Java claims all of its memory as the game starts, so starting takes longer and other apps get less.',
	},
} satisfies Record<ArgIssueKind, unknown>)

const presetMessages = defineMessages({
	'mojang-g1': {
		id: 'plus.java-args.preset.mojang-g1',
		defaultMessage: 'Minecraft launcher settings',
	},
	'mojang-g1-text': {
		id: 'plus.java-args.preset.mojang-g1.text',
		defaultMessage:
			'What Minecraft’s own launcher uses: the G1 garbage collector, tuned for short pauses. Works on every Java version Minecraft runs on.',
	},
	'generational-zgc': {
		id: 'plus.java-args.preset.generational-zgc',
		defaultMessage: 'Generational ZGC',
	},
	'generational-zgc-text': {
		id: 'plus.java-args.preset.generational-zgc.text',
		defaultMessage:
			'Cleans up memory while the game keeps running, with pauses under a millisecond, so there are fewer stutters. It uses more memory and some extra CPU.',
	},
})

const explanations = defineMessages({
	UnlockExperimentalVMOptions: {
		id: 'plus.java-args.explain.unlock',
		defaultMessage: 'Lets Java accept the experimental G1 options that follow.',
	},
	UseG1GC: {
		id: 'plus.java-args.explain.g1',
		defaultMessage: 'Uses the G1 garbage collector, which cleans up memory in small steps.',
	},
	G1NewSizePercent: {
		id: 'plus.java-args.explain.g1-new-size',
		defaultMessage: 'Keeps at least 20% of memory for new objects, which Minecraft makes many of.',
	},
	G1ReservePercent: {
		id: 'plus.java-args.explain.g1-reserve',
		defaultMessage: 'Keeps 20% free as a buffer, so memory doesn’t run out during cleanup.',
	},
	MaxGCPauseMillis: {
		id: 'plus.java-args.explain.pause',
		defaultMessage: 'Aims for cleanup pauses of 50 milliseconds or less.',
	},
	G1HeapRegionSize: {
		id: 'plus.java-args.explain.region',
		defaultMessage: 'Splits memory into 32 MB regions, so large chunks of world data fit in one.',
	},
	UseZGC: {
		id: 'plus.java-args.explain.zgc',
		defaultMessage: 'Uses ZGC, which cleans up memory while the game keeps running.',
	},
	ZGenerational: {
		id: 'plus.java-args.explain.generational',
		defaultMessage:
			'Turns on ZGC’s generational mode, which handles short-lived objects better. Java 23 and newer use it already.',
	},
})

const list = computed(() => splitArgs(props.args))
const issues = computed(() => checkArgs(list.value, props.context))
const blocked = computed(() => props.own && isBlocked(issues.value))
const presets = computed(() => suggestedPresets(props.context))

function issueText(issue: ArgIssue): string {
	return formatMessage(issueMessages[issue.kind], {
		detail: issue.detail ?? '',
		java: props.context.javaMajor ?? formatMessage(messages.unknownJava),
		loader: loaderName(props.context.loader),
	})
}

function fixLabel(issue: ArgIssue): string {
	if (issue.kind === 'experimental') return formatMessage(messages.fix)
	if (issue.kind === 'duplicate') return formatMessage(messages.removeEarlier)
	return formatMessage(messages.remove)
}

function explain(arg: string): string {
	const name = optionName(arg) as keyof typeof explanations | null
	return name && explanations[name] ? formatMessage(explanations[name]) : ''
}

function presetText(id: PresetId): string {
	return formatMessage(presetMessages[`${id}-text`])
}
</script>

<template>
	<div class="mt-3 flex flex-col gap-3">
		<ul v-if="issues.length > 0" class="m-0 flex list-none flex-col gap-2 p-0">
			<li
				v-for="(issue, index) in issues"
				:key="`${issue.kind}-${issue.arg}-${index}`"
				class="flex items-start gap-3 rounded-xl bg-surface-2 p-3"
			>
				<XCircleIcon
					v-if="issue.severity === 'blocking'"
					class="mt-0.5 size-5 shrink-0 text-red"
					aria-hidden="true"
				/>
				<TriangleAlertIcon
					v-else-if="issue.severity === 'warning'"
					class="mt-0.5 size-5 shrink-0 text-orange"
					aria-hidden="true"
				/>
				<InfoIcon v-else class="mt-0.5 size-5 shrink-0 text-secondary" aria-hidden="true" />
				<div class="flex min-w-0 flex-1 flex-col gap-1">
					<code class="w-fit break-all rounded bg-surface-4 px-1.5 py-0.5 text-sm text-contrast">
						{{ issue.arg }}
					</code>
					<span class="text-sm text-primary">{{ issueText(issue) }}</span>
				</div>
				<Button v-if="issue.fixed" class="shrink-0" @click="emit('update', issue.fixed.join(' '))">
					{{ fixLabel(issue) }}
				</Button>
			</li>
		</ul>
		<p v-if="blocked" class="m-0 text-sm font-semibold text-red">
			{{ formatMessage(messages.notSaved) }}
		</p>

		<div class="flex flex-col gap-2">
			<h3 class="m-0 text-base font-semibold text-contrast">
				{{
					context.javaMajor
						? formatMessage(messages.suggestionsFor, { version: context.javaMajor })
						: formatMessage(messages.suggestions)
				}}
			</h3>
			<p class="m-0 text-sm text-secondary">{{ formatMessage(messages.suggestionsNote) }}</p>
			<div
				v-for="preset in presets"
				:key="preset.id"
				class="flex flex-col gap-2 rounded-xl bg-surface-2 p-3"
				:data-preset="preset.id"
			>
				<div class="flex flex-wrap items-center justify-between gap-2">
					<span class="font-semibold text-contrast">
						{{ formatMessage(presetMessages[preset.id]) }}
					</span>
					<Button
						:disabled="own && usesPreset(list, preset)"
						@click="emit('update', applyPreset(list, preset).join(' '))"
					>
						{{
							formatMessage(own && usesPreset(list, preset) ? messages.inUse : messages.useThese)
						}}
					</Button>
				</div>
				<p class="m-0 text-sm text-secondary">{{ presetText(preset.id) }}</p>
				<details>
					<summary class="cursor-pointer text-sm font-semibold text-primary">
						{{ formatMessage(messages.whatTheyDo) }}
					</summary>
					<ul class="m-0 mt-2 flex list-none flex-col gap-1.5 p-0">
						<li v-for="arg in preset.args" :key="arg" class="flex flex-col text-sm">
							<code class="w-fit text-contrast">{{ arg }}</code>
							<span class="text-secondary">{{ explain(arg) }}</span>
						</li>
					</ul>
				</details>
			</div>
		</div>
	</div>
</template>
