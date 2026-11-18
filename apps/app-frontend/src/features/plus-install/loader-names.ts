const LOADER_NAMES: Record<string, string> = {
	fabric: 'Fabric',
	forge: 'Forge',
	neoforge: 'NeoForge',
	quilt: 'Quilt',
	vanilla: 'Vanilla',
}

/** A loader as people write it, such as NeoForge for `neoforge`. */
export function loaderName(loader: string): string {
	return LOADER_NAMES[loader] ?? loader
}
