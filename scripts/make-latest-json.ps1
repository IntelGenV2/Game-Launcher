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
$fileName = $setup.Name
$encoded = [uri]::EscapeDataString($fileName)
# GitHub release asset URL for tag v{version}
$url = "$repo/releases/download/v$Version/$encoded"

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

$manifest | ConvertTo-Json -Depth 6 | Set-Content -Path $out -Encoding utf8

# Copy installer + sig next to latest.json for easy upload
Copy-Item $setup.FullName (Join-Path $OutDir $fileName) -Force
Copy-Item $sigPath (Join-Path $OutDir "$fileName.sig") -Force

Write-Host ""
Write-Host "Wrote assets to: $OutDir"
Write-Host "  $fileName"
Write-Host "  $fileName.sig"
Write-Host "  latest.json"
Write-Host ""
Write-Host "Create a GitHub Release tagged v$Version and upload those three files."
Write-Host "Updater URL the app checks:"
Write-Host "  $repo/releases/latest/download/latest.json"
