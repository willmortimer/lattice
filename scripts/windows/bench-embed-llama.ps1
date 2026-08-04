# Bench lattice-embed-host llama-cpp query embed latency (p50/p95).
param(
  [string]$Exe,
  [string]$Gguf,
  [string]$CargoTargetDir,
  [int]$Warmup = 3,
  [int]$Iterations = 20,
  [int]$Dimensions = 512,
  [string]$Query = "capability grants for plugins",
  [switch]$Json
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_common.ps1"
Ensure-LatticeWindowsRoot

if ($CargoTargetDir) {
  $env:CARGO_TARGET_DIR = $CargoTargetDir
} elseif (-not $env:CARGO_TARGET_DIR) {
  Initialize-LatticeWindowsCargoEnv
}

function Resolve-LatticeEmbedHostExe {
  param([string]$Override)

  if ($Override) {
    if (-not (Test-Path $Override)) {
      throw "bench-embed-llama: missing -Exe path: $Override"
    }
    return (Resolve-Path $Override).Path
  }

  $path = Get-LatticeWindowsSidecarExePath -Name "lattice-embed-host"
  if (-not (Test-Path $path)) {
    throw @"
bench-embed-llama: missing $path
  build first: .\scripts\windows\build-sidecar.ps1 -Package lattice-embed-host -Bin lattice-embed-host -Features llama-cpp
  or pass -Exe / set CARGO_TARGET_DIR to your release tree
"@
  }
  return $path
}

function Resolve-LatticeEmbedGguf {
  param([string]$Override)

  if ($Override) {
    if (-not (Test-Path $Override)) {
      throw "bench-embed-llama: missing -Gguf path: $Override"
    }
    return (Resolve-Path $Override).Path
  }

  $envPath = $env:LATTICE_EMBED_LLAMA_GGUF
  if (-not $envPath) {
    throw @"
bench-embed-llama: missing GGUF path
  pass -Gguf <path> or set LATTICE_EMBED_LLAMA_GGUF to a verified Qwen3-Embedding-0.6B-Q8_0.gguf
"@
  }
  if (-not (Test-Path $envPath)) {
    throw "bench-embed-llama: LATTICE_EMBED_LLAMA_GGUF does not exist: $envPath"
  }
  return (Resolve-Path $envPath).Path
}

$embedHost = Resolve-LatticeEmbedHostExe -Override $Exe
$ggufPath = Resolve-LatticeEmbedGguf -Override $Gguf

$args = @(
  "bench",
  "--gguf", $ggufPath,
  "--dimensions", $Dimensions,
  "--warmup", $Warmup,
  "--iterations", $Iterations,
  "--query", $Query
)
if ($Json) {
  $args += "--json"
}

Write-Host "bench-embed-llama: exe=$embedHost"
Write-Host "bench-embed-llama: gguf=$ggufPath"
Write-Host "bench-embed-llama: warmup=$Warmup iterations=$Iterations dimensions=$Dimensions"

& $embedHost @args
$code = $LASTEXITCODE
if ($code -ne 0) {
  throw "bench-embed-llama: lattice-embed-host bench failed (exit $code)"
}

Write-Host "bench-embed-llama: OK"
exit 0
