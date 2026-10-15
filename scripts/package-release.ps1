<#
.SYNOPSIS
Packages dist/app's version folder as the release zip for one platform.

.DESCRIPTION
Run after dist.ps1 on the platform being packaged. Writes to release/:

  devpanel-<version>-<platform>.zip   the version folder's contents at the
                                      zip's root, plus LICENSE: unzip it and
                                      run devpanel-app, then install it from
                                      Settings or with devpanel-app --install,
                                      which places devpanel-launcher as the
                                      launcher. It is also the update payload
                                      the bones upgrade module unpacks.
  devpanel-<platform>.json            its manifest (version, url, sha256),
                                      which a running DevPanel looks for in
                                      the latest release

The manifest's url points at the zip on the tag's GitHub release, so the two
files are published together.

.PARAMETER Platform
linux-x64, windows-x64 or windows-arm64: the name the zip carries.

.PARAMETER Tag
The release tag. When given it must be v<version>, so a tag can never
publish binaries that report another version.
#>
param(
    [Parameter(Mandatory)]
    [ValidateSet('linux-x64', 'windows-x64', 'windows-arm64')]
    [string]$Platform,
    [string]$Tag = '',
    # owner/name on GitHub, for the manifest's url.
    [string]$Repository = 'an-dr/dev-panel'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

$manifest = Get-Content -Raw (Join-Path $root 'Cargo.toml')
if ($manifest -notmatch '(?ms)^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"') { throw 'no version in Cargo.toml' }
$version = $Matches[1]
if ($Tag -and $Tag -ne "v$version") {
    throw "Tag $Tag does not match version $version; tag v$version or bump the version first"
}

$versionDir = Join-Path $root "dist/app/$version"
$ext = if ($Platform -like 'windows-*') { '.exe' } else { '' }
foreach ($required in @("devpanel-app$ext", "devpanel-launcher$ext", "companions/commits-$Platform.json")) {
    if (-not (Test-Path (Join-Path $versionDir $required))) { throw "dist/app/$version/$required is missing; run scripts/dist.ps1 first" }
}

# Staged so LICENSE joins the version folder's files without touching dist/.
$staging = Join-Path $root 'release/staging'
if (Test-Path $staging) { Remove-Item -Recurse -Force $staging }
New-Item -ItemType Directory -Force -Path $staging | Out-Null
Copy-Item -Recurse (Join-Path $versionDir '*') $staging
Copy-Item (Join-Path $root 'LICENSE') $staging

$zipName = "devpanel-$version-$Platform.zip"
$zip = Join-Path $root "release/$zipName"
if (Test-Path $zip) { Remove-Item -Force $zip }
# ZipFile rather than Compress-Archive: on Linux it records each file's Unix
# permissions, so the executables are still executable after unzipping.
# Entries sit at the zip's root: the upgrade module extracts it straight
# into a new version folder.
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory($staging, $zip)
Remove-Item -Recurse -Force $staging

$releaseTag = if ($Tag) { $Tag } else { "v$version" }
$manifest = [ordered]@{
    version = $version
    url     = "https://github.com/$Repository/releases/download/$releaseTag/$zipName"
    sha256  = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
}
$manifestPath = Join-Path $root "release/devpanel-$Platform.json"
$manifest | ConvertTo-Json | Set-Content -LiteralPath $manifestPath -Encoding utf8NoBOM
Write-Host "Packaged $zip and $manifestPath"
