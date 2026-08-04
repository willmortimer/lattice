# Unsigned Windows NSIS demo bundle: same as tauri-bundle, plus First Look launcher.
# Ships Lattice-FirstLook.cmd (sets LATTICE_SEED_FIRST_LOOK=1) via tauri.windows.demo.conf.json.
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_common.ps1"
Ensure-LatticeWindowsRoot
Initialize-LatticeWindowsCargoEnv

if (-not (Get-Command pnpm.exe -ErrorAction SilentlyContinue) -and -not (Get-Command pnpm.cmd -ErrorAction SilentlyContinue) -and -not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
  throw "pnpm missing — install Node.js + pnpm on the Windows host (winbuild ensure-toolchain)"
}

function Invoke-LatticePnpm {
  param([Parameter(ValueFromRemainingArguments = $true)][string[]]$PnpmArgs)
  $lines = $null
  if (Get-Command pnpm.exe -ErrorAction SilentlyContinue) {
    $lines = & pnpm.exe @PnpmArgs 2>&1
  } elseif (Get-Command pnpm.cmd -ErrorAction SilentlyContinue) {
    $lines = & pnpm.cmd @PnpmArgs 2>&1
  } else {
    $lines = & pnpm @PnpmArgs 2>&1
  }
  $exitCode = 0
  if ($null -ne $LASTEXITCODE) {
    $exitCode = [int]$LASTEXITCODE
  }
  foreach ($line in @($lines)) {
    Write-Host $line
  }
  return $exitCode
}

# Merge windows + demo overlay (First Look launcher resource).
$tauriConfig = "src-tauri/tauri.windows.demo.conf.json"

if (-not (Test-Path "node_modules")) {
  Write-Host "tauri-bundle-demo: pnpm install --frozen-lockfile --prefer-offline"
  $code = Invoke-LatticePnpm @("install", "--frozen-lockfile", "--prefer-offline")
  if ($code -ne 0) {
    throw "tauri-bundle-demo: pnpm install failed (exit $code)"
  }
}

Write-Host "tauri-bundle-demo: tauri build --no-bundle (demo config, capture)"
Push-Location "apps/desktop"
try {
  $code = Invoke-LatticePnpm @(
    "exec", "tauri", "build", "--no-bundle",
    "--features", "capture",
    "--config", $tauriConfig,
    "--target", $script:LatticeWindowsTriple
  )
  if ($code -ne 0) {
    throw "tauri-bundle-demo: tauri build --no-bundle failed (exit $code)"
  }
}
finally {
  Pop-Location
}

& "$PSScriptRoot\assemble-app.ps1"

# Also stage the launcher beside Lattice.exe for unzip/run-from-target dogfood.
$desktopExe = Get-LatticeDesktopExePath
$destDir = Split-Path -Parent $desktopExe
$launcherSrc = Join-Path $PSScriptRoot "..\..\apps\desktop\src-tauri\windows\demo\Lattice-FirstLook.cmd"
$launcherSrc = [System.IO.Path]::GetFullPath($launcherSrc)
if (Test-Path $launcherSrc) {
  Copy-Item -Force -LiteralPath $launcherSrc -Destination (Join-Path $destDir "Lattice-FirstLook.cmd")
  Write-Host "tauri-bundle-demo: staged Lattice-FirstLook.cmd → $destDir"
}

Write-Host "tauri-bundle-demo: tauri bundle --bundles nsis"
Push-Location "apps/desktop"
try {
  $code = Invoke-LatticePnpm @(
    "exec", "tauri", "bundle", "--bundles", "nsis",
    "--features", "capture",
    "--config", $tauriConfig,
    "--target", $script:LatticeWindowsTriple
  )
  if ($code -ne 0) {
    throw "tauri-bundle-demo: tauri bundle nsis failed (exit $code)"
  }
}
finally {
  Pop-Location
}

$bundleDir = Get-LatticeNsisBundleDir
$installers = Get-ChildItem -Path $bundleDir -Filter "*-setup.exe" -ErrorAction SilentlyContinue
if (-not $installers) {
  throw "tauri-bundle-demo: no *-setup.exe under $bundleDir"
}

foreach ($installer in $installers) {
  Write-Host "tauri-bundle-demo: ok → $($installer.FullName)"
  Write-Host "tauri-bundle-demo: after install, run Lattice-FirstLook.cmd (or set LATTICE_SEED_FIRST_LOOK=1) for First Look seed"
}

exit 0
