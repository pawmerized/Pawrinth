<script setup lang="ts">
import { defineMessages, NavTabs, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import { curseforgePathFor, modrinthPathFor } from './provider-mapping'

const { formatMessage } = useVIntl()
const route = useRoute()

const messages = defineMessages({
	modrinth: { id: 'plus.provider.modrinth', defaultMessage: 'Modrinth' },
	curseforge: { id: 'plus.provider.curseforge', defaultMessage: 'CurseForge' },
})

const links = computed(() => {
	const onCurseforge = route.path.startsWith('/curseforge')
	const instance =
		typeof route.query.i === 'string' ? `?i=${encodeURIComponent(route.query.i)}` : ''
	return [
		{
			label: formatMessage(messages.modrinth),
			href: `${onCurseforge ? modrinthPathFor(String(route.params.classSlug ?? '')) : route.path}${instance}`,
		},
		{
			label: formatMessage(messages.curseforge),
			href: `${onCurseforge ? route.path : curseforgePathFor(String(route.params.projectType ?? ''))}${instance}`,
		},
	]
})
</script>

<template>
	<NavTabs :links="links" replace />
</template>
