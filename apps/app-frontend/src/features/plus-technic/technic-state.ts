import { ref } from 'vue'

/** The slug of the Technic pack being downloaded and converted, so only one runs at a time. */
export const technicPreparing = ref<string | null>(null)

export function technicPackPath(slug: string): string {
	return `/technic/pack/${encodeURIComponent(slug)}`
}
