# 00 — Contracts between DevPanel and Commits

Read this with every brief. These are the only things the two apps must agree on.

Every contract is **owned by Commits** and lives on a page in the Commits repository. Commits is the general component with many callers, and DevPanel is one of them, so dependencies point from DevPanel to Commits and never back ([README § Architecture](README.md#architecture)). The owner's page is the authority. DevPanel links to it and conforms, and never keeps its own prose copy. To change a contract, change Commits' page first, release Commits, then update DevPanel.

| # | Contract | Authoritative page in Commits | Created by | DevPanel's side |
| --- | --- | --- | --- | --- |
| 1 | Launching Commits: arguments and flags | `docs/command-line.md` | 10, extended by 11 and 14 | Builds argument vectors (18) |
| 2 | A second launch hands off to the running window | `docs/command-line.md` | 10 | Relies on it; tracks no process (18) |
| 3 | Repository sources: lists of repositories other programs offer | `docs/repository-sources.md` | 13 | Writes `devpanel.json` (19) |
| 4 | Repository health: verdicts, order, labels, urgency, vectors | `docs/repository-health.md` and `docs/repository-health.vectors.json` | 12 | Runs the vectors against `Health::of` (19) |
| 5 | Theme files: a palette a caller passes for one session | `docs/themes.md` | 14 | Ships `commits-theme.json` (18) |

When a brief creates one of these pages, it takes the draft below as its starting point. From then on the page is the authority, and the draft here is replaced by a link.

The mechanism behind contract 2, single instance with argument handoff, is generic and lives in bones (brief 01). It is an API, not a contract between the apps, so bones' own docs describe it.

## Drafts

### 1. Launching Commits

```text
commits [<path>] [--show <view>] [--branch <name>] [--commit <sha>] [--theme light|dark] [--theme-file <file>]
```

| Argument | Meaning | Brief |
| --- | --- | --- |
| `<path>` | A repository, or a folder of repositories. This already works. | 10 |
| `--show changes` | Open the working-tree changes: uncommitted files | 11 |
| `--show conflicts` | Open the conflict or unfinished-operation view: what to resolve or continue | 11 |
| `--show graph` | Open the graph (the default) | 11 |
| `--branch <name>` | Scroll the graph to that branch's tip and select it | 11 |
| `--commit <sha>` | Scroll the graph to that commit and select it; wins over `--branch` | 11 |
| `--theme light\|dark` | Use this mode for this session only, without saving it to settings | 14 |
| `--theme-file <file>` | Use the palette in this theme file (contract 5) for this session only | 14 |

Rules:

- **The launcher's own flags stay first-argument only.** `--version`, `--help` and `--rollback` are answered by the launcher when they are the first argument (today in `docs/updating.md` § The command line; brief 10 moves that section here). Everything else is forwarded unread to the app.
- **Flags may come in any order after the path.** A flag that takes a value consumes the next argument. `--name=value` is accepted too.
- **Unknown is not an error.** An unknown flag, or an unknown value for a known flag, is ignored, so a caller can be newer than Commits.
- **Selections are best effort.** If a branch or commit does not exist, Commits opens the repository normally and says in one non-modal line why it could not select it.
- **No `--theme`, no override.** Without `--theme` or `--theme-file`, Commits uses its own settings.

### 2. A second launch hands off to the running window

When Commits is already running for the same user, a new `commits …` invocation does not open a second window. Instead:

1. It passes its whole argument list and working directory to the running instance, and exits with code 0.
2. The running instance treats them exactly like startup arguments: it opens the path, or switches to it when it is already open, and applies every flag in contract 1.
3. The running instance brings its window to the front.

If handing off fails, for example because the running instance is hung, the new process opens its own window as it does today, after a timeout of 2 s.

### 3. Repository sources

Other programs can offer Commits a list of repositories for its repository selector, without Commits knowing them.

- **Location.** One JSON file per source in `~/.commits/sources/` (`%USERPROFILE%\.commits\sources\` on Windows), beside Commits' `settings.json`. `COMMITS_SOURCES_DIR` overrides the folder, for tests and scratch runs.
- **File name.** `<source-id>.json`, where `<source-id>` is lowercase `[a-z0-9-]+`, for example `devpanel.json`. A source owns its file and nothing else in the folder.
- **Writing.** A source writes a temporary file in the same folder and renames it over its file, so a reader never sees half a file. A source that goes away deletes its file.
- **Format, version 1:**

  ```json
  {
    "version": 1,
    "name": "DevPanel",
    "repositories": [
      { "name": "bones", "path": "C:\\Code\\bones", "group": "engine", "favorite": true }
    ]
  }
  ```

  | Field | Type | Notes |
  | --- | --- | --- |
  | `version` | integer | `1`. A reader skips a file with a higher major version |
  | `name` | string | Section heading in the selector |
  | `repositories[].name` | string | Display name |
  | `repositories[].path` | string | Absolute, already expanded: no `~` |
  | `repositories[].group` | string, optional | Sub-heading inside the section |
  | `repositories[].favorite` | bool, optional | Absent means false; favorites are listed first |

- **Reading.** Commits reads only; unknown fields are ignored; a missing folder, a missing file or a malformed file shows no section and no error, and Commits keeps the last good read of a file it already had.

### 4. Repository health

Each repository gets exactly one verdict, the most urgent that applies. This is DevPanel's existing rule set, moved unchanged to Commits' ownership.

**Facts.** A verdict is a pure function of these facts. Every implementation reads them from git its own way, then applies the same function.

| Fact | Type | Meaning |
| --- | --- | --- |
| `exists` | bool | The folder exists |
| `isRepo` | bool | The folder is the top level of a git working tree |
| `conflicted` | integer | Files with unresolved conflicts |
| `operation` | `merge`, `rebase`, `cherry-pick`, `revert`, `bisect` or null | An unfinished operation |
| `ahead`, `behind` | integer | Commits ahead of and behind the upstream; 0 with no upstream |
| `changes` | integer | Staged, unstaged and untracked files together |
| `detached` | bool | HEAD is detached |
| `hasCommits` | bool | The repository has at least one commit |
| `hasUpstream` | bool | The current branch has an upstream |
| `hasRemote` | bool | At least one remote is configured |

**Order.** The first rule that matches wins:

| # | Rule | Id | Label | Urgency |
| --- | --- | --- | --- | --- |
| 1 | `!exists` | `missing` | Missing | warning |
| 2 | `!isRepo` | `not-a-repo` | Not a repository | warning |
| 3 | `conflicted > 0` | `conflicted` | Conflicts | warning |
| 4 | `operation != null` | `in-progress` | In progress | caution |
| 5 | `ahead > 0 && behind > 0` | `diverged` | Diverged | caution |
| 6 | `behind > 0` | `behind` | Behind | advisory |
| 7 | `changes > 0` | `dirty` | Uncommitted | caution |
| 8 | `ahead > 0` | `ahead` | Unpushed | advisory |
| 9 | `detached && hasCommits` | `detached` | Detached HEAD | caution |
| 10 | `!hasUpstream && hasRemote && hasCommits` | `unpublished` | Unpublished | advisory |
| 11 | otherwise | `clean` | Clean | normal |

**Urgency** has four levels: `normal`, `advisory`, `caution` and `warning`. Each implementation colours them from its own theme, keeping the hue families: no colour for `normal`, yellow-orange for `advisory`, red-orange for `caution` and pink for `warning`. Neither cyan nor green ever means urgency. "Behind" is only as fresh as the last fetch.

**Vectors.** `docs/repository-health.vectors.json` holds the conformance cases. Each implementation runs every case in its tests:

```json
{
  "version": 1,
  "verdicts": [ { "id": "missing", "label": "Missing", "urgency": "warning" } ],
  "cases": [
    {
      "name": "behind and dirty is behind",
      "facts": { "exists": true, "isRepo": true, "conflicted": 0, "operation": null, "ahead": 0, "behind": 2,
                 "changes": 3, "detached": false, "hasCommits": true, "hasUpstream": true, "hasRemote": true },
      "verdict": "behind"
    }
  ]
}
```

`verdicts` lists all eleven verdicts in order. `cases` has at least one case per verdict, plus every tie the order decides: conflicted with an operation, diverged and dirty, behind and dirty, dirty and ahead, detached with no commits, and no upstream with no remote.

### 5. Theme files

A caller can make one Commits session look like part of its own product without Commits knowing the product.

```json
{
  "version": 1,
  "name": "DevPanel",
  "light": { "colours": { "canvas": "#f4f5fa", "…": "…" }, "syntax": { "keyword": "…" } },
  "dark":  { "colours": { "canvas": "#12131c", "…": "…" }, "syntax": { "keyword": "…" } }
}
```

- `colours` has exactly the keys of Commits' `ThemeColours` (`apps/commits/web/src/themes.ts`), including the urgency keys brief 12 adds: `urgencyAdvisory`, `urgencyCaution` and `urgencyWarning`. `syntax` has the keys of `SyntaxColours`, and is optional: a missing `syntax` uses the built-in preset of the same mode.
- Values are CSS colours: `#rgb`, `#rrggbb`, `#rrggbbaa` or `rgba(…)`. Any other value rejects the whole file.
- A file that is missing, malformed or incomplete is ignored with a one-line notice, and Commits keeps its own theme. It never errors.
- `--theme-file` picks the palette and `--theme` picks the mode. Without `--theme`, the mode comes from Commits' `app.mode` setting.
