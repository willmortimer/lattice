# Copy Windows sidecars beside the desktop exe and stage Tauri externalBin inputs.
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_common.ps1"
Ensure-LatticeWindowsRoot
Initialize-LatticeWindowsCargoEnv

$desktopExe = Get-LatticeDesktopExePath
$destDir = Split-Path -Parent $desktopExe
$sidecarStage = Join-Path $PWD "apps\desktop\src-tauri\sidecars"
New-Item -ItemType Directory -Force -Path $sidecarStage | Out-Null

foreach ($name in Get-LatticeWindowsSidecarNames) {
  $src = Get-LatticeWindowsSidecarExePath -Name $name
  if (-not (Test-Path $src)) {
    throw "assemble-app: missing $src (required production sidecar)"
  }
  $dest = Join-Path $destDir "$name.exe"
  $srcFull = (Get-Item -LiteralPath $src).FullName
  $destFull = [System.IO.Path]::GetFullPath($dest)
  # Sidecars and desktop share CARGO_TARGET_DIR/release — skip no-op self-copy.
  if ($srcFull -eq $destFull) {
    Write-Host "assemble-app: $name.exe already in place → $destDir"
  } else {
    Copy-Item -Force -LiteralPath $src -Destination $dest
    Write-Host "assemble-app: bundled $name.exe → $destDir"
  }

  # Tauri externalBin expects name-<triple>.exe under src-tauri/sidecars/.
  $staged = Join-Path $sidecarStage "$name-$($script:LatticeWindowsTriple).exe"
  Copy-Item -Force -LiteralPath $src -Destination $staged
  Write-Host "assemble-app: staged externalBin → $staged"
}

Write-Host "assemble-app: ok → $destDir"
# Do not `exit` here — tauri-bundle.ps1 invokes this script with `&` and must continue to NSIS.
