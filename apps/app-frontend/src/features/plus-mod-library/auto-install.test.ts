import { describe, expect, it, vi } from 'vitest'

import { holdModLibrary, useModLibraryAutoInstall } from './auto-install'

const mocks = vi.hoisted(() => ({
	install: vi.fn(async (_instanceId: string) => ({ installed: [], skipped: [] })),
	onJob: null as null | ((job: unknown) => Promise<void>),
}))

vi.mock('@modrinth/ui', () => ({
	defineMessages: (messages: unknown) => messages,
	useVIntl: () => ({ formatMessage: () => '' }),
}))
vi.mock('@/helpers/plus', () => ({
	plus_mod_library_install: (instanceId: string) => mocks.install(instanceId),
}))
vi.mock('@/composables/use-app-event', () => ({
	useAppEvent: (_type: string, handler: (job: unknown) => Promise<void>) => {
		mocks.onJob = handler
	},
}))

const created = (instanceId: string, jobId: string) => ({
	kind: 'create_instance',
	status: 'succeeded',
	instance_id: instanceId,
	job_id: jobId,
	target: {},
})

describe('the mod library in new instances', () => {
	it('waits while an instance is held, then installs once', async () => {
		useModLibraryAutoInstall({} as never, () => {})
		const release = holdModLibrary('from-a-mod-page')

		await mocks.onJob?.(created('from-a-mod-page', 'job-1'))
		expect(mocks.install).not.toHaveBeenCalled()

		release()
		expect(mocks.install).toHaveBeenCalledWith('from-a-mod-page')

		release()
		await mocks.onJob?.(created('from-create-instance', 'job-2'))
		expect(mocks.install).toHaveBeenCalledWith('from-create-instance')
		expect(mocks.install).toHaveBeenCalledTimes(2)
	})
})
