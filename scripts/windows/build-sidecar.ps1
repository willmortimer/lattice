# Release-build Windows sidecars (latticed, lattice-agentd, lattice-embed-host).
# embed-host is built with llama-cpp (CPU; Metal feature is a no-op on Windows).
# GGUF weights are not bundled — Settings Enable downloads them.
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
Write-Host "build-sidecar: cohort=latticed,lattice-agentd,lattice-embed-host (release, llama-cpp)"

# One Cargo invocation so shared crates compile once (same target dir, same lock).
$cohortArgs = @(
  "build", "--release",
  "--target", $script:LatticeWindowsTriple,
  "-p", "lattice-daemon", "--bin", "latticed",
  "-p", "lattice-agentd", "--bin", "lattice-agentd",
  "-p", "lattice-embed-host", "--bin", "lattice-embed-host",
  "--features", "lattice-embed-host/llama-cpp"
)
Write-Host "build-sidecar: cargo $($cohortArgs -join ' ')"
& cargo.exe @cohortArgs
if ($LASTEXITCODE -ne 0) {
  throw "build-sidecar: cargo cohort build failed (exit $LASTEXITCODE)"
}

foreach ($name in @("latticed", "lattice-agentd", "lattice-embed-host")) {
  $out = Get-LatticeWindowsSidecarExePath -Name $name
  if (-not (Test-Path $out)) {
    throw "build-sidecar: missing $out after cohort build"
  }
  Write-Host "build-sidecar: ok → $out"
}

Write-Host "build-sidecar: OK"
exit 0
