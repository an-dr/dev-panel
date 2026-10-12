# 12 — Commits: repository health contract and line

- **Repository:** commits
- **Depends on:** 10 and 11
- **Needed by:** 14 (urgency colours in theme files), 19 (DevPanel runs the vectors)
- **Contract:** creates contract 4, owned by Commits, from the draft in [00-contracts.md](00-contracts.md#4-repository-health)

## Why

The user wants to see a repository's problems in Commits too, in the same words and colours DevPanel uses, so moving between the two apps never means translating. Commits already reads part of this: unfinished operations and conflicts (`packages/core/src/backend/queries/repoInProgress.ts`), submodule status, and a ready/busy light beside the repository selector. It gives no single verdict that summarises the repository.

The rule set already exists in DevPanel. It moves to Commits' ownership unchanged, because dependencies point from DevPanel to Commits ([README § Architecture](README.md#architecture)). DevPanel switches to Commits' page in brief 19.

## Architecture

The verdict is a pure function of a small set of **facts**, so it splits cleanly by layer:

| Piece | Where | Depends on |
| --- | --- | --- |
| The contract page and vectors | `docs/repository-health.md`, `docs/repository-health.vectors.json` | — |
| `verdictOf(facts)` and the verdict table | new `apps/commits/bones-adapter/src/health/verdict.ts` | nothing: pure |
| Reading the facts | new `apps/commits/bones-adapter/src/health/facts.ts` | the git module, `repoInProgress`, the launch check |
| Sending it to the page | the adapter's existing page protocol, new message `repoHealth` | — |
| The line | `apps/commits/web/src`, beside the ready/busy light | the page protocol |

- **Not in `packages/core`.** The health line is a standalone-app feature. ADR-007 keeps the shared core host-independent and shared with the VS Code extension, and ADR-008's in-place exception is for features both hosts need. `facts.ts` may call `packages/core` queries; it does not change them.
- **One function, tested by vectors.** `verdictOf` is the only place the order lives in Commits. Its test runs every case in the vectors file. DevPanel runs the same file against its Rust `Health::of`, which is how the two implementations stay identical without sharing code. This is the technique of Commits' ADR-002.

## What to build

1. **The contract.** Create `docs/repository-health.md` from the draft: facts, order, labels, urgency and freshness. Create the vectors file with all eleven verdicts and the cases the draft requires. Link both from `docs/index.md`.
2. **Facts.** Read them with as few git calls as possible:
   - `git status --porcelain=v2 --branch --untracked-files=normal` gives `branch.head` (`(detached)`), `branch.upstream`, `branch.ab` (ahead and behind), unmerged entries (`u` lines, counted as `conflicted`) and the changes count.
   - `repoInProgress` gives `operation`. Add `bisect` detection if it lacks it: `.git/BISECT_LOG` exists.
   - `git rev-parse --verify HEAD` gives `hasCommits`, and `git remote` gives `hasRemote`.
   - `exists` and `isRepo` come from the host's existing launch check. An open repository is both, unless its folder disappears; the next refresh then reports `missing`.
3. **The line.** Compact, beside the ready/busy light. It shows the verdict's label in its urgency colour, with a count where it helps: "Behind 3", "Uncommitted 5", "Unpushed 2", "Conflicts 4".
   - `clean`: neutral text colour, no tint and no badge.
   - Clicking it calls brief 11's `applyNavigation`: changes for `dirty`, conflicts for `conflicted` and `in-progress`, the current branch in the graph for `diverged`, `behind`, `ahead` and `unpublished`, and the HEAD commit for `detached`. `missing` and `not-a-repo` do nothing.
   - Tooltip: "Behind counts only what the last fetch saw" whenever `behind` or `diverged` is shown.
4. **Urgency colours as theme tokens.** Add `urgencyAdvisory`, `urgencyCaution` and `urgencyWarning` to `ThemeColours` in `apps/commits/web/src/themes.ts`, with values for Paper and Graphite in the contract's hue families: yellow-orange, red-orange and pink. Do not reuse `warning`, `negative` or `accent`. The line uses only these tokens, so a theme file (brief 14) can set them.
5. **Freshness.** Recompute after every git action, every watcher refresh, and when the window regains focus.

## Acceptance

- `verdictOf` passes every case in the vectors file, and a test fails if the file lists a verdict `verdictOf` cannot return, or the reverse.
- `facts.ts` tests drive it with fixture git output, as the existing read tests do, including a detached HEAD, no upstream with a remote, and a bisect.
- A clean repository shows a neutral, unobtrusive line. Clicking the line for `dirty` opens the changes view.
- Labels are taken from the verdict table, character for character as in the contract, and appear nowhere else in code.
- Screenshots of the line in Paper and Graphite for `clean`, `behind`, `dirty` and `conflicted`.

## Out of scope

- Fix buttons such as Pull, Push or Sync on the line. Commits already has these actions elsewhere.
- A list of many repositories with verdicts. Brief 13's selector shows names, not verdicts.
