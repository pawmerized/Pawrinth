# Pawrinth feature audit

Checked 2026-10-31 by reading the code and running a separate test build with no CurseForge key. "Not yet verified" means the code exists but nobody has run it here. Nothing in this file has been committed.

## CurseForge: where each thing on screen comes from

**Result:** the CurseForge tab shows no real CurseForge projects today. Every project on it is made up, in [curseforge_samples.rs](../packages/app-lib/src/plus/curseforge_samples.rs). Only the tabs, the category filter and the game-version filter are real API data.

The Modrinth tab is different. Its results are real, and they match Modrinth's own API exactly (Fresh Animations by FreshLX, 52.82M downloads, 14.7K follows, 1,884 pages at 20 per page).

| Element | CurseForge tab today | Source type | With an authorized key |
|---|---|---|---|
| Project names | "Sample Tech Mod" and 14 others | Mock, hardcoded | `/mods/search`, `name` |
| Authors | "Sample author" | Mock, hardcoded | `authors[0].name` |
| Icons | None, so a placeholder cube shows | Mock, empty | `logo.thumbnailUrl`, loaded straight from CurseForge's CDN |
| Short description | "Sample data for development..." | Mock, hardcoded | `summary` |
| Full description | One fixed sentence | Mock, hardcoded | `/mods/{id}/description` |
| Download counts | Calculated: 1,000 × position | Mock, calculated | `downloadCount` |
| Heart counts | Calculated: 10 × position | Mock, calculated | Reads `favoriteCount`, which isn't in the documented response. See the changes below |
| Dates | Fixed 2026 dates | Mock, hardcoded | `dateModified`, `dateReleased` |
| Tags on cards | "Sample" | Mock, hardcoded | `categories[].name` |
| Result totals | Number of samples | Mock, calculated | `pagination.totalCount` |
| Tabs (Mods, Shaders...) | Real, 9 classes | Live API, no key | Same. Order and "can install" flags are hardcoded in [provider-mapping.ts](../apps/app-frontend/src/features/plus-curseforge/provider-mapping.ts) |
| Category filter | Real, 148 in total, 52 for mods | Live API, no key | Same. Picking one returns nothing while samples show |
| Game-version filter | Real, 104 versions, newest 26.3 | Live API, no key | Same |
| Game versions on files | "1.21.1" and "1.20.1" | Mock, hardcoded | `/mods/{id}/files`, `gameVersions` |
| Loader filter | Forge, Fabric, Quilt, NeoForge | Hardcoded from CurseForge's documented values | Same |
| Loaders on cards | Fixed per sample | Mock, hardcoded | `latestFilesIndexes[].modLoader` |
| Project files | Two invented files per sample | Mock, hardcoded | `/mods/{id}/files` |
| Download URLs | Always empty, and install is refused | Mock, empty | `downloadUrl`, only when the author allows third-party downloads, and only from CurseForge's own hosts |
| Sort options | Fixed list | Hardcoded from the API docs | Same |

**Stored or cached data:** CurseForge API responses are kept only in memory while the page is open. Nothing is written to disk for browsing. After a real install, the database keeps which project and file it came from (`plus_content_origins`), and the jar stays in the content store so it can be rolled back. Modrinth results do use the app's own database cache.

## What works without a key

Tested directly against `https://api.curseforge.com/v1` with no key sent, 2026-10-31:

| Request | Result |
|---|---|
| `GET /minecraft/version` | 200 |
| `GET /minecraft/modloader` | 200 (5,057 builds, all Forge-named; the app doesn't use it) |
| `GET /categories?gameId=432` | 200 |
| `GET /categories?gameId=432&classesOnly=true` | 200 |
| `GET /categories?gameId=432&classId=6` | 403 |
| `GET /games`, `GET /games/432` | 403 |
| `GET /mods/search` | 403 "API Key missing or invalid" |
| `GET /mods/{id}`, `/description`, `/files`, `/files/{id}`, `/download-url`, `/mods/featured` | 403 |
| `POST /mods`, `POST /mods/files` | 403 |
| `POST /fingerprints/432` | 401 |

So search, project pages, files and downloads all need authorized access. No official route gets around this. CurseForge's docs list the key as the authentication method for every endpoint, so the four open ones may not stay open.

## What needs to change for CurseForge

**Done since this audit, with the key settings:**
- **Samples:** they're now only in development builds. A release with no key shows no projects, explains why, and links to the key settings.
- **Sample cards:** they say View, not Install.
- **Heart counts:** they use `thumbsUpCount`.
- **`CURSEFORGE_API_KEY`:** only development builds read it now. Release builds ignore it. Your Windows account still has the variable set, and it can be deleted.

The table above describes a development build with no key. In a release build with no key, there are no projects at all, so only the tabs' data is fetched. With a key saved, the right-hand column applies.

**Still open:**

1. **Real key:** since tested with the owner's own key, for mods and for modpack installs and updates.
2. **Terms questions to settle before going live**, from the [third-party API terms](https://support.curseforge.com/support/solutions/articles/9000207405-curse-forge-3rd-party-api-terms-and-conditions):
   - Section 2.2: a key is non-transferable and can't be shared with third parties, which is hard to square with shipping one inside a public app.
   - Section 3.1: no building a product that competes with CurseForge, directly or indirectly. A launcher may fall under this, so raise it in your application.
   - Section 3.1: no saving or caching data obtained through the API. It doesn't name downloaded files, but rollback keeps jars, so ask.
3. **Still missing even with a key:** world and plugin installs, and CurseForge in the Update All window. CurseForge's API has no license field, so its project pages don't show one.

[Key applications](https://support.curseforge.com/support/solutions/articles/9000208346-about-the-curseforge-api-and-how-to-apply-for-a-key) go through a form reviewed by the Overwolf team. They look at author earnings, load on CurseForge's servers, and how author consent is handled. The [API docs](https://docs.curseforge.com/rest-api/) separately point to the Studios console for generating a key.

## Fixed in this pass: the Modrinth and CurseForge tabs

**Cause:** the tabs were placed above the page layout, but the layout's sticky instance header uses a negative top margin to sit flush at the top of the page. That pulled it 24px up and over the tabs. Measured at 1280×800: tabs at 72–118px, header starting at 102px, so the bottom 16px of the tabs was hidden. It only happened while installing into an instance, and it wasn't a scaling problem.

**Fix:** a `before-tabs` slot in the shared [browse layout](../packages/ui/src/layouts/shared/browse-tab/layout.vue) puts the tabs below the header. The CurseForge tab also had three related problems, now fixed:
- It lost the instance header and Back button.
- It listed things that can't be installed into an instance, such as modpacks and worlds.
- Its tabs dropped the instance when clicked.

**Checked:**
- Tabs: uncovered on both providers.
- Clicking between providers and types keeps the instance.
- Back returns to the instance page.
- UI scale at 80%, 125% and 150%: no overlap and no sideways overflow.
- Window sizes 940×600 to 2560×1440: same result.
- 3 new unit tests, 44 frontend tests passing.
- The release build, run on temporary data: tabs uncovered on both providers.

## Feature audit

Spec section numbers are in brackets.

| Status | Feature | Where it lives | What it does now | Limits | What Pawrinth wants | New code? | Recommendation |
|---|---|---|---|---|---|---|---|
| Fully implemented | Custom backgrounds with custom themes [4] | [background-model.ts](../apps/app-frontend/src/features/plus-themes/background-model.ts), [theme-model.ts](../apps/app-frontend/src/features/plus-themes/theme-model.ts) | The image shows on stock themes, all six presets, imported themes and live color changes, confirmed from on-screen pixels | The custom theme's more specific rule used to hide the image. Fixed | Backgrounds on every theme | Done | Keep |
| Fully implemented | Corner gap with a background on [4, 5] | `overlayStylesheet` in background-model.ts | The rounded corner is filled with the frame color, confirmed from on-screen pixels | None known | No stray corner | Done | Keep |
| Fully implemented | Headers over a background [4] | `overlayStylesheet` in background-model.ts | Sticky headers stay solid, so text no longer collides with content scrolling beneath them | Only elements using the `sticky` class. All current ones do | Readable text | Done | Keep |
| Fully implemented | Background images [4] | [backgrounds.rs](../packages/app-lib/src/plus/backgrounds.rs), `background-layer.vue`, `background-settings.vue`, `background-state.ts` | PNG, JPEG, WebP and GIF load. Renamed non-images are refused. Fit, position, opacity, blur, brightness, overlay and tint apply. GIFs animate and can be turned off. A deleted or damaged image falls back to the plain theme with a message in settings. A moved app folder no longer breaks the image | Pausing while minimized wasn't rerun in this pass | All listed controls, restart persistence | Done | Keep |
| Partially implemented | Top bar and header [5] | `App.vue`, `app/shell` | Tab overlap fixed. No other fault seen at 1280×800 | Not compared side by side with the official app, which isn't opened here to protect its data | Match the original | Unknown | Compare against screenshots you provide, or a separate official install |
| Partially implemented | Logo and branding [1] | `packages/assets/branding/logo.svg`, `apps/app/icons/`, `apps/app/icons/discord/` | Pixel paw drawn on a 16-cell grid, as a vector in the app and at whole-pixel scales in every icon size, so it stays sharp down to 16px. Discord pictures at 1024px | Installer artwork is in `apps/app/nsis/`. `modrinth-plus.svg` is a leftover | Professional identity at every size | Assets, not code | Keep |
| Fully implemented on Windows | CurseForge key settings [3] | [curseforge_key.rs](../packages/app-lib/src/plus/curseforge_key.rs), [curseforge-settings.vue](../apps/app-frontend/src/features/plus-settings/curseforge-settings.vue) | Save, show or hide, test, replace, remove, in Credential Manager | No real key tried yet. macOS and Linux stores not built or run | Own key, test button, secure storage | Done | Try a real key when you have one |
| Blocked by authorization | Built-in project key [3] | One place in `curseforge_key.rs` to add it | None, on purpose | Terms section 2.2, section 3.1 bars proxies, and a shipped key can always be extracted | Official builds with a key | Depends on CurseForge | Apply first, and ask in writing about shipping a key |
| Blocked by authorization | Real CurseForge browsing | [curseforge.rs](../packages/app-lib/src/plus/curseforge.rs) | Works once a user saves an accepted key | Each user needs a key from CurseForge | Real results | No | See above |
| Implemented | Skin section [2] | `pages/Skins.vue`, [find-skins.vue](../apps/app-frontend/src/features/plus-skins/find-skins.vue), [skin_gallery.rs](../packages/app-lib/src/plus/skin_gallery.rs), [skin_lookup.rs](../packages/app-lib/src/plus/skin_lookup.rs) | Selector with a 3D preview, plus a live gallery from MineSkin with keyword search, pages, a detail view and local favorites. Usernames look up a player's current skin through Mojang | Saving to a real account wasn't run, because it needs your sign-in | Unified skin area | Done | Keep |
| Links only, by their terms | NameMC [2] | [skin-sources.ts](../apps/app-frontend/src/features/plus-skins/skin-sources.ts) | Search, browse and player page links that open on NameMC | Terms section 6 forbids automated access except documented public APIs, and none covers skins or search. Refuses framing | Username lookup and previews | Needs NameMC's permission | Mojang covers usernames. Ask NameMC before anything more |
| Links only, by their terms | MinecraftSkins [2] | [skin-sources.ts](../apps/app-frontend/src/features/plus-skins/skin-sources.ts) | Skin search, creator search and browse links that open on the site. Downloaded PNGs go through the existing add-skin flow | Terms section 5 forbids automated access. Section 4.2 forbids using its logos and names as marks. Refuses framing | Browse, search, download in-app | Needs its permission | Ask the site before anything more |
| Links only, no API | SkinMC [2] | [skin-sources.ts](../apps/app-frontend/src/features/plus-skins/skin-sources.ts) | Player, skin and creation search links, plus browse | No documented API, and its old help site is gone. Its terms don't mention automated access | Browse and search in-app | Needs a published API | Add it if SkinMC publishes one |
| Fully implemented | Separate app data [6] | [guard.rs](../packages/app-lib/src/plus/guard.rs), `tauri.conf.json` | Own identifier and folders. Refuses to start on Modrinth's data. Official database unchanged by hash after today's runs | Moved-directory path not run in this pass | Independent apps | No | Keep |
| Missing | Import from Modrinth [7] | `api/pack/import/` covers Prism, MultiMC, ATLauncher, GDLauncher, CurseForge | Modrinth isn't a source | Opening Modrinth's database directly could change it | Safe copy with verification | Yes | Read a temporary copy of its database, copy instance folders, verify hashes, never write to the source |
| Fully implemented | Pawrinth News [8] | [news.rs](../packages/app-lib/src/plus/news.rs), [queries.ts](../apps/app-frontend/src/features/news/queries.ts), `news-sidebar.vue` | Modrinth's feed is gone. Reads `news/news.json` from this repository's `main` branch, with a clean empty state if it can't be read. Modrinth's news is a separate, labelled link | Tested against a local test file. The real file appears once it's pushed. Format in `PAWRINTH.md` | Pawrinth News | Done | Edit `news/news.json` and push it to publish news |
| Partially implemented | Theme coverage [9] | `theme-model.ts`, `theme-engine.ts`, `theme-editor.vue` | Surfaces, text, borders, buttons, sidebar and accent are tokens | Hover and notification colors not covered. Not every page checked | Every page and component | Some | Sweep every page with a high-contrast test theme and replace hardcoded colors |
| Partially implemented | Backups [10A] | `api/worlds.rs` (`backup_world`), `install/recovery.rs`, `state/db_backup.rs`, snapshots | World backup exists in the backend and helper, but no screen calls it. Snapshots cover content, not config | No restore for world backups | Back up and restore | Some | Add it to the existing Worlds tab |
| Partially implemented | Storage management [10B] | `content-storage.vue`, `purge_cache_types`, `plus_storage_usage` | Shared store usage, cache limit, cleanup, rollback storage | No per-instance disk usage | Usage by instance | Some | Extend the existing screen. Never touch worlds |
| Implemented, needs expansion | Instance organization [10C] | `components/ui/library/` | Groups, favorites, last-played sorting, search, custom icons | No instance colors beyond icon backgrounds | As listed | Very little | Mostly already there |
| Partially implemented | Notifications [10D] | `injectNotificationManager` | In-app notifications | No Windows notifications | Completion and error alerts | Some | Add events to the existing manager first |
| Partially implemented | Diagnostics [10E] | `install/diagnostics.rs`, `pages/instance/logs`, `crash-findings.ts` | Local crash rules, IP and support-text censoring | No report export | Export with redaction | Some | Reuse the existing censoring |
| Missing | Custom home page [10F] | `pages/Index.vue`, `features-settings.vue` | Fixed sections | Only a few show/hide options | Choose sections | Some | Add to existing feature settings |
| Fully implemented | Automatic version choice | `providers/content-install.ts`, `pages/project/Index.vue`, [install-actions.ts](../apps/app-frontend/src/features/plus-curseforge/install-actions.ts) | Picks the newest compatible stable version, then beta, then alpha. Never picks an incompatible one. Install goes straight into the instance being browsed, or through Modrinth's own instance picker, with no version step in between. Project pages only warn when nothing fits. The Versions tab installs any other version | Real Modrinth and CurseForge installs tested | Preserve and extend | Done | Keep |
| Fully implemented | Filter sync with the instance | `features/plus-install/instance-filter-sync.ts`, `browse-tab/sidebar.vue` | "Sync filters with selected instance" is on by default on both providers. Turning it off holds for that instance until the app restarts | Real CurseForge results not tried | As specified | Done | Keep |
| Fully implemented | Creator pages | `pages/User.vue`, `pages/Organization.vue`, [author-page.vue](../apps/app-frontend/src/features/plus-curseforge/author-page.vue) | Authors open inside the app with search, sort, filters and pages. Back restores your place. Modrinth creators tested on live data | CurseForge's API gives no creator bio, so none is shown. Its creator pages were tested only with the local stand-in | As specified | Done | Keep |
| Fully implemented | CurseForge project page | [project-page.vue](../apps/app-frontend/src/features/plus-curseforge/project-page.vue) | Reuses Modrinth's header, versions list and sidebars with CurseForge's own data | Tested only with the local stand-in | Same layout | Done | Keep |
| Fully implemented | Theme files | [theme-editor.vue](../apps/app-frontend/src/features/plus-themes/theme-editor.vue), [theme_files.rs](../packages/app-lib/src/plus/theme_files.rs) | Import with a preview, export, rename, duplicate, delete, restore default. Bad files are refused, and nothing is overwritten without asking | The Windows file pickers weren't clicked in tests; the code behind them was | Complete pair | Done | Keep |
| Fully implemented | Streamer mode, name blur and display name | [privacy-settings.vue](../apps/app-frontend/src/features/plus-settings/privacy-settings.vue), `features/plus-privacy/` | Replaces names with "Hidden account" and hides profile settings and your own profile page. Blurs avatars, friends, screenshots and folder paths. Cleans logs, errors and notifications. One click in the title bar turns it on or off | Only changes what Pawrinth shows. Tested with invented accounts, not a real sign-in. Your name still shows as the creator on your own projects, and exported bug report files aren't changed | Blur, change, spoof | Done, display only | Real name spoofing isn't built, because servers would still see the real one |
| Fully implemented | Instance filter sync | `layouts/shared/browse-tab/sidebar.vue`, [instance-filter-sync.ts](../apps/app-frontend/src/features/plus-install/instance-filter-sync.ts) | One control at the top of the sidebar covers version, loader and environment on both providers. The choice lasts for the session, per instance | CurseForge tested only with the local stand-in | One clear control | Done | Keep |
| Fully implemented | Discord presence | [discord.rs](../packages/app-lib/src/plus/discord.rs), [discord-settings.vue](../apps/app-frontend/src/features/plus-settings/discord-settings.vue) | Pawrinth's own presence under its built-in application, with an Install Pawrinth button that downloads the newest release, custom text and buttons, a live preview and streamer mode support | Tested on a real Discord profile. Discord running as administrator can't be reached, which Settings explains | Own presence | Done | Keep |
| Fully implemented | Category and content-type icons | [category-icons.ts](../apps/app-frontend/src/features/plus-curseforge/category-icons.ts), browse pages | Modrinth's own icon set on every content tab and CurseForge category and loader. New CurseForge categories fall back to a neutral icon | CurseForge's real category list checked only against its names as listed here | Consistent small icons | Done | Keep |
| Not yet verified | Discovery | `platform/adapters/browse/`, `packages/ui/.../browse-tab/` | Upstream, with the changes above | Browsing tested | Preserve | No | Keep |
| Not yet verified | Installed content, enable, disable, remove | `pages/instance/content/`, `packages/ui/.../content-tab/` | Upstream, with Pawrinth additions | Not rerun in this pass | Preserve | No | Keep |
| Implemented | Update All and rollback | `install/runner.rs`, `plus/operations.rs` | Tested earlier with a real round trip | Compatibility checks not built | Preserve and extend | Only for compatibility | Keep |
| Implemented | Download manager | `packages/app-lib/src/install/`, [curseforge_pack.rs](../packages/app-lib/src/plus/curseforge_pack.rs) | Modrinth content and CurseForge modpacks, which are converted to `.mrpack` and run as ordinary install jobs. Single CurseForge files show as installing rows in the content list, as single Modrinth files do | CurseForge tested only with the local stand-in | One system for both | Done | Keep |
| Not yet verified | Java, memory, window and launch settings | `features/instances/settings-modal/`, `features/settings/` | Upstream | Known possible bug: duplicating an instance may drop overrides | Preserve | Maybe | Reproduce the bug first |
| Not yet verified | Accounts and sign-in | `features/minecraft-accounts/` | Upstream, untouched | Needs your sign-in to test | Preserve | No | Don't change |

## Proposed order

1. **Done:** CurseForge key setting and what the app shows without a key.
2. **Done:** backgrounds on custom themes, the corner gap, sticky headers over a background, and missing or moved images.
3. **Done:** Pawrinth News, install and version choice, filter sync, tags, project pages, creator pages, theme files, streamer mode, skin discovery, category icons.
4. **Import from Modrinth,** since it protects user data and is the largest missing feature.
5. **Logo and installer artwork.**
6. **Theme sweep** across every page and component.
7. **Quality-of-life:** world backups, per-instance storage, notifications, diagnostic export, home page.

## Sources

- [CurseForge third-party API terms](https://support.curseforge.com/support/solutions/articles/9000207405-curse-forge-3rd-party-api-terms-and-conditions)
- [About the CurseForge API and how to apply for a key](https://support.curseforge.com/support/solutions/articles/9000208346-about-the-curseforge-api-and-how-to-apply-for-a-key)
- [CurseForge REST API docs](https://docs.curseforge.com/rest-api/)
- [NameMC terms](https://namemc.com/terms)
- [MinecraftSkins terms](https://www.minecraftskins.com/terms-and-conditions/)
- [SkinMC terms](https://skinmc.net/p/terms)
