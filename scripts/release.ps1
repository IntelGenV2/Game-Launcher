<#
.SYNOPSIS
  Bump version, signed-build the launcher, and prepare GitHub Release assets.

.DESCRIPTION
  Updates version in:
    - package.json
    - src-tauri/tauri.conf.json
    - src-tauri/Cargo.toml
  Then runs npm install, a signed tauri build, and make-latest-json.ps1.

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

$keyPath = Join-Path $env:USERPROFILE ".tauri\intelgen-game-launcher.key"
if (-not (Test-Path $keyPath)) {
  throw "Signing key not found: $keyPath"
}

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
  $utf8NoBom = New-Object System.Text.UTF8Encoding $false
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
  $utf8NoBom = New-Object System.Text.UTF8Encoding $false
  [System.IO.File]::WriteAllText($Path, $updated, $utf8NoBom)
  Write-Host "Updated $Path -> $NewVersion"
}

Set-JsonVersion (Join-Path $root "package.json") $Version
Set-JsonVersion (Join-Path $root "src-tauri\tauri.conf.json") $Version
Set-CargoVersion (Join-Path $root "src-tauri\Cargo.toml") $Version

Write-Host ""
Write-Host "=== clean old installers ===" -ForegroundColor Cyan
$nsisDir = Join-Path $root "src-tauri\target\release\bundle\nsis"
$releaseAssets = Join-Path $root "release-assets"
foreach ($dir in @($nsisDir, $releaseAssets)) {
  if (-not (Test-Path $dir)) { continue }
  $removed = 0
  Get-ChildItem $dir -File -ErrorAction SilentlyContinue |
    Where-Object { $_.Extension -in ".exe", ".sig" -or $_.Name -like "*.exe.sig" } |
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
& (Join-Path $PSScriptRoot "make-latest-json.ps1") `
  -Version $Version `
  -Notes $Notes `
  -GitHubRepoUrl $GitHubRepoUrl

Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Green
Write-Host "Upload everything in release-assets\ to a GitHub Release tagged v$Version"
Write-Host "  https://github.com/IntelGenV2/Game-Launcher/releases/new"
Write-Host ""
