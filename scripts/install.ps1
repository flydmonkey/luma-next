[CmdletBinding()]
param(
    [string]$PackagePath,
    [switch]$SkipPack
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($PackagePath)) { $PackagePath = Join-Path $repo 'dist\LumaNext' }
if (-not $SkipPack) {
    & (Join-Path $PSScriptRoot 'pack.ps1') -NoZip
    if ($LASTEXITCODE -ne 0) { throw 'pack.ps1 failed.' }
}
$PackagePath = [System.IO.Path]::GetFullPath($PackagePath)
$packageEngine = Join-Path $PackagePath 'bin\luma-engine.exe'
if (-not (Test-Path -LiteralPath $packageEngine -PathType Leaf)) { throw "Package engine is missing: $packageEngine" }
if (-not (Test-Path -LiteralPath (Join-Path $PackagePath 'obs\data\libobs'))) { throw 'Package OBS data is incomplete.' }
if (-not (Test-Path -LiteralPath (Join-Path $PackagePath 'obs\obs-plugins\64bit'))) { throw 'Package OBS plugins are incomplete.' }

$installRoot = Join-Path $env:LOCALAPPDATA 'LumaNext'
$installedEngine = Join-Path $installRoot 'bin\luma-engine.exe'
$running = @(Get-Process -Name 'luma-engine' -ErrorAction SilentlyContinue | Where-Object { $_.Path -and [System.IO.Path]::GetFullPath($_.Path) -eq [System.IO.Path]::GetFullPath($installedEngine) })
if ($running.Count -gt 0) { throw 'Installed Luma Next is running. Exit it from the tray before installing; the installer will not force-kill it.' }

$stage = Join-Path $env:LOCALAPPDATA 'LumaNext-package-staging'
foreach ($path in @($installRoot, $stage)) {
    $resolved = [System.IO.Path]::GetFullPath($path)
    $local = [System.IO.Path]::GetFullPath($env:LOCALAPPDATA)
    if (-not $resolved.StartsWith($local, [System.StringComparison]::OrdinalIgnoreCase)) { throw "Install path escaped LOCALAPPDATA: $resolved" }
}
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage | Out-Null
Copy-Item -Path (Join-Path $PackagePath '*') -Destination $stage -Recurse -Force
New-Item -ItemType Directory -Path $installRoot -Force | Out-Null
$managedNames = @('bin', 'obs', 'README.txt', 'manifest.json', 'LICENSE-OBS-GPL.txt')
foreach ($name in $managedNames) {
    $destination = Join-Path $installRoot $name
    if (Test-Path -LiteralPath $destination) { Remove-Item -LiteralPath $destination -Recurse -Force }
    $source = Join-Path $stage $name
    if (Test-Path -LiteralPath $source) { Move-Item -LiteralPath $source -Destination $destination }
}
Remove-Item -LiteralPath $stage -Recurse -Force

& $installedEngine --install-autostart
if ($LASTEXITCODE -ne 0) { throw 'Installed engine could not register autostart.' }
Write-Host "Installed: $installRoot" -ForegroundColor Green
Write-Host 'Luma Next will start in the system tray at the next sign-in.'
Write-Host "Start now: & '$installedEngine'"
