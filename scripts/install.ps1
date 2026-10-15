<#
.SYNOPSIS
Installs DevPanel for the current user, or pushes a new version into an
existing install.

.DESCRIPTION
Builds dist/app (dist.ps1), then lets the built app install itself through
the bones upgrade module: a first install places the launcher at ~/.devpanel/app and
the version folder beside it; a later run adds a new version folder and
leaves the running one alone, so the launcher starts the new build next
time. Only the newest two version folders are kept. The app also registers
itself with the desktop: the application menu or Start menu, and "Open in
DevPanel" on folders.

DEVPANEL_INSTALL_DIR overrides the install location, e.g. to try this
script without touching a real install.

.PARAMETER SkipBuild
Install the existing dist/app instead of rebuilding it.
#>
param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$ext = if ($IsWindows) { '.exe' } else { '' }

if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot 'dist.ps1')
}
$versionDir = Get-ChildItem -Directory (Join-Path $root 'dist/app') -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $versionDir) { throw 'dist/app has no version folder; run without -SkipBuild' }

# A release build is a GUI program on Windows, so wait for it explicitly.
$app = Join-Path $versionDir.FullName "devpanel-app$ext"
$process = Start-Process -FilePath $app -ArgumentList '--install' -NoNewWindow -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "installing failed with exit code $($process.ExitCode)" }

$installDir = if ($env:DEVPANEL_INSTALL_DIR) { $env:DEVPANEL_INSTALL_DIR } else { Join-Path $HOME '.devpanel/app' }
Write-Host "Run DevPanel with: $(Join-Path $installDir "devpanel$ext")"
