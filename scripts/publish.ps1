[CmdletBinding()]
param(
    [switch]$Sign,
    [switch]$SkipFfplay,
    [string]$OutDir,
    [string]$Version
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot
if([string]::IsNullOrWhiteSpace($OutDir)){$OutDir=Join-Path $repo 'dist\publish'}
$OutDir=[System.IO.Path]::GetFullPath($OutDir)
if([string]::IsNullOrWhiteSpace($Version)){$Version=(& git -C $repo describe --tags --always 2>$null).Trim()}
if([string]::IsNullOrWhiteSpace($Version)){$Version='0.1.0'}
$safeVersion=$Version -replace '[^A-Za-z0-9._-]','-'
& (Join-Path $PSScriptRoot 'pack.ps1') -NoZip -Sign:$Sign -SkipFfplay:$SkipFfplay
if($LASTEXITCODE -ne 0){throw 'pack.ps1 failed'}
$package=Join-Path $repo 'dist\LumaNext'
$engine=Join-Path $package 'bin\luma-engine.exe'
$signature=Get-AuthenticodeSignature -LiteralPath $engine
$signed=$signature.Status -eq 'Valid'
if($Sign -and ($env:LUMA_CODE_SIGN_CERT -or $env:LUMA_CODE_SIGN_PFX) -and -not $signed){throw "Signing was requested and configured, but Authenticode status is $($signature.Status)."}
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$zip=Join-Path $OutDir "LumaNext-$safeVersion-win-x64.zip"
if(Test-Path -LiteralPath $zip){Remove-Item -LiteralPath $zip -Force}
Compress-Archive -Path (Join-Path $package '*') -DestinationPath $zip -CompressionLevel Optimal
$hash=(Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
"$hash  $([IO.Path]::GetFileName($zip))" | Set-Content -LiteralPath (Join-Path $OutDir 'SHA256SUMS') -Encoding ascii
$status=if($signed){'Signed'}else{'Unsigned'}
$notes=@"
# Luma Next $Version

Luma Next 是 Windows 10/11 x64 屏幕录制工具，通过系统托盘和默认浏览器控制。本版本包含
libobs、WebUI 以及固定版本的 FFmpeg/ffprobe，不依赖 WebView2。

## 安装

1. 下载并完整解压 ``$([IO.Path]::GetFileName($zip))``。
2. 双击 ZIP 根目录的 ``Install.cmd``；无需管理员权限。
3. 安装完成后从系统托盘打开控制页。卸载可双击安装目录或解压包内的 ``Uninstall.cmd``；
   卸载会保留录像和本地设置。

## 完整性与签名

- Authenticode: **$status**
- SHA-256: ``$hash``
- 另附 ``SHA256SUMS``，安装前可用 ``Get-FileHash`` 核对。
- 无签名证书时该版本会明确标为 Unsigned，Windows SmartScreen 警告属于预期，不代表已经签名。

## 已知限制

- 硬件编码器依赖对应 Intel/AMD/NVIDIA GPU 与驱动；不可用时 API/UI 会显示原因，并可诚实降级 x264。
- 全屏播放器的硬件叠加层在部分显卡/播放器组合中可能出现黑边或黑屏；这属于捕获兼容性限制，
  与停录状态机和成片收尾无关。
- 游戏捕获受游戏、反作弊和权限影响；不提供绕过反作弊的注入能力。
- 多显示器、混合 DPI、真实游戏及全局热键应按 ``docs/RELEASE_SMOKE.md`` 在目标机器人工复核。

本发行包包含并链接 libobs，并捆绑 GPLv3 FFmpeg；整体以 GPL-3.0-or-later 分发。源码：
https://github.com/flydmonkey/luma-next
"@
$notes | Set-Content -LiteralPath (Join-Path $OutDir 'RELEASE-NOTES.md') -Encoding utf8
Write-Host "[publish] PASS: $zip" -ForegroundColor Green
Write-Host "[publish] Authenticode: $status; SHA256SUMS written. Review RELEASE-NOTES.md, then upload manually to GitHub Releases."
Write-Host "[publish] Next: powershell -ExecutionPolicy Bypass -File .\scripts\github-release.ps1 -Version '$Version' -SkipBuild -Draft:`$true"
