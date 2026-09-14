<script setup lang="ts">
import { DiscordIcon, ExternalIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { getVersion } from '@tauri-apps/api/app'
import { openUrl } from '@tauri-apps/plugin-opener'
import { onMounted, ref } from 'vue'

const REPOSITORY_URL = 'https://github.com/pawmerized/Pawrinth'
const UPSTREAM_URL = 'https://github.com/modrinth/code'
const DISCORD_INVITE_URL = 'https://discord.gg/HDVG4TmuB7'

const { formatMessage } = useVIntl()
const version = ref<string | null>(null)

onMounted(async () => {
	version.value = await getVersion().catch(() => null)
})

const messages = defineMessages({
	title: { id: 'plus.about.title', defaultMessage: 'About Pawrinth' },
	intro: {
		id: 'plus.about.intro',
		defaultMessage:
			'Pawrinth is a Minecraft launcher by pawmerized, for Modrinth and CurseForge content alike. It updates itself from its GitHub releases.',
	},
	version: { id: 'plus.about.version', defaultMessage: 'Version {version}' },
	repository: { id: 'plus.about.repository', defaultMessage: 'Pawrinth on GitHub' },
	releases: { id: 'plus.about.releases', defaultMessage: 'Release notes' },
	discord: { id: 'plus.about.discord', defaultMessage: 'Discord server' },
	report: { id: 'plus.about.report', defaultMessage: 'Report a problem' },
	support: {
		id: 'plus.about.support',
		defaultMessage:
			"Report problems on Pawrinth's GitHub rather than to Modrinth, since they may come from Pawrinth's own changes.",
	},
	credits: { id: 'plus.about.credits', defaultMessage: 'Built on Modrinth App' },
	upstream: {
		id: 'plus.about.upstream',
		defaultMessage:
			"Pawrinth is built on the open-source code of Modrinth App by the Modrinth team and contributors, and like it is available under the GNU General Public License, version 3. Pawrinth isn't affiliated with or endorsed by Rinth, Inc.",
	},
	notIncluded: { id: 'plus.about.not-included', defaultMessage: 'Left out' },
	notIncludedList: {
		id: 'plus.about.not-included.list',
		defaultMessage:
			'Ads, error reporting to Sentry, Modrinth Hosting and its support chat, and playtime analytics.',
	},
	source: { id: 'plus.about.source', defaultMessage: "Modrinth App's source code" },
})
</script>

<template>
	<section>
		<h2 class="m-0 text-xl font-semibold text-contrast">{{ formatMessage(messages.title) }}</h2>
		<p class="m-0 mt-3">{{ formatMessage(messages.intro) }}</p>
		<p v-if="version" class="m-0 mt-2 font-semibold text-contrast">
			{{ formatMessage(messages.version, { version }) }}
		</p>
		<div class="mt-3 flex flex-wrap gap-2">
			<Button native-type="button" @click="openUrl(REPOSITORY_URL)">
				{{ formatMessage(messages.repository) }}
				<ExternalIcon aria-hidden="true" />
			</Button>
			<Button native-type="button" @click="openUrl(`${REPOSITORY_URL}/releases`)">
				{{ formatMessage(messages.releases) }}
				<ExternalIcon aria-hidden="true" />
			</Button>
			<Button native-type="button" @click="openUrl(DISCORD_INVITE_URL)">
				<DiscordIcon aria-hidden="true" />
				{{ formatMessage(messages.discord) }}
			</Button>
			<Button native-type="button" @click="openUrl(`${REPOSITORY_URL}/issues/new/choose`)">
				{{ formatMessage(messages.report) }}
				<ExternalIcon aria-hidden="true" />
			</Button>
		</div>
		<p class="m-0 mt-3 text-secondary">{{ formatMessage(messages.support) }}</p>

		<h3 class="m-0 mt-6 text-lg font-semibold text-contrast">
			{{ formatMessage(messages.credits) }}
		</h3>
		<p class="m-0 mt-1">{{ formatMessage(messages.upstream) }}</p>
		<Button class="mt-3" native-type="button" @click="openUrl(UPSTREAM_URL)">
			{{ formatMessage(messages.source) }}
			<ExternalIcon aria-hidden="true" />
		</Button>

		<h3 class="m-0 mt-6 text-lg font-semibold text-contrast">
			{{ formatMessage(messages.notIncluded) }}
		</h3>
		<p class="m-0 mt-1">{{ formatMessage(messages.notIncludedList) }}</p>
	</section>
</template>
