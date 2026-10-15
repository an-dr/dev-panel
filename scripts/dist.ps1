<#
.SYNOPSIS
Builds DevPanel in release mode and assembles dist/app in the install layout.

.DESCRIPTION
The layout the bones upgrade module installs and updates:

    dist/app/
      devpanel(.exe)                the launcher, the name users run
      <version>/
        devpanel-app(.exe)          the app
        devpanel-launcher(.exe)     the launcher again, so an update can refresh the entry point
        companions/                 the pinned Commits release, which --install installs
          commits-<platform>.json   Commits' manifest, unchanged
          commits-<v>-<platform>.zip

dist/app/devpanel runs straight out of a build; install.ps1 installs it.

.PARAMETER Platform
The platform whose Commits release to bundle; this machine's by default.

.PARAMETER RequireCommits
Fail when Commits cannot be fetched, as CI does. Without it a local build
offline still assembles, with a warning and no companions/ folder.
#>
param(
    [ValidateSet('', 'linux-x64', 'windows-x64', 'windows-arm64')]
    [string]$Platform = '',
    [switch]$RequireCommits
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$ext = if ($IsWindows) { '.exe' } else { '' }

cargo build --release -p devpanel --manifest-path (Join-Path $root 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

# The app's version names its folder; it is the workspace version.
$manifest = Get-Content -Raw (Join-Path $root 'Cargo.toml')
if ($manifest -notmatch '(?ms)^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"') { throw 'no version in Cargo.toml' }
$version = $Matches[1]

$app = Join-Path $root 'dist/app'
if (Test-Path $app) { Remove-Item -Recurse -Force $app }
$versionDir = Join-Path $app $version
New-Item -ItemType Directory -Force -Path $versionDir | Out-Null

$release = Join-Path $root 'target/release'
Copy-Item (Join-Path $release "devpanel-launcher$ext") (Join-Path $app "devpanel$ext")
Copy-Item (Join-Path $release "devpanel-app$ext") $versionDir
Copy-Item (Join-Path $release "devpanel-launcher$ext") $versionDir

if (-not $Platform) {
    # What cargo just built for, which is the toolchain's host: an x64
    # toolchain on an ARM64 machine builds x64.
    $hostTriple = (rustc -vV | Select-String '^host: (.+)$').Matches[0].Groups[1].Value
    $arch = if ($hostTriple -like 'aarch64-*') { 'arm64' } else { 'x64' }
    $Platform = if ($IsWindows) { "windows-$arch" } else { "linux-$arch" }
}
try {
    & (Join-Path $PSScriptRoot 'fetch-commits.ps1') -Platform $Platform -Destination (Join-Path $versionDir 'companions')
}
catch {
    if ($RequireCommits) { throw }
    Write-Warning "Commits is not bundled: $_"
}
Write-Host "Assembled DevPanel $version in $app"
