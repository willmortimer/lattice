# Copy Windows sidecars beside the desktop exe (unsigned NSIS staging).
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_common.ps1"
Ensure-LatticeWindowsRoot
Initialize-LatticeWindowsCargoEnv

$desktopExe = Get-LatticeDesktopExePath
$destDir = Split-Path -Parent $desktopExe

foreach ($name in Get-LatticeWindowsSidecarNames) {
  $src = Get-LatticeWindowsSidecarExePath -Name $name
  if (-not (Test-Path $src)) {
    throw "assemble-app: missing $src (required production sidecar)"
  }
  $dest = Join-Path $destDir "$name.exe"
  Copy-Item -Force -Path $src -Destination $dest
  Write-Host "assemble-app: bundled $name.exe → $destDir"
}

Write-Host "assemble-app: ok → $destDir"
# Do not `exit` here — tauri-bundle.ps1 invokes this script with `&` and must continue to NSIS.
