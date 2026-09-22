[CmdletBinding()]
param([Parameter(Mandatory)][string]$File)
$ErrorActionPreference = 'Stop'
if (-not $env:LUMA_CODE_SIGN_CERT -and -not $env:LUMA_CODE_SIGN_PFX) {
    Write-Host '[sign] SKIP: neither LUMA_CODE_SIGN_CERT nor LUMA_CODE_SIGN_PFX is configured; the binary remains unsigned.' -ForegroundColor Yellow
    exit 0
}
$signTool = if ($env:SIGNTOOL) { $env:SIGNTOOL } else { (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source }
if (-not $signTool) { throw 'SIGNTOOL is not configured and signtool.exe was not found.' }
$resolved = [System.IO.Path]::GetFullPath($File)
if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) { throw "Signing target does not exist: $resolved" }
$signArguments = @('sign','/fd','SHA256','/tr','http://timestamp.digicert.com','/td','SHA256')
if ($env:LUMA_CODE_SIGN_PFX) {
    if (-not (Test-Path -LiteralPath $env:LUMA_CODE_SIGN_PFX -PathType Leaf)) { throw 'LUMA_CODE_SIGN_PFX does not name a readable PFX file.' }
    if (-not $env:LUMA_CODE_SIGN_PASSWORD) { throw 'LUMA_CODE_SIGN_PASSWORD is required with LUMA_CODE_SIGN_PFX.' }
    $signArguments += @('/f',$env:LUMA_CODE_SIGN_PFX,'/p',$env:LUMA_CODE_SIGN_PASSWORD)
} else {
    $signArguments += @('/sha1',$env:LUMA_CODE_SIGN_CERT)
}
$signArguments += $resolved
& $signTool @signArguments
if ($LASTEXITCODE -ne 0) { throw "signtool failed for $resolved" }
& $signTool verify /pa /v $resolved
if ($LASTEXITCODE -ne 0) { throw "signature verification failed for $resolved" }
Write-Host "[sign] PASS: $resolved" -ForegroundColor Green
