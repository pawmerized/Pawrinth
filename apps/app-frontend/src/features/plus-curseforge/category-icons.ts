/** Used when a category's name says nothing an icon could show. */
export const NEUTRAL_CATEGORY_ICON = 'square'

/**
 * CurseForge's categories change over time and its API only offers full-color artwork, so its
 * names are matched to Modrinth's own category icons. The first match wins, which puts specific
 * names ahead of general words like "magic" or "tech".
 */
const ICON_BY_NAME: [RegExp, string][] = [
	[/create|integrated dynamics|buildcraft|industrial|tech|processing|automation/, 'technology'],
	[/applied energistics|storage/, 'storage'],
	[/redstone|thermal|energy/, 'zap'],
	[/armor|weapons/, 'equipment'],
	[/blood magic|thaumcraft|magic|botania/, 'magic'],
	[/tinker|ores|resources|mining/, 'pickaxe'],
	[/food|delight|farming|recipes/, 'food'],
	[/forestry|nature/, 'tree-pine'],
	[/biomes|dimension|world ?gen|world (generators|editing)|worldgen/, 'worldgen'],
	[/galacticraft|sci-?fi|space/, 'globe'],
	[/structures|building|map based|creation/, 'building-2'],
	[/map|informational|information/, 'map-pinned'],
	[/api|librar|guidebook|education/, 'library'],
	[/kubejs|crafttweaker|script|developer|mcreator/, 'terminal'],
	[/server utility|admin|website/, 'management'],
	[/utility|qol/, 'utility'],
	[/bug|fixes|configuration|tweak/, 'tweaks'],
	[/performance|optimi/, 'optimization'],
	[/mobs|genetics/, 'mobs'],
	[/horror/, 'cursed'],
	[/transport|teleport/, 'transportation'],
	[/adventure|rpg|role playing/, 'adventure'],
	[/exploration/, 'compass'],
	[/quest/, 'quests'],
	[/hardcore|survival/, 'challenging'],
	[/combat|pvp/, 'combat'],
	[/mini ?game|game map|parkour|puzzle|fun/, 'minigame'],
	[/multiplayer|chat|twitch|social/, 'social'],
	[/economy/, 'economy'],
	[/anti-griefing|protection/, 'shield'],
	[/mechanics|behavior/, 'game-mechanics'],
	[/modjam|contest/, 'trophy'],
	[/official/, 'badge-check'],
	[/skyblock/, 'cloud'],
	[/small|light/, 'lightweight'],
	[/extra large|miscellaneous|general/, 'kitchen-sink'],
	[/cosmetic|decorat/, 'decoration'],
	[/fancymenu|gui|menu/, 'gui'],
	[/^\d+x\b/, 'grid-3x3'],
	[/photo|realistic/, 'realistic'],
	[/medieval/, 'castle'],
	[/modern/, 'house'],
	[/steampunk/, 'gauge'],
	[/traditional|simplistic/, 'simplistic'],
	[/animated/, 'film'],
	[/font/, 'fonts'],
	[/fantasy/, 'fantasy'],
	[/vanilla/, 'vanilla-like'],
	[/mod support|modded|addons|data packs/, 'modded'],
	[/skins|texture/, 'palette'],
]

/** The Modrinth category icon that stands for a CurseForge category. */
export function curseforgeCategoryIcon(name: string): string {
	const normalized = name.trim().toLowerCase()
	return ICON_BY_NAME.find(([pattern]) => pattern.test(normalized))?.[1] ?? NEUTRAL_CATEGORY_ICON
}
