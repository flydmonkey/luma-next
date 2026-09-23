[CmdletBinding()]
param(
    [ValidateRange(20, 300)][int]$Seconds = 20,
    [ValidateSet('auto', 'x264', 'amf', 'qsv')][string]$Encoder = 'auto',
    [ValidateSet('1080p30', '1440p30', '2160p30')][string]$Quality = '1080p30',
    [switch]$RequireHw,
    [switch]$SkipMicrophone,
    [switch]$SkipM6,
    [string]$WindowTitleSubstring,
    [string]$GameTitleSubstring,
    [switch]$SkipGameProbe,
    [switch]$EngineAlreadyRunning
)
$ErrorActionPreference = 'Stop'
$baseUrl = 'http://127.0.0.1:18765'
$engineProcess = $null
$recording = $false
$gameProbeProcess = $null
$repository = Split-Path $PSScriptRoot

function Resolve-Ffprobe {
    if ($env:LUMA_FFPROBE) { return $env:LUMA_FFPROBE }
    $installed = Join-Path $env:LOCALAPPDATA 'LumaNext\bin\ffprobe.exe'
    if (Test-Path -LiteralPath $installed -PathType Leaf) { return $installed }
    $packaged = Join-Path $repository 'dist\LumaNext\bin\ffprobe.exe'
    if (Test-Path -LiteralPath $packaged -PathType Leaf) { return $packaged }
    return 'ffprobe.exe'
}

$ffprobe = Resolve-Ffprobe

function Invoke-LumaApi([string]$Path, [string]$Method = 'GET', [object]$Body = $null) {
    $parameters = @{ Uri = "$baseUrl$Path"; Method = $Method; UseBasicParsing = $true; TimeoutSec = 2 }
    if ($null -ne $Body) { $parameters.ContentType = 'application/json'; $parameters.Body = ($Body | ConvertTo-Json -Compress) }
    $response = Invoke-RestMethod @parameters
    if (-not $response.ok) { throw "Luma API failed: $($response.error)" }
    return $response.data
}

function Wait-LumaStop([object]$Initial) {
    $session = $Initial
    $maxLatencyMs = 0.0
    foreach ($attempt in 1..150) {
        if ($session.state -ne 'stopping') { break }
        Start-Sleep -Milliseconds 200
        $started = Get-Date
        $session = Invoke-LumaApi '/api/v1/session'
        $latency = ((Get-Date) - $started).TotalMilliseconds
        $maxLatencyMs = [Math]::Max($maxLatencyMs, $latency)
    }
    if ($session.state -eq 'stopping') { throw 'Stop did not reach a terminal state within 30 seconds.' }
    if ($session.state -eq 'failed') { throw "Stop failed: $($session.error)" }
    if ($session.state -ne 'idle') { throw "Unexpected state after stop: $($session.state)" }
    Write-Host ("[stop-control] session remained responsive; max poll latency {0:N0}ms" -f $maxLatencyMs)
    return $session
}

function Invoke-RecordingCase([string]$EncoderId, [string]$Label, [bool]$Hardware, [string]$Mode = 'display', [string]$WindowId = '', [bool]$SystemAudio = $true, [bool]$Microphone = $false, [string]$DisplayId = 'primary', [hashtable]$Region = $null, [int]$PauseSeconds = 0, [string]$GameId = '') {
    Write-Host "[$Label] Starting a $Seconds-second recording with $EncoderId."
    $startedAt = Get-Date
    $script:recording = $true
    $start = Invoke-LumaApi '/api/v1/session/start' 'POST' @{ mode=$Mode; display_id=$DisplayId; region=$Region; window_id=$(if($WindowId){$WindowId}else{$null}); game_id=$(if($GameId){$GameId}else{$null}); system_audio=$SystemAudio; microphone=$Microphone; mic_device_id='default'; quality=$Quality; encoder=$EncoderId }
    Write-Host "[$Label] requested=$($start.encoder_requested) active=$($start.encoder_active) fallback=$($start.encoder_fallback)"
    if ($start.encoder_fallback) {
        Write-Warning "[$Label] fallback reason: $($start.fallback_reason)"
        if ($Hardware -and $RequireHw) { throw "Hardware encoder was required but fell back to $($start.encoder_active)." }
    }
    $activeSeconds = $Seconds
    if ($PauseSeconds -gt 0) {
        Start-Sleep -Seconds 5
        $beforePause = [double](Invoke-LumaApi '/api/v1/session').media_elapsed_seconds
        $paused = Invoke-LumaApi '/api/v1/session/pause' 'POST'
        Start-Sleep -Seconds $PauseSeconds
        $duringPause = [double](Invoke-LumaApi '/api/v1/session').media_elapsed_seconds
        if ($paused.state -ne 'paused' -or [Math]::Abs($duringPause-$beforePause) -gt 0.5) { throw "Pause clock did not freeze: before=$beforePause during=$duringPause." }
        $null = Invoke-LumaApi '/api/v1/session/resume' 'POST'
        $activeSeconds = [Math]::Max(1,$Seconds-5)
    }
    foreach ($remaining in $activeSeconds..1) {
        Write-Progress -Activity "Luma $Label regression" -Status "$remaining seconds remaining" -PercentComplete ((($Seconds-$remaining)/$Seconds)*100)
        Start-Sleep -Seconds 1
    }
    Write-Progress -Activity "Luma $Label regression" -Completed
    $live = Invoke-LumaApi '/api/v1/session'
    $stop = Wait-LumaStop (Invoke-LumaApi '/api/v1/session/stop' 'POST')
    $script:recording = $false
    $wallSeconds = ((Get-Date)-$startedAt).TotalSeconds
    $outputPath = $stop.output_path
    if (-not (Test-Path -LiteralPath $outputPath -PathType Leaf)) { throw "Output file does not exist: $outputPath" }
    $probeJson = & $ffprobe -v error -show_entries 'stream=codec_type,codec_name,width,height:format=duration,size' -of json -- $outputPath
    if ($LASTEXITCODE -ne 0) { throw "ffprobe rejected $outputPath" }
    $probe = $probeJson | ConvertFrom-Json
    $video = @($probe.streams | Where-Object codec_type -eq 'video') | Select-Object -First 1
    $audio = @($probe.streams | Where-Object codec_type -eq 'audio') | Select-Object -First 1
    $mediaSeconds = [double]$probe.format.duration; $sizeBytes = [long]$probe.format.size
    $expectedWall = $wallSeconds-$PauseSeconds
    $tolerance = [Math]::Max(2.5,$expectedWall*0.005)
    if ($null -eq $audio -or ($Mode -ne 'audio_only' -and $null -eq $video)) { throw "Unexpected streams for mode ${Mode}: video=$($null -ne $video) audio=$($null -ne $audio)." }
    if($Mode -eq 'audio_only' -and ($null -eq $video -or $video.width -ne 32 -or $video.height -ne 32)){throw 'Audio-only must contain the documented 32x32 placeholder video.'}
    if ($Mode -eq 'region') { if ($video.width -ne $Region.width -or $video.height -ne $Region.height) { throw "Region resolution mismatch: expected $($Region.width)x$($Region.height), got $($video.width)x$($video.height)." } }
    elseif ($Mode -ne 'audio_only' -and ($video.width -lt 640 -or $video.height -lt 360)) { throw "Implausible resolution $($video.width)x$($video.height)." }
    if ($sizeBytes -lt 102400) { throw "Output is suspiciously small: $sizeBytes bytes." }
    if ($mediaSeconds -lt 1 -or [Math]::Abs($mediaSeconds-$expectedWall) -gt $tolerance) { throw "Duration mismatch: media=$mediaSeconds active-wall=$expectedWall tolerance=$tolerance." }
    $obsMedia=[double]$stop.media_elapsed_seconds; $uiMedia=[double]$stop.elapsed_seconds; $liveMedia=[double]$live.media_elapsed_seconds
    if([Math]::Abs($obsMedia-$mediaSeconds) -ge 1.5){throw "OBS media/ffprobe mismatch: obs=$obsMedia ffprobe=$mediaSeconds."}
    if([Math]::Abs($uiMedia-$mediaSeconds) -ge 1.0){throw "Stopped UI timer disagreed with ffprobe: ui=$uiMedia ffprobe=$mediaSeconds."}
    Write-Host "[$Label] PASS: $outputPath" -ForegroundColor Green
    $mediaDescription=if($Mode -eq 'audio_only'){"audio-only $($audio.codec_name)"}else{"$($video.width)x$($video.height) $($video.codec_name)"}
    Write-Host ("[$Label] {0}, ffprobe {1:N2}s, OBS media {2:N2}s, stopped UI {3:N2}s, pre-stop live {4:N2}s, OBS wall {5:N2}s, {6:N0} bytes; active={7}, fallback={8}" -f $mediaDescription,$mediaSeconds,$obsMedia,$uiMedia,$liveMedia,$stop.wall_elapsed_seconds,$sizeBytes,$stop.encoder_active,$stop.encoder_fallback)
}

try {
    try { $null=Invoke-LumaApi '/api/v1' } catch {
        if ($EngineAlreadyRunning) { throw 'Luma engine is not reachable on 127.0.0.1:18765.' }
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
    if($Encoder -in @('auto','amf','qsv')){
        $selected=if($Encoder -eq 'amf'){$hardware|Where-Object id -eq 'h264_texture_amf'|Select-Object -First 1}elseif($Encoder -eq 'qsv'){$hardware|Where-Object id -eq 'obs_qsv11_v2'|Select-Object -First 1}else{$hardware|Select-Object -First 1}
        if($null -eq $selected){$reason='No available H.264 hardware encoder was reported by libobs.';if($RequireHw){throw $reason};Write-Host "[hardware] SKIP: $reason" -ForegroundColor Yellow}else{Invoke-RecordingCase $selected.id 'hardware' $true}
    }
    if(-not $SkipMicrophone){
        $inputs=@((Invoke-LumaApi '/api/v1/devices/audio').inputs)
        if($inputs.Count -eq 0){Write-Host '[microphone] SKIP: OBS/WASAPI reported no input device.' -ForegroundColor Yellow}else{Invoke-RecordingCase 'obs_x264' 'microphone' $false 'display' '' $true $true}
    }
    Invoke-RecordingCase 'obs_x264' 'audio-only' $false 'audio_only' '' $true $false
    if($WindowTitleSubstring){
        $windows=@((Invoke-LumaApi '/api/v1/targets').windows)
        $window=$windows|Where-Object {$_.available -and $_.title -like "*$WindowTitleSubstring*"}|Select-Object -First 1
        if($null -eq $window){throw "No capturable window title contains '$WindowTitleSubstring'."}
        Invoke-RecordingCase 'obs_x264' 'window' $false 'window' $window.id $true $false
    }
    if(-not $SkipM6){
        $targets=Invoke-LumaApi '/api/v1/targets';$displays=@($targets.displays);$primary=$displays|Where-Object primary|Select-Object -First 1
        if($null -eq $primary){throw 'No primary display was enumerated for M6 regression.'}
        $region=@{x=[int]$primary.x+100;y=[int]$primary.y+100;width=640;height=360}
        Invoke-RecordingCase 'obs_x264' 'region-pause' $false 'region' '' $true $false $primary.id $region 5
        Invoke-RecordingCase 'obs_x264' 'pause' $false 'display' '' $true $false $primary.id $null 5
        if($displays.Count -lt 2){Write-Host '[multi-monitor] SKIP: only one display is connected.' -ForegroundColor Yellow}else{Invoke-RecordingCase 'obs_x264' 'secondary-display' $false 'display' '' $true $false $displays[1].id}
    }
    if(-not $GameTitleSubstring -and -not $SkipGameProbe){
        & cargo.exe build -p luma-game-capture-probe
        if($LASTEXITCODE -ne 0){throw 'Failed to build the DX11 game-capture probe.'}
        $gameProbeProcess=Start-Process (Join-Path $repository 'target\debug\luma-game-capture-probe.exe') -PassThru
        $GameTitleSubstring='Luma DX11 Game Capture Probe'
        Start-Sleep -Seconds 2
    }
    if($GameTitleSubstring){
        $games=@((Invoke-LumaApi '/api/v1/targets').games)
        $game=$games|Where-Object {$_.available -and $_.title -like "*$GameTitleSubstring*"}|Select-Object -First 1
        if($null -eq $game){throw "No OBS game_capture candidate title contains '$GameTitleSubstring'."}
        Invoke-RecordingCase 'obs_x264' 'game-pause' $false 'game' '' $true $false 'primary' $null 5 $game.id
    } else {
        Write-Host '[game] SKIP: DX11 probe was disabled and no compatible game target was requested.' -ForegroundColor Yellow
    }
} catch {
    if($recording){try{$null=Invoke-LumaApi '/api/v1/session/stop' 'POST'}catch{}}
    Write-Error $_; exit 1
} finally {
    if($null -ne $engineProcess -and -not $engineProcess.HasExited){Stop-Process -Id $engineProcess.Id -Force}
    if($null -ne $gameProbeProcess -and -not $gameProbeProcess.HasExited){Stop-Process -Id $gameProbeProcess.Id}
}
