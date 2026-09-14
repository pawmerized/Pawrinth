<script setup lang="ts">
import { ref, watch } from 'vue'
import { useRoute } from 'vue-router'

import BrowsePage from './browse-page.vue'

const route = useRoute()

/** A new creator is a new page, not a change to the one that's open. */
const pageKey = ref(String(route.params.authorId))
watch(
	() => route.params.authorId,
	(authorId) => {
		if (authorId) pageKey.value = String(authorId)
	},
)
</script>

<template>
	<BrowsePage :key="pageKey" />
</template>
