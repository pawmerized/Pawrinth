# Modrinth Branding Audit

Every place the app still refers to Modrinth, what kind of reference it is, and what was done. Searched: `apps/app`, `apps/app-frontend/src`, `apps/app-frontend/index.html`, `packages/app-lib/src`, and the parts of `packages/ui/src` the app can show. `apps/frontend`, `apps/labrinth` and `apps/docs` are the website, API and docs, which Pawrinth doesn't ship.

Categories:

1. Required Modrinth integration
2. Required legal attribution
3. Pawrinth functionality that needed rebranding
4. Unnecessary inherited functionality
5. Functionality that must stay unchanged

## Removed (category 4)

| What | Where it was | Why |
| --- | --- | --- |
| Report an ad | `features/settings/system/troubleshooting-settings.vue` | Pawrinth has no ads to report |
| Ad consent popup and preferences | `App.vue`, `features/settings/account/privacy-settings.vue` | No ads, and "ads support Modrinth" was false here |
| Telemetry switch, and the Privacy tab it left empty | `features/settings/account/privacy-settings.vue`, `app-settings-modal/index.vue` | Analytics are off in this fork, so the switch did nothing |
| Ad sidebar and its upgrade link | `app/shell/sidebar/promotion.vue` | Never shown in this fork |
| Ad window plugin | `apps/app/src/api/ads.rs`, `ads_occlusion_*.rs`, `ads-consent/`, `capabilities/ads.json`, `helpers/ads.js` | Code that could never run, and it carried the "Modrinth App" browser identity |
| Upgrade to Modrinth+ menu item | `App.vue` | Promotion for a service the fork doesn't sell |
| Hide promotions toggle | `features/plus-settings/interface-settings.vue`, `features/plus-wizard/setup-wizard.vue`, `plus/settings.rs` | Nothing left for it to hide |
| Pride fundraiser banner | `features/campaigns/` | Modrinth's own campaign |
| Survey popup and Tally script | `components/ui/SurveyPopup.vue`, `index.html`, Tauri content policy | It sent feedback to the Modrinth team, and nothing used it any more |
| Modrinth's critical announcement | `App.vue` | Written for Modrinth App's versions, not this fork's |
| "Get support" buttons that led to Modrinth Support for app problems | `ErrorModal.vue`, `MinecraftAuthErrorModal.vue`, `MinecraftRequiredModal.vue` | Problems here may come from the fork, and Pawrinth has no support page yet to link instead |
| Old all-users uninstall migration | `apps/app/nsis/hooks.nsi` | Written for Modrinth App's move away from all-users installs. Pawrinth never had one |
| Windows MSI installer | Bundle targets | A second, unbranded installer with a different upgrade path could leave duplicate installs |
| Translations of removed text | Locale files | Unused labels |

## Rebranded (category 3)

| What | Where | Now |
| --- | --- | --- |
| Error messages that said "the Pawrinth" | `ErrorModal.vue`, `HostingUpdateRequired.vue`, shared instance settings | "Pawrinth" |
| Debug info "to share with Modrinth support" | `troubleshooting-settings.vue` | "to share when reporting a problem" |
| "after Modrinth finds them", "managed by Modrinth's launch settings" | Game settings | Pawrinth |
| "compacted by Modrinth App" in log viewer | `packages/app-lib/src/api/logs.rs` | Pawrinth |
| "another Modrinth process is using this application directory" | `state/content_store/store.rs` | Pawrinth |
| Sign-in reply page title | `oauth_utils/auth_code_reply/page.html` | "Sign In - Pawrinth". The sign-in flow itself is unchanged |
| "preferred color theme across Modrinth", "links which go outside of Modrinth" | `layouts/shared/appearance-settings/messages.ts` | Pawrinth |
| "% of the Modrinth App is available in this language" | Language settings | Pawrinth |
| "from the Modrinth App" on the servers page | `ServerListEmpty.vue` | Pawrinth |
| "Modrinth App" in other languages' translations | Locale files, 172 entries | Pawrinth, where the English no longer mentions it |
| Instance icon named "Modrinth Wrench" | `editor-catalog.ts` | Removed from the picker, because the picture is Modrinth's logo |
| App description, publisher, copyright line | `apps/app/tauri.conf.json` | Says Pawrinth is an unofficial, community-developed fork |
| Database backups | `state/db_backup.rs` | `%LOCALAPPDATA%\Pawrinth Backups`. They were going into Modrinth's own backup folder |
| `.mrpack` files | Installer | Pawrinth offers to open them without replacing Modrinth App's handler. The stock installer would have overwritten it |
| Developer readme | `apps/app/README.md` | Describes Pawrinth |

Already done in earlier rounds: application name, window title, identifiers, icons, loading screen, deep link scheme, user agent, Pawrinth News, onboarding text, About page.

## Kept on purpose

| What | Where | Category |
| --- | --- | --- |
| Sign in to Modrinth, Modrinth accounts, session handling | `mr_auth.rs`, `ModrinthAccountRequiredModal.vue`, checklist | 1, 5 |
| Modrinth search, projects, versions, downloads, updates | `api-client`, browse and project pages | 1, 5 |
| `modrinth.com` and `cdn.modrinth.com` links on projects, users, organizations and reports | `pages/project/*`, `pages/User.vue`, `pages/Organization.vue` | 1 |
| "isn't published on Modrinth" safety warnings | `UnknownFileWarningModal.vue`, `InstallToPlayModal.vue`, `ContentDiffModal.vue` and others | 1, 5 |
| "Modrinth modpack" and the `.mrpack` format | `ExportModal.vue`, `tauri.conf.json` | 1 |
| Friends, shared instances, invites, profile and social settings | `features/friends`, `shared-instances`, `layouts/wrapped/settings` | 1, 5 |
| Modrinth Hosting pages and billing, including their Modrinth Support links | `layouts/wrapped/hosting`, `billing` | 1, 5 |
| Modrinth Pride skins, Modrinth+ and staff badges on profiles | `pages/Skins.vue`, `UserBadges.vue` | 1. They describe a real Modrinth account |
| Modrinth Support link for Modrinth account sign-in trouble | `ModrinthAccountRequiredModal.vue` | 1. Only Modrinth can help with those accounts |
| Modrinth's article on Microsoft sign-in problems | `ErrorModal.vue`, `minecraft-auth-errors.ts` | 1. It applies to any launcher, and it's labelled as Modrinth's |
| Modrinth's own news link | `features/news/news-sidebar.vue` | 1. Clearly labelled, beside Pawrinth News |
| "Open in Modrinth" | `utils/common-messages.ts` | 1 |
| License, copying files, credit to the Modrinth team, source link | `LICENSE`, `COPYING.md`, `about-settings.vue`, `README.md` | 2 |
| "unofficial fork of Modrinth App" in the user agent and About page | `lib.rs`, `minecraft_auth.rs`, `modrinth-client.ts`, `about-settings.vue` | 2. It's the required non-affiliation statement |
| Refusing to open Modrinth App's data folders | `plus/guard.rs` | 5 |
| Internal names such as `ModrinthCredentials`, `theseus`, `@modrinth/ui` | Source code | 5. Identifiers the app is built on, never shown to users |
| `AdsConsentRequired` event and the `show_ads` page context | `event/mod.rs`, `page-context.ts` | 5. Internal and unused. Shared with the frontend's generated code and other shared components |
| `personalized_ads` and `telemetry` database columns | `state/settings.rs` | 5. Removing upstream columns would break upgrades |
| Text only the website can show | `OpenInAppModal.vue`, website-only parts of `packages/ui` | Not reachable in the app |

## Not done, because it doesn't exist yet

- **Pawrinth support page.** The app says to report problems to the fork, but there's nowhere to link. Don't add a link until there's a real one.
- **Pawrinth website and download page.** The Discord "Install Pawrinth" button stays hidden until `PAWRINTH_DOWNLOAD_URL` is set.
