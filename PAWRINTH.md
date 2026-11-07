# Pawrinth: fork notes

Maintained by pawmerized. Fork of `modrinth/code` from commit `e5a5663` (v0.22.1 + 2 commits), first modified 2026-10-31. GPL-3.0 section 5(a) requires modified versions to say so; this file and the README notice do that.

## Differences from upstream

- **Identity:** identifier `com.pawmerized.pawrinth`, product name Pawrinth, deep link scheme `pawrinth://`. Data lives in its own folder, and startup refuses any folder belonging to an official install (`packages/app-lib/src/plus/guard.rs`).
- **Branding:** Modrinth logos, mascot art and cover images are replaced or removed, as `COPYING.md` requires of forks.
- **Not included:** Modrinth's ad placement, Sentry reporting, playtime and server-play analytics, and the implicit import of data from older official installs.
- **Updates come from GitHub.** The updater reads Pawrinth's own releases and checks them against Pawrinth's own signing key, never Modrinth's. See Automatic updates.
- **No Modrinth Hosting.** The server management pages, their sidebar button, hosting server invites and the Intercom support chat that ran on those pages are removed (`App.vue`, `routes.js`), and the app no longer fetches your hosting servers, subscriptions or payments at startup. The app's content security policy no longer allows Intercom, Stripe, hosting server nodes or Modrinth's internal network addresses (`apps/app/tauri.conf.json`). Modrinth's server browsing tab is unaffected. The browse page's mode for adding content to a hosting server is still in `platform/adapters/browse/`, because it's woven through Modrinth's browse page, but nothing opens it any more.
- **User agent:** identifies the fork instead of the official app.
- **Installing several instances at once:** installs share Java, game files and loader setup, and upstream's versions of those steps made each other fail on Windows. Java setup and loader setup now take turns (`install::control::lock_shared_step`), shared files are replaced with retries (`util::io::persist_replacing`), and a finished Java install is reused instead of deleted.

## CurseForge

Uses only the official API. Search, details and downloads need an authorized key, so each user adds their own under Settings, Pawrinth, API configuration. It's kept in the system's credential store and no key ships in the app. Without one, the Discover page shows no projects and explains why. Results are ranked by popularity unless another order is picked, because CurseForge's API has no relevance ranking and returns matches in no useful order without one. CurseForge reports no likes for most projects, so cards only show a like count when there is one. See `pawrinth-docs/Modrinth Audit.md`.

**Mods, resource packs, shaders and data packs** install the way Modrinth content does. Browsing for an instance, Install puts the newest release that fits its Minecraft version and loader straight in, with its required dependencies, and says so in a notification. If nothing fits, or a required dependency can't be downloaded, the notification says why. Without an instance, Modrinth's own picker opens, where an instance can be chosen or created. A project's Versions tab installs a particular version, and Switch version on an installed file changes it.

**Modpacks** go through the same install jobs as Modrinth packs (`packages/app-lib/src/plus/curseforge_pack.rs`). Pawrinth downloads the pack from the address CurseForge gives, checks it against CurseForge's checksum, reads its `manifest.json` and looks its files up in batches. It then writes an `.mrpack` that lists CurseForge's own download addresses, which the normal installer downloads, checks against CurseForge's SHA-1 checksums and installs, with the pack's loader and Minecraft version. Progress shows in the download manager, and the job can be paused, cancelled and retried. A failed install removes the half-made instance.

- **Files that can't be downloaded.** An author can keep their project off other apps, CurseForge can stop offering a file, and some kinds of project, such as worlds, have nowhere to go in an instance yet. They're listed with links to their CurseForge pages before anything is installed, and the pack is only installed without them if you choose to. The instance page keeps listing them. Pawrinth never fetches them another way.
- **Optional files** in a pack aren't installed.
- **Updates.** Switch version on the instance's modpack card lists the pack's releases. The instance is backed up first and restored if the update fails. Worlds the instance already has are never touched, and a config or other file is only replaced while it's still what the last version of the pack put there. Anything you changed or added stays, and you're shown which. Mods you turned off stay off.
- **Which version.** Install uses the newest release, or the newest beta or alpha when there's no release. The Versions tab installs any other.

Converted packs are kept in the app's cache folder for a week, so a failed install can be retried.

**Exporting for CurseForge.** An instance's Export modpack window offers CurseForge `.zip` beside Modrinth `.mrpack` (`packages/app-lib/src/plus/curseforge_export.rs`), so friends on the CurseForge app can import it. The archive has a `manifest.json` in CurseForge's format with the Minecraft version and loader. Content installed from CurseForge is listed by project and file, and so is any other jar CurseForge recognises by its fingerprint. Everything else that's ticked, such as configs and files CurseForge doesn't know, goes in `overrides/`. Without a CurseForge key nothing can be looked up, so only files Pawrinth installed from CurseForge are listed and the rest go in `overrides/`. When it's done, the window says how many files were listed and names the ones packed inside the zip.

## Pawrinth News

The news section reads one JSON file over HTTPS. There's no news service and no key. By default it's [`news/news.json`](news/news.json) on this repository's `main` branch, so publishing news means editing that file and pushing it. Until the file is there, the section says there are no announcements yet. A build can read another file instead, set in `packages/app-lib/.env` before building:

```
PAWRINTH_NEWS_FEED_URL=https://example.com/pawrinth-news.json
```

The file's format:

```json
{
	"all_news_url": "https://example.com/news",
	"articles": [
		{
			"title": "Pawrinth 0.2",
			"summary": "One or two sentences.",
			"date": "2026-11-01",
			"link": "https://example.com/news/0-2",
			"thumbnail": "https://example.com/news/0-2.png",
			"category": "release"
		}
	]
}
```

- **Required:** `title`, `date` and `link`. `date` is `YYYY-MM-DD` or RFC 3339, and links must use HTTPS.
- **Optional:** `summary`, `thumbnail`, `category` and `all_news_url`.
- **Categories:** `release`, `update`, `feature`, `maintenance`, `compatibility`, `community`, `announcement`. Anything else counts as `announcement`.
- **Limits:** the newest 20 articles show, and the file can be up to 512 KB. Invalid entries are skipped, and repeated links are shown once.

## Theme files

Themes are exported as `<name>.pawrinth-theme.json`:

```json
{
	"format": "pawrinth-theme",
	"version": 2,
	"name": "Example",
	"base": "dark",
	"palette": {
		"background": "#101418",
		"panel": "#161b20",
		"surface": "#1c2228",
		"raised": "#232a31",
		"sidebar": "#12171b",
		"button": "#2a323a",
		"border": "#333c45",
		"text": "#e6eaee",
		"textMuted": "#9aa5af",
		"accent": "#1bd96a",
		"selection": "#1bd96a"
	}
}
```

`palette` needs every key above, each as `#rrggbb`. An optional `background` object holds how a background image is adjusted, such as fit, opacity and blur. The image itself and its path are never included. Files must be `.json` and at most 256 KB, and files with a newer `version` than the app understands are refused. Version 1 files have no `background` and still import.

## Privacy and streaming

Name blur, the display name and streamer mode only change what's shown inside Pawrinth. They don't sign anyone out or change the Minecraft or Modrinth account, the name in-game, or what servers and other players see. Pawrinth doesn't spoof account names to anything outside the app, because that would mean impersonating an account to Mojang or to servers.

Streamer mode replaces account names with "Hidden account" unless a display name is set. It hides the profile and social settings and your own profile page, and blurs avatars, friends, shared-instance members, screenshots and folder paths. Account names, the user folder and IP addresses are removed from logs, error dialogs and notifications. Bug report files exported from Settings aren't changed, because support needs them whole.

## Discord

Rich Presence works with nothing to set up, under Pawrinth's own Discord application (`BUILT_IN_APPLICATION_ID` in `packages/app-lib/src/plus/discord.rs`), never Modrinth's. An Application ID isn't a secret, and it's the only thing Discord needs. It connects when Discord starts, recovers when Discord restarts, and follows Minecraft launching and closing.

**Settings, Discord Rich Presence** turns it on or off, chooses whether to show the instance name, Minecraft version and time played, and has a Test connection button. Streamer mode reduces the status to "Playing Minecraft".

Custom mode adds both lines of text, the logo picture and its hover text, a small mod loader picture, up to two profile buttons, and optionally someone's own Application ID. Restore defaults clears all of it.

The logo on the built-in application is the art asset `BUILT_IN_LOGO_ASSET`. Pictures for it are in `apps/app/icons/discord/`. If the asset is replaced under a new name, change that constant. Discord leaves out a picture it can't find rather than showing a broken one, and Test connection reports it.

The built-in status always has two buttons. **Install Pawrinth** downloads `Pawrinth-Setup.exe` from the newest GitHub release (`INSTALLER_URL` in `discord.rs`), and the release workflow uploads that file under the same name every time, so the link never changes. **Join the Discord** opens Pawrinth's server through a permanent invite (`COMMUNITY_INVITE_URL`). If that invite is ever deleted, replace it there and in `about-settings.vue`, then release. Custom mode can replace both with your own buttons. Discord only shows a status's buttons to other people, never on your own profile, so checking them needs a second account.

**If Discord runs as administrator**, Windows won't let Pawrinth reach it, and the same is true of any other app that isn't running as administrator. Settings says so.

Discord answers a handshake in a fraction of a second most of the time, but it has been seen taking over six seconds, and half a minute for an unknown ID once it's throttling. So presence updates run in the background and never hold up a game launch, a custom ID is checked with Discord's website first, and a refused ID isn't retried until the settings change.

The Developer Portal's Rich Presence Visualizer writes sample C code, which is only for previewing and isn't saved or used. These are the matching fields Pawrinth sends, from `presence_content`:

| Visualizer field | Pawrinth sends |
| --- | --- |
| `details` | `Playing {instance}` or your own text |
| `state` | `Minecraft {version} · {loader}` or your own text |
| `startTimestamp` | When the game started |
| `largeImageKey`, `largeImageText` | The logo, `Pawrinth` or your own text |
| `smallImageKey`, `smallImageText` | The mod loader, if turned on in custom mode |
| `endTimestamp` | Nothing, because a launcher can't know when you'll stop |
| `partyId`, `partySize`, `partyMax` | Nothing, because a launcher doesn't know who you're playing with |
| `joinSecret` | Nothing, because Pawrinth has no way to accept a join from Discord |

## Automatic updates

Pawrinth uses the same updater as Modrinth App, Tauri's, pointed at Pawrinth's GitHub releases (`apps/app/tauri-release.conf.json`). When it starts, and every five minutes after, it reads `latest.json` from the newest release. When there's a newer version it downloads the installer in the background, or on a metered connection waits to be asked, and then shows **Reload to update** in the title bar. Closing Pawrinth installs a downloaded update too, without reopening it. The download is checked against the public key in that file before anything runs, so only installers signed with Pawrinth's private key are accepted. The installer runs without questions and keeps the install folder and all data.

**Publishing a release:**

1. Add two repository secrets once, under Settings, Secrets and variables, Actions: `TAURI_SIGNING_PRIVATE_KEY`, the contents of the private key file, and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, empty if the key has none.
2. Push a tag such as `v0.2.0`. `.github/workflows/pawrinth-release.yml` builds the installer with that version, signs it, writes `latest.json` and publishes the release with `Pawrinth_<version>_x64-setup.exe`, its `.sig`, `Pawrinth-Setup.exe` and `latest.json`.

Keep a backup of the private key. If it's lost, installed copies can't accept any later update, and everyone would have to install a new build by hand. Never commit it.

Local release builds need the same two variables set, because `createUpdaterArtifacts` signs the installer.

## Windows installer

`pnpm exec tauri build --config tauri-release.conf.json --bundles nsis`, run from `apps/app` with the signing variables set, makes `target/release/bundle/nsis/Pawrinth_<version>_x64-setup.exe` and its `.sig`. It's the same technology as Modrinth App's installer, NSIS through Tauri, with Pawrinth's own wizard in `apps/app/nsis/installer.nsi` and artwork beside it.

- **Install:** per user, no administrator permission, default folder `%LOCALAPPDATA%\Pawrinth`.
- **Update:** the updater runs the new Setup itself. Running a newer Setup by hand does the same: it updates the existing folder and keeps all data. Files are swapped only once the new one is fully written, so a stopped update leaves the old copy working, and the next run repairs it. Installing an older version over a newer one is refused, because the data may not open in it.
- **Uninstall:** keeps all data unless the separate option is ticked and confirmed.
- **Next to Modrinth App:** separate folder, shortcuts, Windows entry, data and identifier. `.mrpack` files keep whatever handler they already have, and Pawrinth is only added to Open with.

## Finding skins

The skin selector's Find skins tab is a live gallery, with search, pages, 3D previews and favorites saved on this computer.

- **MineSkin** supplies the gallery and keyword search through its [documented API](https://docs.mineskin.org/docs/mineskin-api/get-a-list-of-skins) (`packages/app-lib/src/plus/skin_gallery.rs`). Its [terms](https://legal.inventivetalent.org/terms/mineskin) ask apps to identify themselves, respect rate limits and use an API key for automated requests. The gallery works without a key, with stricter limits, and each user can add their own under Settings, API configuration. It's kept in the system's credential store. The `filter` and `size` parameters aren't in MineSkin's v2 reference, though its API uses them in its own paging links, so a change on their side would show up as a search or paging failure.
- **Mojang** supplies a player's current skin when someone searches a username (`packages/app-lib/src/plus/skin_lookup.rs`).
- **Skins** are community uploads, and MineSkin doesn't say who made them, so no creator is shown.

The other sites are links, because:

- **NameMC:** its [terms](https://namemc.com/terms) forbid automated access except through documented public APIs, and it documents none for skins or search. It also refuses to be framed.
- **MinecraftSkins:** its [terms](https://www.minecraftskins.com/terms-and-conditions/) forbid automated access and using its trademarks and logos without written consent. It also refuses to be framed.
- **SkinMC:** it has no documented API, and its help site no longer exists. The endpoints its own pages call aren't a published interface.

So Pawrinth doesn't scrape, embed or show these sites' logos, and it doesn't show results it can't get from them. If one of them publishes an API or gives permission, it can become another gallery source.

## Before any public release

These are unresolved and need the owner's decision. Local builds are unaffected.

1. **Microsoft sign-in.** `MICROSOFT_CLIENT_ID` in `packages/app-lib/src/state/minecraft_auth.rs` is upstream's and wasn't issued to this fork. A distributed build needs its own registered app and Minecraft API approval.
2. **Modrinth sign-in** still uses upstream's flow.
3. **MineSkin use at scale.** Everyone using the gallery without a key shares MineSkin's stricter limits. Ask MineSkin before distributing a build widely. Its terms offer free plans to open-source, non-profit projects.
4. **SmartScreen.** Updates are signed with Pawrinth's updater key, but the installer itself has no code signing certificate, so Windows warns about it when it's downloaded from the internet. Installs through the updater aren't affected.

## Branding

The paw logo is drawn on a 14 by 12 pixel grid in three greens, `#5cf7ae`, `#1bd96a` and `#0fa958`, on a dark slate gradient from `#21262e` to `#0f1115`. The vector is `packages/assets/branding/logo.svg`, and the app icons are made from `apps/app/icons/pawrinth-source.png`. Text uses Segoe UI.

| File | Size | Use |
| --- | --- | --- |
| `.github/assets/pawrinth-banner.png` | 1600×520 | Top of the README |
| `.github/assets/pawrinth-social-preview.png` | 1280×640 | GitHub's social preview, under Settings, General |
| `.github/assets/screenshots/` | 3200×2000 | README screenshots, from a release build with temporary data at twice the pixel density, so they stay sharp |
| `branding/discord/server-icon.png` | 1024×1024 | Discord server icon. The paw fits inside Discord's circle |
| `branding/discord/server-banner.png` | 1920×1080 | Discord server banner, 16:9 as Discord asks |
| `apps/app/icons/discord/pawrinth.png` | 1024×1024 | The logo on Pawrinth's Discord application |

## Upstream merges

Merge release tags, not arbitrary `main` commits. New code lives under `packages/app-lib/src/plus/`, `apps/app/src/api/plus.rs` and `apps/app-frontend/src/features/plus-*`. Run `cargo sqlx prepare --package theseus` after each merge.

Upstream's GitHub workflows, issue templates and `.github/assets` are deleted here, because they deploy Modrinth's website and servers, use Modrinth's secrets or are Modrinth's branding. Pawrinth's own are `pawrinth-checks.yml` and `pawrinth-release.yml`. Keep them deleted when merging.
