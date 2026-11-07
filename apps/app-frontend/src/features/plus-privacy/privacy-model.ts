const MAX_DISPLAY_NAME_LENGTH = 24
const HIDDEN = '[hidden]'
export const HIDDEN_ACCOUNT_NAME = 'Hidden account'

export interface PrivacySettings {
	streamer_mode: boolean
	blur_usernames: boolean
	display_name: string | null
}

export const DEFAULT_PRIVACY: PrivacySettings = {
	streamer_mode: false,
	blur_usernames: false,
	display_name: null,
}

export function cleanDisplayName(value: string): string | null {
	const cleaned = value.trim().slice(0, MAX_DISPLAY_NAME_LENGTH)
	return cleaned.length > 0 ? cleaned : null
}

export function shouldBlurNames(privacy: PrivacySettings): boolean {
	return privacy.streamer_mode || privacy.blur_usernames
}

/** True when real names never reach the screen, so there's nothing left to blur. */
export function namesAreReplaced(privacy: PrivacySettings): boolean {
	return privacy.streamer_mode || !!privacy.display_name?.trim()
}

/**
 * What to show instead of a real name. Several accounts stay distinguishable, because each
 * gets its own number after the first. This only ever changes what's on screen.
 */
export function shownName(real: string, privacy: PrivacySettings, position = 0): string {
	const replacement =
		privacy.display_name?.trim() || (privacy.streamer_mode ? HIDDEN_ACCOUNT_NAME : '')
	if (!replacement) return real
	return position === 0 ? replacement : `${replacement} ${position + 1}`
}

function escapeForPattern(text: string): string {
	return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

/** Removes what a log line gives away on stream: account names, home folders and addresses. */
export function redactLogLine(line: string, names: readonly string[]): string {
	let result = line
		.replace(/([A-Za-z]:[\\/]+Users[\\/]+)[^\\/\s"']+/g, `$1${HIDDEN}`)
		.replace(/(\/(?:home|Users)\/)[^/\s"']+/g, `$1${HIDDEN}`)
		.replace(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g, HIDDEN)
	for (const name of names) {
		if (name.length < 2) continue
		result = result.replace(new RegExp(`\\b${escapeForPattern(name)}\\b`, 'gi'), HIDDEN)
	}
	return result
}
