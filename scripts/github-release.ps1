[CmdletBinding()]
param(
    [string]$Version,
    [string]$Tag,
    [string]$DistDir,
    [string]$NotesFile,
    [string]$Repo='flydmonkey/luma-next',
    [switch]$Sign,
    [switch]$SkipBuild,
    [bool]$Draft=$true,
    [switch]$Latest,
    [switch]$ReplaceDraft,
    [Alias('WhatIf')][switch]$DryRun
)
$ErrorActionPreference='Stop'
$repoRoot=Split-Path $PSScriptRoot
if([string]::IsNullOrWhiteSpace($DistDir)){$DistDir=Join-Path $repoRoot 'dist\publish'}
$DistDir=[IO.Path]::GetFullPath($DistDir)

if(-not $SkipBuild){
    if([string]::IsNullOrWhiteSpace($Version)){$Version=(& git -C $repoRoot describe --tags --always 2>$null).Trim()}
    & (Join-Path $PSScriptRoot 'publish.ps1') -Version $Version -OutDir $DistDir -Sign:$Sign
    if($LASTEXITCODE -ne 0){throw 'publish.ps1 failed'}
}

$zips=@(Get-ChildItem -LiteralPath $DistDir -Filter 'LumaNext-*-win-x64.zip' -File)
if([string]::IsNullOrWhiteSpace($Version)){
    if($zips.Count -ne 1){throw "Version was omitted and $($zips.Count) matching ZIPs exist in $DistDir; pass -Version explicitly."}
    $Version=$zips[0].BaseName -replace '^LumaNext-','' -replace '-win-x64$',''
}
$safeVersion=$Version -replace '[^A-Za-z0-9._-]','-'
$zip=Join-Path $DistDir "LumaNext-$safeVersion-win-x64.zip"
$sums=Join-Path $DistDir 'SHA256SUMS'
if([string]::IsNullOrWhiteSpace($NotesFile)){$NotesFile=Join-Path $DistDir 'RELEASE-NOTES.md'}
foreach($required in @($zip,$sums,$NotesFile)){if(-not(Test-Path -LiteralPath $required -PathType Leaf)){throw "Required release file is missing: $required"}}
$sumLine=Get-Content -LiteralPath $sums | Where-Object{$_ -match "\s+$([regex]::Escape([IO.Path]::GetFileName($zip)))$"} | Select-Object -First 1
if(-not $sumLine){throw "SHA256SUMS has no entry for $([IO.Path]::GetFileName($zip))"}
$expected=($sumLine -split '\s+')[0].ToLowerInvariant();$actual=(Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
if($expected -ne $actual){throw "ZIP SHA-256 mismatch: expected $expected, got $actual"}
if([string]::IsNullOrWhiteSpace($Tag)){$Tag="v$Version"}

$notes=Get-Content -LiteralPath $NotesFile -Raw
$signature=if($notes -match 'Authenticode:\s*Signed'){'Signed'}else{'Unsigned'}
$matrix=Get-ChildItem -LiteralPath (Join-Path $repoRoot 'artifacts') -Filter 'release-matrix-*.md' -File -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$matrixSummary='No local release matrix report was found.';$manualPending='unknown'
if($matrix){
    $matrixText=Get-Content -LiteralPath $matrix.FullName -Raw
    $autoPass=if($matrixText -match 'AUTO_PASS:\s*(\d+)'){$Matches[1]}else{'unknown'}
    $autoFail=if($matrixText -match 'AUTO_FAIL:\s*(\d+)'){$Matches[1]}else{'unknown'}
    $manualPending=if($matrixText -match 'MANUAL_PENDING:\s*(\d+)'){$Matches[1]}else{'unknown'}
    $matrixSummary="Local matrix $($matrix.Name): AUTO_PASS=$autoPass, AUTO_FAIL=$autoFail, MANUAL_PENDING=$manualPending."
}
$unsignedNotice=if($signature -eq 'Unsigned'){"`n> **Unsigned build:** Windows SmartScreen warnings are expected. Verify SHA256SUMS before running.`n"}else{''}
$body="$notes`n$unsignedNotice`n## Release verification`n`n- Authenticode: $signature`n- $matrixSummary`n- Manual checklist items still pending: $manualPending`n- Local matrix artifacts are not uploaded automatically; complete the repository checklist before publishing this draft.`n"
$assets=@($zip,$sums)
Write-Host "[github-release] Repo=$Repo Tag=$Tag Draft=$Draft Latest=$Latest Signature=$signature"
Write-Host "[github-release] SHA256=$actual"
$assets|ForEach-Object{Write-Host "[github-release] Asset=$_"}
if($DryRun){Write-Host '[github-release] DRY RUN: no tag or release was created.' -ForegroundColor Yellow;exit 0}

$gh=(Get-Command gh.exe -ErrorAction SilentlyContinue).Source
if(-not $gh){throw 'GitHub CLI (gh) is required. Install it, then run gh auth login or set GH_TOKEN/GITHUB_TOKEN.'}
& $gh auth status *> $null
if($LASTEXITCODE -ne 0 -and -not $env:GH_TOKEN -and -not $env:GITHUB_TOKEN){throw 'GitHub authentication is missing. Run gh auth login or set GH_TOKEN/GITHUB_TOKEN with repo scope.'}
$existingJson=& $gh release view $Tag -R $Repo --json isDraft,url 2>$null
if($LASTEXITCODE -eq 0){
    $existing=$existingJson|ConvertFrom-Json
    if(-not $existing.isDraft){throw "Release $Tag already exists and is published: $($existing.url). Refusing to overwrite it."}
    if(-not $ReplaceDraft){throw "Draft release $Tag already exists: $($existing.url). Pass -ReplaceDraft to replace only that draft."}
    & $gh release delete $Tag -R $Repo --yes --cleanup-tag
    if($LASTEXITCODE -ne 0){throw "Failed to delete existing draft $Tag"}
}
$temporaryNotes=Join-Path ([IO.Path]::GetTempPath()) "luma-release-notes-$PID.md"
try{
    [IO.File]::WriteAllText($temporaryNotes,$body,[Text.UTF8Encoding]::new($false))
    $arguments=@('release','create',$Tag)+$assets+@('-R',$Repo,'--title',"Luma Next $Version",'--notes-file',$temporaryNotes,'--target','main')
    if($Draft){$arguments+='--draft'}
    if($Latest){$arguments+='--latest'}
    $url=& $gh @arguments
    if($LASTEXITCODE -ne 0){throw 'gh release create failed'}
    if([string]::IsNullOrWhiteSpace($url)){$url=& $gh release view $Tag -R $Repo --json url --jq .url}
    Write-Host "[github-release] PASS: $url" -ForegroundColor Green
}finally{if(Test-Path -LiteralPath $temporaryNotes){Remove-Item -LiteralPath $temporaryNotes -Force}}
