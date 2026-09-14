import type { Mclogs } from '@modrinth/api-client'
import { defineMessages, useVIntl } from '@modrinth/ui'

import type { Finding, FindingKind } from '@/helpers/plus'

const messages = defineMessages({
	confirmed: { id: 'plus.diagnostics.confirmed', defaultMessage: 'The log says: {detail}' },
	possible: { id: 'plus.diagnostics.possible', defaultMessage: 'Possible cause: {detail}' },
	missing_dependency: {
		id: 'plus.diagnostics.missing-dependency',
		defaultMessage: 'A required mod is missing{subject, select, none {} other { ({subject})}}.',
	},
	missing_dependency_fix: {
		id: 'plus.diagnostics.missing-dependency.fix',
		defaultMessage: 'Install the missing mod for this game version and loader.',
	},
	version_mismatch: {
		id: 'plus.diagnostics.version-mismatch',
		defaultMessage:
			'A mod needs a different version of another mod{subject, select, none {} other { ({subject})}}.',
	},
	version_mismatch_fix: {
		id: 'plus.diagnostics.version-mismatch.fix',
		defaultMessage: 'Update or change the version named in the log line below.',
	},
	wrong_loader: {
		id: 'plus.diagnostics.wrong-loader',
		defaultMessage: 'A mod was made for a different mod loader.',
	},
	wrong_loader_fix: {
		id: 'plus.diagnostics.wrong-loader.fix',
		defaultMessage: 'Replace it with a version made for this instance’s loader.',
	},
	java_version: {
		id: 'plus.diagnostics.java-version',
		defaultMessage: 'The game or a mod needs a different Java version.',
	},
	java_version_fix: {
		id: 'plus.diagnostics.java-version.fix',
		defaultMessage: 'Choose another Java version in this instance’s Java settings.',
	},
	out_of_memory: {
		id: 'plus.diagnostics.out-of-memory',
		defaultMessage: 'The game ran out of memory.',
	},
	out_of_memory_fix: {
		id: 'plus.diagnostics.out-of-memory.fix',
		defaultMessage: 'Increase the memory for this instance in its Java settings.',
	},
	mod_conflict: {
		id: 'plus.diagnostics.mod-conflict',
		defaultMessage:
			'Installed mods conflict with each other{subject, select, none {} other { ({subject})}}.',
	},
	mod_conflict_fix: {
		id: 'plus.diagnostics.mod-conflict.fix',
		defaultMessage: 'Disable one of the mods named in the log line below.',
	},
	mixin_failure: {
		id: 'plus.diagnostics.mixin-failure',
		defaultMessage:
			'A mod failed to patch the game{subject, select, none {} other { ({subject})}}.',
	},
	mixin_failure_fix: {
		id: 'plus.diagnostics.mixin-failure.fix',
		defaultMessage:
			'This is often a mod conflict or version mismatch, but the log alone can’t confirm it.',
	},
	corrupt_config: {
		id: 'plus.diagnostics.corrupt-config',
		defaultMessage: 'A config file may be damaged.',
	},
	corrupt_config_fix: {
		id: 'plus.diagnostics.corrupt-config.fix',
		defaultMessage: 'Check the config file named in the log line below.',
	},
	related: {
		id: 'plus.diagnostics.related',
		defaultMessage: 'Installed files that match: {files}',
	},
	evidence: { id: 'plus.diagnostics.evidence', defaultMessage: 'Log line: {line}' },
})

const fixes: Record<FindingKind, (typeof messages)[keyof typeof messages]> = {
	missing_dependency: messages.missing_dependency_fix,
	version_mismatch: messages.version_mismatch_fix,
	wrong_loader: messages.wrong_loader_fix,
	java_version: messages.java_version_fix,
	out_of_memory: messages.out_of_memory_fix,
	mod_conflict: messages.mod_conflict_fix,
	mixin_failure: messages.mixin_failure_fix,
	corrupt_config: messages.corrupt_config_fix,
}

/** Fits local findings into the crash panel the logs page already has. */
export function useCrashFindings() {
	const { formatMessage } = useVIntl()

	function findingsToAnalysis(findings: Finding[]): Mclogs.Insights.v1.InsightsResponse {
		return {
			id: 'local',
			name: 'local',
			type: 'local',
			version: '1',
			title: 'local',
			analysis: {
				information: [],
				problems: findings.map((finding) => {
					const detail = formatMessage(messages[finding.kind], {
						subject: finding.subject ?? 'none',
					})
					const solutions = [
						formatMessage(fixes[finding.kind]),
						formatMessage(messages.evidence, { line: finding.evidence }),
					]
					if (finding.related_files.length > 0) {
						solutions.push(
							formatMessage(messages.related, { files: finding.related_files.join(', ') }),
						)
					}
					return {
						message: formatMessage(
							finding.confidence === 'confirmed' ? messages.confirmed : messages.possible,
							{ detail },
						),
						counter: 1,
						entry: { level: 0, time: null, prefix: '', lines: [] },
						solutions: solutions.map((message) => ({ message })),
					}
				}),
			},
		}
	}

	return { findingsToAnalysis }
}
