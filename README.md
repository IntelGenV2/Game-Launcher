# IntelGen Game Launcher

Windows desktop app that pulls your installed games from a bunch of store clients into one cover-art library. Built with Tauri 2 + React. Open source.

Repo: [IntelGenV2/Game-Launcher](https://github.com/IntelGenV2/Game-Launcher)

## What it does

- Scans the stores you already have installed and lists those games in one place
- Launch from cover art (usually through the store client — same idea as Playnite)
- Groups, bulk select, keyboard + controller navigation
- Favorites, hide, custom sort order
- Playtime for sessions started here; Steam local playtime is imported when available
- Cover art from Steam / Epic / Xbox catalog / Wikipedia / optional SteamGridDB key
- Themes and grid customization in Settings
- Auto-update from GitHub Releases

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
