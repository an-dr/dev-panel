<#
.SYNOPSIS
Packages dist/app as the release zip for one platform.

.DESCRIPTION
Run after dist.ps1 on the platform being packaged. Writes
release/devpanel-<version>-<platform>.zip holding a devpanel/ folder in the
install layout (docs/installation.md): unzip it and run devpanel, or run
<version>/devpanel-app --install to install it for the current user.

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
    [string]$Tag = ''
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

$manifest = Get-Content -Raw (Join-Path $root 'Cargo.toml')
if ($manifest -notmatch '(?ms)^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"') { throw 'no version in Cargo.toml' }
$version = $Matches[1]
if ($Tag -and $Tag -ne "v$version") {
    throw "Tag $Tag does not match version $version; tag v$version or bump the version first"
}

$app = Join-Path $root 'dist/app'
$ext = if ($Platform -like 'windows-*') { '.exe' } else { '' }
foreach ($required in @("devpanel$ext", "$version/devpanel-app$ext", "$version/devpanel-launcher$ext")) {
    if (-not (Test-Path (Join-Path $app $required))) { throw "dist/app/$required is missing; run scripts/dist.ps1 first" }
}

# Staged under a devpanel/ folder so unzipping never scatters files.
$staging = Join-Path $root 'release/staging'
if (Test-Path $staging) { Remove-Item -Recurse -Force $staging }
$folder = Join-Path $staging 'devpanel'
New-Item -ItemType Directory -Force -Path $folder | Out-Null
Copy-Item -Recurse (Join-Path $app '*') $folder
Copy-Item (Join-Path $root 'LICENSE') $folder

$zip = Join-Path $root "release/devpanel-$version-$Platform.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
# ZipFile rather than Compress-Archive: on Linux it records each file's Unix
# permissions, so the executables are still executable after unzipping.
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory($staging, $zip)
Remove-Item -Recurse -Force $staging
Write-Host "Packaged $zip"
