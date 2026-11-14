# Studio 256 Game Launcher

Studio 256 brings your installed PC games into one library. Launch a game from its cover, organize games into groups, or switch to Big Picture to browse with a controller.

## Download for Windows

Visit the [latest Windows release](https://github.com/IntelGenV2/Game-Launcher/releases/latest) and run the x64 setup file. For the portable version, extract the x64 portable ZIP and open the launcher inside. To check for a newer version from the launcher, open **Settings → Updates**.

## What it does

- Scans the stores you already have installed and lists those games in one place
- Launch from cover art (usually through the store client — same idea as Playnite)
- Groups with a fanned cover stack; each grouped cover keeps that group’s color border
- Bulk select and desktop keyboard navigation; controller support in Big Picture only
- **Big Picture** — fullscreen, controller-first couch mode
- Pinned games, hide, custom sort order (old favorites are merged into pins)
- Pinned games above the games list, separate from groups
- Grid/list views for games and groups, combined search/store/status filters, and random-pick previews
- Linked store copies with a preferred installation; original launch targets are retained
- Process-based launch status and playtime; Steam local playtime is imported when available
- Library backup/restore, plus seven rotating daily local backups while the launcher is open
- Custom draggable title bar with minimize, maximize/restore, and close controls
- Cover art from Steam / Epic / Xbox catalog / Wikipedia / optional SteamGridDB key
- Themes and grid customization in Settings
- Auto-update from GitHub Releases

## Big Picture

Sidebar → **Big Picture**. The couch UI covers the library, then the window goes fullscreen. Built for a gamepad first; keyboard still works.

**Sections** (left rail): Library, Stats, Power. Enable optional **System** in Settings → System for component sections covering the processor, RAM modules, graphics drivers, displays, storage and Windows, plus file browsing. **B** focuses the rail — it pops out and enlarges. Up/down highlight, **A** opens that section. **LB / RB** (or **Q / E**) also jump between sections.

**Library** is a wrapping 3D cover wheel. Left/right pick a game. Up opens a folder or game info. Down closes the folder. **A** plays or opens a folder. Groups show as a fanned stack of covers.

**Game info** opens with **Back** selected, above the cover. Left/right move within a row. Up/down move between rows: Back → Overview/Sessions → Play / Pin / folders.

**Files:** left/right moves the place chips or the file list, up/down switches between those rows, **A** / Enter opens. **B** / Esc opens the side menu. **LB / RB** still switch sections. Extra keyboard shortcuts stay off on that screen.

**System** shows hardware details in component sections. **Power** provides sleep, lock, restart and shut down with confirmation. Leave Big Picture from Power → Desktop.

| Control | Gamepad | Keyboard |
| --- | --- | --- |
| Move | D-pad / left stick | Arrows |
| Select / play / open | A | Enter |
| Side menu | B | Esc |
| Game info / open folder | Up | Up |
| Close folder | Down | Down |
| Browse (filter / sort / keyboard) | Y | Y |
| Cycle sections | LB / RB | Q / E |
| System | Start | — |

Controller input is active only in Big Picture. The desktop library supports mouse and keyboard navigation.

## Find your games

Choose **Rescan** after installing a game. The launcher checks for installations from Steam, Epic, GOG, Xbox, EA, Battle.net, Ubisoft, Riot, Rockstar, Amazon Games, itch.io, Humble, Wargaming, and Roblox. You can also add a game by dragging its executable onto the library.

Games that disappear from disk show **Unavailable** and retain their pins and playtime. Open the game to locate it or retry detection.

## Cover art

Tried in order (portrait/square box art only — no wide headers):

1. Steam library capsule (when an AppID is known or found by name)
2. Epic product art (Epic titles)
3. Microsoft Store catalog (Xbox titles)
4. IGDB box art (optional Twitch Client ID + Secret in Settings → Covers)
5. SteamGridDB (optional key in Settings → Covers)
6. Wikipedia
7. Roblox brand art (Roblox only)

IGDB is the game catalog Twitch uses: look up the title (Steam ID first, then name) and take that record’s cover/metadata. Discord is different — it only matches a running `.exe` filename against its own detectable-games list, which is why some launches never show as “Playing …”.

You can also set a custom cover on a game (Edit → Cover art); the image is copied into app storage.

Settings → Covers separates **Refresh downloaded artwork** from **Delete custom artwork**. Refresh preserves manually chosen and install-folder artwork. Deleting custom artwork creates a recovery backup first. Provider credentials live under **Advanced artwork providers**.

## Data

`%APPDATA%\IntelLauncher\`

Library DB, cached covers, etc. live there.

Settings → Library previews backup contents before a restore. Restoration replaces the library on restart and preserves a recovery zip in the backup folder. Backups exclude provider credentials and game saves. Imported histories for linked copies use the largest baseline; locally observed sessions are added once.

## Notes

- Opening a game often flashes the store client. That’s normal.
- Only one launcher window runs at a time. Opening it again (or clicking the shortcut while it’s in the tray) focuses the existing instance.

Appearance settings include a live translucency slider (0–70%). Cancel restores the previous appearance; Save persists it. The Studio 256 gradient wordmark and library controls share the custom window title bar.

Drag anywhere on the title bar background, wordmark or empty control-container space to move the window; inputs and buttons remain interactive. Double-click the background to maximize or restore. The quick filters use an animated sliding selection, with separate Stores and Sort menus.
