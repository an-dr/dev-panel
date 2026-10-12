# 19 — DevPanel: share repositories and health rules with Commits

- **Repository:** dev-panel
- **Depends on:** 15 (Commits release with contracts 3 and 4)
- **Needed by:** 20
- **Contracts:** Commits' repository sources (contract 3) and repository health (contract 4)
- **Priority:** recommended

## Why

Two things that DevPanel used to promise to Commits now run the other way, so dependencies point only from DevPanel to Commits ([README § Architecture](README.md#architecture)):

- **Repositories.** Commits no longer reads DevPanel's workspace file. DevPanel writes a repository source in Commits' format, and its workspace file becomes private again: free to change without announcing it.
- **Health rules.** The verdict table now lives in Commits' `docs/repository-health.md`. DevPanel keeps its Rust implementation and proves it matches by running Commits' vectors.

## Architecture

| Piece | Layer | Where |
| --- | --- | --- |
| Port `RepoSource { fn publish(&self, repos: &[Repo]) -> HubResult<()>; }` | port | `devpanel-core::ports` |
| Publishing after each change | use case | `Hub`: every use case that saves the workspace calls `publish` after a successful save, and logs a failure without failing the use case |
| `CommitsSourceFile` | adapter | `devpanel-infra`: writes `~/.commits/sources/devpanel.json` atomically, honouring `COMMITS_SOURCES_DIR` |
| Vectors | test data | `crates/devpanel-core/tests/repository-health.vectors.json`, a copy of Commits' file at the pinned tag, with that tag recorded in a sibling `README.md` |

`Hub` names only the port, so `devpanel-core` keeps its no-I/O rule. The `Ports` struct gains the field, and `main.rs` passes the adapter.

## What to build

1. **The source file.** `CommitsSourceFile::publish` maps each `Repo` to contract 3's entry: `name`, `path` expanded through the `FileSystem` port's `expand` so it holds no `~`, `group` and `favorite`. The file `name` is `DevPanel` and `version` is `1`. It writes a temporary file in the same folder and renames it over `devpanel.json`. It creates the folder when missing, and skips silently when the home folder cannot be resolved.
2. **When.** After every successful workspace save, and once at startup, so a file deleted while DevPanel was closed comes back.
3. **Health vectors.** Copy `docs/repository-health.vectors.json` from the Commits release tag. Add a test in `devpanel-core` that maps each case's facts to a `RepoStatus` and asserts `Health::of` gives the case's verdict, and that DevPanel's ids and labels equal the file's `verdicts` list. Any mismatch is a DevPanel bug: the file is the authority.
4. **Docs.**
   - `docs/design/domain.md` § Verdicts: replace "this table is a contract: change it here first, and Commits follows" with a link to Commits' `docs/repository-health.md` as the authority. Keep DevPanel's table as a description of what DevPanel shows, and name the vectors test.
   - `docs/design/workspace-file.md`: delete it and its line in `docs/index.md`. No other program reads the workspace file any more.
   - Add `docs/design/commits.md`, listing what DevPanel gives Commits and under which Commits contract: launch arguments, the theme file, the repository source and the health vectors. Link it from `docs/index.md`.

## Acceptance

- Adding, renaming, regrouping, favoriting or removing a repository updates `~/.commits/sources/devpanel.json` within the same action, and Commits' selector shows the change when it next opens.
- An adapter test writes into a temporary `COMMITS_SOURCES_DIR` and checks the JSON, including a `~` path expanded.
- A hub test with a failing fake `RepoSource` shows that the use case still succeeds.
- The vectors test passes, and changing one label in DevPanel makes it fail.
