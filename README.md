# IntelGen Game Launcher

A Windows desktop launcher that pulls your games from multiple stores into one translucent cover-art library.

## Features

- Auto-discovers games from **Steam**, **Epic**, **GOG**, **Battle.net**, **Ubisoft Connect**, **Xbox / Game Pass** (best-effort), plus **manual** `.exe` adds
- Clickable cover art to launch
- Search, store filters, favorites
- Playtime & last played (imports Steam playtime when available; tracks sessions launched from this app)
- In-app updates from **GitHub Releases** (Settings → Check for updates)
- Glass / acrylic UI (Windows acrylic where supported)

## Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/tools/install) (rustup)
- Windows: [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the “Desktop development with C++” workload

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

## Updates (GitHub)

Step-by-step: **[UPDATES.md](./UPDATES.md)**

1. Build with the signing key loaded (see UPDATES.md)
2. Run `.\scripts\make-latest-json.ps1 -GitHubRepoUrl "https://github.com/IntelGenV2/Game-Launcher"`
3. Create a GitHub Release tagged `vX.Y.Z` and upload the three files from `release-assets`

Repo: [IntelGenV2/Game-Launcher](https://github.com/IntelGenV2/Game-Launcher) (must stay public)

## Cover art

Covers are fetched automatically from (in order):

1. Steam store app details
2. Epic store-content API
3. Wikipedia game pages
4. SteamGridDB (optional free API key in Settings)

You can also **Set cover art…** on any game; the image is copied into app storage.

## FPS

While a game is running, the launcher captures live FPS with Intel PresentMon (downloaded once into the app data folder).

## Data location

`%APPDATA%\IntelLauncher\`

## Notes

- Launching often briefly opens the store client (Steam/Epic/etc.) — that is normal.
- Xbox / Microsoft Store discovery is best-effort and may miss UWP-only titles.
- Games removed from disk are marked **Missing** but keep favorites and playtime history.
- Do not commit `node_modules`, `src-tauri/target`, or the updater private `.key` file.
