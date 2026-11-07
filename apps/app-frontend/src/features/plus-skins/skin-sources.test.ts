import { describe, expect, it } from 'vitest'

import {
	bytesFromDataUrl,
	couldBePlayer,
	isLinkedSiteId,
	isSkinSourceId,
	mineskinPageUrl,
	playerPageLinks,
	skinSourceBrowseUrl,
	skinSourceSearchLinks,
	withRecentSearch,
} from './skin-sources'

describe('skin site links', () => {
	it('searches NameMC the way its own search box does', () => {
		expect(skinSourceSearchLinks('namemc', 'Notch')).toEqual([
			{ kind: 'everything', url: 'https://namemc.com/search?q=Notch' },
		])
	})

	it('offers each of the searches SkinMC has', () => {
		expect(skinSourceSearchLinks('skinmc', 'fox').map((link) => link.url)).toEqual([
			'https://skinmc.net/s?search=fox&tab=profiles',
			'https://skinmc.net/s?search=fox&tab=skins',
			'https://skinmc.net/s?search=fox&tab=skin_creations',
		])
	})

	it('searches MinecraftSkins by skin and by creator', () => {
		expect(skinSourceSearchLinks('minecraftskins', 'fox')).toEqual([
			{ kind: 'skins', url: 'https://www.minecraftskins.com/search/skin/fox/1/' },
			{ kind: 'creators', url: 'https://www.minecraftskins.com/search/user/fox/1/' },
		])
	})

	it('keeps what was typed from changing the address', () => {
		expect(skinSourceSearchLinks('minecraftskins', '../a b?c=1')[0].url).toBe(
			'https://www.minecraftskins.com/search/skin/..%2Fa%20b%3Fc%3D1/1/',
		)
		expect(skinSourceSearchLinks('namemc', 'a&b=c')[0].url).toBe(
			'https://namemc.com/search?q=a%26b%3Dc',
		)
	})

	it('has nothing to search for without a query', () => {
		expect(skinSourceSearchLinks('namemc', '   ')).toEqual([])
	})

	it('can always browse', () => {
		expect(skinSourceBrowseUrl('namemc')).toBe('https://namemc.com/minecraft-skins')
		expect(skinSourceBrowseUrl('skinmc')).toBe('https://skinmc.net/skins')
		expect(skinSourceBrowseUrl('minecraftskins')).toBe('https://www.minecraftskins.com/')
	})

	it('links a player to their own pages', () => {
		expect(
			playerPageLinks({ id: '069a79f444e94726a5befca90e38aaf5', name: 'Notch' }).map((link) => link.url),
		).toEqual([
			'https://namemc.com/profile/069a79f4-44e9-4726-a5be-fca90e38aaf5',
			'https://skinmc.net/s?search=Notch&tab=profiles',
		])
	})

	it('links a gallery skin to its own page', () => {
		expect(mineskinPageUrl('abc123')).toBe('https://mineskin.org/skins/abc123')
	})

	it('recognizes the sources and nothing else', () => {
		for (const source of ['mineskin', 'namemc', 'skinmc', 'minecraftskins']) {
			expect(isSkinSourceId(source)).toBe(true)
		}
		expect(isSkinSourceId('favorites')).toBe(false)
		expect(isSkinSourceId('other')).toBe(false)
		expect(isSkinSourceId(undefined)).toBe(false)
	})

	it('only treats sites without an API as links', () => {
		expect(isLinkedSiteId('namemc')).toBe(true)
		expect(isLinkedSiteId('mineskin')).toBe(false)
	})
})

describe('what could be a player', () => {
	it('accepts usernames and UUIDs', () => {
		expect(couldBePlayer('Some_Player1')).toBe(true)
		expect(couldBePlayer(' Notch ')).toBe(true)
		expect(couldBePlayer('069a79f444e94726a5befca90e38aaf5')).toBe(true)
		expect(couldBePlayer('069a79f4-44e9-4726-a5be-fca90e38aaf5')).toBe(true)
	})

	it('leaves everything else alone', () => {
		expect(couldBePlayer('')).toBe(false)
		expect(couldBePlayer('fox with a hat')).toBe(false)
		expect(couldBePlayer('seventeen_letters')).toBe(false)
		expect(couldBePlayer('name?x=1')).toBe(false)
	})
})

describe('recent searches', () => {
	it('puts the newest first without repeating', () => {
		expect(withRecentSearch(['a', 'b'], ' B ')).toEqual(['B', 'a'])
	})

	it('stays short', () => {
		expect(withRecentSearch(['a', 'b', 'c', 'd', 'e'], 'f')).toEqual(['f', 'a', 'b', 'c', 'd'])
	})

	it('ignores empty text', () => {
		expect(withRecentSearch(['a'], '  ')).toEqual(['a'])
	})
})

describe('skin image data', () => {
	it('turns a data URL back into its bytes', () => {
		expect([...bytesFromDataUrl('data:image/png;base64,AQID')]).toEqual([1, 2, 3])
	})
})
