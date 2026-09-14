import { readdirSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { curseforgeCategoryIcon, NEUTRAL_CATEGORY_ICON } from './category-icons'

const ICON_FOLDER = resolve(
	dirname(fileURLToPath(import.meta.url)),
	'../../../../../packages/assets/icons/tags/categories',
)
const AVAILABLE_ICONS = new Set(readdirSync(ICON_FOLDER).map((file) => file.replace('.svg', '')))

/** As CurseForge lists them for each project class. */
const CATEGORIES = {
	mods: [
		'Armor, Tools, and Weapons',
		'Blood Magic',
		'Create',
		'Integrated Dynamics',
		'Dimensions',
		'Server Utility',
		'Map and Information',
		'Addons',
		'Galacticraft',
		'Bug Fixes',
		'Applied Energistics 2',
		'KubeJS',
		'Biomes',
		'API and Library',
		'Food',
		"Tinker's Construct",
		'Industrial Craft',
		'Farming',
		'Mobs',
		'Education',
		'Buildcraft',
		'Technology',
		'Storage',
		'Automation',
		'CraftTweaker',
		'Utility & QoL',
		'Structures',
		'Thermal Expansion',
		'Energy, Fluid, and Item Transport',
		'Redstone',
		'ModJam 2025',
		'Forestry',
		"Farmer's Delight",
		'Horror',
		'Adventure and RPG',
		'Magic',
		'Cosmetic',
		'Miscellaneous',
		'World Gen',
		'Ores and Resources',
		'Player Transport',
		'Processing',
		'Genetics',
		'Performance',
		'Twitch Integration',
		'MCreator',
		'Thaumcraft',
	],
	modpacks: [
		'Tech',
		'Magic',
		'Sci-Fi',
		'Adventure and RPG',
		'Exploration',
		'Mini Game',
		'Quests',
		'Hardcore',
		'Map Based',
		'Small / Light',
		'Extra Large',
		'Combat / PvP',
		'Multiplayer',
		'FTB Official Pack',
		'Skyblock',
		'Vanilla+',
	],
	resourcePacks: [
		'16x',
		'32x',
		'64x',
		'128x',
		'256x',
		'512x and Higher',
		'Steampunk',
		'Photo Realistic',
		'Modern',
		'Medieval',
		'Traditional',
		'Animated',
		'Mod Support',
		'Font Packs',
		'Data Packs',
	],
	shaders: ['Fantasy', 'Realistic', 'Vanilla'],
	worlds: ['Adventure', 'Creation', 'Game Map', 'Modded World', 'Parkour', 'Puzzle', 'Survival'],
	plugins: [
		'Admin Tools',
		'Anti-Griefing Tools',
		'Chat Related',
		'Developer Tools',
		'Economy',
		'Fixes',
		'Fun',
		'General',
		'Informational',
		'Mechanics',
		'Role Playing',
		'Teleportation',
		'Website Administration',
		'World Editing and Management',
		'World Generators',
	],
	customization: ['Configuration', 'FancyMenu', 'Guidebook', 'Quests', 'Scripts', 'Recipes'],
	addons: ['Skins', 'Behavior Packs', 'Texture Packs'],
}

describe('category icons', () => {
	const every = Object.values(CATEGORIES).flat()

	it('only uses icons that exist', () => {
		for (const name of every) {
			expect(AVAILABLE_ICONS, name).toContain(curseforgeCategoryIcon(name))
		}
		expect(AVAILABLE_ICONS).toContain(NEUTRAL_CATEGORY_ICON)
	})

	it('finds a meaningful icon for every known category', () => {
		const unmatched = every.filter((name) => curseforgeCategoryIcon(name) === NEUTRAL_CATEGORY_ICON)
		expect(unmatched).toEqual([])
	})

	it('matches a category to what it is', () => {
		expect(curseforgeCategoryIcon('Food')).toBe('food')
		expect(curseforgeCategoryIcon('Armor, Tools, and Weapons')).toBe('equipment')
		expect(curseforgeCategoryIcon('Blood Magic')).toBe('magic')
		expect(curseforgeCategoryIcon('Applied Energistics 2')).toBe('storage')
		expect(curseforgeCategoryIcon('Map and Information')).toBe('map-pinned')
		expect(curseforgeCategoryIcon('Map Based')).toBe('building-2')
		expect(curseforgeCategoryIcon('Developer Tools')).toBe('terminal')
		expect(curseforgeCategoryIcon('512x and Higher')).toBe('grid-3x3')
	})

	it('falls back to a neutral icon instead of guessing', () => {
		expect(curseforgeCategoryIcon('Something New')).toBe(NEUTRAL_CATEGORY_ICON)
		expect(curseforgeCategoryIcon('')).toBe(NEUTRAL_CATEGORY_ICON)
	})
})
