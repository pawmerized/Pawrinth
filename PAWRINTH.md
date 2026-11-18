# Pawrinth: fork notes

Maintained by pawmerized. Fork of `modrinth/code` from commit `e5a5663` (v0.22.1 + 2 commits), first modified 2026-10-31. GPL-3.0 section 5(a) requires modified versions to say so; this file and the README notice do that.

## Differences from upstream

- **Identity:** identifier `com.pawmerized.pawrinth`, product name Pawrinth, deep link scheme `pawrinth://`. Data lives in its own folder, and startup refuses any folder belonging to an official install (`packages/app-lib/src/plus/guard.rs`).
- **Branding:** Modrinth logos, mascot art and cover images are replaced or removed, as `COPYING.md` requires of forks.
- **Not included:** Modrinth's ad placement, Sentry reporting, playtime and server-play analytics, and the implicit import of data from older official installs. `PRIVACY.md` lists every service Pawrinth contacts.
- **Updates come from GitHub.** The updater reads Pawrinth's own releases and checks them against Pawrinth's own signing key, never Modrinth's. See Automatic updates.
- **No Modrinth Hosting.** The server management pages, their sidebar button, hosting server invites and the Intercom support chat that ran on those pages are removed (`App.vue`, `routes.js`), and the app no longer fetches your hosting servers, subscriptions or payments at startup. The app's content security policy no longer allows Intercom, Stripe, hosting server nodes or Modrinth's internal network addresses (`apps/app/tauri.conf.json`). Modrinth's server browsing tab is unaffected. The browse page's mode for adding content to a hosting server is still in `platform/adapters/browse/`, because it's woven through Modrinth's browse page, but nothing opens it any more.
- **User agent:** identifies the fork instead of the official app.
- **Installing several instances at once:** installs share Java, game files and loader setup, and upstream's versions of those steps made each other fail on Windows. Java setup and loader setup now take turns (`install::control::lock_shared_step`), shared files are replaced with retries (`util::io::persist_replacing`), and a finished Java install is reused instead of deleted.

## CurseForge

Uses the official API. Search, details and downloads need an authorized key, so each user adds their own under Settings, Pawrinth, API configuration. It's kept in the system's credential store and no key ships in the app. The key is only ever sent to `api.curseforge.com`, never to the download servers, and requests that carry it don't follow redirects, because reqwest keeps custom headers like `x-api-key` when a redirect changes host. Without one, the Discover page shows no projects and explains why, unless curse.tools is turned on. Results are ranked by popularity unless another order is picked, because CurseForge's API has no relevance ranking and returns matches in no useful order without one. CurseForge reports no likes for most projects, so cards only show a like count when there is one. See `pawrinth-docs/Modrinth Audit.md`.

**Mods, resource packs, shaders and data packs** install the way Modrinth content does. Browsing for an instance, Install puts the newest release that fits its Minecraft version and loader straight in, with its required dependencies, and says so in a notification. If nothing fits, or a required dependency can't be downloaded, the notification says why. Without an instance, Modrinth's own picker opens, where an instance can be chosen or created. A project's Versions tab works like Modrinth's. Opened from an instance, it starts filtered to the instance's Minecraft version and loader. Each version has Install, Switch to this version, or a check on the one that's installed, plus a link to the file on CurseForge, and clicking a version shows its changelog. Switching goes through the same recorded update as the content page, so Update history can undo it. Switch version on an installed file in the content page changes it too.

**Modpacks** go through the same install jobs as Modrinth packs (`packages/app-lib/src/plus/curseforge_pack.rs`). Pawrinth downloads the pack from the address CurseForge gives, checks it against CurseForge's checksum, reads its `manifest.json` and looks its files up in batches. It then writes an `.mrpack` that lists CurseForge's own download addresses, which the normal installer downloads, checks against CurseForge's SHA-1 checksums and installs, with the pack's loader and Minecraft version. Progress shows in the download manager, and the job can be paused, cancelled and retried. A failed install removes the half-made instance.

- **Files that can't be downloaded.** An author can keep their project off other apps, CurseForge can stop offering a file, and some kinds of project, such as worlds, have nowhere to go in an instance yet. They're listed with links to their CurseForge pages before anything is installed, and the pack is only installed without them if you choose to. The instance page keeps listing them. Pawrinth never fetches them another way.
- **Optional files** in a pack aren't installed.
- **Updates.** Switch version on the instance's modpack card lists the pack's releases. The instance is backed up first and restored if the update fails. Worlds the instance already has are never touched, and a config or other file is only replaced while it's still what the last version of the pack put there. Anything you changed or added stays, and you're shown which. Mods you turned off stay off.
- **Which version.** Install uses the newest release, or the newest beta or alpha when there's no release. The Versions tab installs any other.

**Updates for installed content** are checked whenever an instance's content page opens, the same as Modrinth's. A file installed from CurseForge whose project has a newer file for the instance's Minecraft version and loader shows Modrinth's green update button on its row. Clicking it opens the version picker with the newest file chosen, and Update all (or Update on selected rows) includes CurseForge files beside Modrinth ones, with their CurseForge changelogs. CurseForge updates are applied as one recorded operation, so they show in Update history and roll back together. There's no separate CurseForge updates window any more.

Converted packs are kept in the app's cache folder for a week, so a failed install can be retried.

**curse.tools.** The same settings page has an option, off by default, to send CurseForge requests through [curse.tools](https://api.curse.tools/), a free community proxy that answers like the official API without a key. It isn't run by CurseForge or Pawrinth, and the setting says so: it sees what you search for and install, and it can stop working at any time. While it's on, the user's own key isn't sent anywhere (`Access::CommunityProxy` in `curseforge.rs`), requests only go to curse.tools itself and to CurseForge's own download servers, and downloads are still checked against CurseForge's checksums. curse.tools' key returns download links even for projects whose authors don't allow other apps, so Pawrinth checks `allowModDistribution` itself and those projects stay blocked. A 404 on `/files` isn't used as the test, because that key never returns one for them. curse.tools uses one key for everyone, while CurseForge's terms make keys non-transferable, so it's an option for people who choose it rather than something Pawrinth relies on.

**Exporting for CurseForge.** An instance's Export modpack window offers CurseForge `.zip` beside Modrinth `.mrpack` (`packages/app-lib/src/plus/curseforge_export.rs`), so friends on the CurseForge app can import it. The archive has a `manifest.json` in CurseForge's format with the Minecraft version and loader. Content installed from CurseForge is listed by project and file, and so is any other jar CurseForge recognises by its fingerprint. Everything else that's ticked, such as configs and files CurseForge doesn't know, goes in `overrides/`. Without a CurseForge key nothing can be looked up, so only files Pawrinth installed from CurseForge are listed and the rest go in `overrides/`. When it's done, the window says how many files were listed and names the ones packed inside the zip.

## Technic

Technic modpacks show as a third tab next to Modrinth and CurseForge on the Modpacks page (`packages/app-lib/src/plus/technic.rs`, `apps/app-frontend/src/features/plus-technic/`). They come from the Technic Platform's public API at `api.technicpack.net`, which asks which launcher build is calling, so requests say `build=pawrinth`. The tab uses the same browse layout as Modrinth and CurseForge, with the same search bar, cards, Install buttons and pages. Technic has no sorting or filters, so the only order is Trending, or Relevance while searching. Its API has no list of every pack: trending is always its 20 trending packs, and a search returns at most 50 (15 unless asked, so Pawrinth asks for 50). So without a search, the page lists the trending packs followed by everything twelve broad searches (single letters) find, each pack once, about 550 packs in all. Technic ranks each search by its own relevance, so that order puts well-known packs early. Newest sorts by Technic's pack id, which grows as packs are made, and Name sorts alphabetically. The list is gathered three searches at a time and remembered for an hour, or five minutes if Technic limited some of the searches, so only the first visit waits. The page says the list isn't everything, and searching finds any other pack. Technic answers too many requests in a row with 429, so results come a page at a time (5, 10 or 20 packs), and only that page's packs are looked up, two at a time, and remembered for an hour. A pack Technic refused to describe shows just its name and icon until it's opened.

**Installing.** A pack is downloaded the way the Technic launcher downloads it: one zip from the address its creator gave, or, for packs on a Solder server, every zip that server lists for the recommended build, each checked against its MD5. A file that doesn't match is downloaded once more. If it still doesn't match and came over HTTPS, it's kept and Pawrinth says which files didn't match, because a listed MD5 comes from the same server as the file and real packs have stale ones; over plain HTTP the install stops. The files are put together in order, with later ones replacing earlier ones, and the mod loader and its version are read from the pack's own `bin/modpack.jar` or `bin/version.json`, which is how Technic packs ship Forge, NeoForge, Fabric and Quilt. Pawrinth then writes an `.mrpack` whose overrides are the pack's files, without `bin/`, and installs it as a new instance through the normal install jobs. The converted pack holds the whole modpack, so it's deleted once the install finishes, and one that never got installed is deleted after a day. Dropbox share links are changed to download the file itself.

- **Packs on plain HTTP.** About one pack in five downloads over HTTP, and zip packs come with no checksum at all, so nothing would catch a file swapped on the way. The pack page says so, and Install asks before downloading one.
- **What can't be installed.** Packs that patch Minecraft's own jar instead of using a loader, mostly from 1.5.2 and earlier, are refused with an explanation, and so are packs with mods but no loader. Modrinth's loader data covers Forge back to 1.5.2.
- **Not yet:** updating an installed Technic pack, and choosing a build other than the recommended one.

**FTB isn't included.** In June 2023 Feed The Beast's legal advisers had them ask Prism Launcher, and MultiMC before it, to stop downloading FTB packs, and FTB said it has never let anyone but CurseForge distribute them ([Prism's post](https://prismlauncher.org/news/ftb-removal/)). FTB's [mod terms](https://www.feed-the-beast.com/policies/modpacks-mods-policy) also rule out using its mods to provide services to others. FTB packs that are published on CurseForge can be installed from there.

## Modpack updates

Pawrinth checks whether each instance's modpack has a newer version, for packs from Modrinth, CurseForge and Technic (`packages/app-lib/src/plus/pack_updates.rs`). Answers are reused for 15 minutes and forgotten when a pack is installed or updated. A CurseForge pack only counts files made for the instance's Minecraft version, so an update never moves an instance to another version of the game. A library card with an update shows a green download badge, and the modpack card on the instance's content page turns its button into Update available.

- **Modrinth** uses the update Modrinth App already finds for a linked modpack.
- **CurseForge** compares the installed file with the pack's newest release. File ids only grow, so a higher id is newer. It needs a key or curse.tools.
- **Technic** compares the installed version with the one Technic would install now: the Solder server's recommended build, or the pack's version on the Technic Platform. A pack on a plain HTTP server is only asked when someone agreed to that server when installing it.

**Updating a Technic pack** downloads the current version and installs it over the instance through the same job as other pack updates, which backs the instance up first and restores it if the update fails. What each pack installed is remembered in `technic-packs/<instance>.json` beside the instances, with every file's SHA-1, so no database change was needed. Jars in content folders belong to the pack and are replaced. Any other file someone changed is left alone and listed afterwards, worlds the instance already has are never touched, a file the new version dropped is only removed while it's still exactly what the pack installed, and mods someone turned off stay off. Packs installed before this was added have no record, so they can't be updated this way. Pack updates aren't in Update history.

## Recognising mods and duplicates

Check mods on an instance's content page, and every pack install or launcher import, recognises mods that weren't installed through Pawrinth (`packages/app-lib/src/plus/mod_matching.rs`).

- **Modrinth first.** A jar whose exact bytes are on Modrinth is already recognised by its SHA-1, the way the content tab always has, so it's never also linked to CurseForge.
- **CurseForge.** Other jars are looked up by CurseForge's fingerprint, which needs a key or curse.tools. An exact match is only linked when the loaders CurseForge lists for that file agree with the jar's own metadata. Linking records the CurseForge project and file the way a CurseForge install does, so CurseForge updates manage the mod from then on. Nothing about the file changes. Fingerprints CurseForge doesn't know are only asked about once a session.
- **What a jar says about itself.** `fabric.mod.json`, `quilt.mod.json`, `META-INF/mods.toml`, `META-INF/neoforge.mods.toml` and old `mcmod.info` give the mod ids, version, loader, supported Minecraft versions and required mods, plus the ids of mods bundled inside. Compatibility with the instance needs the loader to match and the Minecraft range to include the instance's version. Ranges Pawrinth can't read count as unknown, not compatible.
- **Duplicates** are enabled jars that declare the same mod id or belong to the same Modrinth or CurseForge project, never files with similar names. The copy to keep is the compatible, newest, managed one. Removing another copy is only offered when the one kept is known to work with the instance, a snapshot is taken first, and Update history brings it back.
- **The same mod on Modrinth.** A CurseForge file can be switched to Modrinth only when Modrinth has the same version for the instance's Minecraft version and loader, and both projects link the same source repository. Before switching, Pawrinth downloads the Modrinth jar and checks it declares the same mod, works with the instance and needs nothing that isn't installed. If the Modrinth install fails, the CurseForge file is put back. A version that isn't on Modrinth stays on CurseForge, and nothing is upgraded along the way.

## Installed on either site

Whether a mod is installed is decided by what the instance has, not by the site being browsed (`packages/app-lib/src/plus/installed_identity.rs`). Files are matched to CurseForge projects by fingerprint, the way the CurseForge app does it, and by Pawrinth's own install records. Every file also gives names: the mod ids and display name in its own metadata, and its project's slug and title on Modrinth or CurseForge. Names are compared as lowercase letters and digits, and only names of three characters or more count, so `fabric-api` and Fabric API match but short ids don't. A project on either site counts as installed when its id or one of its names matches, which also stops CurseForge from installing a dependency the instance already has from Modrinth, and stops Install on CurseForge from adding a second copy of a mod installed from Modrinth. Files are read once and remembered by their hash. A fingerprint needs the whole file, so it's only worked out while CurseForge can be asked and the file has no install record; fingerprints CurseForge was already asked about aren't asked again in that session.

## Mod library

Settings, Mod library keeps a list of mods from either site (`packages/app-lib/src/plus/mod_library.rs`). Mods are added by searching Modrinth or CurseForge on that tab, which shows each mod's icon, author, summary, loaders and downloads and marks the ones already saved, even from the other site, or from any mod's ⋯ menu on an instance's content page. Saving a mod doesn't install it anywhere. It starts with New instances on, and only mods with it on go into instances created afterwards. The list lives in Pawrinth's settings. When Create instance finishes an instance, each mod set to go into new instances is installed if a version fits the instance's Minecraft version and loader, with its required dependencies: Modrinth's own resolver picks the version, and CurseForge mods get the file CurseForge recommends for the instance. Modrinth mods go first, so a CurseForge copy of the same mod is then found installed and skipped. When an instance is created from a mod's page, the library waits until that mod and its dependencies are in, so the two installs never add the same mod. A mod already in the instance is skipped quietly. A mod with no fitting version, and CurseForge without a key, are reported in one notification instead of installed. Vanilla and Horizon instances can't load mods, so the library leaves them alone without a notification. Modpack installs, imports and copies of instances don't get the library.

## Copying to another instance

Mods, resource packs, shaders and data packs can be copied from an instance's content page, and worlds from its Worlds tab (`packages/app-lib/src/plus/copy_content.rs`). The other instance is checked first and each item says what will happen. Content from Modrinth or CurseForge is installed again, keeping the same version when it's made for the other instance's Minecraft version and loader, otherwise taking the version that is, with its dependencies. A mod with no version for the other instance is left out. Files from anywhere else are copied as they are only when nothing else would be needed: resource packs, shaders and data packs always, mods only between the same Minecraft version and loader. Anything already in the other instance is left out, and anything turned off stays off in the copy. Worlds are copied under a new name if the name is taken, never to an older Minecraft version, since opening a world in an older version can damage it, and never while their own instance is running, since the game may be writing to them.

## Server packs

Make a server pack, in an instance's ⋯ menu, builds a new folder named after the instance in a folder you choose (`packages/app-lib/src/plus/server_pack.rs`). It has the enabled mods, `config`, `defaultconfigs`, `kubejs` and `scripts`, the loader's server and `start.bat` and `start.sh` with the instance's memory. A mod is left out only when something says it only works in the client: `"environment": "client"` in its `fabric.mod.json` or `quilt.mod.json`, Modrinth saying servers are unsupported, or CurseForge tagging the file Client without Server. Every other mod stays, and `README.txt` lists what was left out. The server comes from Mojang for vanilla, Fabric's server launcher, or the Quilt, Forge or NeoForge installer, which the start script runs once before the first start. Everything is downloaded over HTTPS, and a failed build removes its folder. The Minecraft EULA isn't accepted for anyone; the server asks on its first start.

## Importing from the FTB App

The import screen finds the FTB App's instances in `~/.ftba`, or wherever its `storage/settings.json` or `bin/settings.json` says they are, and lists them by pack name, Minecraft version and loader (`packages/app-lib/src/api/pack/import/ftb_app.rs`). The loader comes from `instance.json`'s `modLoader`, or from `version.json`'s targets in older instances. The icon is `folder.jpg`, or `.ftbapp/logo`, which is one byte for the image type followed by the image. Files are copied, never moved, so the FTB App's own instance stays exactly as it was, and the copy is a normal Pawrinth instance named after the pack. Nothing is downloaded from FTB. An instance with mods but no loader Pawrinth can read is refused rather than imported as vanilla.

## Importing from the Modrinth App

The import screen lists the official Modrinth App's instances when its data folder (`ModrinthApp` in the system's app-data folder) has an `app.db` (`packages/app-lib/src/api/pack/import/modrinth_app.rs`). Pawrinth's own database code never opens that folder, since Pawrinth's migrations would stop the official app from starting, so the database and its write-ahead log are copied to a temporary folder and only the copy is read. The copy gives each instance's folder, name, icon, Minecraft version, loader and loader version, and follows the app's custom data folder setting to find the `profiles` folder. Folder names from the database are only used when they're one plain name. The instance's files are copied the way other launchers' are, never moved. An instance on a loader Pawrinth doesn't know is refused if it has mods, rather than imported as vanilla. Modpack links aren't carried over, because a copied folder doesn't record which files belong to the pack, so an imported modpack updates mod by mod.

## Settings from the Modrinth App

The setup wizard's first page, and Settings, Pawrinth interface, offer the official Modrinth App's settings when it's installed (`packages/app-lib/src/plus/modrinth_app_settings.rs`). Its database is read from a copy, the same way as importing its instances. Only settings that differ from Pawrinth's are listed, each with both values, and only the ticked ones change: theme, language, start page, navigation and sidebar, rendering, window frame, Discord status, hiding while playing, download and write limits, default memory, fullscreen, window size, default Java arguments and environment variables, and launch hooks. Its data folder, telemetry and ad choices are never copied, and the Modrinth App keeps its own settings.

## Java arguments

Custom Java arguments in an instance's Java and memory settings are checked as they're typed, against the instance's Java version, memory and loader (`apps/app-frontend/src/features/plus-java/java-args.ts`). Problems in red stop Java from starting: something that isn't an option, more than one garbage collector, starting memory above the maximum, ZGC before Java 15, generational ZGC before Java 21, and experimental G1 options without `-XX:+UnlockExperimentalVMOptions` before them. Arguments with a red problem aren't saved, so the game keeps starting with the last ones that worked. Warnings cover options Java has removed, memory set outside the memory setting, options set twice, G1 tuning with another collector, Shenandoah, which only some Java builds include, and Fabric, Quilt or Forge properties on a different loader. Most come with a one-click fix. Suggestions are only shown when they suit the instance: Minecraft's own launcher settings on any Java, and generational ZGC on Java 21 or newer with at least 4 GB. Using one replaces the collector settings and keeps the rest, and each option is explained.

Crash help reads Java's own refusals too, such as an unrecognized option or a garbage collector conflict, and a crash caused by Java, memory or arguments has a button to the instance's Java settings. Findings say whether the log confirms the cause or only suggests it, and nothing is changed without you.

## Memory

A pack's recommended memory becomes its instance's memory (`packages/app-lib/src/plus/memory.rs`). Recommendations come from Technic packs on Solder, whose builds list one, and from FTB App instances. Windows and other apps keep a quarter of the RAM, or on PCs with less than 16 GB, half of it up to 4 GB, so a recommendation larger than what's left is reduced, and the Java and memory settings say why. The amount stays automatic until someone changes it, including from anywhere else, and then it's never changed again unless they choose Use the recommended amount. Plans are checked again when Pawrinth starts, so a PC with more or less RAM gets a new amount. Instances without a recommendation keep Pawrinth's usual default.

## World backups

The Worlds tab's World backups window schedules backups for one instance, every hour, 6 hours, day or week, keeping 3 to 50 automatic backups per world (`packages/app-lib/src/plus/world_backups.rs`). Backups are zips of the whole world folder, every dimension included, in `world-backups/<instance>/<world>/` beside the instances, apart from content rollback and Minecraft's own `backups` folder.

- **When.** Schedules are checked a minute after Pawrinth starts and every five minutes after that, so a backup missed while Pawrinth was closed happens soon after it opens. A world is only backed up again once something in it has changed, and never while Minecraft has it open, using Minecraft's own `session.lock`.
- **Keeping.** The oldest automatic backups beyond the chosen number are removed. Backups made with Back up now are never removed automatically, and the five newest backups made before restores are kept.
- **Restoring** needs the instance to be closed and asks first. The world as it is now is backed up, the backup is unpacked beside it with every path checked, and only then do the folders swap. If anything fails, the world is left as it was. A deleted world's backups are still listed, and restoring one brings it back.

## Horizon

Vanilla 26.3 instances have a Horizon tab in their settings. It plays them with [Horizon](https://github.com/horizon-rust/horizon), a fan-made rebuild of Minecraft: Java Edition 26.3 in Rust (MIT licensed, not from Mojang or Microsoft). The code is in `packages/app-lib/src/plus/horizon.rs` and `apps/app-frontend/src/features/plus-horizon/`. Nothing is downloaded until someone presses Set up Horizon.

- **Setup.** Pawrinth downloads `horizon-windows-x64.zip` from Horizon's `nightly` GitHub release and unpacks it into `horizon/game` in Pawrinth's data folder. Horizon contains none of Minecraft's code or assets, so its preparer (`horizon-prepare --prepare --complete`) builds them into `horizon/resources` from Minecraft 26.3, using the instance's Java 25. Check for updates downloads a new nightly only when the signed checksum has changed.
- **Signature check.** A build is installed only when Horizon's release key signed its checksum for exactly that build. The release key is the Ed25519 key in Horizon's `crates/horizon-release-sign/release-keys.txt`, and it's built into Pawrinth as `RELEASE_KEY`.
  - **How it's checked.** The release title (`Nightly 2026-11-13 · 9f42064`) gives the build name `nightly-2026-11-13-9f42064`. `SHA256SUMS.v2.sig` must be that key's signature over `horizon release <build>\n` followed by `SHA256SUMS`, and the zip's SHA-256 must match the signed list. A changed list, another build's signature or the channel-wide `SHA256SUMS.sig` is refused.
  - **What it doesn't cover.** Someone holding Horizon's private signing key (a Horizon maintainer, or a compromise of Horizon's protected `release` environment) can still ship a signed build. A new key only arrives with a Pawrinth update.
  - **Downloads.** Every Horizon and Mojang download uses the HTTPS-only client.
- **Mojang's files.** Horizon pins the SHA-1 of the 26.3 version JSON and asset index, and the launcher meta's copies can differ from Mojang's. So the version JSON, asset index and server jar come from Mojang's version manifest into `horizon/minecraft`. The client jar and asset objects Pawrinth already has are copied or hard-linked instead of downloaded again, and the libraries are read from Pawrinth's own folder.
- **Creating one.** The Create instance window lists Horizon after the other loaders. Choosing it locks the game version to 26.3 and hides the loader version. It then creates a vanilla 26.3 instance with Play with Horizon already on. Duplicating a Horizon instance keeps Play with Horizon on for the copy. The window's `fixedVersionLoaders` option (`{ horizon: '26.3' }`, set in `App.vue`) treats Horizon like Vanilla with one game version.
- **Before setup.** Play with Horizon can be on before Horizon is set up. Pressing Play on the instance page then opens the instance's Horizon tab, where setup is offered, instead of starting Java Minecraft. A launch from anywhere else refuses and says to set it up.
- **Playing.** With Play with Horizon on, the Play button starts `horizon-client` through Horizon's Launch API. Quick play into a world or server works the same way. The instance is the game folder, and Pawrinth's window settings apply.
- **The account.** The signed-in account goes in a session file in a temporary folder, which Horizon deletes once it has read it, so the access token is never on the command line.
- **While it runs.** The game shows as running like Minecraft does: Stop works, and its output goes to the instance's log. Java arguments and memory don't apply, since Horizon isn't Java. The instance's launch hooks (pre-launch, wrapper and post-exit) and environment variables work the same as for Minecraft.
- **Limits.**
  - Only the Windows x64 build is set up for now.
  - Horizon doesn't load Java mods.
  - Its Launch API is marked experimental, so a new nightly can need preparing again. The tab says so, and Play refuses until that's done.
  - GitHub keeps old nightlies for only about a week, so Pawrinth keeps whichever build it installed last.
- **Removing.** Remove Horizon deletes `horizon/` and turns Horizon off for every instance.
- **Shown as Horizon.** A vanilla 26.3 instance with Play with Horizon on is a Horizon instance wherever an instance's loader shows: library cards and filters, sorting and grouping by loader, the instance header, its Installation tab, Jump in, worlds on Home and the delete dialog (`apps/app-frontend/src/features/plus-horizon/instance-loader.ts`). It's worked out from the same setting that makes Play start Horizon, not by renaming Vanilla. Minecraft's files and Modrinth still see vanilla 26.3, since that's what its content has to fit, and ordinary vanilla instances still say Vanilla.
- **Resource packs and shader packs.** A Horizon instance's Discover is Horizon's own, with two tabs, Resource Packs and Shaders, and nothing from Modrinth or CurseForge. It uses the same browse layout, header, search, sorting, pages and filter sidebar as the rest of the app. Horizon's registry uses Modrinth's category names, so its packs get Modrinth's filter groups and icons, and card tags filter when clicked (`apps/app-frontend/src/features/plus-horizon/browse-page.vue`, `packages/app-lib/src/plus/horizon_content.rs`). Every way into Discover (Browse content, Add content, the library's menus) goes there through `discoverLocation`, and a router check (`discover-guard.ts`) sends any other Modrinth or CurseForge Discover address for a Horizon instance to Horizon's matching tab before that page opens: shaders to Shaders, everything else to Resource Packs. When the registry has nothing of a kind, the tab says so instead of "No results". Horizon's packs come from the registry Horizon's launcher uses, `https://horizon-rust.github.io/horizon-mods/index.json`, which is read again after 10 minutes. Only versions made for the instance's Minecraft version are listed, and revoked files never are. A download is installed only when its size and SHA-256 match the registry, and it replaces an older version of the same pack, which is removed only once the new one is in. A file only counts as one of a pack's versions when its name is exactly one of that pack's file names in the registry, so a pack someone renamed or edited is never taken for it or removed. Installs go through the app's own content system into the instance's game folder, where Horizon reads them: `resourcepacks/` and `shaderpacks/` beside `options.txt`. They show in the instance's content list under their registry name, icon, version and authors, like CurseForge files do, so they can be turned off or removed there too. Each pack links to its page on playhorizon.dev.

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


### Theme gallery

The Themes tab shows the theme gallery as Coming soon (`apps/app-frontend/src/features/plus-themes/theme-gallery.vue`), with a few sample themes behind it and nothing to download, because sharing themes needs somewhere to host them first. The sample themes are in `gallery.json`, and `readGallery` already checks a gallery file the way an imported theme file is checked, so the gallery can be filled in there later. Its test checks every theme in the file passes with no contrast warnings.

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

Your own logo is a picture chosen in the same place (`apps/app-frontend/src/features/plus-logos/custom-logo.ts`). It's checked and copied into Pawrinth's data the way background pictures are, so the original can move. Its width and height are set in pixels for the title bar, from 16 to 320 wide and 12 to 44 tall. With Keep the picture's proportions on, changing one side changes the other, and with it off, the sides set the space the picture gets: either way the picture is fitted inside, never stretched. Rounded corners are optional and follow the shorter side. Changes show in a preview and only apply with Apply, and Reset to default goes back to the paw. The window icon is drawn from the picture too, and stays the paw if it can't be. If the picture goes missing, the paw shows instead.

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
