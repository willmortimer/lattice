# Unsigned Windows NSIS bundle: frontend + desktop exe + sidecars.
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_common.ps1"
Ensure-LatticeWindowsRoot
Initialize-LatticeWindowsCargoEnv

if (-not (Get-Command pnpm.exe -ErrorAction SilentlyContinue)) {
  throw "pnpm.exe missing — install Node.js + pnpm on the Windows host"
}

$tauriConfig = "src-tauri/tauri.windows.conf.json"

if (-not (Test-Path "node_modules")) {
  Write-Host "tauri-bundle: pnpm install --frozen-lockfile --prefer-offline"
  & pnpm.exe install --frozen-lockfile --prefer-offline
  if ($LASTEXITCODE -ne 0) {
    throw "tauri-bundle: pnpm install failed (exit $LASTEXITCODE)"
  }
}

Write-Host "tauri-bundle: tauri build --no-bundle (windows config, no voice)"
Push-Location "apps/desktop"
try {
  & pnpm.exe exec tauri build --no-bundle --config $tauriConfig -- --target $script:LatticeWindowsTriple
  if ($LASTEXITCODE -ne 0) {
    throw "tauri-bundle: tauri build --no-bundle failed (exit $LASTEXITCODE)"
  }
}
finally {
  Pop-Location
}

& "$PSScriptRoot\assemble-app.ps1"
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

Write-Host "tauri-bundle: tauri bundle --bundles nsis"
Push-Location "apps/desktop"
try {
  & pnpm.exe exec tauri bundle --bundles nsis --config $tauriConfig -- --target $script:LatticeWindowsTriple
  if ($LASTEXITCODE -ne 0) {
    throw "tauri-bundle: tauri bundle nsis failed (exit $LASTEXITCODE)"
  }
}
finally {
  Pop-Location
}

$bundleDir = Get-LatticeNsisBundleDir
$installers = Get-ChildItem -Path $bundleDir -Filter "*-setup.exe" -ErrorAction SilentlyContinue
if (-not $installers) {
  throw "tauri-bundle: no *-setup.exe under $bundleDir"
}

foreach ($installer in $installers) {
  Write-Host "tauri-bundle: ok → $($installer.FullName)"
}

exit 0
