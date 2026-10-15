<#
.SYNOPSIS
Checks check-coauthors.ps1 against a throwaway repository: a clean range
passes, an AI co-author fails, and only the new commits are read.

.DESCRIPTION
Builds a repository in a temporary folder and runs the script against
ranges of it, so it needs no network. Exits non-zero when a check fails.
CI runs it in the co-author job.
#>
$ErrorActionPreference = 'Stop'
$script = Join-Path $PSScriptRoot 'check-coauthors.ps1'
$work = Join-Path ([IO.Path]::GetTempPath()) "check-coauthors-test-$PID"
$failures = 0

function Check([string]$Name, [bool]$Passed) {
    if ($Passed) { Write-Host "ok   $Name" } else { Write-Host "FAIL $Name"; $script:failures++ }
}

function Commit([string]$Message) {
    git -C $work commit --quiet --allow-empty -m $Message
    if ($LASTEXITCODE -ne 0) { throw "git commit failed" }
    (git -C $work rev-parse HEAD).Trim()
}

# The exit code and output of one run, in a child process so `exit` stays there.
function Run([string]$Base, [string]$Head = 'HEAD') {
    $output = pwsh -NoProfile -File $script -Repository $work -Base $Base -Head $Head -Fallback 'none' 2>&1
    [pscustomobject]@{ Code = $LASTEXITCODE; Output = ($output -join "`n") }
}

try {
    New-Item -ItemType Directory -Force -Path $work | Out-Null
    git -C $work init --quiet
    git -C $work config user.name 'Test'
    git -C $work config user.email 'test@example.com'
    git -C $work config commit.gpgsign false

    $old = Commit "feat: old work`n`nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
    $clean = Commit "fix: a plain change`n`nCo-authored-by: Jane Doe <jane@example.com>"
    $result = Run $old
    Check 'a human co-author passes' ($result.Code -eq 0)
    Check 'commits before the base are not read' ($result.Code -eq 0)

    foreach ($line in @(
            'Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>',
            'co-authored-by: Codex <codex@openai.com>',
            'Co-authored-by: Copilot <175728472+Copilot@users.noreply.github.com>')) {
        $bad = Commit "feat: assisted`n`n$line"
        $result = Run $clean $bad
        Check "fails on '$line'" ($result.Code -eq 1 -and $result.Output -like "*$($bad.Substring(0, 12))*")
        git -C $work reset --quiet --hard $clean
    }

    $result = Run $clean
    Check 'an empty range passes' ($result.Code -eq 0)

    $bad = Commit "feat: one`n`nCo-authored-by: GitHub Copilot <copilot@github.com>"
    $result = Run ('0' * 40) $bad
    Check 'a new ref with no usable base checks everything it reaches' ($result.Code -eq 1 -and $result.Output -like '*2 of 3*')
    Check 'the failure says how to remove the line' ($result.Output -like '*git commit --amend*')

    $result = Run 'deadbeefdeadbeefdeadbeefdeadbeefdeadbeef' $bad
    Check 'an unknown base falls back instead of erroring' ($result.Code -eq 1)

    $mention = Commit "docs: say Claude helped`n`nWritten with Claude Code, reviewed by hand."
    $result = Run $bad $mention
    Check 'a mention outside a Co-authored-by line passes' ($result.Code -eq 0)
}
finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
if ($failures) { exit 1 }
