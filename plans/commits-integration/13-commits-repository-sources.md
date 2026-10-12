# 13 — Commits: repository sources in the selector

- **Repository:** commits
- **Depends on:** 10
- **Needed by:** 19 (DevPanel writes its source file)
- **Contract:** creates contract 3, owned by Commits, from the draft in [00-contracts.md](00-contracts.md#3-repository-sources)
- **Priority:** recommended

## Why

The user curates their repositories in DevPanel: names, groups, favorites. Commits keeps only a bounded most-recent list (`recentRepositories` in `apps/commits/bones-adapter/src/commits-core.ts`). When both are installed, Commits' repository selector should offer DevPanel's list too, so switching repositories inside Commits does not mean browsing for folders.

Commits does not read DevPanel's files. It defines a **repository source**: a file in a folder Commits owns, in a format Commits owns, which any program may write. DevPanel becomes one source (brief 19). Commits never names DevPanel, and DevPanel's internal workspace file stays private to DevPanel. This is dependency inversion: the general component owns the abstraction, and the integrator implements it.

## Architecture

| Piece | Where | Notes |
| --- | --- | --- |
| The contract page | `docs/repository-sources.md` | From the draft |
| Reading the folder | new native module `apps/commits/host/src/sources.rs`, endpoint `sources` | The guest has no filesystem. Parses with `serde_json::Value`, as the host does elsewhere |
| Merging into the selector | `bones-adapter`, beside `recentRepositories` | Asks the `sources` endpoint |
| Showing sections | `apps/commits/web/src`, the repository selector | — |

It lives in the app, not in `crates/`, because it exists only for the standalone app's selector (`apps/README.md`). It needs no repository, but it is not generic either: the format is Commits' own, so it does not belong in bones.

## What to build

1. **The contract.** Create `docs/repository-sources.md` from the draft. Link it from `docs/index.md`.
2. **The module.** On request, it lists `*.json` in the sources folder (`~/.commits/sources/`, or `COMMITS_SOURCES_DIR`), parses each one, drops files of a higher major version, and answers with every valid source in file-name order. It keeps the last good parse per file, and returns that when a file is mid-write or malformed. It reads only the fields in the contract and never writes.
3. **Selector sections, in order:**
   - **Recent:** Commits' existing list.
   - **One section per source,** headed by its `name`: favorites first, then one sub-heading per `group` in alphabetical order, then the ungrouped ones.

   Show `name`, with the path as secondary text. Mark an entry whose path does not exist as unavailable, greyed and not clickable. Never error.
4. **Refresh.** Ask the module when the selector opens and when the window regains focus.
5. **Deduplicate.** A repository in several sections shows once per section, and opening it is the same action wherever it is clicked.

## Acceptance

- With a fixture sources folder holding a grouped, favorited source, the selector shows the sections in order with the right names.
- Deleting a file removes its section with no error. An empty or missing folder shows only Recent.
- A malformed file keeps that file's last good list, or shows none, and never crashes.
- A file with `"version": 2` is skipped.
- Tests parse the fixture and ignore unknown fields.

## Out of scope

- Writing to a source's file, for example adding a repository to DevPanel from Commits. That would need a write contract.
- Showing health verdicts in the selector.
