# IntelGen Game Launcher

Windows desktop app that pulls your installed games from a bunch of store clients into one cover-art library. Built with Tauri 2 + React. Open source.

Repo: [IntelGenV2/Game-Launcher](https://github.com/IntelGenV2/Game-Launcher)

## What it does

- Scans the stores you already have installed and lists those games in one place
- Launch from cover art (usually through the store client — same idea as Playnite)
- Groups with a fanned cover stack; each grouped cover keeps that group’s color border
- Bulk select, keyboard + controller navigation
- **Big Picture** — fullscreen, controller-first couch mode
- Favorites, hide, custom sort order
- Playtime for sessions started here; Steam local playtime is imported when available
- Cover art from Steam / Epic / Xbox catalog / Wikipedia / optional SteamGridDB key
- Themes and grid customization in Settings
- Auto-update from GitHub Releases

## Big Picture

Sidebar → **Big Picture** (goes fullscreen). Built for a gamepad first; keyboard still works.

**Sections** (left rail, or **LB / RB** / **Q / E**): Library, Files, Stats, System.

**Library** is a wrapping 3D cover wheel. Groups show as a fanned stack of covers. **A** / Enter opens a group (covers slide out of the fan onto the wheel). **B** / Esc slides them back in; the rest of the library slides back into place. Ungrouped games sit on the same wheel.

**System** can sleep, lock, restart, or shut down the PC after a confirm. Cancel is the default.

| Control | Gamepad | Keyboard |
| --- | --- | --- |
| Move | D-pad / left stick | Arrows |
| Play / open / select | A | Enter |
| Back / close group | B | Esc |
| Game info | X | — |
| Browse (filter / sort) | Y | Y |
| Cycle sections | LB / RB | Q / E |
| System | Start | — |
| Rail | Up from the list, then left / right | Up, then left / right |

Desktop library still uses D-pad to move, A to open, B back, X favorite, View for select mode.

## How scanning works

Rescan walks local install metadata only. No store logins. If a game isn’t installed (or the client never wrote a path), it won’t show up.

| Store | Where it looks |
| --- | --- |
| **Steam** | Steam library folders / `appmanifest_*.acf` |
| **Epic** | `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` |
| **GOG** | `HKLM\SOFTWARE\…\GOG.com\Games` |
| **Battle.net** | Uninstall entries with `Battle.net --uid=…`, `Battle.net.config` **only if Path exists**, `product.db` path hints, Call of Duty HQ folders |
| **Ubisoft** | `HKLM\…\Ubisoft\Launcher\Installs` |
| **Xbox / Game Pass** | `XboxGames` / `Xbox Games` folders on each drive (best-effort) |
| **EA App** | `C:\ProgramData\EA Desktop\InstallData` + matching install folders |
| **Roblox** | Local Roblox Player install under LocalAppData |
| **Wargaming** | Game Center prefs / uninstall / common WoT·WoWs·WoWp folders |
| **Riot** | `%ProgramData%\Riot Games\Metadata\*.live\*.product_settings.yaml` (needs `product_install_full_path`) and HKCU uninstall keys |
| **Rockstar** | Uninstall strings with `uninstall={titleId}` (known title map) and `HKLM\SOFTWARE\Rockstar Games\*\InstallFolder` |
| **Amazon Games** | `%LOCALAPPDATA%\Amazon Games\Data\Games\Sql\GameInstallInfo.sqlite` (`Installed = 1`) |
| **itch.io** | `%APPDATA%\itch\db\butler.db` caves + install locations |
| **Humble App** | `%APPDATA%\Humble App\config.json` → `game-collection-4` (`downloaded` / `installed`) |
| **Manual** | Add an `.exe` from the UI, or drop one onto the library |

Owned-but-not-installed library lists (Amazon account, Humble website keys, itch owned keys, etc.) are **not** imported. That needs account APIs and we deliberately skip it.

Games that disappear from disk get marked **Missing** so favorites / playtime stick around.

## Prerequisites

- Node.js 18+
- Rust (rustup)
- Windows: Visual Studio Build Tools with the “Desktop development with C++” workload

## Run

```powershell
npm install
.\scripts\dev.ps1
```

Or:

```bash
npm install
npm run tauri dev
```

## Build

```powershell
npm install
npm run tauri build
```

Installer is NSIS (`lzma`). For a signed build the in-app updater can use, see **Releasing**.

## Releasing

GitHub Actions **Release** (`.github/workflows/release.yml`) runs on `v*` tags and `workflow_dispatch`. It checks out **that git commit**, then `tauri-apps/tauri-action` builds it. Local uncommitted files are not in the installer.

Push every file the tag needs **before** tagging (frontend + `src-tauri` + workflow). A tag on an old `main` will ship old code even if your working tree is newer.

Repo secret required: `TAURI_SIGNING_PRIVATE_KEY` (full contents of `%USERPROFILE%\.tauri\intelgen-game-launcher.key`).

Local alternative:

```powershell
.\scripts\release.ps1 -Version 0.3.5 -Notes "Short release notes"
```

That bumps `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`, signed-builds, and writes `release-assets\` (installer, `.sig`, `latest.json`, optional portable zip). Upload those to a GitHub Release tagged `v0.3.5`. The app checks:

`https://github.com/IntelGenV2/Game-Launcher/releases/latest/download/latest.json`

## Cover art

Tried in order (portrait/square box art only — no wide headers):

1. Steam library capsule (when an AppID is known or found by name)
2. Epic product art (Epic titles)
3. Microsoft Store catalog (Xbox titles)
4. SteamGridDB (optional key in Settings → Covers)
5. Wikipedia
6. Roblox brand art (Roblox only)

You can also set a custom cover on a game (Edit → Cover art); the image is copied into app storage.

## Data

`%APPDATA%\IntelLauncher\`

Library DB, cached covers, etc. live there.

## Notes

- Opening a game often flashes the store client. That’s normal.
- Battle.net / Rockstar / Riot detection follows the same local signals other launchers use; short product codes are matched carefully so random folders don’t become “Overwatch 2”.
- Don’t commit `node_modules` or `src-tauri/target`.
