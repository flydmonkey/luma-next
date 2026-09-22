[CmdletBinding()]
param([switch]$NoStart)
$ErrorActionPreference = 'Stop'

try {
    $packageRoot = [IO.Path]::GetFullPath($PSScriptRoot)
    $packageEngine = Join-Path $packageRoot 'bin\luma-engine.exe'
    foreach ($required in @($packageEngine, (Join-Path $packageRoot 'obs\data\libobs'), (Join-Path $packageRoot 'obs\obs-plugins\64bit'))) {
        if (-not (Test-Path -LiteralPath $required)) { throw "发布包不完整，缺少：$required" }
    }

    $installRoot = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'LumaNext'))
    $localRoot = [IO.Path]::GetFullPath($env:LOCALAPPDATA)
    if (-not $installRoot.StartsWith($localRoot, [StringComparison]::OrdinalIgnoreCase)) { throw "安装路径越界：$installRoot" }
    if ($packageRoot.TrimEnd('\') -eq $installRoot.TrimEnd('\')) {
        throw '当前脚本已经位于安装目录，无需重复安装。若要升级，请从新解压的发布包运行 Install.cmd。'
    }

    $installedEngine = Join-Path $installRoot 'bin\luma-engine.exe'
    $running = @(Get-Process -Name 'luma-engine' -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -and [IO.Path]::GetFullPath($_.Path) -eq [IO.Path]::GetFullPath($installedEngine) } catch { $false }
    })
    if ($running.Count -gt 0) { throw 'Luma Next 正在运行。请从系统托盘退出后重试；安装器不会强制结束进程。' }

    $stage = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'LumaNext-package-staging'))
    if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
    New-Item -ItemType Directory -Path $stage | Out-Null
    Copy-Item -Path (Join-Path $packageRoot '*') -Destination $stage -Recurse -Force
    New-Item -ItemType Directory -Path $installRoot -Force | Out-Null
    $managedNames = @('bin', 'obs', 'Install.cmd', 'Install.ps1', 'Uninstall.cmd', 'Uninstall.ps1', 'README.txt', 'manifest.json', 'LICENSE-OBS-GPL.txt', 'LICENSE-FFMPEG.txt')
    foreach ($name in $managedNames) {
        $destination = Join-Path $installRoot $name
        if (Test-Path -LiteralPath $destination) { Remove-Item -LiteralPath $destination -Recurse -Force }
        $source = Join-Path $stage $name
        if (Test-Path -LiteralPath $source) { Move-Item -LiteralPath $source -Destination $destination }
    }
    Remove-Item -LiteralPath $stage -Recurse -Force

    & $installedEngine --install-autostart
    if ($LASTEXITCODE -ne 0) { throw '引擎未能注册当前用户开机自启。' }
    Write-Host "安装成功：$installRoot" -ForegroundColor Green
    Write-Host '已注册当前用户登录自启；录像与本地设置不会被安装器覆盖。'
    if (-not $NoStart) {
        Start-Process -FilePath $installedEngine -WorkingDirectory (Split-Path $installedEngine) | Out-Null
        Write-Host 'Luma Next 已启动，请在系统托盘中打开控制页。' -ForegroundColor Green
    }
    exit 0
} catch {
    Write-Error $_.Exception.Message
    exit 1
}
