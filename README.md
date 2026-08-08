# IntelGen Game Launcher

A Windows desktop launcher that pulls your games from multiple stores into one translucent cover-art library.

## Features

- Auto-discovers games from **Steam**, **Epic**, **GOG**, **Battle.net**, **Ubisoft Connect**, **Xbox / Game Pass** (best-effort), plus **manual** `.exe` adds
- Clickable cover art to launch
- Search, store filters, favorites
- Playtime & last played (imports Steam playtime when available; tracks sessions launched from this app)
- Glass / acrylic UI (Windows acrylic where supported)

## Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/tools/install) (rustup)
- Windows: [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the “Desktop development with C++” workload

## Run

From a normal PowerShell (sets up the MSVC toolchain automatically):

```powershell
.\scripts\dev.ps1
```

Or manually:

```bash
npm install
npm run tauri dev
```

(If `link.exe` / `kernel32.lib` errors appear, open an “x64 Native Tools Command Prompt for VS” or run `.\scripts\dev.ps1`.)

## Build

```bash
npm run tauri build
```

## Cover art

Covers are fetched automatically from (in order):

1. Steam store app details (works for modern Steam titles like PEAK / RV There Yet)
2. Epic store-content API (Fortnite and other Epic games)
3. Wikipedia game pages
4. SteamGridDB (optional free API key in Settings — improves match rate further)

You can also **Set cover art…** on any game; the image is copied into app storage.

## FPS

While a game is running, the launcher captures live FPS with Intel PresentMon (downloaded once into the app data folder). Charts update from those samples.

## Data location

Library database and cached covers live under:

`%APPDATA%\IntelLauncher\`

(Older installs under `%APPDATA%\UnifiedGameLauncher\` are moved here automatically on launch.)

## Notes

- Launching often briefly opens the store client (Steam/Epic/etc.) — that is normal.
- Xbox / Microsoft Store discovery is best-effort and may miss UWP-only titles.
- Games removed from disk are marked **Missing** but keep favorites and playtime history.
