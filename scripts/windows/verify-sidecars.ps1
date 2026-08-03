# Verify Windows release sidecars (no seatbelt / voice / llama-cpp).
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_common.ps1"
Ensure-LatticeWindowsRoot
Initialize-LatticeWindowsCargoEnv

foreach ($name in Get-LatticeWindowsSidecarNames) {
  $path = Get-LatticeWindowsSidecarExePath -Name $name
  if (-not (Test-Path $path)) {
    throw "verify-sidecars: missing $path after build"
  }
}

$embedHost = Get-LatticeWindowsSidecarExePath -Name "lattice-embed-host"
$backends = & $embedHost backends 2>&1
Write-Host "verify-sidecars: lattice-embed-host backends:"
Write-Host $backends

$backendLines = @(
  $backends -split "`r?`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ }
)
if ($backendLines -notcontains "fake") {
  throw "verify-sidecars: lattice-embed-host must list fake backend on Windows (no llama-cpp)"
}

Write-Host "verify-sidecars: OK"
exit 0
