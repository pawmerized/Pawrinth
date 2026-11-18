import { describe, expect, it } from 'vitest'

import {
	CREATION_FIXED_VERSION_LOADERS,
	CREATION_LOADERS,
	type HorizonStatus,
	horizonView,
	isHorizonCandidate,
	playedLoader,
} from './horizon'

describe('the loader an instance plays with', () => {
	const vanilla = { id: 'a', loader: 'vanilla', game_version: '26.3' }

	it('is Horizon only for a vanilla 26.3 instance with Horizon turned on', () => {
		expect(playedLoader(vanilla, { a: true })).toBe('horizon')
		expect(playedLoader(vanilla, { a: false })).toBe('vanilla')
		expect(playedLoader(vanilla, undefined)).toBe('vanilla')
		expect(playedLoader({ ...vanilla, game_version: '1.21.1' }, { a: true })).toBe('vanilla')
		expect(playedLoader({ ...vanilla, loader: 'fabric' }, { a: true })).toBe('fabric')
	})
})

const ready: HorizonStatus = {
	supported: true,
	version: 'Nightly 2026-11-13 · 9f42064',
	ready: true,
	setting_up: null,
	enabled: false,
	eligible: true,
}

describe('the Horizon tab', () => {
	it('can be turned on before setup, and still asks for it', () => {
		const view = horizonView({ ...ready, version: null, ready: false })
		expect(view.note).toBe('not-set-up')
		expect(view.canToggle).toBe(true)
		expect(view.canSetUp).toBe(true)
		expect(view.setUpIsUpdate).toBe(false)
		expect(view.canRemove).toBe(false)
	})

	it('offers updates once Horizon is ready', () => {
		const view = horizonView(ready)
		expect(view.note).toBeNull()
		expect(view.canToggle).toBe(true)
		expect(view.setUpIsUpdate).toBe(true)
		expect(view.canRemove).toBe(true)
	})

	it('still lets Horizon be turned off when it needs preparing again', () => {
		const view = horizonView({ ...ready, ready: false, enabled: true })
		expect(view.note).toBe('needs-setup')
		expect(view.canToggle).toBe(true)
		expect(view.setUpIsUpdate).toBe(false)
	})

	it('locks everything while a setup runs', () => {
		const view = horizonView({ ...ready, setting_up: { message: 'Downloading', fraction: 0.1 } })
		expect(view).toMatchObject({ canToggle: false, canSetUp: false, canRemove: false })
	})

	it('explains why it isn’t available', () => {
		expect(horizonView({ ...ready, supported: false }).note).toBe('unsupported')
		expect(horizonView({ ...ready, eligible: false }).note).toBe('ineligible')
		expect(horizonView({ ...ready, eligible: false }).canSetUp).toBe(false)
	})

	it('is a Create instance choice locked to 26.3', () => {
		expect(CREATION_LOADERS.at(-1)).toBe('horizon')
		expect(CREATION_FIXED_VERSION_LOADERS).toEqual({ horizon: '26.3' })
	})

	it('is only offered for vanilla 26.3', () => {
		expect(isHorizonCandidate({ loader: 'vanilla', game_version: '26.3' })).toBe(true)
		expect(isHorizonCandidate({ loader: 'fabric', game_version: '26.3' })).toBe(false)
		expect(isHorizonCandidate({ loader: 'vanilla', game_version: '1.21.11' })).toBe(false)
	})
})
