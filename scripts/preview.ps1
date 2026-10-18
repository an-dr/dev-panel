<#
.SYNOPSIS
Builds a browser preview of the DevPanel page backed by a mock host.

.DESCRIPTION
Inlines the page exactly as the app does, with web/dev/mock-host.js in front
of the app script, and writes .artifacts/preview/index.html. Open that file in
any browser to work on the UI without building or running the app.
#>
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$web = Join-Path $root 'apps/devpanel/web'
$out = Join-Path $root '.artifacts/preview'
New-Item -ItemType Directory -Force -Path $out | Out-Null

$read = { param($name) Get-Content -Raw -Encoding utf8 (Join-Path $web $name) }
$script = (& $read 'icons.js') + "`n" + (& $read 'fonts.js') + "`n" + (& $read 'dev/mock-host.js') + "`n" + (& $read 'app.js')
$style = (& $read 'fonts.css') + "`n" + (& $read 'style.css')
$html = (& $read 'index.html').Replace('/*@STYLE@*/', $style).Replace('/*@SCRIPT@*/', $script)
$target = Join-Path $out 'index.html'
Set-Content -Encoding utf8 -NoNewline -Path $target -Value $html
Write-Host "Preview written to $target"
