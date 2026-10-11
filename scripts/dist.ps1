<#
.SYNOPSIS
Builds DevPanel in release mode and assembles dist/app in the install layout.

.DESCRIPTION
The layout bones-upgrader installs and updates:

    dist/app/
      devpanel(.exe)                the launcher, the name users run
      <version>/
        devpanel-app(.exe)          the app
        devpanel-launcher(.exe)     the launcher again, so an update can refresh the entry point
        devpanel.svg                the icon

dist/app/devpanel runs straight out of a build; install.ps1 installs it.
#>
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
Copy-Item (Join-Path $root 'apps/devpanel/assets/devpanel.svg') $versionDir
Write-Host "Assembled DevPanel $version in $app"
