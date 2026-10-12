# 05 — bones: launch programs and terminals

- **Repository:** bones
- **Depends on:** nothing
- **Needed by:** optional adoptions in 10 (Commits' external tools) and 16 (DevPanel's tools)
- **Priority:** recommended

## Why

Both apps start external programs against a folder: DevPanel's "Open in VS Code / Claude Code / Terminal / Commits", and Commits' external diff and merge tools. Neither needs a repository; it is "start this program in this folder" or "open a terminal here and run this". Today each app has its own code, and DevPanel's carries hard-won platform fixes the next app would otherwise rediscover.

## What exists

DevPanel, in `crates/devpanel-infra/src/`:

- **Program resolution (`launcher.rs`):**
  - expands `~`
  - searches `PATH`, honouring `PATHEXT` on Windows
  - runs `.cmd` and `.bat` through `cmd /C`
  - substitutes `{path}` in arguments; this stays in DevPanel
  - starts the program detached
- **Terminals (`launcher/terminal.rs`):**
  - Windows: Windows Terminal (`wt -d <dir>`) when present, otherwise `cmd`
  - Linux: a table of known terminals
  - macOS: Terminal.app
  - optionally runs a command and keeps the terminal open afterwards
- **Child processes (`process.rs`):** on Windows, children get no console window. On Linux, DevPanel restores the user's `GDK_BACKEND` and `SDL_VIDEO_DRIVER` for children; brief 08 B removes the need for that.

Commits, in `crates/os/src/lib.rs`: `run_tool` starts a configured diff or merge tool with materialised arguments, and does not wait for it.

## Architecture

A **leaf library only**, `crates/bones-launch`, depending on std and nothing in bones.

- **No bus actions.** Both consumers are native: DevPanel's `SystemLauncher` adapter in `devpanel-infra`, which implements a `devpanel-core` port and never speaks the bus, and Commits' `repo-os` module. A guest-facing "start any program" action would widen the trust boundary far more than anything in `bones-module-os` (ADR-034) and has no caller. Adding one later is a separate decision.
- **No knowledge of tools, `{path}` or settings.** Templating and tool configuration stay in the apps.

## What to build

```rust
pub struct Program { pub program: String, pub args: Vec<String>, pub cwd: PathBuf, pub env: Vec<(String, String)> }
pub enum Launched { Started, NotFound { searched: String } }
pub fn resolve(program: &str) -> Option<PathBuf>;
pub fn launch(program: &Program) -> Result<Launched, String>;
pub fn open_terminal(dir: &Path, run: Option<&str>) -> Result<(), String>;
```

1. **`resolve`:** `~` expansion, then `PATH` and `PATHEXT` search, as DevPanel does. Public, because apps use it to tell whether a tool is installed (brief 18) without starting it.
2. **`launch`:** resolves the program, runs `.cmd` and `.bat` through `cmd /C`, starts it detached with no console window on Windows, and returns `Started` or `NotFound` with the name it searched.
3. **`open_terminal`:** the per-platform terminal table. Probing for an installed terminal goes through an internal trait, so tests use fake probes. With no terminal found, the error lists every terminal tried.
4. **Environment:** children inherit the process environment plus `Program::env`. Nothing is restored or filtered: after brief 08 B the engine no longer changes the process environment, and an app that still overrides its own variables must pass the user's values in `env` itself.
5. **Move, don't rewrite:** port DevPanel's `launcher.rs`, `launcher/terminal.rs` and the console-hiding part of `process.rs` with their tests, then apply bones' code style.

## Acceptance

- **Resolution tests:** `PATHEXT` handling, `~` expansion, and a not-found result.
- **Terminal tests:** the table, with fake "is this installed" probes, including the not-found error listing.
- On Windows, launching a `.cmd` shim such as `code.cmd` works and opens no console window. Check this by hand and record it in the crate README.
- The crate README documents the API and the platform behaviour. `docs/architecture/structure.md` lists the crate.

## Out of scope

- Bus actions for launching. See Architecture.
- DevPanel's tool configuration and Commits' tool settings. They stay in the apps and call this crate.
- Running programs with captured output. DevPanel's jobs, git and install scripts stay in the apps.
