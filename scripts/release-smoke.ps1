[CmdletBinding()]
param([switch]$SkipPack,[switch]$SkipUninstall,[switch]$Sign,[ValidateRange(5,60)][int]$Seconds=8)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot
$installRoot=Join-Path $env:LOCALAPPDATA 'LumaNext'
$engine=Join-Path $installRoot 'bin\luma-engine.exe'
$ffprobe=Join-Path $installRoot 'bin\ffprobe.exe'
$process=$null
try {
    if(-not $SkipPack){& (Join-Path $PSScriptRoot 'pack.ps1') -NoZip -Sign:$Sign;if($LASTEXITCODE -ne 0){throw 'pack failed'}}
    & (Join-Path $repo 'dist\LumaNext\Install.ps1') -NoStart
    if($LASTEXITCODE -ne 0){throw 'install failed'}
    $run=(Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name LumaNext).LumaNext
    if($run -notlike "*$engine*"){throw "Run value does not point to installed engine: $run"}
    & $ffprobe -version | Select-Object -First 1
    if($LASTEXITCODE -ne 0){throw 'bundled ffprobe failed'}
    $process=Start-Process $engine -ArgumentList '--no-tray','--allow-second-instance','--port','18767' -WindowStyle Hidden -PassThru
    $base='http://127.0.0.1:18767';$ready=$false
    foreach($attempt in 1..60){Start-Sleep -Milliseconds 500;try{$probe=Invoke-RestMethod "$base/api/v1";$ready=$probe.ok;break}catch{}}
    if(-not $ready){throw 'installed engine did not become ready'}
    $start=Invoke-RestMethod "$base/api/v1/session/start" -Method Post -ContentType application/json -Body '{"mode":"audio_only","system_audio":true,"microphone":false,"encoder":"obs_x264"}'
    if(-not $start.ok){throw $start.error};Start-Sleep -Seconds $Seconds
    $stop=Invoke-RestMethod "$base/api/v1/session/stop" -Method Post
    if(-not $stop.ok){throw $stop.error}
    if(-not (Test-Path -LiteralPath $stop.data.output_path)){throw 'smoke recording is missing'}
    Write-Host "[release-smoke] PASS: $($stop.data.output_path); Run=$run" -ForegroundColor Green
} finally {
    if($process -and -not $process.HasExited){Stop-Process -Id $process.Id}
    if(-not $SkipUninstall){& (Join-Path $repo 'dist\LumaNext\Uninstall.ps1')}
}
