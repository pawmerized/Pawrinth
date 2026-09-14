import { computed, type ComputedRef, type Ref, watch } from 'vue'

/** The filters that describe what an instance runs, as opposed to any other filter. */
export const SYNCED_FILTER_TYPES = [
	'game_version',
	'mod_loader',
	'shader_loader',
	'plugin_loader',
	'environment',
]

/** Kept per instance for this session, so a choice isn't silently undone by moving around. */
const unlockedByInstance = new Map<string, string[]>()

export function isSyncEnabled(providedTypes: readonly string[], overridden: readonly string[]) {
	return !providedTypes.some(
		(type) => SYNCED_FILTER_TYPES.includes(type) && overridden.includes(type),
	)
}

export function hasSyncedFilter(providedTypes: readonly string[]): boolean {
	return providedTypes.some((type) => SYNCED_FILTER_TYPES.includes(type))
}

export function nextOverrides(overridden: readonly string[], enabled: boolean): string[] {
	const others = overridden.filter((type) => !SYNCED_FILTER_TYPES.includes(type))
	return enabled ? others : [...others, ...SYNCED_FILTER_TYPES]
}

export function rememberedOverrides(instanceId: string): string[] {
	return unlockedByInstance.get(instanceId) ?? []
}

export function forgetRememberedOverrides(): void {
	unlockedByInstance.clear()
}

export interface InstanceFilterSync {
	enabled: ComputedRef<boolean>
	set: (enabled: boolean) => void
}

export function useInstanceFilterSync(
	instanceId: Ref<string>,
	overridden: Ref<string[]>,
	providedTypes: Ref<string[]>,
): InstanceFilterSync {
	watch(
		instanceId,
		(id) => {
			if (id) overridden.value = rememberedOverrides(id)
		},
		{ immediate: true },
	)

	watch(
		overridden,
		(value) => {
			if (instanceId.value) unlockedByInstance.set(instanceId.value, [...value])
		},
		{ deep: true },
	)

	return {
		enabled: computed(() => isSyncEnabled(providedTypes.value, overridden.value)),
		set(enabled) {
			overridden.value = nextOverrides(overridden.value, enabled)
		},
	}
}
