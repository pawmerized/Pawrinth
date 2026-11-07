import { computed } from 'vue'

import { usePlusSettings } from '@/features/plus-settings/use-plus-settings'

import {
	DEFAULT_PRIVACY,
	namesAreReplaced,
	type PrivacySettings,
	redactLogLine,
	shouldBlurNames,
	shownName,
} from './privacy-model'

const { settings } = usePlusSettings()
const privacy = computed<PrivacySettings>(() => settings.value?.privacy ?? DEFAULT_PRIVACY)
const streamerMode = computed(() => privacy.value.streamer_mode)

/** Every account name this app knows about, so text from elsewhere can have them taken out. */
const privateNames = new Set<string>()

export function rememberPrivateNames(names: readonly (string | null | undefined)[]): void {
	for (const name of names) {
		if (name) privateNames.add(name)
	}
}

/** Takes names, home folders and addresses out of text from errors, logs and notifications. */
export function redactForStream(text: string): string {
	return streamerMode.value ? redactLogLine(text, [...privateNames]) : text
}

export function usePrivacy() {
	return {
		privacy,
		streamerMode,
		namesReplaced: computed(() => namesAreReplaced(privacy.value)),
		blurNames: computed(() => shouldBlurNames(privacy.value)),
		shownName: (real: string, position = 0) => shownName(real, privacy.value, position),
		redactForStream,
	}
}
