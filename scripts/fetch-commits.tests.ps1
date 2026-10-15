<#
.SYNOPSIS
Checks fetch-commits.ps1 offline: a matching checksum bundles both files, a
wrong one fails and bundles nothing.

.DESCRIPTION
Builds a stand-in archive and manifest in a temporary folder and runs the
script against them through -Manifest and -Cache, so it needs no network.
Exits non-zero when a check fails. CI runs it on every platform.
#>
$ErrorActionPreference = 'Stop'
$script = Join-Path $PSScriptRoot 'fetch-commits.ps1'
$work = Join-Path ([IO.Path]::GetTempPath()) "fetch-commits-test-$PID"
$failures = 0

function Check([string]$Name, [bool]$Passed) {
    if ($Passed) { Write-Host "ok   $Name" } else { Write-Host "FAIL $Name"; $script:failures++ }
}

function New-Release([string]$Folder, [string]$Sha256) {
    New-Item -ItemType Directory -Force -Path $Folder | Out-Null
    $archive = Join-Path $Folder 'commits-9.9.9-linux-x64.zip'
    Set-Content -LiteralPath $archive -Value 'not really a zip' -NoNewline
    if (-not $Sha256) { $Sha256 = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() }
    $manifest = Join-Path $Folder 'commits-linux-x64.json'
    @{ version = '9.9.9'; url = $archive; sha256 = $Sha256 } | ConvertTo-Json | Set-Content -LiteralPath $manifest
    $manifest
}

try {
    $good = New-Release (Join-Path $work 'good') ''
    $out = Join-Path $work 'out-good'
    & $script -Platform linux-x64 -Destination $out -Manifest $good -Cache (Join-Path $work 'cache-good') | Out-Null
    Check 'a matching checksum bundles the manifest and the archive' (
        (Test-Path (Join-Path $out 'commits-linux-x64.json')) -and (Test-Path (Join-Path $out 'commits-9.9.9-linux-x64.zip')))

    $bad = New-Release (Join-Path $work 'bad') ('0' * 64)
    $out = Join-Path $work 'out-bad'
    $threw = $false
    try { & $script -Platform linux-x64 -Destination $out -Manifest $bad -Cache (Join-Path $work 'cache-bad') | Out-Null }
    catch { $threw = $_.Exception.Message -like 'Checksum mismatch*' }
    Check 'a wrong checksum fails' $threw
    Check 'a wrong checksum bundles nothing' (-not (Test-Path $out))
}
finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
if ($failures) { exit 1 }
