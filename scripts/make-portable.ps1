<#
.SYNOPSIS
  Package the built launcher exe as a GitHub-safe portable zip.

.DESCRIPTION
  Copies the release binary from src-tauri\target\release into a zip named
  like IntelGen.Game.Launcher_0.2.5_x64-portable.zip (dots instead of spaces
  so GitHub Release URLs stay stable). Requires a prior ``npm run tauri build``.

.PARAMETER Version
  SemVer without a leading "v". Defaults to src-tauri/tauri.conf.json.

.PARAMETER OutDir
  Where to write the zip. Defaults to <repo>\release-assets.

.EXAMPLE
  .\scripts\make-portable.ps1

.EXAMPLE
  .\scripts\make-portable.ps1 -Version 0.2.5
#>
param(
  [string]$Version = "",
  [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$conf = Get-Content (Join-Path $root "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
if (-not $Version) { $Version = $conf.version }
$Version = $Version.Trim().TrimStart('v', 'V')

$productName = [string]$conf.productName
if (-not $productName) { $productName = "IntelGen Game Launcher" }

$releaseDir = Join-Path $root "src-tauri\target\release"
$exe = @(
  (Join-Path $releaseDir "game-launcher.exe"),
  (Join-Path $releaseDir "$productName.exe")
) |
  Where-Object { Test-Path $_ } |
  Select-Object -First 1

if (-not $exe) {
  throw "No release exe found under $releaseDir. Run a ``npm run tauri build`` first."
}

if (-not $OutDir) {
  $OutDir = Join-Path $root "release-assets"
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$zipName = ($productName -replace ' ', '.') + "_${Version}_x64-portable.zip"
$zipPath = Join-Path $OutDir $zipName
if (Test-Path $zipPath) { Remove-Item $zipPath -Force }

$staging = Join-Path ([System.IO.Path]::GetTempPath()) ("intelgen-portable-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $staging | Out-Null
try {
  Copy-Item $exe (Join-Path $staging "$productName.exe") -Force

  Add-Type -AssemblyName System.IO.Compression.FileSystem
  [System.IO.Compression.ZipFile]::CreateFromDirectory(
    $staging,
    $zipPath,
    [System.IO.Compression.CompressionLevel]::Optimal,
    $false
  )
}
finally {
  Remove-Item $staging -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "Wrote portable zip: $zipPath"
Write-Host "  inner file: $productName.exe"
Write-Host "Unzip anywhere and run. Library data still lives in %APPDATA%\IntelLauncher\"
Write-Host "Needs the WebView2 Runtime (already on most Windows 10/11 machines)."
