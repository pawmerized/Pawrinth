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

Uses the official API. Search, details and downloads need an authorized key, so each user adds their own under Settings, Pawrinth, API configuration. It's kept in the system's credential store and no key ships in the app. Without one, the Discover page shows no projects and explains why, unless curse.tools is turned on. Results are ranked by popularity unless another order is picked, because CurseForge's API has no relevance ranking and returns matches in no useful order without one. CurseForge reports no likes for most projects, so cards only show a like count when there is one. See `pawrinth-docs/Modrinth Audit.md`.

**Mods, resource packs, shaders and data packs** install the way Modrinth content does. Browsing for an instance, Install puts the newest release that fits its Minecraft version and loader straight in, with its required dependencies, and says so in a notification. If nothing fits, or a required dependency can't be downloaded, the notification says why. Without an instance, Modrinth's own picker opens, where an instance can be chosen or created. A project's Versions tab installs a particular version, and Switch version on an installed file changes it.

**Modpacks** go through the same install jobs as Modrinth packs (`packages/app-lib/src/plus/curseforge_pack.rs`). Pawrinth downloads the pack from the address CurseForge gives, checks it against CurseForge's checksum, reads its `manifest.json` and looks its files up in batches. It then writes an `.mrpack` that lists CurseForge's own download addresses, which the normal installer downloads, checks against CurseForge's SHA-1 checksums and installs, with the pack's loader and Minecraft version. Progress shows in the download manager, and the job can be paused, cancelled and retried. A failed install removes the half-made instance.

- **Files that can't be downloaded.** An author can keep their project off other apps, CurseForge can stop offering a file, and some kinds of project, such as worlds, have nowhere to go in an instance yet. They're listed with links to their CurseForge pages before anything is installed, and the pack is only installed without them if you choose to. The instance page keeps listing them. Pawrinth never fetches them another way.
- **Optional files** in a pack aren't installed.
- **Updates.** Switch version on the instance's modpack card lists the pack's releases. The instance is backed up first and restored if the update fails. Worlds the instance already has are never touched, and a config or other file is only replaced while it's still what the last version of the pack put there. Anything you changed or added stays, and you're shown which. Mods you turned off stay off.
- **Which version.** Install uses the newest release, or the newest beta or alpha when there's no release. The Versions tab installs any other.

Converted packs are kept in the app's cache folder for a week, so a failed install can be retried.

**curse.tools.** The same settings page has an option, off by default, to send CurseForge requests through [curse.tools](https://api.curse.tools/), a free community proxy that answers like the official API without a key. It isn't run by CurseForge or Pawrinth, and the setting says so: it sees what you search for and install, and it can stop working at any time. While it's on, the user's own key isn't sent anywhere (`Access::CommunityProxy` in `curseforge.rs`), requests only go to curse.tools itself and to CurseForge's own download servers, and downloads are still checked against CurseForge's checksums. curse.tools' key returns download links even for projects whose authors don't allow other apps, so Pawrinth checks `allowModDistribution` itself and those projects stay blocked. A 404 on `/files` isn't used as the test, because that key never returns one for them. curse.tools uses one key for everyone, while CurseForge's terms make keys non-transferable, so it's an option for people who choose it rather than something Pawrinth relies on.

**Exporting for CurseForge.** An instance's Export modpack window offers CurseForge `.zip` beside Modrinth `.mrpack` (`packages/app-lib/src/plus/curseforge_export.rs`), so friends on the CurseForge app can import it. The archive has a `manifest.json` in CurseForge's format with the Minecraft version and loader. Content installed from CurseForge is listed by project and file, and so is any other jar CurseForge recognises by its fingerprint. Everything else that's ticked, such as configs and files CurseForge doesn't know, goes in `overrides/`. Without a CurseForge key nothing can be looked up, so only files Pawrinth installed from CurseForge are listed and the rest go in `overrides/`. When it's done, the window says how many files were listed and names the ones packed inside the zip.

## Technic

Technic modpacks show as a third tab next to Modrinth and CurseForge on the Modpacks page (`packages/app-lib/src/plus/technic.rs`, `apps/app-frontend/src/features/plus-technic/`). They come from the Technic Platform's public API at `api.technicpack.net`, which asks which launcher build is calling, so requests say `build=pawrinth`. Technic answers too many requests in a row with 429, so a list only looks up the details of its first ten packs, two at a time, and remembers them for an hour. The rest show their name and icon until they're opened.

**Installing.** A pack is downloaded the way the Technic launcher downloads it: one zip from the address its creator gave, or, for packs on a Solder server, every zip that server lists for the recommended build, each checked against its MD5. A file that doesn't match is downloaded once more. If it still doesn't match and came over HTTPS, it's kept and Pawrinth says which files didn't match, because a listed MD5 comes from the same server as the file and real packs have stale ones; over plain HTTP the install stops. The files are put together in order, with later ones replacing earlier ones, and the mod loader and its version are read from the pack's own `bin/modpack.jar` or `bin/version.json`, which is how Technic packs ship Forge, NeoForge, Fabric and Quilt. Pawrinth then writes an `.mrpack` whose overrides are the pack's files, without `bin/`, and installs it as a new instance through the normal install jobs. The converted pack holds the whole modpack, so it's deleted once the install finishes, and one that never got installed is deleted after a day. Dropbox share links are changed to download the file itself.

- **Packs on plain HTTP.** About one pack in five downloads over HTTP, and zip packs come with no checksum at all, so nothing would catch a file swapped on the way. The pack page says so, and Install asks before downloading one.
- **What can't be installed.** Packs that patch Minecraft's own jar instead of using a loader, mostly from 1.5.2 and earlier, are refused with an explanation, and so are packs with mods but no loader. Modrinth's loader data covers Forge back to 1.5.2.
- **Not yet:** updating an installed Technic pack, and choosing a build other than the recommended one.

**FTB isn't included.** In June 2023 Feed The Beast's legal advisers had them ask Prism Launcher, and MultiMC before it, to stop downloading FTB packs, and FTB said it has never let anyone but CurseForge distribute them ([Prism's post](https://prismlauncher.org/news/ftb-removal/)). FTB's [mod terms](https://www.feed-the-beast.com/policies/modpacks-mods-policy) also rule out using its mods to provide services to others. FTB packs that are published on CurseForge can be installed from there.

## Recognising mods and duplicates

Check mods on an instance's content page, and every pack install or launcher import, recognises mods that weren't installed through Pawrinth (`packages/app-lib/src/plus/mod_matching.rs`).

- **Modrinth first.** A jar whose exact bytes are on Modrinth is already recognised by its SHA-1, the way the content tab always has, so it's never also linked to CurseForge.
- **CurseForge.** Other jars are looked up by CurseForge's fingerprint, which needs a key or curse.tools. An exact match is only linked when the loaders CurseForge lists for that file agree with the jar's own metadata. Linking records the CurseForge project and file the way a CurseForge install does, so CurseForge updates manage the mod from then on. Nothing about the file changes. Fingerprints CurseForge doesn't know are only asked about once a session.
- **What a jar says about itself.** `fabric.mod.json`, `quilt.mod.json`, `META-INF/mods.toml`, `META-INF/neoforge.mods.toml` and old `mcmod.info` give the mod ids, version, loader, supported Minecraft versions and required mods, plus the ids of mods bundled inside. Compatibility with the instance needs the loader to match and the Minecraft range to include the instance's version. Ranges Pawrinth can't read count as unknown, not compatible.
- **Duplicates** are enabled jars that declare the same mod id or belong to the same Modrinth or CurseForge project, never files with similar names. The copy to keep is the compatible, newest, managed one. Removing another copy is only offered when the one kept is known to work with the instance, a snapshot is taken first, and Update history brings it back.
- **The same mod on Modrinth.** A CurseForge file can be switched to Modrinth only when Modrinth has the same version for the instance's Minecraft version and loader, and both projects link the same source repository. Before switching, Pawrinth downloads the Modrinth jar and checks it declares the same mod, works with the instance and needs nothing that isn't installed. If the Modrinth install fails, the CurseForge file is put back. A version that isn't on Modrinth stays on CurseForge, and nothing is upgraded along the way.

## Importing from the FTB App

The import screen finds the FTB App's instances in `~/.ftba`, or wherever its `storage/settings.json` or `bin/settings.json` says they are, and lists them by pack name, Minecraft version and loader (`packages/app-lib/src/api/pack/import/ftb_app.rs`). The loader comes from `instance.json`'s `modLoader`, or from `version.json`'s targets in older instances. The icon is `folder.jpg`, or `.ftbapp/logo`, which is one byte for the image type followed by the image. Files are copied, never moved, so the FTB App's own instance stays exactly as it was, and the copy is a normal Pawrinth instance named after the pack. Nothing is downloaded from FTB. An instance with mods but no loader Pawrinth can read is refused rather than imported as vanilla.

## Memory

A pack's recommended memory becomes its instance's memory (`packages/app-lib/src/plus/memory.rs`). Recommendations come from Technic packs on Solder, whose builds list one, and from FTB App instances. Windows and other apps keep a quarter of the RAM, or on PCs with less than 16 GB, half of it up to 4 GB, so a recommendation larger than what's left is reduced, and the Java and memory settings say why. The amount stays automatic until someone changes it, including from anywhere else, and then it's never changed again unless they choose Use the recommended amount. Plans are checked again when Pawrinth starts, so a PC with more or less RAM gets a new amount. Instances without a recommendation keep Pawrinth's usual default.

## World backups

The Worlds tab's World backups window schedules backups for one instance, every hour, 6 hours, day or week, keeping 3 to 50 automatic backups per world (`packages/app-lib/src/plus/world_backups.rs`). Backups are zips of the whole world folder, every dimension included, in `world-backups/<instance>/<world>/` beside the instances, apart from content rollback and Minecraft's own `backups` folder.

- **When.** Schedules are checked a minute after Pawrinth starts and every five minutes after that, so a backup missed while Pawrinth was closed happens soon after it opens. A world is only backed up again once something in it has changed, and never while Minecraft has it open, using Minecraft's own `session.lock`.
- **Keeping.** The oldest automatic backups beyond the chosen number are removed. Backups made with Back up now are never removed automatically, and the five newest backups made before restores are kept.
- **Restoring** needs the instance to be closed and asks first. The world as it is now is backed up, the backup is unpacked beside it with every path checked, and only then do the folders swap. If anything fails, the world is left as it was. A deleted world's backups are still listed, and restoring one brings it back.

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

The built-in status always has two buttons. **Install Pawrinth** opens the newest release's page on GitHub (`RELEASES_URL` in `discord.rs`), where people download `Pawrinth-Setup.exe`. **Join the Discord** opens Pawrinth's server through a permanent invite (`COMMUNITY_INVITE_URL`). If that invite is ever deleted, replace it there and in `about-settings.vue`, then release. Custom mode can replace both with your own buttons. Discord only shows a status's buttons to other people, never on your own profile, so checking them needs a second account.

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

Pawrinth uses the same updater as Modrinth App, Tauri's, pointed at Pawrinth's GitHub releases (`apps/app/tauri-release.conf.json`). When it starts, and every five minutes after, it reads [`updates/latest.json`](updates/latest.json) from this repository's `main` branch. When there's a newer version it downloads the installer in the background, or on a metered connection waits to be asked, and then shows **Reload to update** in the title bar. Closing Pawrinth installs a downloaded update too, without reopening it. The download is checked against the public key in that file before anything runs, so only installers signed with Pawrinth's private key are accepted. The installer runs without questions and keeps the install folder and all data.

**Publishing a release:**

1. Add the repository secret `TAURI_SIGNING_PRIVATE_KEY` once, under Settings, Secrets and variables, Actions: the contents of the private key file. Add `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` too only if the key has a password. GitHub doesn't accept empty secrets, and none is needed.
2. Push a tag such as `v0.2.0`. `.github/workflows/pawrinth-release.yml` builds the installer with that version, signs it, publishes the release with `Pawrinth-Setup.exe` as its only file, then commits the matching `updates/latest.json` to `main`. Don't edit that file by hand.

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

Interface settings can swap the paw for one of two original characters, Mochi the cat and Nova the unicorn (`apps/app-frontend/src/features/plus-logos/logos.ts`). Each is a 16 by 16 grid shaded the paw's way: the accent colour, lightened by 30% white on top edges and darkened by 20% black underneath, with darker accent shades for eyes and Nova's mane and a few fixed colours (white eye shine, pink, and a gold horn). In the app they follow the accent colour like the paw. Picking one also sets the window icon, drawn at 128 pixels in the paw icon's green on its dark rounded square. Tauri only sets a window's small icon, and the taskbar uses it because the window has no large one. The installer and shortcuts keep the paw, because Windows takes their icon from the exe, and a taskbar button pinned from a shortcut may too. A logo name a build doesn't know shows the paw.

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
