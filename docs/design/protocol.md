# Page protocol

The page and the host exchange JSON objects, each with a `type`. The page sends one object per `window.ipc.postMessage`; the host sends a JSON **array** of objects per frame, delivered as the `detail` of a `bones-message` window event. The Rust side of this contract is `apps/devpanel/src/protocol.rs`.

```mermaid
sequenceDiagram
    participant P as Page
    participant H as Host
    P->>H: ready
    H-->>P: [workspace, status × n]
    P->>H: install-recipes (request: 7)
    H-->>P: [reply (request: 7, data)]
    P->>H: pull
    H-->>P: [job running]
    H-->>P: [log, log, …]
    H-->>P: [job succeeded, status]
```

## Page → host

| `type` | Fields | Effect |
| --- | --- | --- |
| `ready` | — | Sends the workspace and refreshes every status |
| `refresh` | `repo?` | Re-reads status of one repository, or all |
| `fetch` | `repo?` | Fetches one repository, or every one with a remote |
| `pull` · `push` · `sync` · `stash` | `repo` | Runs that git operation as a job |
| `commit` | `repo`, `message` | Stages everything and commits |
| `fix` | `repo`, `fix`, `message?` | Runs one suggested fix (`reclone`, `pull`, `sync`, `push`, `publish`, `commit-all`, `stash`, `open-in-tool`) |
| `open` | `repo`, `tool?` | Opens the repository in a tool, the default when absent |
| `add-existing` | `path` | Tracks a local repository |
| `scan` | `path` | Tracks every repository under a folder, as a job |
| `clone` | `url`, `parent?` | Clones into `parent` or the clone folder, as a job |
| `remove` | `repo` | Stops tracking; files stay on disk |
| `edit` | `repo`, `name`, `group?`, `install_command?` | Updates a repository's settings |
| `favorite` | `repo`, `favorite` | Pins a repository as a favourite, or clears the mark |
| `export` | `folder` | Writes a portable repository list (`devpanel-repos.json`) into `folder` |
| `import` | `folder`, `dest?` | Reads the list in `folder` and clones every repository into `dest` or the clone folder, as a job |
| `install-recipes` | `request`, `repo` | Answered with a `reply` holding the recipe list |
| `run` | `repo`, `command`, `title?` | Runs a shell command in the repository, as a job |
| `save-settings` | `settings` | Replaces the settings |
| `save-tools` | `tools` | Replaces the tool list |
| `pick-folder` | `request` | Native folder dialog; `reply` data is the path, or `""` when cancelled |
| `open-url` | `url` | Opens the system browser |
| `copy` | `text` | Writes the clipboard |

The last three are answered by the bones `os` module; the rest by the controller.

## Host → page

| `type` | Fields | Meaning |
| --- | --- | --- |
| `workspace` | `workspace`, `platform`, `version` | Everything persisted: `repos`, `tools`, `settings`; `version` is the app's own. Sent at start and after any change to them |
| `status` | `repo`, `status`, `health`, `fixes` | A fresh status read with its verdict ([domain.md](domain.md)) |
| `job` | `job`, `repo?`, `title`, `state`, `error?` | A job started (`running`) or ended (`succeeded`, `failed`) |
| `log` | `job`, `line` | One line of a job's output |
| `reply` | `request`, `ok`, `data`, `error?` | The answer to a request-carrying message |
| `toast` | `level`, `message` | A notice for the user: `info` or `error` |

A job's output arrives between its `running` and final `job` events. A job on a repository is always followed by that repository's `status`; a job that changes what is tracked is followed by `workspace` and every `status`.
