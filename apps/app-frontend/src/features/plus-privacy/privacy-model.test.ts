import { describe, expect, it } from 'vitest'

import {
	cleanDisplayName,
	DEFAULT_PRIVACY,
	HIDDEN_ACCOUNT_NAME,
	namesAreReplaced,
	redactLogLine,
	shouldBlurNames,
	shownName,
} from './privacy-model'

describe('blurring', () => {
	it('is off by default', () => {
		expect(shouldBlurNames(DEFAULT_PRIVACY)).toBe(false)
	})

	it('follows its own setting, and streamer mode always turns it on', () => {
		expect(shouldBlurNames({ ...DEFAULT_PRIVACY, blur_usernames: true })).toBe(true)
		expect(shouldBlurNames({ ...DEFAULT_PRIVACY, streamer_mode: true })).toBe(true)
	})
})

describe('the name shown on screen', () => {
	it('is the real name until one is chosen', () => {
		expect(shownName('RealName', DEFAULT_PRIVACY)).toBe('RealName')
		expect(shownName('RealName', { ...DEFAULT_PRIVACY, display_name: '   ' })).toBe('RealName')
	})

	it('uses the chosen name', () => {
		expect(shownName('RealName', { ...DEFAULT_PRIVACY, display_name: 'Paw' })).toBe('Paw')
	})

	it('keeps several accounts apart', () => {
		const privacy = { ...DEFAULT_PRIVACY, display_name: 'Paw' }
		expect(shownName('First', privacy, 0)).toBe('Paw')
		expect(shownName('Second', privacy, 1)).toBe('Paw 2')
	})

	it('is hidden in streamer mode', () => {
		const privacy = { ...DEFAULT_PRIVACY, streamer_mode: true }
		expect(shownName('RealName', privacy)).toBe(HIDDEN_ACCOUNT_NAME)
		expect(shownName('Other', privacy, 1)).toBe(`${HIDDEN_ACCOUNT_NAME} 2`)
	})

	it('prefers the chosen name over the neutral one', () => {
		expect(
			shownName('RealName', { ...DEFAULT_PRIVACY, streamer_mode: true, display_name: 'Paw' }),
		).toBe('Paw')
	})

	it('comes back when streamer mode ends', () => {
		expect(shownName('RealName', { ...DEFAULT_PRIVACY, streamer_mode: false })).toBe('RealName')
	})

	it('knows when the real name never shows', () => {
		expect(namesAreReplaced(DEFAULT_PRIVACY)).toBe(false)
		expect(namesAreReplaced({ ...DEFAULT_PRIVACY, blur_usernames: true })).toBe(false)
		expect(namesAreReplaced({ ...DEFAULT_PRIVACY, streamer_mode: true })).toBe(true)
		expect(namesAreReplaced({ ...DEFAULT_PRIVACY, display_name: 'Paw' })).toBe(true)
	})

	it('cleans what gets saved', () => {
		expect(cleanDisplayName('  Paw  ')).toBe('Paw')
		expect(cleanDisplayName('   ')).toBeNull()
		expect(cleanDisplayName('x'.repeat(40))).toHaveLength(24)
	})
})

describe('redacting a log line', () => {
	it('removes account names, whatever the case', () => {
		expect(redactLogLine('Setting user: RealName', ['RealName'])).toBe('Setting user: [hidden]')
		expect(redactLogLine('realname joined', ['RealName'])).toBe('[hidden] joined')
	})

	it('does not touch other words that merely contain the name', () => {
		expect(redactLogLine('Paws are great', ['Paw'])).toBe('Paws are great')
	})

	it('copes with names that have special characters', () => {
		expect(redactLogLine('hello a.b+c', ['a.b+c'])).toContain('[hidden]')
	})

	it('removes the home folder from Windows, Linux and Mac paths', () => {
		expect(redactLogLine('C:\\Users\\someone\\AppData\\x.log', [])).toBe(
			'C:\\Users\\[hidden]\\AppData\\x.log',
		)
		expect(redactLogLine('/home/someone/.minecraft', [])).toBe('/home/[hidden]/.minecraft')
		expect(redactLogLine('/Users/someone/Library', [])).toBe('/Users/[hidden]/Library')
	})

	it('removes IP addresses', () => {
		expect(redactLogLine('Connecting to 192.168.1.50, 25565', [])).toBe(
			'Connecting to [hidden], 25565',
		)
	})

	it('leaves everything else alone', () => {
		expect(redactLogLine('[Render thread/INFO]: Loaded 7 mods', [])).toBe(
			'[Render thread/INFO]: Loaded 7 mods',
		)
	})

	it('ignores an empty or one-letter name', () => {
		expect(redactLogLine('a b c', ['', 'a'])).toBe('a b c')
	})
})
