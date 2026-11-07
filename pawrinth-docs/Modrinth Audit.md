# Modrinth audit and Pawrinth status

Audited at upstream `e5a5663` (v0.22.1 + 2 commits) on 2026-10-31. Status words: **Done** was built and tested here, **Partial** has real gaps listed, **Not started** has no fork work yet, **Upstream** already exists and was left alone, **Blocked** needs someone outside this project.

| Feature | What upstream has | Gap | Pawrinth status |
|---|---|---|---|
| Installed-mod management, Content tab | Search, sort, filters, bulk enable/disable/update/delete, live changes | No provider labels | Upstream. Provider recorded in history, not yet shown as a Content tab label |
| Update All, version selection, changelogs | Modal with versions and changelogs, runs through download manager | No summary, no history | Done: summary popup and history added |
| Review before updating | Update All already opens a selection modal | None | Upstream. A separate "review" setting would duplicate it |
| Update rollback | Backup deleted when an update succeeds; crash rollback only | No user-facing undo | Done: full, selective and per-file restore, conflict protection, integrity checks, undoable, crash recovery |
| Update history and retention settings | None | All | Done |
| Download manager | Progress, pause, resume, cancel | Retry and history not assessed | Upstream |
| Dependencies, compatibility | Two resolvers. Follows required only; `Incompatible` is never enforced; pinned dependencies skip checks | Labels, enforcement, explanations | Not started |
| Modpack updates | Exists | Not assessed | Upstream |
| Java, memory, instance settings | Exists, with inherit-from-global overrides | Duplicating an instance appears to drop overrides (read from code, not run) | Upstream. Bug not fixed |
| Instance duplication | Exists | See above | Upstream |
| Instance snapshots | "Backup" is only a duplicate, with no restore | Named snapshots | Partial: create and restore tested for content. Config files aren't included yet, and there is no snapshot-to-snapshot compare |
| Instance comparison | Only local vs shared instance | General compare | Partial: backend tested, modal built but not clicked through, and only "only in the other instance" items can be copied |
| Crash diagnostics | Auto-posts console text to mclo.gs when the logs page opens | Local analysis, privacy | Partial: local rules tested with sample lines, not a real game crash. mclo.gs is off unless turned on in Updates and recovery |
| Themes, accent colors | Dark, light, OLED, retro, system; no accent | Editor and presets | Done: editor, presets, import/export, contrast checks. Only Nord exercised live; editor color picking not clicked through |
| UI scaling | None. Windows display scaling only | All | Done: 80–150%, tested at 125%. Not tested on other monitors or Windows scaling levels |
| Compact layout, reduced motion | Compact library cards only | Global options | Partial: built, not visually checked |
| Modrinth Plus settings section | Tabbed settings | Section | Done as a "Pawrinth" section with Themes, Interface, Streamer mode, Update recovery, API configuration, About. No Obsidian or Experimental tab, because no in-app feature was defined for them |
| Promotional settings | Sidebar ad and Modrinth+ link | Toggle | Done: ad placement removed; hide-promotions toggle built |
| First-launch wizard | Welcome screen and 3-step checklist | Wizard | Done: six skippable steps ahead of the existing welcome screen, for new users only, and it can be reopened from settings. No Obsidian step, because no in-app Obsidian feature is defined |
| Project discovery, search history, recently viewed | Browse with filters; no history | All listed | Not started. Storage fields exist, nothing uses them yet |
| Duplicate project detection | None | All | Not started |
| Resource packs, shaders, datapacks | Managed like mods | None found | Upstream, and covered by rollback |
| Backups | Database backup on version change | Different purpose from rollback | Upstream |
| Logs | Live and historical | None | Upstream |
| CurseForge | Local instance import only; no API use | Everything else | Partial: the discovery page and key settings are finished, but real projects need each user's own API key, and no real key has been tested with this version. See below |
| Obsidian documentation | None | Connection | Blocked: the plugin isn't installed. These notes are ready to copy in |

## CurseForge

Pawrinth uses only the official API at `https://api.curseforge.com/v1`. It doesn't use scrapers, unofficial services or a proxy. Each user can add their own key under Settings, Pawrinth, API configuration.

**What the API allows without a key, checked 2026-10-31:**

| Endpoint | Without a key |
|---|---|
| `/categories` (classes and categories) | 200 |
| `/minecraft/version`, `/minecraft/modloader` | 200 |
| `/mods/search`, `/mods/{id}`, descriptions, files, changelogs | 403 |
| `/games`, `/mods/featured`, `/fingerprints` | 401 or 403 |

So real project search, details and downloads aren't possible without authorized access. A release build with no key shows no projects and explains why, with a button to the key settings. Only development builds contain the labelled made-up samples (`packages/app-lib/src/plus/curseforge_samples.rs`).

### How the key is handled

- **Storage:** only the operating system's credential store, through the `keyring` crate. On Windows that's Credential Manager, under `curseforge-api-key.com.pawmerized.pawrinth`. There's no plaintext fallback: if the store fails, the key isn't saved.
- **Exposure:** no command returns the key. Settings shows only its last four characters, and only for keys of 12 characters or more. It prints as redacted in debug output, is marked sensitive on HTTP requests, and is replaced in support reports.
- **Where it's sent:** only to CurseForge's own hosts, over HTTPS.
- **Saving:** a key CurseForge refuses is never saved. One that can't be checked right now, for example while offline, is saved and the screen says so.
- **Development and production:** development builds also read `CURSEFORGE_API_KEY`, and can point at a local stand-in with `PAWRINTH_CURSEFORGE_API_BASE`. Release builds ignore both. A saved key always wins over the variable.
- **Limits:** any program running under your Windows account can ask Credential Manager for the key. That's how the store works, and it's the same for every app using it.

### Built-in project key

Not implemented, on purpose. CurseForge's terms make a key non-transferable and bar sharing it with third parties, and they also bar reaching the API through a proxy, so a server that hides the key isn't an option either. Any key inside a public download can be extracted. If CurseForge approves one in writing, it would be one more `KeySource` in `curseforge_key.rs`, set when an official build is made and never committed. A key the user saved would still win.

### What has been tested

**Live with a real key (earlier build, before key settings):** search, Fabric API install, update check, update, rollback with a hash match.

**With invented keys against a local stand-in:** save, replace, cancel, remove, restart, rejected keys, cut-off keys, server errors, rate limits, a key revoked mid-session, and being offline. The test keys were then searched for in every file the app wrote, and none were found.

**With sample data only:** the discovery page, version selection, dependency planning, restricted projects, wrong-loader detection.

**Not tested:** a real CurseForge key in this version, and macOS or Linux credential stores.

**Not built:** modpack installs, creating an instance from the install screen, download manager progress for CurseForge, CurseForge in Modrinth's Update All screen, a source label in the Content tab, unified search, duplicate detection.

Terms, read 2026-10-31 (last modified 14 Aug 2024). This is a reading, not legal advice:

- Section 3.1 bars using the API to build anything competing with CurseForge, directly or indirectly.
- Section 3.1 bars saving or caching API data. Pawrinth stores project and file IDs for installed files, and rollback keeps old files.
- Section 2.2 makes keys unique and non-transferable, which is why no key ships in the app.

## Findings worth knowing

- Upstream's debug export copies `app.db` raw, and that database holds sign-in tokens. The troubleshooting screen suggests sharing the export with support. Not changed here.
- Instance files are plain copies, not hard links, so recovery data uses real disk space.
- `MICROSOFT_CLIENT_ID` is upstream's. See `PAWRINTH.md` before distributing.
