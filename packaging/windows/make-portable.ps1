<#
.SYNOPSIS
  Build the portable archive for the Office suite on Windows.

.DESCRIPTION
  Builds iridium-office-<version>-windows-<arch>-portable.zip from the executable, the licence
  files and the font licences.

  This is a separate script from package.ps1 because the SignPath route signs the executable
  after package.ps1 built the installer: the portable archive has to be built from the signed
  executable, or the archive would carry an unsigned copy.

.EXAMPLE
  pwsh packaging/windows/make-portable.ps1 -Arch x64 -SignedExe dist\signpath\iridium-office.exe
#>
param(
  [ValidateSet('x64', 'x86', 'arm64')] [string] $Arch = 'x64',
  # Mandatory: the archive must be built from the executable that was signed, not from
  # whatever the build left lying around. A default here silently produces an unsigned zip.
  [Parameter(Mandatory = $true)]
  [string] $SignedExe
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

$Version = $env:IRIDIUM_OFFICE_VERSION
if (-not $Version) {
  $inPkg = $false
  foreach ($line in Get-Content (Join-Path $Root 'Cargo.toml')) {
    if ($line -match '^\s*\[') { $inPkg = ($line.Trim() -eq '[workspace.package]'); continue }
    if ($inPkg -and $line -match '^\s*version\s*=\s*"([^"]+)"') { $Version = $Matches[1]; break }
  }
}
if (-not $Version) { throw 'could not read [workspace.package] version from Cargo.toml' }

$Dist = if ($env:DIST) { $env:DIST } else { Join-Path $Root 'dist\release' }
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
New-Item -ItemType Directory -Force -Path $Dist | Out-Null

if (-not (Test-Path $SignedExe)) { throw "no executable at $SignedExe" }

$Portable = Join-Path $TargetDir "windows-package\iridium-office-$Version-windows-$Arch-portable"
Remove-Item -Recurse -Force $Portable -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Portable | Out-Null
Copy-Item $SignedExe $Portable
foreach ($f in 'README.md', 'LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE') {
  $p = Join-Path $Root $f
  if (Test-Path $p) { Copy-Item $p $Portable }
}
# Builds made with craft-fonts (CRAFT_FONTS_DIR) embed its fonts: ship their licences.
if ($env:CRAFT_FONTS_DIR) {
  foreach ($ofl in Get-ChildItem -Path (Join-Path $env:CRAFT_FONTS_DIR 'fonts\*\OFL.txt') -File -ErrorAction SilentlyContinue) {
    Copy-Item $ofl.FullName (Join-Path $Portable "OFL-$($ofl.Directory.Name).txt")
  }
}
$Zip = Join-Path $Dist "iridium-office-$Version-windows-$Arch-portable.zip"
Remove-Item -Force $Zip -ErrorAction SilentlyContinue
Compress-Archive -Path $Portable -DestinationPath $Zip

Get-Item $Zip | Format-Table Name, Length
