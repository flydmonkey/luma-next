[CmdletBinding()]
param(
    [string]$ObsRundir = $(if ($env:LUMA_OBS_RUNDIR) { $env:LUMA_OBS_RUNDIR } else { 'C:\Users\Administrator\Projects\obs-studio\build_x64\rundir\RelWithDebInfo' }),
    [switch]$NoZip,
    [switch]$Sign,
    [switch]$SkipFfplay
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
$obsSource = Split-Path (Split-Path (Split-Path $ObsRundir))
$encoderPluginInputs = @(
    (Join-Path $obsPlugins 'obs-qsv11.dll'),
    (Join-Path $obsPlugins 'obs-nvenc.dll'),
    (Join-Path $obsPlugins 'obs-ffmpeg.dll'),
    (Join-Path $obsBin 'obs-qsv-test.exe'),
    (Join-Path $obsBin 'obs-nvenc-test.exe'),
    (Join-Path $obsBin 'obs-amf-test.exe')
)
$ffmpegVersion = '9.0.2'
$ffmpegArchiveName = "ffmpeg-$ffmpegVersion-essentials_build.zip"
$ffmpegUrl = "https://www.gyan.dev/ffmpeg/builds/packages/$ffmpegArchiveName"
$ffmpegSha256 = '60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba'
$ffmpegCache = Join-Path $repo 'third_party\ffmpeg\cache'
$ffmpegArchive = Join-Path $ffmpegCache $ffmpegArchiveName
$ffmpegExtracted = Join-Path $ffmpegCache "ffmpeg-$ffmpegVersion-essentials_build"

foreach ($required in @($obsBin, $obsPlugins, $obsData, $obsLicense) + $encoderPluginInputs) {
    if (-not (Test-Path -LiteralPath $required)) { throw "Required OBS runtime input is missing: $required" }
}

New-Item -ItemType Directory -Force -Path $ffmpegCache | Out-Null
if (-not (Test-Path -LiteralPath $ffmpegArchive)) {
    Write-Host "Downloading pinned FFmpeg $ffmpegVersion essentials build..."
    Invoke-WebRequest -UseBasicParsing -Uri $ffmpegUrl -OutFile $ffmpegArchive
}
$actualHash = (Get-FileHash -LiteralPath $ffmpegArchive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualHash -ne $ffmpegSha256) {
    throw "FFmpeg archive SHA-256 mismatch. Expected $ffmpegSha256, got $actualHash. Delete $ffmpegArchive and retry."
}
if (-not (Test-Path -LiteralPath (Join-Path $ffmpegExtracted 'bin\ffprobe.exe'))) {
    if (Test-Path -LiteralPath $ffmpegExtracted) { Remove-Item -LiteralPath $ffmpegExtracted -Recurse -Force }
    Expand-Archive -LiteralPath $ffmpegArchive -DestinationPath $ffmpegCache -Force
}
$ffmpegBin = Join-Path $ffmpegExtracted 'bin'
$ffmpegLicense = Join-Path $ffmpegExtracted 'LICENSE'
foreach ($required in @((Join-Path $ffmpegBin 'ffmpeg.exe'), (Join-Path $ffmpegBin 'ffprobe.exe'), $ffmpegLicense)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) { throw "Pinned FFmpeg package is incomplete: $required" }
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
$vendorRuntimePatterns = [ordered]@{
    qsv = '(?i)^(lib)?(vpl|mfx).*\.dll$'
    nvenc = '(?i)^(nvEncodeAPI(64)?|nvml)\.dll$'
    amf = '(?i)^amfrt(32|64)\.dll$'
}
$vendorRuntimeDlls = [ordered]@{}
$depsRoot = Join-Path $obsSource '.deps'
$runtimeSearchRoots = @($obsBin)
if (Test-Path -LiteralPath $depsRoot) { $runtimeSearchRoots += $depsRoot }
foreach ($vendor in $vendorRuntimePatterns.Keys) {
    $runtimeDlls = @($runtimeSearchRoots | ForEach-Object {
            Get-ChildItem -LiteralPath $_ -Recurse -File -ErrorAction SilentlyContinue
        } | Where-Object { $_.Name -match $vendorRuntimePatterns[$vendor] } |
        Sort-Object Name -Unique)
    [string[]]$runtimeNames = @($runtimeDlls | ForEach-Object Name)
    $vendorRuntimeDlls[$vendor] = [object]$runtimeNames
    foreach ($runtimeDll in $runtimeDlls) {
        Copy-Item -LiteralPath $runtimeDll.FullName -Destination $binDestination.FullName -Force
    }
    if ($runtimeDlls.Count) {
        Write-Host "Bundled $vendor runtime DLLs: $($runtimeDlls.Name -join ', ')"
    } else {
        switch ($vendor) {
            qsv { Write-Warning 'QSV: no VPL/MFX runtime DLL exists in the official OBS rundir/deps; this build relies on its linked dispatcher and the Intel media driver/Driver Store.' }
            nvenc { Write-Warning 'NVENC: nvEncodeAPI64.dll/nvml.dll are not OBS redistributables; a supported NVIDIA driver must provide them.' }
            amf { Write-Warning 'AMF: amfrt64.dll is not an OBS redistributable; a supported AMD driver must provide it.' }
        }
    }
}
Get-ChildItem -LiteralPath $obsPlugins -File | Where-Object Extension -eq '.dll' | Copy-Item -Destination $pluginDestination.FullName
Copy-Item -LiteralPath $obsData -Destination $obsDestination.FullName -Recurse
Copy-Item -LiteralPath $obsLicense -Destination (Join-Path $packageRoot 'LICENSE-OBS-GPL.txt')
foreach ($tool in @('ffmpeg.exe', 'ffprobe.exe', 'ffplay.exe')) {
    if ($SkipFfplay -and $tool -eq 'ffplay.exe') { continue }
    $source = Join-Path $ffmpegBin $tool
    if (Test-Path -LiteralPath $source -PathType Leaf) { Copy-Item -LiteralPath $source -Destination $binDestination.FullName }
}
Copy-Item -LiteralPath $ffmpegLicense -Destination (Join-Path $packageRoot 'LICENSE-FFMPEG.txt')
foreach ($installer in @('Install.cmd', 'Install.ps1', 'Uninstall.cmd', 'Uninstall.ps1')) {
    $installerSource = Join-Path $PSScriptRoot "package\$installer"
    $installerDestination = Join-Path $packageRoot $installer
    if ($installer.EndsWith('.ps1', [StringComparison]::OrdinalIgnoreCase)) {
        # Windows PowerShell 5.1 treats UTF-8 without BOM as the current ANSI
        # code page. Package user-facing Chinese scripts with a BOM so the
        # double-click CMD entry points work on stock Windows.
        $installerText = [IO.File]::ReadAllText($installerSource, [Text.UTF8Encoding]::new($false))
        [IO.File]::WriteAllText($installerDestination, $installerText, [Text.UTF8Encoding]::new($true))
    } else {
        # cmd.exe expects conventional CRLF batch files. Normalize here so the
        # repository can retain its ordinary text-file line-ending policy.
        $installerText = [IO.File]::ReadAllText($installerSource, [Text.UTF8Encoding]::new($false)) -replace "`r?`n", "`r`n"
        [IO.File]::WriteAllText($installerDestination, $installerText, [Text.UTF8Encoding]::new($false))
    }
}
if ($Sign) {
    & (Join-Path $PSScriptRoot 'sign.ps1') -File (Join-Path $binDestination.FullName 'luma-engine.exe')
    if ($LASTEXITCODE -ne 0) { throw 'sign.ps1 failed.' }
}

$readmeTemplate = [IO.File]::ReadAllText((Join-Path $PSScriptRoot 'package\README.txt'), [Text.UTF8Encoding]::new($false))
$readme = $readmeTemplate.Replace('{{VERSION}}', $version.Trim()).Replace('{{FFMPEG_VERSION}}', $ffmpegVersion)
[IO.File]::WriteAllText((Join-Path $packageRoot 'README.txt'), $readme, [Text.UTF8Encoding]::new($true))
@{ name='LumaNext'; version=$version.Trim(); architecture='x64'; ffmpeg_version=$ffmpegVersion; ffmpeg_sha256=$ffmpegSha256; built_at=(Get-Date).ToUniversalTime().ToString('o'); encoder_runtime=@{required_plugins=@('obs-qsv11.dll','obs-nvenc.dll','obs-ffmpeg.dll'); required_helpers=@('obs-qsv-test.exe','obs-nvenc-test.exe','obs-amf-test.exe'); bundled_dlls=$vendorRuntimeDlls} } |
    ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $packageRoot 'manifest.json') -Encoding UTF8

if (-not $NoZip) {
    $zip = Join-Path $distRoot "LumaNext-$($version.Trim())-win-x64.zip"
    if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
    Compress-Archive -Path (Join-Path $packageRoot '*') -DestinationPath $zip -CompressionLevel Optimal
    Write-Host "ZIP: $zip"
}
Write-Host "Package: $packageRoot"
