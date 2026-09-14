export interface TagFilterOption {
	id: string
	value?: string
	formatted_name?: string
}

export interface TagFilterType {
	id: string
	display?: string
	options: TagFilterOption[]
}

export interface TagFilterValue {
	type: string
	option: string
	negative?: boolean
}

/** A card tag can be a filter's id or its display name, depending on the provider. */
export function findTagFilter(
	tag: string,
	filters: readonly TagFilterType[],
): { type: string; option: string } | undefined {
	for (const filter of filters) {
		if (filter.display === 'none') continue
		const option = filter.options.find(
			(candidate) =>
				candidate.id === tag || candidate.value === tag || candidate.formatted_name === tag,
		)
		if (option) return { type: filter.id, option: option.id }
	}
	return undefined
}

export function toggleTagFilter(
	current: readonly TagFilterValue[],
	match: { type: string; option: string },
): TagFilterValue[] {
	const selected = current.some(
		(filter) => filter.type === match.type && filter.option === match.option,
	)
	return selected
		? current.filter((filter) => !(filter.type === match.type && filter.option === match.option))
		: [...current, match]
}
