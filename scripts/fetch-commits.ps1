<#
.SYNOPSIS
Fetches the pinned Commits release for one platform and verifies it.

.DESCRIPTION
Downloads Commits' own update manifest, commits-<platform>.json, from the
GitHub release named by COMMITS_VERSION in .cargo/config.toml, then the
archive it names, and checks the archive against the manifest's sha256.
Both files are copied into -Destination unchanged: that is the
companions/ folder DevPanel's --install hands to bones.

Downloads are cached under .artifacts/commits/<version>/, so repeated builds
do not fetch again; a cached archive is checked like a fresh one.

A wrong checksum is an error and leaves nothing in -Destination.

.PARAMETER Platform
linux-x64, windows-x64 or windows-arm64. Commits names its platforms the
same way DevPanel does.

.PARAMETER Destination
The folder to place the manifest and archive in, created when missing.

.PARAMETER Manifest
A manifest path or URL to use instead of the pinned release's. For tests:
its url may then be a local path too.

.PARAMETER Cache
The cache folder; .artifacts/commits by default.
#>
param(
    [Parameter(Mandatory)]
    [ValidateSet('linux-x64', 'windows-x64', 'windows-arm64')]
    [string]$Platform,
    [Parameter(Mandatory)]
    [string]$Destination,
    [string]$Manifest = '',
    [string]$Cache = ''
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

function Get-File([string]$Source, [string]$Target) {
    if ($Source -match '^https?://') {
        Invoke-WebRequest -Uri $Source -OutFile $Target -UseBasicParsing
    }
    else {
        Copy-Item -LiteralPath $Source -Destination $Target
    }
}

$config = Get-Content -Raw (Join-Path $root '.cargo/config.toml')
if ($config -notmatch '(?m)^COMMITS_VERSION\s*=\s*"([^"]+)"') { throw 'no COMMITS_VERSION in .cargo/config.toml' }
$pinned = $Matches[1]
if (-not $Manifest) { $Manifest = "https://github.com/an-dr/commits/releases/download/v$pinned/commits-$Platform.json" }
if (-not $Cache) { $Cache = Join-Path $root '.artifacts/commits' }
$cacheDir = Join-Path $Cache $pinned
New-Item -ItemType Directory -Force -Path $cacheDir | Out-Null

$manifestFile = Join-Path $cacheDir "commits-$Platform.json"
if (-not (Test-Path -LiteralPath $manifestFile) -or $PSBoundParameters.ContainsKey('Manifest')) {
    Get-File $Manifest $manifestFile
}
$entry = Get-Content -Raw -LiteralPath $manifestFile | ConvertFrom-Json
foreach ($field in 'version', 'url', 'sha256') {
    if (-not $entry.$field) { throw "$Manifest has no $field" }
}
if (-not $PSBoundParameters.ContainsKey('Manifest') -and $entry.version -ne $pinned) {
    throw "$Manifest names Commits $($entry.version), not the pinned $pinned"
}

$archiveName = Split-Path -Leaf ($entry.url -replace '[?#].*$', '')
$archive = Join-Path $cacheDir $archiveName
$expected = $entry.sha256.ToLowerInvariant()
$actual = if (Test-Path -LiteralPath $archive) { (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() } else { '' }
if ($actual -ne $expected) {
    Get-File $entry.url $archive
    $actual = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
}
if ($actual -ne $expected) {
    Remove-Item -LiteralPath $archive -Force
    throw "Checksum mismatch for $archiveName`: the manifest says $expected, the download is $actual"
}

New-Item -ItemType Directory -Force -Path $Destination | Out-Null
Copy-Item -LiteralPath $manifestFile, $archive -Destination $Destination
Write-Host "Bundled Commits $($entry.version) for $Platform in $Destination"
