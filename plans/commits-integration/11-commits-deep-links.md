# 11 — Commits: deep links

- **Repository:** commits
- **Depends on:** 10
- **Needed by:** 12 (the health line reuses the navigation), 18 (DevPanel opens each problem in the right view)
- **Contract:** contract 1, owned by Commits, in `docs/command-line.md`; draft in [00-contracts.md](00-contracts.md#1-launching-commits)

## Why

A caller that knows what is wrong with a repository, such as uncommitted changes, conflicts or an unfinished rebase, should open Commits on the view where it is fixed, not on the default graph. Today Commits reads only its first argument and ignores the rest (`launch.rs`, `from_args`).

## Architecture

| Concern | Where |
| --- | --- |
| Parsing the flags | `launch_request.rs` from brief 10: new fields `show: Option<View>`, `branch: Option<String>`, `commit: Option<String>` |
| Carrying them | the `launch` endpoint and `launch/request` encoding, extended in `ipc/` with fixtures |
| Applying them | one function in `bones-adapter`, `applyNavigation(target)`, which brief 12's health line calls too |

The host only parses and validates. The WASM guest has no filesystem and receives only validated values. Navigation is one function with two callers, the launch request and the health line, so it is written once.

## What to build

1. **Parse** `--show changes|conflicts|graph`, `--branch <name>` and `--commit <sha>` into the request. A `--commit` value must be 4–64 hex characters, or it is ignored. `--theme` is brief 14.
2. **`--show changes`.** Select the working-tree changes row, the uncommitted-changes entry, in the graph.
3. **`--show conflicts`.**
   - With an unfinished operation (merge, rebase, cherry-pick or revert, as `repoInProgress` detects): show the existing in-progress banner and select the uncommitted-changes row, where the conflicted files are listed with continue and abort.
   - With no such operation: the same as `--show changes`.
   - If that view does not show conflicted files distinctly, record the gap in `docs/techdebt.md`. Build no new view.
4. **`--branch` and `--commit`.** Scroll the graph to the target and select it.
   - A commit outside the loaded range: load further history up to the configured maximum. If it is still not found, show a one-line non-modal notice that it is outside the loaded history.
   - A missing branch or commit: open normally and show the notice, never an error dialog.
5. **Ordering.** Apply a deep link only after the repository's first load finishes, never before, so it cannot race the load (see BUG-003 on long loads). This holds at startup and on a handoff.
6. **Update the contract.** Add `--show`, `--branch` and `--commit` to `docs/command-line.md`, with the rules for unknown values and missing targets.

## Acceptance

- `commits <repo> --show changes` opens with the working-tree changes selected.
- During a rebase with conflicts, `commits <repo> --show conflicts` shows the in-progress banner with the changes row selected.
- `commits <repo> --commit <sha>` selects that commit. With a bogus sha, the repository opens and a notice explains.
- `commits <repo> --frobnicate` opens normally.
- All of the above also work as a handoff to a running window.
- Tests cover parsing, including unknown flags and missing values, the `ipc/` fixtures, and `applyNavigation` for each target.

## Out of scope

- The theme flags (brief 14).
- New views.
