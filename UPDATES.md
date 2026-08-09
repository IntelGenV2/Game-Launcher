# How to put IntelGen Game Launcher on GitHub

There are **two different places** on GitHub. Do not mix them up.


| Place         | What it is                                                                                                   | What goes there                       |
| ------------- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------- |
| **Code repo** | [https://github.com/IntelGenV2/Game-Launcher](https://github.com/IntelGenV2/Game-Launcher)                   | Source code only (small)              |
| **Release**   | [https://github.com/IntelGenV2/Game-Launcher/releases](https://github.com/IntelGenV2/Game-Launcher/releases) | The Windows installer people download |


The in-app **Check for updates** button only reads the **Release**, not the code folder.

---

## Part 0 — What to keep on your PC vs what to push

### Delete / never push (already cleaned for you if you asked the agent)

These are rebuildable or private:


| Folder / file                                                | Why                                                              |
| ------------------------------------------------------------ | ---------------------------------------------------------------- |
| `node_modules\`                                              | Reinstall with `npm install`                                     |
| `src-tauri\target\`                                          | Multi‑GB Rust build cache                                        |
| `dist\`                                                      | Frontend build output                                            |
| `release-assets\`                                            | Upload to the **Release** only — do **not** put in the code repo |
| `*.key` / `C:\Users\lawbo\.tauri\intelgen-game-launcher.key` | Private signing key — **never** on GitHub                        |




### Do push (code repo)

```
.github/
app-icon/
public/
scripts/
src/
src-tauri/          (but NOT src-tauri/target)
.gitignore
UPDATES.md
README.md
index.html
package.json
package-lock.json
tsconfig.json
tsconfig.node.json
vite.config.ts
```

Your `.gitignore` already blocks `node_modules`, `dist`, `src-tauri/target`, and `release-assets`.

---



## Part 1 — Push the code to the repo (one time, then whenever you change code)

Open **PowerShell**:

```powershell
cd "G:\Software Projects\IntelGenV2\IntelGenV2 Game Launcher"

# Start git if needed
git init
git remote remove origin 2>$null
git remote add origin https://github.com/IntelGenV2/Game-Launcher.git

git add .
git status
```

Look at `git status`. You should **not** see `node_modules`, `target`, `dist`, or `release-assets`.

```powershell
git commit -m "Update launcher with GitHub updater"
git branch -M main
git push -u origin main
```



### If push is rejected

GitHub already has older commits. Use:

```powershell
git pull origin main --rebase
git push -u origin main
```

Or, only if you intend to replace everything on GitHub with your local copy:

```powershell
git push -u origin main --force
```

(Only use `--force` if you understand it overwrites the remote `main` branch.)

After this, open [https://github.com/IntelGenV2/Game-Launcher](https://github.com/IntelGenV2/Game-Launcher) and confirm the code looks right.  
**This step alone does not create a downloadable installer.** You still need Part 2.

---



## Part 2 — Create a downloadable Release (the installer)

You already built a signed installer. On your PC you should have:

```
G:\Software Projects\IntelGenV2\IntelGenV2 Game Launcher\release-assets\
  IntelGen Game Launcher_0.1.0_x64-setup.exe
  IntelGen Game Launcher_0.1.0_x64-setup.exe.sig
  latest.json
```

If that folder is missing, rebuild first (Part 3), then come back here.

### Upload to GitHub Releases (click-by-click)

1. Open: [https://github.com/IntelGenV2/Game-Launcher/releases/new](https://github.com/IntelGenV2/Game-Launcher/releases/new)
2. **Choose a tag** → type exactly: `v0.1.0` → **Create new tag: v0.1.0 on publish**
3. **Release title**: `IntelGen Game Launcher v0.1.0`
4. Description (optional): `First public Windows installer.`
5. Drag **all 3 files** from `release-assets` into the “Attach binaries” box:
  - `.exe`  
  - `.sig`  
  - `latest.json`
6. Leave “Set as the latest release” checked
7. Click **Publish release**



### Verify

Open: [https://github.com/IntelGenV2/Game-Launcher/releases/latest](https://github.com/IntelGenV2/Game-Launcher/releases/latest)  

You should see the three files listed as assets.  
This URL must work in a browser (no login):

[https://github.com/IntelGenV2/Game-Launcher/releases/latest/download/latest.json](https://github.com/IntelGenV2/Game-Launcher/releases/latest/download/latest.json)  

The file must start with `{`. If **Check for updates** says `error decoding response body`, the uploaded `latest.json` has a UTF-8 BOM — use the fixed file from `release-assets` (re-run `make-latest-json.ps1` if needed) and **replace** that asset on the Release.

People install with the `.exe`. The app updater uses `latest.json` + the `.exe` + `.sig`.

---

## Fix a broken latest.json on an existing Release

1. Use the fixed `release-assets\latest.json` on your PC (no BOM).
2. Open https://github.com/IntelGenV2/Game-Launcher/releases
3. Edit the latest release → delete the old `latest.json` asset → upload the new one
4. Try **Check for updates** again

**404 Not Found on download:** GitHub turns spaces in filenames into dots  
(`IntelGen Game Launcher_…` → `IntelGen.Game.Launcher_…`).  
`latest.json` must use that exact name in `"url"`. Re-run `make-latest-json.ps1` (it does this now) and replace `latest.json` on the Release.

Note: if the installed app is already `0.1.0` and the Release is also `0.1.0`, a successful check will say you’re up to date (that’s correct). To test an actual update, publish `0.1.1`.

---



## Part 3 — How to build a new signed installer later

### Fast way (recommended)

```powershell
cd "G:\Software Projects\IntelGenV2\IntelGenV2 Game Launcher"
.\scripts\release.ps1 -Version 0.1.3
```

**Version format:** `MAJOR.MINOR.PATCH` with **no** leading `v`  
Examples: `0.1.3` · `1.0.0` · `0.2.0-beta.1`

The script updates the three version files, runs a signed build, and fills `release-assets\`. Then create a GitHub Release tagged `v0.1.3` and upload those files.

### Manual way

Do this when you change the app or bump the version.

### 1. Bump version (same number in all three)

- `package.json`
- `src-tauri/tauri.conf.json` → `"version"`
- `src-tauri/Cargo.toml` → `version`

Example: `0.1.0` → `0.1.1`

### 2. Build (same PowerShell window for all lines)

```powershell
cd "G:\Software Projects\IntelGenV2\IntelGenV2 Game Launcher"
npm install

$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\intelgen-game-launcher.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""

npm run tauri build
```

Success = you get **both** an `.exe` and a `.sig` under:

`src-tauri\target\release\bundle\nsis\`

### 3. Pack files for upload

```powershell
.\scripts\make-latest-json.ps1 -GitHubRepoUrl "https://github.com/IntelGenV2/Game-Launcher"
```



### 4. New GitHub Release

Same as Part 2, but tag `v0.1.1` (match your new version), upload the new 3 files from `release-assets`.

### 5. Test the update button

Install the **old** version, open **Settings → Check for updates**. It should offer the new one.

---



## Quick checklist

**Code repo**

- [ ] No `node_modules` / `target` / `dist` / `release-assets` / `.key` in the push  
- [ ] `git push` to [https://github.com/IntelGenV2/Game-Launcher](https://github.com/IntelGenV2/Game-Launcher)  

**Release**

- [ ] Tag like `v0.1.0`  
- [ ] Uploaded `.exe` + `.sig` + `latest.json`  
- [ ] `latest.json` opens in a browser with no login  

**Signing key**

- [ ] Exists at `C:\Users\lawbo\.tauri\intelgen-game-launcher.key`  
- [ ] Backed up privately  
- [ ] Never committed  

---

## GitHub Actions signing secret (fixes CI “Missing comment in secret key”)

The Actions error `failed to decode secret key: Missing comment in secret key` means the repo secret is missing, empty, or not the full `.key` file.

1. Open https://github.com/IntelGenV2/Game-Launcher/settings/secrets/actions  
2. **New repository secret** (not an Environment secret)  
3. Name: `TAURI_SIGNING_PRIVATE_KEY`  
4. Value = **entire** contents of:
   `C:\Users\lawbo\.tauri\intelgen-game-launcher.key`  
   One long line starting with `dW50cnVzdGVk…` (~348 characters). No quotes.  
5. Do **not** set `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (the key has no password; the workflow sets it to empty).

Copy the key in PowerShell:

```powershell
Get-Content "$env:USERPROFILE\.tauri\intelgen-game-launcher.key" -Raw | Set-Clipboard
```

Then paste into the GitHub secret value box and save.

Also push your latest local code before re-running the workflow (CI was still building `0.1.0`).

---

## Common mistakes


| Mistake                                                | Result                                            |
| ------------------------------------------------------ | ------------------------------------------------- |
| Only push code, no Release                             | No installer / update button fails                |
| Upload only the `.exe`                                 | Updater cannot verify the update                  |
| Put `release-assets` in the code repo                  | Repo gets huge; still doesn’t create a Release    |
| Private repo                                           | Updater cannot download files                     |
| Forget to set `TAURI_SIGNING_PRIVATE_KEY` before build | No `.sig` file                                    |
| Wrong / old key password                               | Build signing fails — use empty password as above |
| **Missing comment in secret key** (GitHub Actions)     | Repo secret wrong/empty — paste full `.key` file as a **repository** secret |
| **error decoding response body**                       | Bad `latest.json` (UTF-8 BOM). Replace the Release asset with the fixed file from `release-assets` |
| **404 Not Found** on download                          | `latest.json` URL doesn’t match the Release filename (spaces vs dots). Replace `latest.json` |


