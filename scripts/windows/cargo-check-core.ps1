# Headless Lattice Windows compile smoke (no Tauri / no voice).
$ErrorActionPreference = "Stop"

$cargoHome = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoHome) {
  $env:Path = "$cargoHome;$env:Path"
}

if (-not (Get-Command cargo.exe -ErrorAction SilentlyContinue)) {
  throw "cargo.exe missing — run winbuild ensure-toolchain first"
}

# Prefer DevDrive for target dir when present.
$targetDir = Join-Path $PWD "target\windows-msvc"
if (Test-Path "D:\") {
  $targetDir = "D:\lattice-target\windows-msvc"
}
New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
$env:CARGO_TARGET_DIR = $targetDir

# Ensure protoc from ensure-toolchain is visible if installed under LocalAppData.
$protocBin = Join-Path $env:LOCALAPPDATA "NixPlane\protoc\bin"
if (Test-Path $protocBin) {
  $env:Path = "$protocBin;$env:Path"
  $env:PROTOC = Join-Path $protocBin "protoc.exe"
}

Write-Host "lattice-winbuild-check: CARGO_TARGET_DIR=$env:CARGO_TARGET_DIR"
Write-Host "lattice-winbuild-check: packages=lattice-cli,lattice-storage,lattice-core,lattice-protocol,lattice-cloud-client,lattice-daemon,lattice-client"

& cargo.exe check `
  -p lattice-cli `
  -p lattice-storage `
  -p lattice-core `
  -p lattice-protocol `
  -p lattice-cloud-client `
  -p lattice-daemon `
  -p lattice-client `
  --target x86_64-pc-windows-msvc
$code = $LASTEXITCODE
if ($code -ne 0) {
  Write-Error "lattice-winbuild-check: cargo check failed (exit $code)"
  exit $code
}

Write-Host "lattice-winbuild-check: OK"
exit 0
