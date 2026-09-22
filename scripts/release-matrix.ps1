[CmdletBinding()]
param(
    [switch]$SkipClippy,
    [switch]$SkipRegression,
    [switch]$SkipReleaseSmoke,
    [switch]$RequireManualPass,
    [string]$ManualChecklist,
    [ValidateRange(20,300)][int]$RegressionSeconds=20
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot
$sha=(& git -C $repo rev-parse --short HEAD).Trim()
$artifactRoot=Join-Path $repo 'artifacts'
New-Item -ItemType Directory -Path $artifactRoot -Force | Out-Null
$report=Join-Path $artifactRoot "release-matrix-$sha.md"
if([string]::IsNullOrWhiteSpace($ManualChecklist)){$ManualChecklist=Join-Path $artifactRoot "release-manual-checklist-$sha.md"}
if(-not (Test-Path -LiteralPath $ManualChecklist)){Copy-Item -LiteralPath (Join-Path $repo 'docs\release-manual-checklist.md') -Destination $ManualChecklist}
$results=[Collections.Generic.List[object]]::new()
function Invoke-MatrixStep([string]$Name,[scriptblock]$Action){
    $log=Join-Path $artifactRoot ((($Name -replace '[^A-Za-z0-9.-]','-').Trim('-'))+'.log')
    Write-Host "[matrix] $Name"
    try {
        $oldPreference=$ErrorActionPreference;$ErrorActionPreference='Continue'
        & $Action *>&1 | Tee-Object -FilePath $log
        $code=$LASTEXITCODE;$ErrorActionPreference=$oldPreference
        if($code -ne 0){throw "exit code $code"}
        $results.Add([pscustomobject]@{Name=$Name;Status='PASS';Log=$log})
    }
    catch {if($null -ne $oldPreference){$ErrorActionPreference=$oldPreference};$results.Add([pscustomobject]@{Name=$Name;Status='FAIL';Log=$log});"`nERROR: $_"|Add-Content $log;Write-Warning "$Name failed: $_"}
}
Push-Location $repo
try {
    Invoke-MatrixStep 'cargo-test-workspace' { cargo test --workspace }
    if(-not $SkipClippy){Invoke-MatrixStep 'cargo-clippy-deny-warnings' { cargo clippy --workspace --all-targets -- -D warnings }}
    if(-not $SkipRegression){Invoke-MatrixStep 'record-regression-full' { powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'record-regression.ps1') -Seconds $RegressionSeconds }}
    if(-not $SkipReleaseSmoke){Invoke-MatrixStep 'release-smoke' { powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'release-smoke.ps1') }}
    Invoke-MatrixStep 'single-instance-exit-2' {
        cargo build -p luma-engine;if($LASTEXITCODE -ne 0){throw "cargo build exit=$LASTEXITCODE"}
        $engine=Join-Path $repo 'target\debug\luma-engine.exe';$first=Start-Process $engine -ArgumentList '--no-tray','--no-hotkeys','--port','18768' -WindowStyle Hidden -PassThru
        try { Start-Sleep -Seconds 3;$second=Start-Process $engine -ArgumentList '--no-tray','--no-hotkeys','--port','18769' -WindowStyle Hidden -Wait -PassThru;if($second.ExitCode -ne 2){throw "second instance exit=$($second.ExitCode), expected 2"} }
        finally { if(-not $first.HasExited){Stop-Process -Id $first.Id} }
    }
} finally { Pop-Location }
$manualLines=@([IO.File]::ReadAllLines([IO.Path]::GetFullPath($ManualChecklist),[Text.Encoding]::UTF8))
[int]$manualPending=0
[int]$manualPassed=0
foreach($line in $manualLines){
    if($line -match '^- \[ \]'){$manualPending++}
    elseif($line -match '(?i)^- \[x\]'){$manualPassed++}
}
$autoFail=@($results|Where-Object Status -eq 'FAIL').Count;$autoPass=@($results|Where-Object Status -eq 'PASS').Count
$rows=($results|ForEach-Object{"| $($_.Name) | $($_.Status) | ``$($_.Log)`` |"}) -join "`n"
$content=@"
# Luma Next Release Matrix - $sha

Generated: $((Get-Date).ToString('o'))

| Automatic check | Result | Log |
|---|---|---|
$rows

## Manual checklist

- Checklist: ``$ManualChecklist``
- MANUAL_PASS: $manualPassed
- MANUAL_PENDING: $manualPending

## Summary

- AUTO_PASS: $autoPass
- AUTO_FAIL: $autoFail
- MANUAL_PENDING: $manualPending
"@
$content|Set-Content -LiteralPath $report -Encoding utf8
Write-Host "[matrix] Report: $report"
Write-Host "AUTO_PASS=$autoPass AUTO_FAIL=$autoFail MANUAL_PENDING=$manualPending"
if($autoFail -gt 0){exit 1}
if($RequireManualPass -and $manualPending -gt 0){Write-Error 'Manual checklist still contains unchecked items.';exit 2}
