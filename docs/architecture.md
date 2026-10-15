# Architecture

DevPanel is a clean-architecture application in three crates and a web page, hosted by the bones engine.

## Layers

```mermaid
graph TD
    subgraph app["apps/devpanel — presentation and composition"]
        Page["web page<br/>HTML · CSS · JS"]
        Dashboard["Dashboard<br/>bones native module"]
        Controller["Controller<br/>actions → use cases → events"]
        Main["main.rs<br/>composition root"]
    end
    subgraph core["crates/devpanel-core — no I/O"]
        Hub["Hub<br/>use cases"]
        Domain["domain<br/>Repo · RepoStatus · Health · Tool · install recipes"]
        Ports["ports<br/>Git · Shell · Launcher · FileSystem · Store · RepoWatch · RepoSource"]
    end
    subgraph infra["crates/devpanel-infra — adapters"]
        GitCli["GitCli"]
        Shell["SystemShell"]
        Launcher["SystemLauncher"]
        Fs["LocalFileSystem"]
        Store["JsonStore"]
        Watch["GitWatch"]
        Source["CommitsSourceFile"]
    end
    Bones["bones engine<br/>window · web panel · os module · bus"]
    Launch["bones-kernel-launch<br/>programs · terminals"]
    WatchLib["bones-kernel-watch<br/>debounced folder watching"]

    Page <-->|JSON over the web panel| Bones
    Bones <-->|bus| Dashboard
    Dashboard --> Controller
    Controller --> Hub
    Hub --> Domain
    Hub --> Ports
    GitCli -.implements.-> Ports
    Shell -.implements.-> Ports
    Launcher -.implements.-> Ports
    Launcher --> Launch
    Fs -.implements.-> Ports
    Store -.implements.-> Ports
    Watch -.implements.-> Ports
    Source -.implements.-> Ports
    Watch --> WatchLib
    Main --> Dashboard
    Main --> infra
```

| Crate | Holds | Depends on |
| --- | --- | --- |
| `devpanel-core` | Entities, the rules that judge them, the `Hub` use cases, and the port traits | `serde` only |
| `devpanel-infra` | One adapter per port: `git` CLI, platform shell, tool launcher, filesystem, JSON file, repository watcher, Commits' repository source | `devpanel-core`, and bones' plain `bones-kernel-launch` and `bones-kernel-watch` libraries, never the engine |
| `devpanel` (app) | The bones module, the controller, the page protocol, the page itself, and `main` | all of the above, `bones-kernel` and `bones-launcher` |

The dependency rule points inward. `devpanel-core` names no adapter and no engine, so every rule in it is tested with in-memory fakes. Only `main.rs` knows every concrete type; only `dashboard.rs` speaks the bones bus.

## Why the app is a native module

The bones way to build a product is a sandboxed WASM extension. DevPanel's work is the opposite of sandboxed — run `git`, launch editors, read arbitrary folders — so the whole product lives in one native module, and the page is the only UI ([ADR-001](adr/ADR-001-one-native-module-no-wasm-guest.md)). Everything the engine already offers is used rather than rebuilt: the window, the web panel, the folder picker, opening URLs, and the clipboard via the `os` module.

## Outside changes

The controller keeps one `RepoWatch` watch per tracked repository, matched to the workspace whenever `Hub::revision` moves. `GitWatch` watches the top of `.git` and `.git/refs`, not the working tree, and reports a change once `.git` has been quiet for 500 ms; the controller then re-reads that repository's status on its next tick. Lock files alone are not a change, and DevPanel runs git with `GIT_OPTIONAL_LOCKS=0` so its own status reads never rewrite the index and wake the watcher. Focus and interval refreshes stay, for what a watch cannot see: an edited file before it is staged, a worktree whose `.git` is a file.

## One window

`main` claims the single instance before the engine opens a window, and an `--install` never claims it. When the claim itself fails, DevPanel starts as a window of its own. A later `devpanel <folder>` hands its arguments to the running process and exits; bones raises the window and publishes the handoff as `instance/launched`, which `dashboard.rs` passes to the controller. The startup folder takes the same path once the page is ready: `Hub::add_or_scan` tracks it, and a `select` event opens the first repository's drawer.

## An action, end to end

```mermaid
sequenceDiagram
    participant Page as Page (JS)
    participant Web as bones web module
    participant D as Dashboard
    participant C as Controller
    participant W as job thread
    participant H as Hub
    participant G as GitCli

    Page->>Web: ipc.postMessage {"type":"pull","repo":"r3"}
    Web-->>D: web/page-message
    D->>C: handle(Action::Pull)
    C-->>C: queue Job running
    C->>W: spawn
    W->>H: pull(r3, output)
    H->>G: pull(path, ff-only)
    G-->>W: output lines
    W-->>C: Log, Job succeeded, Status
    Note over D,C: next frame (core/tick)
    D->>C: drain()
    D->>Web: SendJson [events]
    Web-->>Page: bones-message event
```

The frame thread never waits on git. The controller queues events on a channel from any thread; on every `core/tick` the dashboard drains them and sends one JSON batch to the page.

## Threads

| Work | Runs on | Why |
| --- | --- | --- |
| Page messages, quick use cases (add, edit, open a tool, list recipes) | frame thread | They finish in milliseconds |
| Status reads | a pool of 4 | Refreshing fifty repositories must not start fifty `git` processes at once |
| "Fetch all" | a pool of 4 | Same, for the network |
| Clone, pull, push, sync, commit, stash, install, run | one thread each | Few, possibly long, and must not queue behind each other |

Two jobs never run in the same repository at once: the second is refused, since two `git` processes in one working tree only fight over its lock. The workspace sits behind one mutex that is never held while a command runs.

## State

The workspace — tracked repositories, tools and settings — is one JSON document, loaded at start and rewritten atomically after every change. A change that fails to save is rolled back in memory too. Repository status is never stored: it is read from git every time.

## Linux display

bones embeds GTK webviews as children of the SDL window, which only works on X11. bones points SDL at X11 (Xwayland on Wayland desktops) through a hint and asks GTK for X11, but GTK ignores that request whenever `GDK_BACKEND` is set, which Wayland sessions such as COSMIC do. So `main` sets `GDK_BACKEND=x11` for DevPanel's own process before any thread starts. Programs DevPanel launches get the user's original value back through `env`, and git and shell commands through their environment. A terminal is the exception: bones opens it, so it inherits `x11` and runs under Xwayland.
