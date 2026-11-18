<script setup lang="ts">
import { ExternalIcon } from '@modrinth/assets'
import { Button, defineMessages, useVIntl } from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'

const DOCUMENTS = 'https://github.com/pawmerized/Pawrinth/blob/main'

const { formatMessage } = useVIntl()

const messages = defineMessages({
	privacy: { id: 'plus.about.privacy', defaultMessage: 'Privacy policy' },
	terms: { id: 'plus.about.terms', defaultMessage: 'Terms of Service' },
})
</script>

<template>
	<Button native-type="button" @click="openUrl(`${DOCUMENTS}/PRIVACY.md`)">
		{{ formatMessage(messages.privacy) }}
		<ExternalIcon aria-hidden="true" />
	</Button>
	<Button native-type="button" @click="openUrl(`${DOCUMENTS}/TERMS.md`)">
		{{ formatMessage(messages.terms) }}
		<ExternalIcon aria-hidden="true" />
	</Button>
</template>
