# 02 — bones: desktop registration

- **Repository:** bones
- **Depends on:** nothing
- **Needed by:** 10 (Commits deletes its copy), 16 (DevPanel adopts it)

## Why

An installed app should appear in the system's application list and offer "Open in <App>" on folders in the file manager. Commits already does this with its own crate, `crates/desktop-integration`. That crate knows nothing about git, and Commits' `crates/README.md` and tech-debt entry BUG-006 say it belongs in bones. DevPanel does only part of it: `scripts/install.ps1` writes a Start menu shortcut or a `.desktop` entry, with no file-manager action and no way to unregister.

## What exists

Commits' `crates/desktop-integration` (see its README):

- **Input:** an `Integration` with `id`, `name`, `comment`, `open_label`, `executable` and `icon_png`.
- **Operations:** `register`, `unregister`, `is_registered`. Both writes are idempotent, and a partial registration counts as unregistered.
- **Locations:** resolved through `Locations::for_current_user()`. Tests point it at a scratch directory and a scratch registry key.
- **Linux:** a freedesktop `.desktop` entry with `MimeType=inode/directory;` and a hicolor icon. Refreshes `update-desktop-database` and `gtk-update-icon-cache`, best effort.
- **Windows:** per-user `HKCU` entries plus a folder verb (`windows_verbs.rs`). No administrator rights.
- **Tests:** in `crates/desktop-integration/src/tests.rs`.
- **App side:** `apps/commits/host/src/desktop.rs` supplies Commits' names and icon. `updater.rs` exposes REGISTER and UNREGISTER on the updater bus endpoint, which the page's "Register with system" uses.

## Architecture

| Piece | Crate | Depends on | Responsibility |
| --- | --- | --- | --- |
| Registration | new leaf crate `crates/bones-desktop` | std, `dirs`, and `winreg` or `windows-sys` on Windows | The moved crate: register, unregister and status for one `Integration` |
| Messages | `bones-messages::desktop` | — | `Request::{Register, Unregister, Status}` on `desktop/request`, and `Result::{Registered(bool), Failed(String)}` on `desktop/result`, correlated like `os/*` |
| Bus adapter | new module `crates/bones-engine/bones-module-desktop` | `bones-kernel`, `bones-messages`, `bones-desktop` | Answers the requests for one `Integration` fixed at construction |
| Composition | `bones-engine`, feature `desktop` | — | `Engine::desktop(integration)` registers the module |

- **Separate from self-update.** `bones-upgrader` does not depend on `bones-desktop`. Registration is a different responsibility from replacing versions. An app's `--install` calls both, one after the other.
- **A guest cannot choose what is registered.** The `Integration`, including the executable path, is fixed by the composition root. A page or guest may only ask to register, unregister or read the status of that one integration. This is the trust-boundary rule of ADR-034, and the reason this is its own module rather than new `os/*` actions: it is the per-capability split ADR-034 anticipates.

## What to build

1. **Move the crate.** Move `desktop-integration` into bones as `bones-desktop`, keeping its API, behaviour and tests. Rename the Rust identifiers only where bones' naming requires it. Apply bones' code style to the moved code: one type per file, tests in their own file, a crate README.
2. **The module and messages,** as in the table. The module runs registration off the frame thread, as `bones-module-os` does with its requests.
3. **Keep behaviour identical.** Same files, same registry keys, same labels for the same inputs. An install registered by today's Commits must read as registered afterwards.
4. **Show the install pairing.** Extend `examples/embedding/self-updating-app` so its `--install` calls `bones_upgrader` and then `bones_desktop::register`, as a real app would.
5. **Record the placement.** Add a bones ADR: desktop registration is a leaf crate plus an opt-in module, separate from `bones-upgrader` and from `bones-module-os`, as the first per-capability split under ADR-034.

## Acceptance

- The moved tests pass in bones, unchanged except for paths and names.
- The crate README documents every `Integration` field and every location written on each platform.
- Through the bus, a test registers against scratch locations, reads back that it is registered, and unregisters. Through the crate API, the same works directly.
- `bones-messages` has codec tests and wire-format vectors for `desktop/*`.
- `docs/architecture/structure.md` shows the new crates and their edges.

## Out of scope

- macOS registration.
- Deleting Commits' copy. That is brief 10, after the bones release.
