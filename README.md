# DevPanel

A desktop dashboard for the git repositories you work on. Add them once, then see at a glance which ones need a pull, a push or a commit, fix them in one click, open them in VS Code, Claude Code or Commits, and run their install scripts — without a terminal or a file explorer.

Inspired by Microsoft's discontinued Dev Home, built on the [bones](https://github.com/an-dr/bones) engine.

![DevPanel overview](docs/images/overview.png)

## What it does

| Area | You can |
| --- | --- |
| Track | Clone from a URL, add a local folder, or scan a folder and add every repository under it. Group repositories and rename them |
| See | The condition of every repository: branch, upstream, ahead/behind, staged/modified/untracked/conflicted files, stashes, last commit, unfinished merges or rebases, missing folders |
| Fix | One-click suggested fixes — pull, push, sync, publish a new branch, commit all, stash, re-clone a missing folder — plus fetch, pull, push and sync on demand, or for all at once |
| Open | Each repository in any tool you configure. VS Code, Claude Code (in a terminal), Commits, a terminal and the file manager are set up by default |
| Install | Run the install or build script a repository ships (`install.ps1`, `install.sh`, `scripts/…`), an ecosystem default (`cargo install`, `npm install`, …), or your own command, and watch its output live |

## Getting it

**Download** the zip for your platform — Linux x64, Windows x64 or Windows ARM64 — from the [latest release](https://github.com/an-dr/dev-panel/releases/latest), unzip it, and either run `devpanel/devpanel` directly or install it for your user:

```sh
devpanel/<version>/devpanel-app --install
```

That installs to `~/.devpanel/app`; start it from there with `devpanel`. The Linux build needs WebKitGTK 4.1 and GTK 3, which most desktops already have.

**From source**, it needs a Rust toolchain, a C compiler with CMake (bones builds SDL3), and `git` on `PATH`; Linux also needs the WebKitGTK development packages. PowerShell 7 runs the helper scripts.

```sh
git clone --recurse-submodules https://github.com/an-dr/dev-panel.git
cd dev-panel
pwsh scripts/install.ps1
```

That builds a release, installs it to `~/.devpanel/app`, and adds DevPanel to the application menu on Linux or the Start menu on Windows. Run it again after pulling to update: the new build goes into a new version folder and starts next time, while the running one is left alone. [docs/installation.md](docs/installation.md) explains the layout, which is the same as Commits uses.

To run without installing: `cargo run --release -p devpanel`.

Your repositories, tools and settings live in one JSON file — `~/.config/devpanel/workspace.json` on Linux, `%APPDATA%\devpanel\workspace.json` on Windows, `~/Library/Application Support/devpanel/workspace.json` on macOS — outside the install, so updates keep them.

## Using it

- **Add repository** (Ctrl+N) — clone, add a folder, or scan a folder.
- **Overview** shows totals and every repository that needs attention, with its fix buttons.
- Click a card to open its details: changed files, every git action, tools, install, recent output.
- **Activity** keeps the output of everything DevPanel ran.
- **Settings** sets the clone folder, the default tool, automatic fetching, and the tool list.
- `/` searches, Ctrl+R refreshes, Esc closes whatever is open.

Status is read when DevPanel starts, when its window regains focus, and after every action. "Behind" is only as fresh as the last fetch: use **Fetch all**, or turn on automatic fetching.

## Platforms

Linux x64, Windows x64 and Windows ARM64 are built and tested by [CI](docs/ci.md) on every push. Day-to-day use has been on Linux (Pop!_OS 24.04, COSMIC via Xwayland); the Windows desktop paths — terminals, Start menu, `.cmd` shims — pass the tests but have not been clicked through by a person, and macOS is not built. See [VERIFY.md](VERIFY.md).

## Documentation

[docs/index.md](docs/index.md) is the map: architecture, the domain rules behind the health verdict, the page protocol, development, and decisions.

## License

MIT.
