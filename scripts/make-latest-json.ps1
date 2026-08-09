# Builds latest.json for GitHub Releases (or a website mirror)
param(
  [string]$Version = "",
  [string]$Notes = "Bug fixes and improvements",
  # Example: https://github.com/YOUR_USER/YOUR_REPO
  [Parameter(Mandatory = $true)]
  [string]$GitHubRepoUrl,
  [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$conf = Get-Content (Join-Path $root "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
if (-not $Version) { $Version = $conf.version }

$repo = $GitHubRepoUrl.TrimEnd("/")
if ($repo -notmatch "^https://github\.com/[^/]+/[^/]+$") {
  throw "GitHubRepoUrl must look like https://github.com/USER/REPO"
}

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

if (-not $OutDir) {
  $OutDir = Join-Path $root "release-assets"
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$out = Join-Path $OutDir "latest.json"

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
$utf8NoBom = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($out, $json, $utf8NoBom)

# Copy installer + sig using GitHub-safe names (no spaces)
Copy-Item $setup.FullName (Join-Path $OutDir $uploadName) -Force
Copy-Item $sigPath (Join-Path $OutDir "$uploadName.sig") -Force

Write-Host ""
Write-Host "Wrote assets to: $OutDir"
Write-Host "  $uploadName"
Write-Host "  $uploadName.sig"
Write-Host "  latest.json"
Write-Host ""
Write-Host "Create a GitHub Release tagged v$Version and upload those three files."
Write-Host "(Filenames use dots instead of spaces so GitHub URLs match.)"
Write-Host "Updater URL the app checks:"
Write-Host "  $repo/releases/latest/download/latest.json"
