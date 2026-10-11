# To verify

Everything decided without asking. Each item says what was assumed; tick it, or say what you want instead.

## Decisions to confirm

- [ ] **Name and locations.** Product DevPanel; launcher `devpanel`, app `devpanel-app`; installed to `~/.devpanel/app`; workspace in `~/.config/devpanel/workspace.json` (platform config folder elsewhere).
- [ ] **No WASM guest.** The whole product is one native bones module plus a web page ([ADR-001](docs/adr/ADR-001-one-native-module-no-wasm-guest.md)), since nearly everything it does needs the host. Commits splits host and guest; is the simpler shape acceptable here?
- [ ] **Git through its CLI**, never prompting: `GIT_TERMINAL_PROMPT=0` and SSH in batch mode unless you set `GIT_SSH_COMMAND` ([ADR-002](docs/adr/ADR-002-git-through-its-cli.md)). A passphrase-protected SSH key without an agent therefore fails instead of asking.
- [ ] **Pull only fast-forwards.** Diverged branches get **Sync** (`pull --rebase`, then `push`), offered only on a clean tree. No merge option. Right default for you?
- [ ] **Commit all stages untracked files too** (`git add --all`).
- [ ] **Remove only stops tracking**; it never deletes the folder. Do you want a "delete from disk" option?
- [ ] **Automatic fetch is off by default**; status refreshes at start, when the window regains focus, and after every action. "Behind" is as fresh as the last fetch.
- [ ] **Default clone folder `~/Code`.** Your code lives in `/media/an-dr/LStorage/@Code`; should that be the default?
- [ ] **Default tools:** VS Code (`code {path}`), Claude Code (`claude` in a terminal, which stays open as a shell afterwards), Commits (`~/.commits/app/commits {path}`), Terminal, File manager. The Open button uses VS Code.
- [ ] **Install recipes** run without a terminal through `sh -c` / `cmd /C`, in this order: your saved command, repository scripts (`install`/`setup` in root, `scripts/`, `tools/`; `.ps1` with `pwsh -ExecutionPolicy Bypass`), then ecosystem defaults ([domain.md](docs/design/domain.md#install-recipes)). Interactive installers fail rather than hang.
- [ ] **One group per repository**, free text. Tags (several per repository) instead?
- [ ] **License MIT**, matching bones and Commits.

## Installation

- [ ] **Same layout as Commits, built only on bones.** Launcher plus version folders through `bones-upgrader`; `install.ps1` calls the built app's `--install`, which uses bones-upgrader's own `install_fresh` / `copy_version_from_dir` instead of a PowerShell copy of that logic as Commits' script has. Nothing was copied from Commits, so nothing needed moving into bones. Tested on Linux with a scratch `HOME`: fresh install, second install pushing `0.1.0-<hash>`, launcher starting the newest version.
- [ ] **Self-update from a hosted release is not implemented.** Commits' `UpdaterModule` (manifest check, download, in-app Update) is Commits code. Per your rule it would move into bones first. Do you want that, and where would DevPanel's releases be published (GitHub Releases of `an-dr/dev-panel`)?
- [ ] **"Register with system" is not in the app.** The install script adds a `.desktop` entry (Linux) or a Start menu shortcut (Windows). Commits' `desktop-integration` crate (file-manager "Open in …" verbs, registry entries) would be the thing to move into bones and share. The bones repository runs its own workflow with your approvals, so I did not start that change. Say the word and I will.

## bones

- [ ] **Pinned to `ff90d95`, not a tag.** The local bones clone has `v1.1.1` at that commit, but the tag was never pushed to GitHub. Push the tag, then repin the submodule to it?
- [ ] **Likely bones bug: web panels fail on Wayland sessions that export `GDK_BACKEND=wayland`** (COSMIC does). `bones-module-web` calls `gdk::set_allowed_backends("x11")`, but `gtk::init` still fails ("Failed to initialize GTK"). DevPanel works around it by setting `GDK_BACKEND=x11` and `SDL_VIDEO_DRIVER=x11` for its own process only, and restoring your values for every program it launches. Should bones do this itself? Commits is probably affected the same way.
- [ ] **A file watcher** (Commits has `crates/watcher`) would refresh status the moment files change. Worth moving into bones and using here?

## Not tested

- [ ] **Windows and macOS.** Everything platform-specific is written (Windows Terminal or `cmd`, `PATHEXT` and `.cmd` shims such as `code.cmd`, Explorer, Start menu shortcut, no console window; Terminal.app and `open` on macOS) but has only been compiled and run on Linux.
- [ ] **The native folder picker, opening a URL and the clipboard** go through bones' `os` module. They are wired and covered by the protocol, but were not clicked in the real window: doing so would have opened dialogs and changed your clipboard on your desktop.
- [ ] **Launching VS Code, Claude Code and a real terminal** from the window. Launching was verified end to end with a harmless test tool, and the terminal command line with `cosmic-term -e`.

## Repository

- [ ] **Nothing is committed yet.** Git is initialised on `main`, bones is a submodule, and `origin` is `https://github.com/an-dr/dev-panel.git`. Nothing was pushed.
- [ ] **No `AGENTS.md`.** The repository does not opt into your agent workflow. Add one, as bones and Commits have?
