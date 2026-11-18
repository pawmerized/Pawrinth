# Pawrinth privacy policy

Last updated 17 November 2026.

Pawrinth is a Minecraft launcher that runs on your computer. Its developer runs no servers for it and receives nothing from it: no analytics, no telemetry, no crash reports and no ads. Pawrinth's analytics calls inherited from Modrinth App do nothing, and Modrinth App's ads and Sentry error reporting were removed.

## What stays on your computer

- **Minecraft accounts.** Signing in goes through Microsoft. The sign-in tokens Pawrinth needs to start the game are kept in Pawrinth's own database on your computer.
- **CurseForge API key.** If you add one, it's kept in your operating system's credential store (Windows Credential Manager, macOS Keychain or the Secret Service on Linux). It is never written to a settings file or a log, and it's only ever sent to `api.curseforge.com`.
- **Instances, settings, themes, skins, world backups, Update history and your mod library.** These stay in Pawrinth's data folder.
- **Logs.** Game and launcher logs stay on your computer unless you share them.

## What goes to other services, and when

Pawrinth contacts these services only to do what you ask it to. Each one receives your IP address and what's needed for the request, and handles it under its own privacy policy.

| Service | When |
| --- | --- |
| Microsoft, Mojang (`minecraftservices.com`, `mojang.com`, `minecraft.net`) | Signing in, playing, skins and capes, downloading game files and server jars |
| Modrinth (`modrinth.com`) | Browsing and installing content, checking for updates, and any Modrinth account features you use, such as friends and shared instances |
| CurseForge (`api.curseforge.com`, `forgecdn.net`) | Only when CurseForge is set up: browsing, installing and recognising files. Files are matched by fingerprints calculated on your computer |
| curse.tools | Only if you turn on the community proxy in API configuration, instead of using your own CurseForge key |
| Technic (`technicpack.net`) and the servers Technic packs are hosted on | Browsing and installing Technic packs |
| Fabric, Quilt, Forge and NeoForge | Downloading mod loaders, and making server packs |
| MineSkin (`mineskin.org`) | Browsing the skin gallery |
| mclo.gs | Only if you turn on sending logs for crash analysis. Pawrinth's own crash help works without it |
| GitHub (`github.com`, `githubusercontent.com`, `github.io`) | Checking for Pawrinth updates, Pawrinth News, Horizon downloads, and Horizon's registry of resource packs and shader packs when you open it |
| Discord | Only if Discord status is on. Pawrinth talks to the Discord app on your computer, and Discord shows what you're playing |

Streamer mode and name blur only change what Pawrinth shows on screen. They don't change what these services receive.

## Importing from other launchers

Importing from the Modrinth App, the FTB App or other launchers reads their files on your computer. Pawrinth reads the Modrinth App's database from a copy and never changes the original. When importing Modrinth App settings, its telemetry and ad choices are never copied.

## Children

Pawrinth isn't directed at children and collects nothing from anyone. Minecraft accounts are subject to Microsoft's own rules for younger players.

## Changes and questions

Changes to this policy are made in this file, so its history shows every change. Questions can be asked on [Pawrinth's GitHub](https://github.com/pawmerized/Pawrinth/issues).
