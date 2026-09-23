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
$notesTemplate=Join-Path $PSScriptRoot 'package\RELEASE-NOTES.zh-CN.md'
$notes=[IO.File]::ReadAllText($notesTemplate,[Text.Encoding]::UTF8)
$notes=$notes.Replace('{{VERSION}}',$Version)
$notes=$notes.Replace('{{ZIP_NAME}}',[IO.Path]::GetFileName($zip))
$notes=$notes.Replace('{{SIGNATURE_STATUS}}',$status)
$notes=$notes.Replace('{{SHA256}}',$hash)
$notes | Set-Content -LiteralPath (Join-Path $OutDir 'RELEASE-NOTES.md') -Encoding utf8
Write-Host "[publish] PASS: $zip" -ForegroundColor Green
Write-Host "[publish] Authenticode: $status; SHA256SUMS written. Review RELEASE-NOTES.md, then upload manually to GitHub Releases."
Write-Host "[publish] Next: powershell -ExecutionPolicy Bypass -File .\scripts\github-release.ps1 -Version '$Version' -SkipBuild -Draft:`$true"
