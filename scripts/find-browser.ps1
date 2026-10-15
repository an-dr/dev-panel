<#
.SYNOPSIS
Returns the path of an installed Chrome, Chromium or Edge, for the scripts
that render the page or the icons headless. Dot-source it.
#>
function Find-Browser {
    $browser = @(
        'C:\Program Files\Google\Chrome\Application\chrome.exe',
        'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe',
        (Get-Command google-chrome, chromium, chromium-browser, microsoft-edge -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty Source)
    ) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
    if (-not $browser) { throw 'Chrome, Chromium or Edge is needed.' }
    $browser
}
