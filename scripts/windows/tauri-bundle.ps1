# Unsigned Windows NSIS bundle: frontend + desktop exe + sidecars.
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_common.ps1"
Ensure-LatticeWindowsRoot
Initialize-LatticeWindowsCargoEnv

if (-not (Get-Command pnpm.exe -ErrorAction SilentlyContinue) -and -not (Get-Command pnpm.cmd -ErrorAction SilentlyContinue) -and -not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
  throw "pnpm missing — install Node.js + pnpm on the Windows host (winbuild ensure-toolchain)"
}

function Invoke-LatticePnpm {
  param([Parameter(ValueFromRemainingArguments = $true)][string[]]$PnpmArgs)
  # Do not let pnpm stdout become the function return value when callers assign `$code = …`.
  # Out-Host keeps the console stream separate from the success/output stream.
  if (Get-Command pnpm.exe -ErrorAction SilentlyContinue) {
    & pnpm.exe @PnpmArgs | Out-Host
  } elseif (Get-Command pnpm.cmd -ErrorAction SilentlyContinue) {
    & pnpm.cmd @PnpmArgs | Out-Host
  } else {
    & pnpm @PnpmArgs | Out-Host
  }
  if ($null -eq $LASTEXITCODE) {
    return 0
  }
  return [int]$LASTEXITCODE
}

$tauriConfig = "src-tauri/tauri.windows.conf.json"

if (-not (Test-Path "node_modules")) {
  Write-Host "tauri-bundle: pnpm install --frozen-lockfile --prefer-offline"
  $code = Invoke-LatticePnpm @("install", "--frozen-lockfile", "--prefer-offline")
  if ($code -ne 0) {
    throw "tauri-bundle: pnpm install failed (exit $code)"
  }
}

Write-Host "tauri-bundle: tauri build --no-bundle (windows config, no voice)"
Push-Location "apps/desktop"
try {
  $code = Invoke-LatticePnpm @(
    "exec", "tauri", "build", "--no-bundle",
    "--config", $tauriConfig,
    "--", "--target", $script:LatticeWindowsTriple
  )
  if ($code -ne 0) {
    throw "tauri-bundle: tauri build --no-bundle failed (exit $code)"
  }
}
finally {
  Pop-Location
}

& "$PSScriptRoot\assemble-app.ps1"
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

Write-Host "tauri-bundle: tauri bundle --bundles nsis"
Push-Location "apps/desktop"
try {
  $code = Invoke-LatticePnpm @(
    "exec", "tauri", "bundle", "--bundles", "nsis",
    "--config", $tauriConfig,
    "--", "--target", $script:LatticeWindowsTriple
  )
  if ($code -ne 0) {
    throw "tauri-bundle: tauri bundle nsis failed (exit $code)"
  }
}
finally {
  Pop-Location
}

$bundleDir = Get-LatticeNsisBundleDir
$installers = Get-ChildItem -Path $bundleDir -Filter "*-setup.exe" -ErrorAction SilentlyContinue
if (-not $installers) {
  throw "tauri-bundle: no *-setup.exe under $bundleDir"
}

foreach ($installer in $installers) {
  Write-Host "tauri-bundle: ok → $($installer.FullName)"
}

exit 0
