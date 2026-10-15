# Commits

[Commits](https://github.com/an-dr/commits) is DevPanel's git client. Any repository opens in it from its card or drawer, a second repository reuses the running Commits window, and each problem DevPanel reports opens in the Commits view where it is fixed.

DevPanel depends on Commits, never the reverse ([ADR-003](../adr/ADR-003-commits-is-a-bundled-companion.md)). Every agreement between the two belongs to Commits and is documented there. DevPanel conforms and supplies data in Commits' formats, and Commits reads no DevPanel file. To change an agreement, change Commits' page and release Commits first, then DevPanel.

## What DevPanel gives Commits

| What | Commits contract | DevPanel side |
| --- | --- | --- |
| Launch arguments: the repository path, then the view for its verdict, `--theme` and `--theme-file` | [Command line](https://github.com/an-dr/commits/blob/main/docs/command-line.md) | `CommitsLink::for_status` in `devpanel-core`; `Hub::open_in_commits` passes it to the Commits tool |
| One window: a second launch hands off to the running Commits | [Command line § One window](https://github.com/an-dr/commits/blob/main/docs/command-line.md#one-window) | Nothing to track; DevPanel starts the program each time |
| DevPanel's palette for one session | [Session theme files](https://github.com/an-dr/commits/blob/main/docs/themes.md) | `apps/devpanel/assets/commits-theme.json`, written beside the workspace file at start ([colours.md](colours.md)) |
| The tracked repositories, for Commits' repository selector | [Repository sources](https://github.com/an-dr/commits/blob/main/docs/repository-sources.md) | `CommitsSourceFile` writes `~/.commits/sources/devpanel.json` after every saved change and at start; `COMMITS_SOURCES_DIR` moves it |
| The same verdicts, labels and urgencies | [Repository health](https://github.com/an-dr/commits/blob/main/docs/repository-health.md) | `Health::of`, held to Commits' vectors by `crates/devpanel-core/tests/health_vectors.rs` ([domain.md](domain.md#verdicts)) |
| Commits itself, installed with DevPanel when absent | Commits' release manifest and archive | `companions/` in each version folder ([installation.md](../installation.md#commits-comes-along)) |

## Which view a problem opens

| Verdict | Commits opens |
| --- | --- |
| `conflicted`, `in-progress` | the conflict view: `--show conflicts` |
| `dirty` | the working-tree changes: `--show changes` |
| `diverged`, `behind`, `ahead`, `unpublished` | the graph with the current branch selected: `--branch <name>` |
| `detached` | the graph with HEAD's commit selected: `--commit <sha>`, or the plain graph when it is unknown |
| `clean` | the graph |
| `missing`, `not-a-repo` | nothing: the button is hidden and the use case refuses |

Every launch ends with the theme the page is showing, `--theme light` or `--theme dark`, and `--theme-file` with DevPanel's palette, so Commits opens looking like part of the same product. An older Commits ignores flags it does not know and opens on its default view.

## When the button shows

"Open in Commits" sits beside every card's Open button and at the top of the drawer's Open in section. It shows while the Commits tool, id `commits`, is in the tool list and its program resolves. DevPanel checks at start and again whenever its window regains focus, so installing or removing Commits, or deleting the tool, shows or hides the button without a restart. The Commits tool stays in the "Open in…" menu either way.
