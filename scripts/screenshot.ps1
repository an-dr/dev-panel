<#
.SYNOPSIS
Screenshots the browser preview of the DevPanel page.

.DESCRIPTION
Builds the preview with preview.ps1, opens it on the given view and theme in
headless Chrome or Edge, and writes one PNG per width to
.artifacts/screenshots/<view>-<theme>-<width>.png. The mock host supplies the
data, so nothing needs to be built or running.

.EXAMPLE
pwsh scripts/screenshot.ps1 -View settings -Theme dark -Width 1280,900

.EXAMPLE
pwsh scripts/screenshot.ps1 -View repos -Select r2
Plays `devpanel <path>` selecting repository r2, and names the files
<view>-<theme>-<width>-r2.png.
#>
param(
    [ValidateSet('overview', 'repos', 'activity', 'settings')]
    [string] $View = 'overview',
    [ValidateSet('system', 'light', 'dark')]
    [string] $Theme = 'light',
    # Comma-separated, so it also works through pwsh -File.
    [string] $Width = '1280',
    [int] $Height = 820,
    # A repository id the mock host selects after loading, as `devpanel <path>` does.
    [string] $Select = ''
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'find-browser.ps1')
$root = Split-Path -Parent $PSScriptRoot
& (Join-Path $PSScriptRoot 'preview.ps1') 6>&1 | Out-Null

$browser = Find-Browser

$out = Join-Path $root '.artifacts/screenshots'
New-Item -ItemType Directory -Force -Path $out | Out-Null

# The page opens on the view it remembered last and the theme the mock host sends.
$html = Get-Content -Raw -Encoding utf8 (Join-Path $root '.artifacts/preview/index.html')
$html = $html.Replace('<head>', "<head><script>localStorage.setItem('devpanel.view','$View');localStorage.setItem('devpanel.page','$View');window.devpanelSelect='$Select'</script>")
$html = $html.Replace("theme: 'system' }", "theme: '$Theme' }")
$page = Join-Path $out 'page.html'
Set-Content -Encoding utf8 -NoNewline -Path $page -Value $html
$url = 'file:///' + ((Resolve-Path $page).Path -replace '\\', '/')

$userData = Join-Path ([IO.Path]::GetTempPath()) 'devpanel-screenshot-profile'
foreach ($w in $Width -split ',') {
    $w = [int]$w.Trim()
    $suffix = if ($Select) { "-$Select" } else { '' }
    $png = Join-Path $out "$View-$Theme-$w$suffix.png"
    Remove-Item -LiteralPath $png -ErrorAction SilentlyContinue
    & $browser --headless=new --disable-gpu --hide-scrollbars --user-data-dir="$userData" `
        --window-size="$w,$Height" --virtual-time-budget=6000 --screenshot="$png" $url 2>&1 | Out-Null
    if (-not (Test-Path $png)) { throw "No screenshot was written for width $w." }
    Write-Host $png
}
