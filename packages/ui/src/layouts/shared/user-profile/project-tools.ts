export type ProjectSort = 'downloads' | 'followers' | 'updated' | 'name'

export interface ToolProject {
	name: string
	summary?: string
	downloads?: number
	followers?: number
	updated?: string
	categories?: string[]
	additional_categories?: string[]
	loaders?: string[]
	game_versions?: string[]
}

export interface ProjectToolsState {
	search: string
	sort: ProjectSort
	category: string | null
	gameVersion: string | null
	loader: string | null
	page: number
}

export const DEFAULT_PROJECT_TOOLS: ProjectToolsState = {
	search: '',
	sort: 'downloads',
	category: null,
	gameVersion: null,
	loader: null,
	page: 1,
}

export const PROJECTS_PER_PAGE = 20

function categoriesOf(project: ToolProject): string[] {
	return [...(project.categories ?? []), ...(project.additional_categories ?? [])]
}

export function filterProjects<T extends ToolProject>(
	projects: readonly T[],
	state: ProjectToolsState,
): T[] {
	const search = state.search.trim().toLowerCase()
	const matching = projects.filter((project) => {
		if (
			search &&
			!project.name.toLowerCase().includes(search) &&
			!project.summary?.toLowerCase().includes(search)
		) {
			return false
		}
		if (state.category && !categoriesOf(project).includes(state.category)) return false
		if (state.loader && !project.loaders?.includes(state.loader)) return false
		if (state.gameVersion && !project.game_versions?.includes(state.gameVersion)) return false
		return true
	})
	return matching.sort((a, b) => {
		switch (state.sort) {
			case 'followers':
				return (b.followers ?? 0) - (a.followers ?? 0)
			case 'updated':
				return (b.updated ?? '').localeCompare(a.updated ?? '')
			case 'name':
				return a.name.localeCompare(b.name)
			default:
				return (b.downloads ?? 0) - (a.downloads ?? 0)
		}
	})
}

export function pageOf<T>(projects: readonly T[], page: number): T[] {
	const start = (page - 1) * PROJECTS_PER_PAGE
	return projects.slice(start, start + PROJECTS_PER_PAGE)
}

export function pageCountOf(total: number): number {
	return Math.max(1, Math.ceil(total / PROJECTS_PER_PAGE))
}

/** Only what these projects actually have, so no filter can come up empty by itself. */
export function projectFilterOptions(projects: readonly ToolProject[]) {
	const unique = (values: (string | undefined)[]) => [...new Set(values.filter(Boolean) as string[])]
	return {
		categories: unique(projects.flatMap(categoriesOf)).sort((a, b) => a.localeCompare(b)),
		loaders: unique(projects.flatMap((project) => project.loaders ?? [])).sort((a, b) =>
			a.localeCompare(b),
		),
		gameVersions: unique(projects.flatMap((project) => project.game_versions ?? [])).sort((a, b) =>
			b.localeCompare(a, undefined, { numeric: true }),
		),
	}
}

const rememberedStates = new Map<string, ProjectToolsState>()

/** Kept for this session, so coming back from a project restores the search and filters. */
export function rememberProjectTools(key: string, state: ProjectToolsState): void {
	rememberedStates.set(key, { ...state })
}

export function rememberedProjectTools(key: string): ProjectToolsState {
	return { ...DEFAULT_PROJECT_TOOLS, ...rememberedStates.get(key) }
}
