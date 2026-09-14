import { beforeEach, describe, expect, it } from 'vitest'
import { nextTick, ref } from 'vue'

import {
	forgetRememberedOverrides,
	isSyncEnabled,
	nextOverrides,
	useInstanceFilterSync,
} from './instance-filter-sync'
import { readInstanceTarget } from './instance-target'
import { preferStableVersion } from './version-choice'

describe('preferring a version', () => {
	const beta = { id: 'beta', version_type: 'beta' }
	const alpha = { id: 'alpha', version_type: 'alpha' }
	const newest = { id: 'newest', version_type: 'release' }
	const older = { id: 'older', version_type: 'release' }

	it('takes a stable release over a newer beta or alpha', () => {
		expect(preferStableVersion([alpha, beta, newest, older])?.id).toBe('newest')
	})

	it('falls back to a beta, then an alpha, only when it has to', () => {
		expect(preferStableVersion([alpha, beta])?.id).toBe('beta')
		expect(preferStableVersion([alpha])?.id).toBe('alpha')
	})

	it('has nothing to offer from an empty list', () => {
		expect(preferStableVersion([])).toBeUndefined()
	})
})

describe('reading what an instance runs', () => {
	it.each(['fabric', 'forge', 'neoforge', 'quilt'])('recognizes %s', (loader) => {
		expect(readInstanceTarget({ game_version: '1.20.1', loader })).toEqual({
			gameVersion: '1.20.1',
			loader,
			status: 'ready',
		})
	})

	it('knows when there is no mod loader', () => {
		expect(readInstanceTarget({ game_version: '1.20.1', loader: 'vanilla' })).toEqual({
			gameVersion: '1.20.1',
			loader: null,
			status: 'no-loader',
		})
	})

	it('reports an unrecognized loader instead of guessing', () => {
		expect(readInstanceTarget({ game_version: '1.20.1', loader: 'mystery' }).status).toBe(
			'unknown-loader',
		)
		expect(readInstanceTarget({ game_version: '1.20.1', loader: 'mystery' }).loader).toBeNull()
	})

	it('reports missing metadata instead of guessing', () => {
		expect(readInstanceTarget({ game_version: '1.20.1' }).status).toBe('unknown-loader')
		expect(readInstanceTarget({ game_version: '1.20.1', loader: 42 }).status).toBe('unknown-loader')
		expect(readInstanceTarget({ game_version: '  ', loader: 'forge' }).status).toBe(
			'unknown-version',
		)
		expect(readInstanceTarget(null).status).toBe('unknown-loader')
	})

	it('ignores differences in case and spacing', () => {
		expect(readInstanceTarget({ game_version: ' 1.21 ', loader: ' Fabric ' })).toEqual({
			gameVersion: '1.21',
			loader: 'fabric',
			status: 'ready',
		})
	})
})

describe('syncing filters with an instance', () => {
	beforeEach(forgetRememberedOverrides)

	it('is on until a synced filter is unlocked', () => {
		expect(isSyncEnabled(['game_version', 'mod_loader'], [])).toBe(true)
		expect(isSyncEnabled(['game_version', 'mod_loader'], ['mod_loader'])).toBe(false)
	})

	it('ignores filters that have nothing to do with the instance', () => {
		expect(isSyncEnabled(['game_version', 'project_id'], ['project_id'])).toBe(true)
	})

	it('turning off unlocks everything and turning on restores it, leaving others alone', () => {
		const off = nextOverrides(['project_id'], false)
		expect(off).toContain('game_version')
		expect(off).toContain('mod_loader')
		expect(off).toContain('project_id')
		expect(nextOverrides(off, true)).toEqual(['project_id'])
	})

	it('stays off when coming back to the same instance', async () => {
		const first = ref<string[]>([])
		const sync = useInstanceFilterSync(ref('a'), first, ref(['game_version']))
		sync.set(false)
		await nextTick()
		expect(sync.enabled.value).toBe(false)

		const returning = ref<string[]>([])
		const again = useInstanceFilterSync(ref('a'), returning, ref(['game_version']))
		expect(again.enabled.value).toBe(false)
	})

	it('is remembered per instance when switching between them', async () => {
		const instance = ref('a')
		const overridden = ref<string[]>([])
		const sync = useInstanceFilterSync(instance, overridden, ref(['game_version']))
		sync.set(false)
		await nextTick()

		instance.value = 'b'
		await nextTick()
		expect(sync.enabled.value).toBe(true)

		instance.value = 'a'
		await nextTick()
		expect(sync.enabled.value).toBe(false)
	})

	it('can be turned back on', async () => {
		const sync = useInstanceFilterSync(ref('a'), ref<string[]>([]), ref(['game_version']))
		sync.set(false)
		await nextTick()
		sync.set(true)
		expect(sync.enabled.value).toBe(true)
	})
})
