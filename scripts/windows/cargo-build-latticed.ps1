# Release build latticed (+ lattice-client) on Windows MSVC — expected to succeed after named-pipe IPC.
$ErrorActionPreference = "Stop"

$cargoHome = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoHome) {
  $env:Path = "$cargoHome;$env:Path"
}

if (-not (Get-Command cargo.exe -ErrorAction SilentlyContinue)) {
  throw "cargo.exe missing — run winbuild ensure-toolchain first"
}

$targetDir = Join-Path $PWD "target\windows-msvc"
if (Test-Path "D:\") {
  $targetDir = "D:\lattice-target\windows-msvc"
}
New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
$env:CARGO_TARGET_DIR = $targetDir

$protocBin = Join-Path $env:LOCALAPPDATA "NixPlane\protoc\bin"
if (Test-Path $protocBin) {
  $env:Path = "$protocBin;$env:Path"
  $env:PROTOC = Join-Path $protocBin "protoc.exe"
}

Write-Host "lattice-winbuild-latticed: CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR"
Write-Host "lattice-winbuild-latticed: packages=latticed,lattice-client (release)"

& cargo.exe build --release `
  -p latticed `
  -p lattice-client `
  --target x86_64-pc-windows-msvc
$code = $LASTEXITCODE
if ($code -ne 0) {
  Write-Error "lattice-winbuild-latticed: cargo build failed (exit $code)"
  exit $code
}

Write-Host "lattice-winbuild-latticed: OK"
exit 0
