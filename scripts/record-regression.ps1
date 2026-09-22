[CmdletBinding()]
param(
    [ValidateRange(20, 300)][int]$Seconds = 20,
    [ValidateSet('auto', 'x264', 'amf')][string]$Encoder = 'auto',
    [switch]$RequireHw,
    [switch]$EngineAlreadyRunning
)
$ErrorActionPreference = 'Stop'
$baseUrl = 'http://127.0.0.1:18765'
$engineProcess = $null
$recording = $false

function Invoke-LumaApi([string]$Path, [string]$Method = 'GET', [object]$Body = $null) {
    $parameters = @{ Uri = "$baseUrl$Path"; Method = $Method; UseBasicParsing = $true }
    if ($null -ne $Body) { $parameters.ContentType = 'application/json'; $parameters.Body = ($Body | ConvertTo-Json -Compress) }
    $response = Invoke-RestMethod @parameters
    if (-not $response.ok) { throw "Luma API failed: $($response.error)" }
    return $response.data
}

function Invoke-RecordingCase([string]$EncoderId, [string]$Label, [bool]$Hardware) {
    Write-Host "[$Label] Starting a $Seconds-second recording with $EncoderId."
    $startedAt = Get-Date
    $script:recording = $true
    $start = Invoke-LumaApi '/api/v1/session/start' 'POST' @{ mode='display'; system_audio=$true; microphone=$false; quality='1080p30'; encoder=$EncoderId }
    Write-Host "[$Label] requested=$($start.encoder_requested) active=$($start.encoder_active) fallback=$($start.encoder_fallback)"
    if ($start.encoder_fallback) {
        Write-Warning "[$Label] fallback reason: $($start.fallback_reason)"
        if ($Hardware -and $RequireHw) { throw "Hardware encoder was required but fell back to $($start.encoder_active)." }
    }
    foreach ($remaining in $Seconds..1) {
        Write-Progress -Activity "Luma $Label regression" -Status "$remaining seconds remaining" -PercentComplete ((($Seconds-$remaining)/$Seconds)*100)
        Start-Sleep -Seconds 1
    }
    Write-Progress -Activity "Luma $Label regression" -Completed
    $stop = Invoke-LumaApi '/api/v1/session/stop' 'POST'
    $script:recording = $false
    $wallSeconds = ((Get-Date)-$startedAt).TotalSeconds
    $outputPath = $stop.output_path
    if (-not (Test-Path -LiteralPath $outputPath -PathType Leaf)) { throw "Output file does not exist: $outputPath" }
    $probeJson = & ffprobe.exe -v error -show_entries 'stream=codec_type,codec_name,width,height:format=duration,size' -of json -- $outputPath
    if ($LASTEXITCODE -ne 0) { throw "ffprobe rejected $outputPath" }
    $probe = $probeJson | ConvertFrom-Json
    $video = @($probe.streams | Where-Object codec_type -eq 'video') | Select-Object -First 1
    $audio = @($probe.streams | Where-Object codec_type -eq 'audio') | Select-Object -First 1
    $mediaSeconds = [double]$probe.format.duration; $sizeBytes = [long]$probe.format.size
    $tolerance = [Math]::Max(3.0,$wallSeconds*0.35)
    if ($null -eq $video -or $null -eq $audio) { throw 'ffprobe did not find both video and audio streams.' }
    if ($video.width -lt 640 -or $video.height -lt 360) { throw "Implausible resolution $($video.width)x$($video.height)." }
    if ($sizeBytes -lt 102400) { throw "Output is suspiciously small: $sizeBytes bytes." }
    if ($mediaSeconds -lt 1 -or [Math]::Abs($mediaSeconds-$wallSeconds) -gt $tolerance) { throw "Duration mismatch: media=$mediaSeconds wall=$wallSeconds tolerance=$tolerance." }
    Write-Host "[$Label] PASS: $outputPath" -ForegroundColor Green
    Write-Host ("[$Label] {0}x{1} {2}, video+audio, media {3:N2}s, wall {4:N2}s, {5:N0} bytes; active={6}, fallback={7}" -f $video.width,$video.height,$video.codec_name,$mediaSeconds,$wallSeconds,$sizeBytes,$stop.encoder_active,$stop.encoder_fallback)
}

try {
    try { $null=Invoke-LumaApi '/api/v1' } catch {
        if ($EngineAlreadyRunning) { throw 'Luma engine is not reachable on 127.0.0.1:18765.' }
        $repository=Split-Path $PSScriptRoot
        & cargo.exe build -p luma-engine
        if ($LASTEXITCODE -ne 0) { throw 'cargo build -p luma-engine failed.' }
        $enginePath=Join-Path $repository 'target\debug\luma-engine.exe'; $temp=[System.IO.Path]::GetTempPath()
        $engineProcess=Start-Process $enginePath -ArgumentList '--no-tray' -WorkingDirectory $repository -WindowStyle Hidden -RedirectStandardOutput (Join-Path $temp 'luma-regression-engine.log') -RedirectStandardError (Join-Path $temp 'luma-regression-engine-error.log') -PassThru
        $ready=$false; foreach($attempt in 1..90){Start-Sleep -Milliseconds 500;try{$null=Invoke-LumaApi '/api/v1';$ready=$true;break}catch{};if($engineProcess.HasExited){throw "Engine exited during startup with code $($engineProcess.ExitCode)."}}
        if(-not $ready){throw 'Engine did not become ready within 45 seconds.'}
    }
    $encoders=(Invoke-LumaApi '/api/v1/encoders').encoders
    $hardware=@($encoders | Where-Object {$_.available -and $_.hardware}) | Sort-Object @{Expression={if($_.id -eq 'h264_texture_amf'){0}else{1}}}
    if($Encoder -in @('auto','x264')){Invoke-RecordingCase 'obs_x264' 'x264' $false}
    if($Encoder -in @('auto','amf')){
        $selected=if($Encoder -eq 'amf'){$hardware|Where-Object id -eq 'h264_texture_amf'|Select-Object -First 1}else{$hardware|Select-Object -First 1}
        if($null -eq $selected){$reason='No available H.264 hardware encoder was reported by libobs.';if($RequireHw){throw $reason};Write-Host "[hardware] SKIP: $reason" -ForegroundColor Yellow}else{Invoke-RecordingCase $selected.id 'hardware' $true}
    }
} catch {
    if($recording){try{$null=Invoke-LumaApi '/api/v1/session/stop' 'POST'}catch{}}
    Write-Error $_; exit 1
} finally {
    if($null -ne $engineProcess -and -not $engineProcess.HasExited){Stop-Process -Id $engineProcess.Id -Force}
}
