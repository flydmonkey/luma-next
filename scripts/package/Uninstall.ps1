[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'

try {
    $installRoot = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'LumaNext'))
    $expected = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'LumaNext'))
    if ($installRoot -ne $expected) { throw "拒绝修改非预期路径：$installRoot" }
    $installedEngine = Join-Path $installRoot 'bin\luma-engine.exe'
    $running = @(Get-Process -Name 'luma-engine' -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -and [IO.Path]::GetFullPath($_.Path) -eq [IO.Path]::GetFullPath($installedEngine) } catch { $false }
    })
    if ($running.Count -gt 0) { throw 'Luma Next 正在运行。请从系统托盘退出后重试；卸载器不会强制结束进程。' }

    if (Test-Path -LiteralPath $installedEngine) {
        & $installedEngine --uninstall-autostart
        if ($LASTEXITCODE -ne 0) { throw '引擎未能移除开机自启。' }
    } else {
        Remove-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name LumaNext -ErrorAction SilentlyContinue
    }

    $runningFromInstall = [IO.Path]::GetFullPath($PSScriptRoot).TrimEnd('\') -eq $installRoot.TrimEnd('\')
    $managedNames = @('bin', 'obs', 'README.txt', 'manifest.json', 'LICENSE-OBS-GPL.txt', 'LICENSE-FFMPEG.txt', 'LumaNext.installing')
    if (-not $runningFromInstall) { $managedNames += @('Install.cmd', 'Install.ps1', 'Uninstall.cmd', 'Uninstall.ps1') }
    foreach ($name in $managedNames) {
        $path = Join-Path $installRoot $name
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
    }
    Write-Host '卸载成功：发行文件与开机自启已移除。录像和本地设置已保留。' -ForegroundColor Green
    exit 0
} catch {
    Write-Error $_.Exception.Message
    exit 1
}
