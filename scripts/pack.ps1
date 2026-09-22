[CmdletBinding()]
param(
    [string]$ObsRundir = $(if ($env:LUMA_OBS_RUNDIR) { $env:LUMA_OBS_RUNDIR } else { 'C:\Users\Administrator\Projects\obs-studio\build_x64\rundir\RelWithDebInfo' }),
    [switch]$NoZip
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot
$distRoot = Join-Path $repo 'dist'
$packageRoot = Join-Path $distRoot 'LumaNext'
$engine = Join-Path $repo 'target\release\luma-engine.exe'
$obsBin = Join-Path $ObsRundir 'bin\64bit'
$obsPlugins = Join-Path $ObsRundir 'obs-plugins\64bit'
$obsData = Join-Path $ObsRundir 'data'
$obsLicense = Join-Path (Split-Path (Split-Path (Split-Path $ObsRundir))) 'COPYING'

foreach ($required in @($obsBin, $obsPlugins, $obsData, $obsLicense)) {
    if (-not (Test-Path -LiteralPath $required)) { throw "Required OBS runtime input is missing: $required" }
}

Push-Location $repo
try {
    & cargo.exe build --release -p luma-engine
    if ($LASTEXITCODE -ne 0) { throw 'cargo build --release -p luma-engine failed.' }
    $version = (& git.exe describe --tags --always --dirty 2>$null)
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($version)) { $version = '0.1.0' }
} finally { Pop-Location }

$resolvedDist = [System.IO.Path]::GetFullPath($distRoot)
$resolvedRepo = [System.IO.Path]::GetFullPath($repo)
if (-not $resolvedDist.StartsWith($resolvedRepo, [System.StringComparison]::OrdinalIgnoreCase)) { throw 'dist path escaped repository' }
if (Test-Path -LiteralPath $packageRoot) { Remove-Item -LiteralPath $packageRoot -Recurse -Force }
$binDestination = New-Item -ItemType Directory -Force -Path (Join-Path $packageRoot 'bin')
$obsDestination = New-Item -ItemType Directory -Force -Path (Join-Path $packageRoot 'obs')
$pluginDestination = New-Item -ItemType Directory -Force -Path (Join-Path $obsDestination 'obs-plugins\64bit')

Copy-Item -LiteralPath $engine -Destination $binDestination.FullName
Get-ChildItem -LiteralPath $obsBin -File | Where-Object Extension -In @('.dll', '.exe') | Copy-Item -Destination $binDestination.FullName
Get-ChildItem -LiteralPath $obsPlugins -File | Where-Object Extension -eq '.dll' | Copy-Item -Destination $pluginDestination.FullName
Copy-Item -LiteralPath $obsData -Destination $obsDestination.FullName -Recurse
Copy-Item -LiteralPath $obsLicense -Destination (Join-Path $packageRoot 'LICENSE-OBS-GPL.txt')

$readme = @"
Luma Next $version

Run: bin\luma-engine.exe
Uninstall: use scripts\uninstall.ps1 from the source repository or remove autostart first.

This distribution includes and links libobs. OBS Studio/libobs is GPL-2.0-or-later;
see LICENSE-OBS-GPL.txt. Luma Next is GPL-3.0-or-later.
Corresponding source: https://github.com/flydmonkey/luma-next
OBS source: https://github.com/obsproject/obs-studio
"@
Set-Content -LiteralPath (Join-Path $packageRoot 'README.txt') -Value $readme -Encoding UTF8
@{ name='LumaNext'; version=$version.Trim(); architecture='x64'; built_at=(Get-Date).ToUniversalTime().ToString('o') } |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $packageRoot 'manifest.json') -Encoding UTF8

if (-not $NoZip) {
    $zip = Join-Path $distRoot "LumaNext-$($version.Trim())-win-x64.zip"
    if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
    Compress-Archive -Path (Join-Path $packageRoot '*') -DestinationPath $zip -CompressionLevel Optimal
    Write-Host "ZIP: $zip"
}
Write-Host "Package: $packageRoot"
