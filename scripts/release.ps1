<#
.SYNOPSIS
  Bump version, signed-build the launcher, and prepare GitHub Release assets.

.DESCRIPTION
  Updates version in:
    - package.json
    - src-tauri/tauri.conf.json
    - src-tauri/Cargo.toml
  Then runs npm install, a signed tauri build, writes latest.json + installer
  assets, and packages a portable zip.

.PARAMETER Version
  SemVer without a leading "v". Examples: 0.1.3   1.0.0   0.2.0-beta.1

.EXAMPLE
  .\scripts\release.ps1 -Version 0.1.3

.EXAMPLE
  .\scripts\release.ps1 0.1.3
#>
param(
  [Parameter(Mandatory = $true, Position = 0)]
  [ValidatePattern('^\d+\.\d+\.\d+([+-][0-9A-Za-z.-]+)?$')]
  [string]$Version,

  [string]$Notes = "Bug fixes and improvements",

  [string]$GitHubRepoUrl = "https://github.com/IntelGenV2/Game-Launcher"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# Strip accidental leading v
$Version = $Version.Trim().TrimStart('v', 'V')
if ($Version -notmatch '^\d+\.\d+\.\d+([+-][0-9A-Za-z.-]+)?$') {
  throw "Version format must be like 0.1.3 or 1.0.0-beta.1 (no leading v)."
}

$repo = $GitHubRepoUrl.TrimEnd("/")
if ($repo -notmatch "^https://github\.com/[^/]+/[^/]+$") {
  throw "GitHubRepoUrl must look like https://github.com/USER/REPO"
}

$keyPath = Join-Path $env:USERPROFILE ".tauri\intelgen-game-launcher.key"
if (-not (Test-Path $keyPath)) {
  throw "Signing key not found: $keyPath"
}

$releaseAssets = Join-Path $root "release-assets"
$utf8NoBom = New-Object System.Text.UTF8Encoding $false

Write-Host ""
Write-Host "=== Release $Version ===" -ForegroundColor Cyan
Write-Host "Root: $root"
Write-Host ""

function Set-JsonVersion([string]$Path, [string]$NewVersion) {
  $raw = [System.IO.File]::ReadAllText($Path)
  $updated = [regex]::Replace(
    $raw,
    '("version"\s*:\s*")[^"]*(")',
    "`${1}$NewVersion`${2}",
    1
  )
  if ($updated -eq $raw -and $raw -notmatch [regex]::Escape("`"$NewVersion`"")) {
    throw "Could not update version in $Path"
  }
  [System.IO.File]::WriteAllText($Path, $updated, $utf8NoBom)
  Write-Host "Updated $Path -> $NewVersion"
}

function Set-CargoVersion([string]$Path, [string]$NewVersion) {
  $raw = [System.IO.File]::ReadAllText($Path)
  # Only the package version at the top of Cargo.toml
  $updated = [regex]::Replace(
    $raw,
    '(?m)^(version\s*=\s*")[^"]*(")',
    "`${1}$NewVersion`${2}",
    1
  )
  if ($updated -eq $raw) {
    if ($raw -match '(?m)^version\s*=\s*"' + [regex]::Escape($NewVersion) + '"') {
      Write-Host "Already $NewVersion in $Path"
      return
    }
    throw "Could not update version in $Path"
  }
  [System.IO.File]::WriteAllText($Path, $updated, $utf8NoBom)
  Write-Host "Updated $Path -> $NewVersion"
}

function New-LatestJsonAssets {
  $nsisDir = Join-Path $root "src-tauri\target\release\bundle\nsis"
  $setup = Get-ChildItem $nsisDir -Filter "*$Version*_x64-setup.exe" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

  if (-not $setup) {
    throw "No NSIS setup found for v$Version under $nsisDir. Run a signed ``npm run tauri build`` first."
  }

  $sigPath = "$($setup.FullName).sig"
  if (-not (Test-Path $sigPath)) {
    throw "Missing signature file: $sigPath (set TAURI_SIGNING_PRIVATE_KEY from the .key file and rebuild)."
  }

  $signature = (Get-Content $sigPath -Raw).Trim()
  # GitHub's web upload replaces spaces with '.' in asset names — use that form so
  # the updater URL matches what ends up on the Release.
  $uploadName = $setup.Name -replace ' ', '.'
  $url = "$repo/releases/download/v$Version/$uploadName"

  New-Item -ItemType Directory -Force -Path $releaseAssets | Out-Null
  $out = Join-Path $releaseAssets "latest.json"

  $manifest = [ordered]@{
    version   = $Version
    notes     = $Notes
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
      "windows-x86_64" = [ordered]@{
        signature = $signature
        url       = $url
      }
    }
  }

  $json = $manifest | ConvertTo-Json -Depth 6
  # Windows PowerShell's utf8 encoding adds a BOM that breaks Tauri's JSON parser.
  [System.IO.File]::WriteAllText($out, $json, $utf8NoBom)

  Copy-Item $setup.FullName (Join-Path $releaseAssets $uploadName) -Force
  Copy-Item $sigPath (Join-Path $releaseAssets "$uploadName.sig") -Force

  Write-Host ""
  Write-Host "Wrote installer assets to: $releaseAssets"
  Write-Host "  $uploadName"
  Write-Host "  $uploadName.sig"
  Write-Host "  latest.json"
  Write-Host ""
  Write-Host "Updater URL the app checks:"
  Write-Host "  $repo/releases/latest/download/latest.json"
}

Set-JsonVersion (Join-Path $root "package.json") $Version
Set-JsonVersion (Join-Path $root "src-tauri\tauri.conf.json") $Version
Set-CargoVersion (Join-Path $root "src-tauri\Cargo.toml") $Version

Write-Host ""
Write-Host "=== clean old installers ===" -ForegroundColor Cyan
$nsisDir = Join-Path $root "src-tauri\target\release\bundle\nsis"
foreach ($dir in @($nsisDir, $releaseAssets)) {
  if (-not (Test-Path $dir)) { continue }
  $removed = 0
  Get-ChildItem $dir -File -ErrorAction SilentlyContinue |
    Where-Object {
      $_.Extension -in ".exe", ".sig", ".zip", ".json" -or $_.Name -like "*.exe.sig"
    } |
    ForEach-Object {
      Remove-Item $_.FullName -Force
      Write-Host "  removed $($_.Name)"
      $removed++
    }
  if ($removed -eq 0) {
    Write-Host "  (none in $dir)"
  }
}

Write-Host ""
Write-Host "=== npm install ===" -ForegroundColor Cyan
npm install
if ($LASTEXITCODE -ne 0) { throw "npm install failed" }

Write-Host ""
Write-Host "=== signed tauri build ===" -ForegroundColor Cyan
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
npm run tauri build
if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }

Write-Host ""
Write-Host "=== make latest.json ===" -ForegroundColor Cyan
New-LatestJsonAssets

Write-Host ""
Write-Host "=== make portable zip ===" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "make-portable.ps1") -Version $Version -OutDir $releaseAssets
if ($LASTEXITCODE -ne 0) { throw "portable zip failed" }

Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Green
Write-Host "Upload everything in release-assets\ to a GitHub Release tagged v$Version"
Write-Host "  $repo/releases/new"
Write-Host "  (installer + .sig + latest.json for auto-update; portable zip is optional)"
Write-Host ""
