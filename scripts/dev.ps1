# Sets up a minimal MSVC + Node env and runs the Tauri dev server.
# Avoids VsDevCmd.bat (PATH too long) and keeps PATH short so cmd/npm can find node.
$ErrorActionPreference = "Stop"

function Find-VsBuildTools {
  foreach ($root in @(
      "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools",
      "$env:ProgramFiles\Microsoft Visual Studio\2022\BuildTools",
      "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\Community",
      "$env:ProgramFiles\Microsoft Visual Studio\2022\Community"
    )) {
    if (Test-Path $root) { return $root }
  }
  return $null
}

function Find-NodeExe {
  foreach ($candidate in @(
      "$env:ProgramFiles\nodejs\node.exe",
      "${env:ProgramFiles(x86)}\nodejs\node.exe",
      "$env:LOCALAPPDATA\Programs\node\node.exe"
    )) {
    if (Test-Path $candidate) { return $candidate }
  }
  $cmd = Get-Command node -ErrorAction SilentlyContinue
  if ($cmd -and $cmd.Source -notmatch 'WindowsApps') {
    return $cmd.Source
  }
  return $null
}

$vsRoot = Find-VsBuildTools
if (-not $vsRoot) {
  Write-Error "Visual Studio Build Tools not found. Install the C++ workload first."
}

$msvcDir = Get-ChildItem (Join-Path $vsRoot "VC\Tools\MSVC") -Directory -ErrorAction SilentlyContinue |
  Sort-Object Name -Descending |
  Select-Object -First 1
if (-not $msvcDir) {
  Write-Error "MSVC toolset not found under $vsRoot"
}

$kitsRoot = "${env:ProgramFiles(x86)}\Windows Kits\10"
$sdkInclude = Get-ChildItem (Join-Path $kitsRoot "Include") -Directory -ErrorAction SilentlyContinue |
  Sort-Object Name -Descending |
  Select-Object -First 1
$sdkLib = Get-ChildItem (Join-Path $kitsRoot "Lib") -Directory -ErrorAction SilentlyContinue |
  Sort-Object Name -Descending |
  Select-Object -First 1
if (-not $sdkInclude -or -not $sdkLib) {
  Write-Error "Windows 10/11 SDK not found under $kitsRoot"
}

$nodeExe = Find-NodeExe
if (-not $nodeExe) {
  Write-Error "Node.js not found. Install from https://nodejs.org/ and retry."
}
$npmCmd = Join-Path (Split-Path $nodeExe -Parent) "npm.cmd"
if (-not (Test-Path $npmCmd)) {
  Write-Error "npm.cmd not found next to $nodeExe"
}

$hostBin = Join-Path $msvcDir.FullName "bin\Hostx64\x64"
$msvcLib = Join-Path $msvcDir.FullName "lib\x64"
$msvcInclude = Join-Path $msvcDir.FullName "include"
$ucrtLib = Join-Path $sdkLib.FullName "ucrt\x64"
$umLib = Join-Path $sdkLib.FullName "um\x64"
$ucrtInc = Join-Path $sdkInclude.FullName "ucrt"
$umInc = Join-Path $sdkInclude.FullName "um"
$sharedInc = Join-Path $sdkInclude.FullName "shared"
$winrtInc = Join-Path $sdkInclude.FullName "winrt"

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$nodeDir = Split-Path $nodeExe -Parent
$localBin = Join-Path $projectRoot "node_modules\.bin"
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"

# Short PATH only — appending the full user PATH blows past cmd.exe limits
# and makes child processes lose `node` / `vite`.
$env:Path = @(
  $nodeDir,
  $localBin,
  $cargoBin,
  $hostBin,
  "$env:SystemRoot\System32",
  "$env:SystemRoot",
  "$env:SystemRoot\System32\Wbem"
) -join ";"

$env:LIB = @($msvcLib, $ucrtLib, $umLib) -join ";"
$env:INCLUDE = @($msvcInclude, $ucrtInc, $umInc, $sharedInc, $winrtInc) -join ";"
$env:LIBPATH = $msvcLib

Set-Location $projectRoot

if (-not (Test-Path (Join-Path $projectRoot "node_modules\vite\bin\vite.js"))) {
  Write-Host "Installing npm dependencies..."
  & $npmCmd install
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

# Invoke via absolute node.exe so npm scripts can't lose node on PATH
& $nodeExe (Join-Path $projectRoot "node_modules\@tauri-apps\cli\tauri.js") dev
