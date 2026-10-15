<#
.SYNOPSIS
Renders the desktop icon set from apps/devpanel/assets/devpanel.svg.

.DESCRIPTION
Writes, next to the SVG:

    icons/devpanel-<size>.png   16, 24, 32, 48, 64, 128, 256 and 512 px, for
                                desktop registration and Linux menus
    devpanel.ico                16 to 256 px, embedded in both Windows
                                executables by apps/devpanel/build.rs

A headless Chrome or Edge draws the SVG into a canvas at each size, so every
size is rasterised from the vector rather than scaled from a bitmap. The
outputs are committed; run this again after changing the SVG.
#>
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'find-browser.ps1')
$root = Split-Path -Parent $PSScriptRoot
$assets = Join-Path $root 'apps/devpanel/assets'
$pngSizes = 16, 24, 32, 48, 64, 128, 256, 512
$icoSizes = 16, 24, 32, 48, 64, 128, 256

$svg = [Convert]::ToBase64String([IO.File]::ReadAllBytes((Join-Path $assets 'devpanel.svg')))
$html = @"
<html><body><pre id="out"></pre><script>
const img = new Image();
img.onload = () => {
  document.getElementById('out').textContent = [$($pngSizes -join ',')].map((n) => {
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = n;
    canvas.getContext('2d').drawImage(img, 0, 0, n, n);
    return n + ':' + canvas.toDataURL('image/png').split(',')[1];
  }).join('\n');
};
img.src = 'data:image/svg+xml;base64,$svg';
</script></body></html>
"@
$work = Join-Path $root '.artifacts/icons'
New-Item -ItemType Directory -Force -Path $work | Out-Null
$page = Join-Path $work 'render.html'
Set-Content -Encoding utf8 -NoNewline -Path $page -Value $html
$url = 'file:///' + ((Resolve-Path $page).Path -replace '\\', '/')
$dom = & (Find-Browser) --headless=new --disable-gpu --virtual-time-budget=5000 --dump-dom $url 2>$null | Out-String

$png = @{}
foreach ($match in [regex]::Matches($dom, '(\d+):([A-Za-z0-9+/=]+)')) {
    $png[[int]$match.Groups[1].Value] = [Convert]::FromBase64String($match.Groups[2].Value)
}
$missing = $pngSizes | Where-Object { -not $png.ContainsKey($_) }
if ($missing) { throw "The browser rendered no image for: $($missing -join ', ')" }

$icons = Join-Path $assets 'icons'
New-Item -ItemType Directory -Force -Path $icons | Out-Null
foreach ($size in $pngSizes) {
    [IO.File]::WriteAllBytes((Join-Path $icons "devpanel-$size.png"), $png[$size])
}

# An .ico is a directory of images; each entry here is a whole PNG, which
# Windows has read since Vista. A width or height of 0 means 256.
$stream = [IO.MemoryStream]::new()
$writer = [IO.BinaryWriter]::new($stream)
$writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$icoSizes.Count)
$offset = 6 + 16 * $icoSizes.Count
foreach ($size in $icoSizes) {
    $edge = if ($size -ge 256) { 0 } else { $size }
    $writer.Write([byte]$edge); $writer.Write([byte]$edge); $writer.Write([byte]0); $writer.Write([byte]0)
    $writer.Write([uint16]1); $writer.Write([uint16]32)
    $writer.Write([uint32]$png[$size].Length); $writer.Write([uint32]$offset)
    $offset += $png[$size].Length
}
foreach ($size in $icoSizes) { $writer.Write($png[$size]) }
$writer.Flush()
[IO.File]::WriteAllBytes((Join-Path $assets 'devpanel.ico'), $stream.ToArray())
Write-Host "Rendered $($pngSizes.Count) PNGs and devpanel.ico into $assets"
