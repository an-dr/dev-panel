<#
.SYNOPSIS
Installs DevPanel for the current user, or pushes a new version into an
existing install.

.DESCRIPTION
Builds dist/app (dist.ps1), then lets the built app install itself through
bones-upgrader: a first install places the launcher at ~/.devpanel/app and
the version folder beside it; a later run adds a new version folder and
leaves the running one alone, so the launcher starts the new build next
time. Only the newest two version folders are kept. Finally it adds DevPanel
to the application menu (Linux) or the Start menu (Windows).

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
$launcher = Join-Path $installDir "devpanel$ext"
# The icon sits at the install root so menu entries survive version changes.
$icon = Join-Path $installDir 'devpanel.svg'
Copy-Item -Force (Join-Path $versionDir.FullName 'devpanel.svg') $icon

if ($IsLinux) {
    $applications = Join-Path $HOME '.local/share/applications'
    New-Item -ItemType Directory -Force -Path $applications | Out-Null
    @"
[Desktop Entry]
Type=Application
Name=DevPanel
Comment=Dashboard for your git repositories
Exec=$launcher
Icon=$icon
Terminal=false
Categories=Development;
"@ | Set-Content -Encoding utf8 (Join-Path $applications 'devpanel.desktop')
    Write-Host 'Added DevPanel to the application menu'
}
elseif ($IsWindows) {
    $shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'DevPanel.lnk'
    $link = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcut)
    $link.TargetPath = $launcher
    $link.WorkingDirectory = $installDir
    $link.Save()
    Write-Host 'Added DevPanel to the Start menu'
}
Write-Host "Run DevPanel with: $launcher"
