# 16 — DevPanel: adopt the new bones

- **Repository:** dev-panel
- **Depends on:** 09 (bones release)
- **Needed by:** 17, 18 and 20
- **Can run in parallel with:** 10–15

## Why

The bones release brings capabilities DevPanel lacks: single instance, desktop registration, self-update, and the companion install brief 17 needs. DevPanel's rule (`docs/installation.md`, `VERIFY.md`) is to take installer behaviour from bones, never from a PowerShell copy.

## Architecture

DevPanel's layers ([docs/architecture.md](../../docs/architecture.md)) decide where each adoption goes. The dependency rule points inward, `devpanel-core` names no adapter and no engine, and only `dashboard.rs` speaks the bus.

| Adoption | Layer | File |
| --- | --- | --- |
| Instance claim | composition | `apps/devpanel/src/main.rs` |
| A launch with a folder argument | use case | `devpanel-core` `Hub::add_or_scan(path)`; called by the controller |
| Receiving handoffs | presentation | `dashboard.rs` subscribes to `instance/launched` and hands the controller an action |
| Desktop registration | composition | `apps/devpanel/src/install.rs`, plus `Engine::desktop` in `main.rs` |
| Program and terminal launching | adapter | `devpanel-infra` `SystemLauncher`, on `bones-launch`; the `Launcher` port is unchanged |
| Self-update | presentation | `dashboard.rs` and the page; `Engine::upgrader` in `main.rs` |
| Watching repositories | port and adapter | new `devpanel-core` port `RepoWatch`; adapter in `devpanel-infra` on `bones-watch` |

`devpanel-infra` may depend on the bones leaf crates (`bones-launch`, `bones-watch`), which are plain libraries. It must not depend on `bones-engine`.

## What to do

1. **Bump the pin.** Point `vendor/bones` at `v1.2.0`, read `docs/architecture/upgrading.md` there, and run `cargo test --workspace`.
2. **Single instance (required).**
   - In `main.rs`, handle `--install` first, exactly as now: an install never claims the instance.
   - Then call `bones_instance::claim(host_identity().app_stem, handoff, Duration::from_secs(2))`. On `HandedOff`, exit 0. On `Primary`, pass it to `Engine::single_instance`.
3. **`devpanel [<path>]` (required).** Step 4 below registers "Open in DevPanel" on folders, which starts `devpanel <folder>`, so DevPanel now takes one optional argument.
   - Add `Hub::add_or_scan(path) -> HubResult<Vec<Repo>>`: when the folder is itself a repository, the existing `add_existing`; otherwise the existing `scan`. A folder that is already tracked is not an error; it returns that repository. Test it in `hub/tests.rs` with the in-memory fakes.
   - At startup, the controller applies the startup argument once the page is ready. On a handoff, `dashboard.rs` turns `instance/launched` into the same controller call, resolving a relative path against the handed-off working directory. Either way the page selects the first added repository and opens its drawer. Add an `Event` for that selection to the protocol, and document it in `docs/design/protocol.md` and `web/dev/mock-host.js`.
   - A handoff with no argument only raises the window, which bones does.
4. **Desktop registration (required).**
   - `install.rs` calls `bones_desktop::register` after installing, with `id` `devpanel`, `name` `DevPanel`, `comment` `Dashboard for your git repositories`, `open_label` `Open in DevPanel`, the installed launcher as `executable`, and `apps/devpanel/assets/devpanel.png` as the icon. Add that PNG, rendered at 256 px from `devpanel.svg`. A failed registration prints a warning; the install still succeeds.
   - `main.rs` composes `Engine::desktop` with the same `Integration`, and Settings shows "Register with system" or "Unregister" from `desktop/result`.
   - Remove the shortcut and `.desktop` writing from `scripts/install.ps1`, so it only builds and calls `--install`.
   - Update `docs/installation.md` and tick the matching `VERIFY.md` item.
5. **Each of these only if bones' release includes it:**
   - **Program launching (brief 05):** `SystemLauncher` calls `bones_launch::launch` and `open_terminal`, keeping `{path}` templating via `Tool::expand_args` in DevPanel. Delete `launcher/terminal.rs` and the resolution code in `launcher.rs`, and move their tests to the bones crate if they are not there yet.
   - **Display backend (brief 08 B):** delete `prefer_x11` from `main.rs` and `override_own_env` from `process.rs`, and the restore logic for children. Update `docs/architecture.md` § Linux display.
   - **Self-update (brief 04):** compose `Engine::upgrader` with `ReleaseSource::GitHub { owner: "an-dr", repo: "dev-panel", app: "devpanel" }`. Settings shows a quiet "Update to X" item next to the version, which checks once per start and on demand. Never install without the user's click. The page talks to the dashboard, the dashboard to `upgrader/*`. CI publishes the manifests in brief 20.
   - **Watcher (brief 06):** add the port `RepoWatch { fn watch(&self, path: &Path, on_change: Box<dyn Fn() + Send>) -> WatchHandle; }` to `devpanel-core::ports`, with an adapter in `devpanel-infra` that watches `.git/index`, `.git/HEAD` and `.git/refs` with a 500 ms quiet period. The controller re-reads that repository's status on change. The focus and interval refreshes stay.
6. **Same behaviour otherwise.** No other visible change. Follow the repository's `AGENTS.md` screenshot rule for every page change: Settings, and the drawer selection after `devpanel <folder>`.

## Acceptance

- Launching `devpanel` twice gives one window, brought to the front.
- `devpanel <folder of clones>` with DevPanel running adds every clone, and the window comes to the front with the first one selected.
- After `pwsh scripts/install.ps1` on Windows, DevPanel is in the Start menu and "Open in DevPanel" appears on folders. On Linux the same through the `.desktop` entry.
- No launcher or terminal code is left that duplicates a bones crate in use.
- `cargo test --workspace` passes, including the new `add_or_scan` tests, and CI is green on all three platforms.
- `docs/architecture.md`'s diagram and crate table name every new port and dependency.
