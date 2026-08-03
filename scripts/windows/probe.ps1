# Lattice Windows build probe — toolchain + DevDrive facts (no installs).
$ErrorActionPreference = "Stop"

Write-Host "lattice-winbuild-probe: cwd=$PWD"
Write-Host "lattice-winbuild-probe: os=$([System.Environment]::OSVersion.VersionString)"

function Test-Cmd([string]$Name) {
  $cmd = Get-Command $Name -ErrorAction SilentlyContinue
  if ($cmd) {
    Write-Host "lattice-winbuild-probe: $Name => $($cmd.Source)"
    return $true
  }
  Write-Host "lattice-winbuild-probe: $Name => MISSING"
  return $false
}

Test-Cmd "cargo.exe" | Out-Null
Test-Cmd "rustc.exe" | Out-Null
Test-Cmd "rustup.exe" | Out-Null
Test-Cmd "dotnet.exe" | Out-Null
Test-Cmd "node.exe" | Out-Null
Test-Cmd "pnpm.cmd" | Out-Null
Test-Cmd "pnpm.exe" | Out-Null

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
  Write-Host "lattice-winbuild-probe: vswhere => $vswhere"
  & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath 2>$null | ForEach-Object {
      Write-Host "lattice-winbuild-probe: msvc install => $_"
    }
  $cl = Get-ChildItem -Path "${env:ProgramFiles(x86)}\Microsoft Visual Studio","${env:ProgramFiles}\Microsoft Visual Studio" `
    -Filter cl.exe -Recurse -ErrorAction SilentlyContinue |
    Select-Object -First 3 -ExpandProperty FullName
  foreach ($path in $cl) {
    Write-Host "lattice-winbuild-probe: cl.exe => $path"
  }
} else {
  Write-Host "lattice-winbuild-probe: vswhere => MISSING"
}

$drive = Get-PSDrive -Name D -ErrorAction SilentlyContinue
if ($drive) {
  Write-Host ("lattice-winbuild-probe: D: free={0:N1} GiB used={1:N1} GiB" -f `
    ($drive.Free / 1GB), (($drive.Used) / 1GB))
} else {
  Write-Host "lattice-winbuild-probe: D: drive not found"
}

Write-Host "lattice-winbuild-probe: OK"
exit 0
