[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$installRoot = Join-Path $env:LOCALAPPDATA 'LumaNext'
$installedEngine = Join-Path $installRoot 'bin\luma-engine.exe'
$running = @(Get-Process -Name 'luma-engine' -ErrorAction SilentlyContinue | Where-Object { $_.Path -and [System.IO.Path]::GetFullPath($_.Path) -eq [System.IO.Path]::GetFullPath($installedEngine) })
if ($running.Count -gt 0) { throw 'Luma Next is running. Exit it from the tray before uninstalling; the uninstaller will not force-kill it.' }

if (Test-Path -LiteralPath $installedEngine) {
    & $installedEngine --uninstall-autostart
    if ($LASTEXITCODE -ne 0) { throw 'Installed engine could not remove autostart.' }
} else {
    Remove-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name 'LumaNext' -ErrorAction SilentlyContinue
}

if (Test-Path -LiteralPath $installRoot) {
    $resolved = [System.IO.Path]::GetFullPath($installRoot)
    $expected = [System.IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'LumaNext'))
    if ($resolved -ne $expected) { throw "Refusing to modify unexpected path: $resolved" }
    foreach ($name in @('bin', 'obs', 'README.txt', 'manifest.json', 'LICENSE-OBS-GPL.txt', 'LICENSE-FFMPEG.txt', 'LumaNext.installing')) {
        $managedPath = Join-Path $resolved $name
        if (Test-Path -LiteralPath $managedPath) { Remove-Item -LiteralPath $managedPath -Recurse -Force }
    }
}
Write-Host 'Luma Next release files and autostart were removed. Recordings and local settings were preserved.' -ForegroundColor Green
