# How to put IntelGen Game Launcher on GitHub

There are **two different places** on GitHub. Do not mix them up.

| Place | What it is | What goes there |
| --- | --- | --- |
| **Code repo** | https://github.com/IntelGenV2/Game-Launcher | Source code only (small) |
| **Release** | https://github.com/IntelGenV2/Game-Launcher/releases | The Windows installer people download |

The in-app **Check for updates** button only reads the **Release**, not the code folder.

---

## Part 0 — What to keep on your PC vs what to push

### Delete / never push (already cleaned for you if you asked the agent)

These are rebuildable or private:

| Folder / file | Why |
| --- | --- |
| `node_modules\` | Reinstall with `npm install` |
| `src-tauri\target\` | Multi‑GB Rust build cache |
| `dist\` | Frontend build output |
| `release-assets\` | Upload to the **Release** only — do **not** put in the code repo |
| `*.key` / `C:\Users\lawbo\.tauri\intelgen-game-launcher.key` | Private signing key — **never** on GitHub |

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

After this, open https://github.com/IntelGenV2/Game-Launcher and confirm the code looks right.  
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

1. Open: https://github.com/IntelGenV2/Game-Launcher/releases/new  
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

Open: https://github.com/IntelGenV2/Game-Launcher/releases/latest  

You should see the three files listed as assets.  
This URL must work in a browser (no login):

https://github.com/IntelGenV2/Game-Launcher/releases/latest/download/latest.json  

People install with the `.exe`. The app updater uses `latest.json` + the `.exe` + `.sig`.

---

## Part 3 — How to build a new signed installer later

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
- [ ] `git push` to https://github.com/IntelGenV2/Game-Launcher  

**Release**

- [ ] Tag like `v0.1.0`  
- [ ] Uploaded `.exe` + `.sig` + `latest.json`  
- [ ] `latest.json` opens in a browser with no login  

**Signing key**

- [ ] Exists at `C:\Users\lawbo\.tauri\intelgen-game-launcher.key`  
- [ ] Backed up privately  
- [ ] Never committed  

---

## Common mistakes

| Mistake | Result |
| --- | --- |
| Only push code, no Release | No installer / update button fails |
| Upload only the `.exe` | Updater cannot verify the update |
| Put `release-assets` in the code repo | Repo gets huge; still doesn’t create a Release |
| Private repo | Updater cannot download files |
| Forget to set `TAURI_SIGNING_PRIVATE_KEY` before build | No `.sig` file |
| Wrong / old key password | Build signing fails — use empty password as above |
