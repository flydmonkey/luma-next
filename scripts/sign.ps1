[CmdletBinding()]
param([Parameter(Mandatory)][string]$File)
$ErrorActionPreference = 'Stop'
if (-not $env:LUMA_CODE_SIGN_CERT) {
    Write-Host '[sign] SKIP: LUMA_CODE_SIGN_CERT is not configured; the binary remains unsigned.' -ForegroundColor Yellow
    exit 0
}
$signTool = if ($env:SIGNTOOL) { $env:SIGNTOOL } else { (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source }
if (-not $signTool) { throw 'SIGNTOOL is not configured and signtool.exe was not found.' }
$resolved = [System.IO.Path]::GetFullPath($File)
if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) { throw "Signing target does not exist: $resolved" }
& $signTool sign /sha1 $env:LUMA_CODE_SIGN_CERT /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $resolved
if ($LASTEXITCODE -ne 0) { throw "signtool failed for $resolved" }
& $signTool verify /pa /v $resolved
if ($LASTEXITCODE -ne 0) { throw "signature verification failed for $resolved" }
Write-Host "[sign] PASS: $resolved" -ForegroundColor Green
