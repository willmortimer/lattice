# Shared helpers for Lattice Windows release / winbuild scripts.
$ErrorActionPreference = "Stop"

$script:LatticeWindowsTriple = "x86_64-pc-windows-msvc"

function Ensure-LatticeWindowsRoot {
  if (-not $env:OS -or $env:OS -notlike "*Windows*") {
    throw "lattice-windows: Windows only"
  }
  if ((Test-Path ".\lattice\Cargo.toml") -and (Test-Path ".\lattice\apps\daemon")) {
    Set-Location ".\lattice"
  } elseif (-not (Test-Path ".\Cargo.toml") -or -not (Test-Path ".\apps\daemon")) {
    throw "lattice-windows: run from lattice repo root (or ecosystem root with .\lattice)"
  }
}

function Initialize-LatticeWindowsCargoEnv {
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

  foreach ($dir in @("${env:ProgramFiles}\nodejs", (Join-Path $env:APPDATA "npm"))) {
    if (Test-Path $dir) {
      $env:Path = "$dir;$env:Path"
    }
  }

  # llama-cpp-sys bindgen needs libclang.dll (LLVM). Prefer an explicit LIBCLANG_PATH.
  if (-not $env:LIBCLANG_PATH) {
    $libclangDirs = @(
      (Join-Path ${env:ProgramFiles} "LLVM\bin"),
      (Join-Path ${env:ProgramFiles(x86)} "LLVM\bin"),
      (Join-Path $env:LOCALAPPDATA "Programs\LLVM\bin")
    )
    foreach ($dir in $libclangDirs) {
      if (Test-Path (Join-Path $dir "libclang.dll")) {
        $env:LIBCLANG_PATH = $dir
        $env:Path = "$dir;$env:Path"
        break
      }
    }
  } elseif (Test-Path $env:LIBCLANG_PATH) {
    $env:Path = "$($env:LIBCLANG_PATH);$env:Path"
  }
}

function Get-LatticeWindowsReleaseDir {
  $candidates = @(
    (Join-Path $env:CARGO_TARGET_DIR "$script:LatticeWindowsTriple\release"),
    (Join-Path $env:CARGO_TARGET_DIR "release")
  )
  foreach ($candidate in $candidates) {
    if (Test-Path $candidate) {
      return $candidate
    }
  }
  Join-Path $env:CARGO_TARGET_DIR "$script:LatticeWindowsTriple\release"
}

function Get-LatticeWindowsSidecarNames {
  @(
    "latticed",
    "lattice-agentd",
    "lattice-embed-host"
  )
}

function Get-LatticeWindowsSidecarExePath {
  param([Parameter(Mandatory = $true)][string]$Name)
  Join-Path (Get-LatticeWindowsReleaseDir) "$Name.exe"
}

function Get-LatticeDesktopExePath {
  $releaseDir = Get-LatticeWindowsReleaseDir
  $candidates = @(
    (Join-Path $releaseDir "Lattice.exe"),
    (Join-Path $releaseDir "lattice-desktop.exe")
  )
  foreach ($candidate in $candidates) {
    if (Test-Path $candidate) {
      return $candidate
    }
  }
  throw "lattice-windows: missing desktop exe under $releaseDir (run tauri build --no-bundle first)"
}

function Get-LatticeNsisBundleDir {
  $bundleDir = Join-Path (Get-LatticeWindowsReleaseDir) "bundle\nsis"
  if (-not (Test-Path $bundleDir)) {
    throw "lattice-windows: missing NSIS bundle dir at $bundleDir"
  }
  return $bundleDir
}
