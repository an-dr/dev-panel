# 18 — DevPanel: open in Commits

- **Repository:** dev-panel
- **Depends on:** 15 (Commits with single instance, deep links and theme flags) and 16
- **Needed by:** 20
- **Contracts:** Commits' command line (contract 1) and theme files (contract 5), in Commits' `docs/command-line.md` and `docs/themes.md`

## Why

Commits is DevPanel's git client, not one tool among five. The user should open any repository in it from where they are, and each problem DevPanel reports should open in the Commits view where it gets fixed, looking like part of the same product.

## What exists

- **Tools:** `crates/devpanel-core/src/domain/tool.rs` defines the default tools. Commits is `Launch::Program { program: "~/.commits/app/commits", args: ["{path}"] }` with id `commits`. Tools appear in the card's "Open in…" menu and in the drawer, and the user can edit or delete them.
- **The port:** `Launcher::launch(&self, tool: &Tool, path: &Path)` in `devpanel-core::ports`. It cannot pass extra arguments today.
- **Fixes:** each verdict has fix buttons such as Pull, Push, Sync, Commit… and Re-clone (`docs/design/domain.md`, `apps/devpanel/web/app.js`).
- **Status:** `RepoStatus` has `branch` (`None` when detached) and `last_commit.hash`.
- **Refresh:** regaining focus already refreshes every status.

## Architecture

Which arguments a verdict gets is a **domain rule**, so it lives in `devpanel-core`, is pure, and is tested there. Starting the program is the adapter's job. The theme the user sees is known only to the page.

| Piece | Layer | Where |
| --- | --- | --- |
| `CommitsLink::for_status(status, theme) -> Vec<String>` | domain | new `crates/devpanel-core/src/domain/commits_link.rs` |
| `ToolId::COMMITS` | domain | `tool.rs`: the constant `"commits"`, replacing the string literal in `Tool::defaults` |
| `Hub::open_in_commits(repo, theme) -> HubResult<()>` | use case | `hub.rs`: finds the tool with id `commits`, builds the link, calls the port |
| `Launcher::launch(&self, tool, path, extra_args: &[String])` | port | `ports.rs`: the existing method gains `extra_args`, appended after the tool's expanded args. Existing callers pass `&[]` |
| `Launcher::is_available(&self, tool) -> bool` | port | `ports.rs`: resolves a `Program` tool's program without starting it |
| `SystemLauncher` | adapter | `devpanel-infra`: both methods, with `bones_launch::resolve` when brief 05 shipped |
| The theme file | asset | `apps/devpanel/assets/commits-theme.json` |
| Action, event and buttons | presentation | `protocol.rs`, `controller.rs`, `web/app.js` |

DevPanel does not track Commits' process. It relies on Commits' single instance (contract 2) to reuse the window.

## What to build

1. **The link rule.** `CommitsLink::for_status(status: &RepoStatus, theme: ResolvedTheme) -> Vec<String>`, with `ResolvedTheme` being `Light` or `Dark`:

   | Verdict (`Health::of`) | Arguments |
   | --- | --- |
   | `conflicted`, `in-progress` | `--show conflicts` |
   | `dirty` | `--show changes` |
   | `diverged`, `behind`, `ahead`, `unpublished` | `--branch <status.branch>` |
   | `detached` | `--commit <status.last_commit.hash>`, or nothing when unknown |
   | `clean` | nothing |
   | `missing`, `not-a-repo` | the use case refuses: there is nothing to open |

   Every link then ends with `--theme light|dark` and `--theme-file <path>` (step 3). The path itself is not in the vector: `Tool::expand_args` already puts `{path}` first.
2. **Detect Commits.** On startup and on focus, the controller asks `Launcher::is_available` for the tool with id `commits`, and adds `commits: bool` to the `workspace` event. A user who deleted that tool has no Commits. Update `docs/design/protocol.md` and `web/dev/mock-host.js`.
3. **The theme file.** Write `apps/devpanel/assets/commits-theme.json` in Commits' theme-file format (contract 5), from the values in `docs/design/colours.md`:
   - **dark:** canvas `#12131c`, surfaces `#1b1c2a`, cyan `#9afff5` accent, urgency `#ffb84a`, `#ff7b4a` and `#ff5c9d`
   - **light:** canvas `#f4f5fa`, surfaces `#ffffff`, navy `#191b39` text and accent, aqua `#c9f0ea` selection, urgency `#b86e00`, `#d2501c` and `#e0287a`

   The rest follows the same page. Embed it with `include_str!`. At startup, `main.rs` writes it to `commits-theme.json` beside `workspace.json` when the content differs, and gives the controller that path. A test parses the asset and checks it has every key Commits' contract lists, and that its urgency values equal the urgency tokens in `apps/devpanel/web/style.css`, so the two cannot drift. Update `docs/design/colours.md`: the theme file is generated from that page, and replaces the line about a Commits preset.
4. **The action.** Add `Action::OpenInCommits { repo, theme: ResolvedTheme }`. The page sends the theme it is actually showing, so `system` is already resolved; only the page knows that. The controller calls `Hub::open_in_commits`, on the frame thread like other quick use cases.
5. **Buttons.**
   - **"Open in Commits"** on every card and in the drawer when `commits` is true: a secondary button next to the primary Open button, with Lucide `git-commit-horizontal` copied into `icons.js` from the `lucide-static` version named there. It is hidden when Commits is not available, and the tool stays in the "Open in…" menu either way.
   - **Each fix bar** gets an "Open in Commits" action for every verdict except `clean`, `missing` and `not-a-repo`. It sends the same action, and the host picks the link from the verdict.
6. **Older Commits.** Commits ignores unknown flags (contract 1), so nothing breaks; it opens on the default view.

## Acceptance

- `CommitsLink` tests cover every row of the table, both themes, and a detached HEAD with no known commit.
- `Hub::open_in_commits` tests, with a fake `Launcher`, check the arguments passed and the refusal for `missing`.
- With Commits installed, every card and the drawer show "Open in Commits". Clicking it opens that repository in the running Commits window, or starts Commits, in DevPanel's mode and colours.
- In a repository with a conflicted rebase, the fix bar's Commits action opens Commits on the conflicts view. A dirty repository opens on the changes view.
- Deleting the Commits tool, or uninstalling Commits, hides the buttons at the next focus.
- Screenshots per this repository's `AGENTS.md` rule: overview and repositories, both themes, both widths.
- Protocol tests cover the new field and action.
