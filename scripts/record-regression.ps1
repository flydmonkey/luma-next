[CmdletBinding()]
param(
    [ValidateRange(20, 300)]
    [int]$Seconds = 20,
    [switch]$EngineAlreadyRunning
)

$ErrorActionPreference = 'Stop'
$baseUrl = 'http://127.0.0.1:18765'
$engineProcess = $null
$startedAt = $null

function Invoke-LumaApi {
    param([string]$Path, [string]$Method = 'GET', [object]$Body = $null)
    $parameters = @{ Uri = "$baseUrl$Path"; Method = $Method; UseBasicParsing = $true }
    if ($null -ne $Body) {
        $parameters.ContentType = 'application/json'
        $parameters.Body = ($Body | ConvertTo-Json -Compress)
    }
    $response = Invoke-RestMethod @parameters
    if (-not $response.ok) { throw "Luma API failed: $($response.error)" }
    return $response.data
}

try {
    try { $null = Invoke-LumaApi '/api/v1' } catch {
        if ($EngineAlreadyRunning) { throw 'Luma engine is not reachable on 127.0.0.1:18765.' }
        $repository = Split-Path $PSScriptRoot
        Write-Host 'Engine not detected; building luma-engine...'
        & cargo.exe build -p luma-engine
        if ($LASTEXITCODE -ne 0) { throw 'cargo build -p luma-engine failed.' }
        $enginePath = Join-Path $repository 'target\debug\luma-engine.exe'
        $engineLog = Join-Path ([System.IO.Path]::GetTempPath()) 'luma-regression-engine.log'
        $engineErrorLog = Join-Path ([System.IO.Path]::GetTempPath()) 'luma-regression-engine-error.log'
        Write-Host "Starting engine (logs: $engineLog, $engineErrorLog)..."
        $engineProcess = Start-Process -FilePath $enginePath -WorkingDirectory $repository -WindowStyle Hidden -RedirectStandardOutput $engineLog -RedirectStandardError $engineErrorLog -PassThru
        $ready = $false
        foreach ($attempt in 1..90) {
            Start-Sleep -Milliseconds 500
            try { $null = Invoke-LumaApi '/api/v1'; $ready = $true; break } catch {}
            if ($engineProcess.HasExited) { throw "Engine exited during startup with code $($engineProcess.ExitCode). See $engineLog." }
        }
        if (-not $ready) { throw 'Engine did not become ready within 45 seconds.' }
    }

    Write-Host "Starting a $Seconds-second display + system-audio recording."
    $startedAt = Get-Date
    $start = Invoke-LumaApi '/api/v1/session/start' 'POST' @{
        mode = 'display'; system_audio = $true; microphone = $false; quality = '1080p30'
    }
    foreach ($remaining in $Seconds..1) {
        Write-Progress -Activity 'Luma recording regression' -Status "$remaining seconds remaining" -PercentComplete ((($Seconds - $remaining) / $Seconds) * 100)
        Start-Sleep -Seconds 1
    }
    Write-Progress -Activity 'Luma recording regression' -Completed
    $stop = Invoke-LumaApi '/api/v1/session/stop' 'POST'
    $wallSeconds = ((Get-Date) - $startedAt).TotalSeconds
    $outputPath = $stop.output_path
    if (-not (Test-Path -LiteralPath $outputPath -PathType Leaf)) { throw "Output file does not exist: $outputPath" }

    $probeJson = & ffprobe.exe -v error -show_entries 'stream=codec_type,width,height,duration:format=duration,size' -of json -- $outputPath
    if ($LASTEXITCODE -ne 0) { throw "ffprobe rejected the output file: $outputPath" }
    $probe = $probeJson | ConvertFrom-Json
    $video = @($probe.streams | Where-Object codec_type -eq 'video') | Select-Object -First 1
    $audio = @($probe.streams | Where-Object codec_type -eq 'audio') | Select-Object -First 1
    $mediaSeconds = [double]$probe.format.duration
    $sizeBytes = [long]$probe.format.size
    $tolerance = [Math]::Max(3.0, $wallSeconds * 0.35)
    if ($null -eq $video) { throw 'ffprobe found no video stream.' }
    if ($null -eq $audio) { throw 'ffprobe found no audio stream.' }
    if ($video.width -lt 640 -or $video.height -lt 360) { throw "Video resolution is implausible: $($video.width)x$($video.height)." }
    if ($sizeBytes -lt 102400) { throw "Output is suspiciously small: $sizeBytes bytes." }
    if ($mediaSeconds -lt 1 -or [Math]::Abs($mediaSeconds - $wallSeconds) -gt $tolerance) { throw "Duration mismatch: media=$mediaSeconds wall=$wallSeconds tolerance=$tolerance." }

    Write-Host 'PASS: recording contains credible video and audio.' -ForegroundColor Green
    Write-Host "Output: $outputPath"
    Write-Host ("Summary: {0}x{1}, video+audio, media {2:N2}s, wall {3:N2}s, {4:N0} bytes" -f $video.width, $video.height, $mediaSeconds, $wallSeconds, $sizeBytes)
} catch {
    if ($null -ne $startedAt) {
        try { $null = Invoke-LumaApi '/api/v1/session/stop' 'POST' } catch {}
    }
    Write-Error $_
    exit 1
} finally {
    if ($null -ne $engineProcess -and -not $engineProcess.HasExited) { Stop-Process -Id $engineProcess.Id -Force }
}
