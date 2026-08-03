# Ensure rustup + stable-x86_64-pc-windows-msvc are available for Lattice winbuild.
$ErrorActionPreference = "Stop"

function Assert-Msvc {
  $cl = Get-ChildItem -Path "${env:ProgramFiles(x86)}\Microsoft Visual Studio","${env:ProgramFiles}\Microsoft Visual Studio" `
    -Filter cl.exe -Recurse -ErrorAction SilentlyContinue |
    Select-Object -First 1
  if (-not $cl) {
    throw "MSVC cl.exe not found. Install VS Build Tools with C++ workload."
  }
  Write-Host "lattice-winbuild-toolchain: cl.exe => $($cl.FullName)"
}

Assert-Msvc

$cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
if (-not $cargo) {
  Write-Host "lattice-winbuild-toolchain: installing rustup (default host msvc)…"
  $tmp = Join-Path $env:TEMP "rustup-init.exe"
  $uri = "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe"
  Invoke-WebRequest -Uri $uri -OutFile $tmp
  & $tmp -y --default-host x86_64-pc-windows-msvc --default-toolchain stable
  $cargoHome = Join-Path $env:USERPROFILE ".cargo\bin"
  $env:Path = "$cargoHome;$env:Path"
  if (-not (Get-Command cargo.exe -ErrorAction SilentlyContinue)) {
    throw "rustup install finished but cargo.exe still not on PATH (expected under $cargoHome)"
  }
} else {
  Write-Host "lattice-winbuild-toolchain: cargo already present => $($cargo.Source)"
}

& rustup.exe default stable-x86_64-pc-windows-msvc
& rustup.exe target add x86_64-pc-windows-msvc

# lance / prost-build need protoc on PATH for Windows builds.
$protoc = Get-Command protoc.exe -ErrorAction SilentlyContinue
if (-not $protoc) {
  Write-Host "lattice-winbuild-toolchain: installing protoc…"
  $ver = "29.3"
  $zip = Join-Path $env:TEMP "protoc-$ver-win64.zip"
  $dest = Join-Path $env:LOCALAPPDATA "NixPlane\protoc"
  $uri = "https://github.com/protocolbuffers/protobuf/releases/download/v$ver/protoc-$ver-win64.zip"
  Invoke-WebRequest -Uri $uri -OutFile $zip
  New-Item -ItemType Directory -Force -Path $dest | Out-Null
  Expand-Archive -Path $zip -DestinationPath $dest -Force
  $env:Path = "$(Join-Path $dest 'bin');$env:Path"
  $env:PROTOC = Join-Path $dest "bin\protoc.exe"
  if (-not (Test-Path $env:PROTOC)) {
    throw "protoc install failed (expected $env:PROTOC)"
  }
} else {
  Write-Host "lattice-winbuild-toolchain: protoc already present => $($protoc.Source)"
  $env:PROTOC = $protoc.Source
}

& rustc.exe --version
& cargo.exe --version
& protoc.exe --version

# Node + pnpm for Tauri/NSIS (optional for cargo-only tasks).
$nodeDirs = @(
  "${env:ProgramFiles}\nodejs",
  (Join-Path $env:APPDATA "npm")
)
foreach ($dir in $nodeDirs) {
  if (Test-Path $dir) {
    $env:Path = "$dir;$env:Path"
  }
}
$pnpm = Get-Command pnpm.exe -ErrorAction SilentlyContinue
if (-not $pnpm) {
  $pnpm = Get-Command pnpm.cmd -ErrorAction SilentlyContinue
}
if (-not $pnpm) {
  Write-Host "lattice-winbuild-toolchain: installing pnpm via npm…"
  $npm = Get-Command npm.cmd -ErrorAction SilentlyContinue
  if (-not $npm) {
    throw "npm.cmd missing — install Node.js before Windows NSIS packaging"
  }
  & npm.cmd install -g pnpm@9.15.0
  $env:Path = "$(Join-Path $env:APPDATA 'npm');$env:Path"
  if (-not (Get-Command pnpm.exe -ErrorAction SilentlyContinue) -and -not (Get-Command pnpm.cmd -ErrorAction SilentlyContinue)) {
    throw "pnpm install finished but pnpm still not on PATH"
  }
} else {
  Write-Host "lattice-winbuild-toolchain: pnpm already present => $($pnpm.Source)"
}
if (Get-Command pnpm.exe -ErrorAction SilentlyContinue) {
  & pnpm.exe --version
} else {
  & pnpm.cmd --version
}

Write-Host "lattice-winbuild-toolchain: OK"
exit 0
