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
	technic: { id: 'plus.provider.technic', defaultMessage: 'Technic' },
})

/** Technic only has modpacks, so its tab shows next to modpacks and nothing else. */
const links = computed(() => {
	const onTechnic = route.path.startsWith('/technic')
	const onCurseforge = route.path.startsWith('/curseforge')
	const instance =
		typeof route.query.i === 'string' ? `?i=${encodeURIComponent(route.query.i)}` : ''
	const modpacks =
		onTechnic ||
		(onCurseforge ? route.params.classSlug === 'modpacks' : route.params.projectType === 'modpack')
	const modrinthPath = onTechnic
		? '/browse/modpack'
		: onCurseforge
			? modrinthPathFor(String(route.params.classSlug ?? ''))
			: route.path
	const curseforgePath = onTechnic
		? curseforgePathFor('modpack')
		: onCurseforge
			? route.path
			: curseforgePathFor(String(route.params.projectType ?? ''))
	const tabs = [
		{ label: formatMessage(messages.modrinth), href: `${modrinthPath}${instance}` },
		{ label: formatMessage(messages.curseforge), href: `${curseforgePath}${instance}` },
	]
	if (modpacks && !instance) {
		tabs.push({
			label: formatMessage(messages.technic),
			href: onTechnic ? route.fullPath : '/technic',
		})
	}
	return tabs
})
</script>

<template>
	<NavTabs :links="links" replace />
</template>
