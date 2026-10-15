<#
.SYNOPSIS
Fails when a commit names an AI assistant as a co-author.

.DESCRIPTION
Reads every commit reachable from -Head but not from -Base and looks for a
Co-authored-by line naming Claude, Anthropic, Codex, OpenAI or Copilot. Each
one found is listed with how to remove it, and the script exits 1; with none
it exits 0. The README discloses AI assistance once, so commits do not
credit an assistant one by one (docs/ci.md).

.PARAMETER Base
What the commits are new relative to: a pull request's base, or the commit a
push replaced. Empty, all zeros (a new ref) or unknown to this clone (a
force-push), it falls back to -Fallback.

.PARAMETER Head
The newest commit to check. Defaults to HEAD.

.PARAMETER Fallback
The base used when -Base names nothing usable. Defaults to origin/main.

.PARAMETER Repository
The repository to check. Defaults to the current folder.
#>
param(
    [string]$Base = '',
    [string]$Head = 'HEAD',
    [string]$Fallback = 'origin/main',
    [string]$Repository = '.'
)
$ErrorActionPreference = 'Stop'

function Test-Commit([string]$Rev) {
    if (-not $Rev -or $Rev -match '^0+$') { return $false }
    git -C $Repository cat-file -e "$Rev^{commit}" 2>$null
    $LASTEXITCODE -eq 0
}

$from = if (Test-Commit $Base) { $Base } elseif (Test-Commit $Fallback) { $Fallback } else { '' }
$range = if ($from) { "$from..$Head" } else { $Head }

# One record per commit: hash, subject and full message, split by control
# characters that never occur in a message.
$log = git -C $Repository log --format='%H%x1f%s%x1f%B%x1e' $range
if ($LASTEXITCODE -ne 0) { Write-Host "::error::git log $range failed"; exit 2 }

$pattern = '(?im)^[ \t]*co-authored-by:.*\b(claude|anthropic|codex|openai|copilot)\b.*$'
$offenders = @()
foreach ($record in (($log -join "`n") -split "`u{1e}")) {
    $fields = $record.Trim("`n") -split "`u{1f}", 3
    if ($fields.Count -lt 3) { continue }
    $lines = [regex]::Matches($fields[2], $pattern) | ForEach-Object { $_.Value.Trim() }
    if ($lines) { $offenders += [pscustomobject]@{ Hash = $fields[0]; Subject = $fields[1]; Lines = @($lines) } }
}

$count = @(git -C $Repository rev-list $range).Count
if (-not $offenders) {
    Write-Host "No AI co-authors in $count commit(s) ($range)."
    exit 0
}

Write-Host "::error::$($offenders.Count) of $count commit(s) name an AI assistant as a co-author."
foreach ($offender in $offenders) {
    Write-Host ''
    Write-Host "  $($offender.Hash.Substring(0, 12)) $($offender.Subject)"
    foreach ($line in $offender.Lines) { Write-Host "      $line" }
}
Write-Host @'

This project discloses AI assistance once, in the README; commits do not
credit an assistant. Remove these Co-authored-by lines and push again:

  the last commit     git commit --amend, delete the line, save
  an earlier commit   git rebase -i <base>, mark it "reword", delete the line
  then                git push --force-with-lease
'@
exit 1
