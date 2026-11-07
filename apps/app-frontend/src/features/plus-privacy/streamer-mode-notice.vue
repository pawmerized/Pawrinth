<script setup lang="ts">
import { EyeIcon } from '@modrinth/assets'
import { Admonition, Button, defineMessages, useVIntl } from '@modrinth/ui'

import { patchPlusSettings } from '@/features/plus-settings/use-plus-settings'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	header: { id: 'plus.privacy.notice.header', defaultMessage: 'Hidden by streamer mode' },
	text: {
		id: 'plus.privacy.notice.text',
		defaultMessage:
			'Your account details are kept off the screen while streamer mode is on. Turn it off to see or change them.',
	},
	turnOff: { id: 'plus.privacy.streamer.turn-off', defaultMessage: 'Turn off streamer mode' },
})
</script>

<template>
	<Admonition type="info" :header="formatMessage(messages.header)">
		{{ formatMessage(messages.text) }}
		<template #actions>
			<Button @click="patchPlusSettings({ privacy: { streamer_mode: false } })">
				<EyeIcon aria-hidden="true" />
				{{ formatMessage(messages.turnOff) }}
			</Button>
		</template>
	</Admonition>
</template>
