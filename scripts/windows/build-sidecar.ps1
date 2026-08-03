# Release-build Windows sidecars (latticed, lattice-agentd, lattice-embed-host).
# No seatbelt, voice, or llama-cpp/Metal backends on Windows beta.
param(
  [string]$Package,
  [string]$Bin,
  [string]$Features
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_common.ps1"
Ensure-LatticeWindowsRoot
Initialize-LatticeWindowsCargoEnv

function Build-LatticeSidecar {
  param(
    [Parameter(Mandatory = $true)][string]$Pkg,
    [Parameter(Mandatory = $true)][string]$Binary,
    [string]$FeatureList
  )

  $args = @(
    "build", "--release",
    "-p", $Pkg,
    "--bin", $Binary,
    "--target", $script:LatticeWindowsTriple
  )
  if ($FeatureList) {
    $args += @("--features", $FeatureList)
  }

  Write-Host "build-sidecar: cargo $($args -join ' ')"
  & cargo.exe @args
  $code = $LASTEXITCODE
  if ($code -ne 0) {
    throw "build-sidecar: cargo build failed for $Binary (exit $code)"
  }

  $out = Get-LatticeWindowsSidecarExePath -Name $Binary
  if (-not (Test-Path $out)) {
    throw "build-sidecar: missing $out after build"
  }
  Write-Host "build-sidecar: ok → $out"
}

if ($Package -and $Bin) {
  Build-LatticeSidecar -Pkg $Package -Binary $Bin -FeatureList $Features
  exit 0
}

Write-Host "build-sidecar: CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR"
Write-Host "build-sidecar: packages=latticed,lattice-agentd,lattice-embed-host (release, no llama-cpp)"

Build-LatticeSidecar -Pkg "lattice-daemon" -Binary "latticed"
Build-LatticeSidecar -Pkg "lattice-agentd" -Binary "lattice-agentd"
Build-LatticeSidecar -Pkg "lattice-embed-host" -Binary "lattice-embed-host"

Write-Host "build-sidecar: OK"
exit 0
