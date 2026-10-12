# 01 — bones: single instance with argument handoff

- **Repository:** bones
- **Depends on:** nothing
- **Needed by:** 10 (Commits), 16 (DevPanel)
- **Contract:** contract 2, owned by Commits; draft in [00-contracts.md](00-contracts.md#2-a-second-launch-hands-off-to-the-running-window). This brief builds only the generic mechanism.

## Why

DevPanel launches `commits <path>` for every repository the user opens. Today each launch starts a new process and a new window, so opening five repositories gives five Commits windows. The user wants one Commits window that switches repository and comes to the front.

"Only one instance per user, and later launches hand their arguments to it" needs no git repository, so under Commits' ADR-012 it belongs in bones. DevPanel uses it too: a second `devpanel` launch, or "Open in DevPanel" on a folder, goes to the existing window.

## What exists

- **Install layout:** `bones-upgrader` provides a launcher plus version folders. `launcher::run` starts the newest version's app detached, with the user's arguments, and answers `--version`, `--help` and `--rollback` itself.
- **A starting point in Commits:** `crates/os/src/rendezvous.rs` is a file-backed handshake for its askpass and editor helpers. It works, but it is specific to prompts. Read it; do not depend on it.
- **The window:** the kernel's `platform` owns the SDL window and provides the `window-surface` service (ADR-017, ADR-031).
- **No single-instance mechanism anywhere:** not in bones, not in Commits, not in DevPanel.

## Architecture

Two pieces, following the README's layer table:

| Piece | Crate | Depends on | Responsibility |
| --- | --- | --- | --- |
| Claim and handoff | new leaf crate `crates/bones-instance` | std, plus a platform crate if needed (for example `windows-sys`) | Become the instance for a key, or hand arguments to the one that is. No bus, no window. |
| Delivery | new module `crates/bones-engine/bones-module-instance` | `bones-kernel`, `bones-messages`, `bones-instance` | Publish each received launch on the bus, and bring the window to the front |
| Messages | `bones-messages::instance` | — | `Launched { args: Vec<String>, cwd: String }` on topic `instance/launched` |
| Composition | `bones-engine`, feature `instance` | — | `Engine::single_instance(primary)` registers the module |

The claim lives in a leaf crate because it must run in the app's `main` before `Engine::new()`, when no bus and no window exist yet. Delivery is bus traffic, so it is a module. The module never depends on `bones-module-web` or any other module crate (ADR-031).

## What to build

### `bones-instance`

```rust
pub struct Handoff { pub args: Vec<String>, pub cwd: PathBuf }
pub enum Claim { Primary(Primary), HandedOff }
pub fn claim(key: &str, handoff: Handoff, timeout: Duration) -> Result<Claim, String>;
impl Primary { pub fn incoming(&self) -> std::sync::mpsc::Receiver<Handoff>; }
```

1. **Key.** A plain string such as `"commits"`, scoped per user by the crate. It is not `AppIdentity`, so single instance does not depend on `bones-upgrader`. Apps pass `bones_upgrader::host_identity().app_stem`.
2. **Claim.** `claim` tries to become the instance for the key. On success it returns `Primary`, which owns a listener thread and yields each received `Handoff` on `incoming()`. Dropping `Primary` releases the claim.
3. **Hand off.** When another process holds the claim, `claim` sends it the `Handoff`, waits for an acknowledgement, and returns `HandedOff`. The caller then exits 0.
4. **Fall back.** With no acknowledgement within `timeout`, `claim` returns `Primary` for a fresh claim, so the new process starts normally as a second instance. Apps pass 2 s. A hung instance must never stop the user opening the app.
5. **Stale claims.** A claim left by a crashed process is detected and taken over, never trusted.
6. **Mechanism.** A named pipe on Windows (`\\.\pipe\bones-instance-<user-sid>-<key>`) and a Unix domain socket in `$XDG_RUNTIME_DIR`, or in a per-user temp folder when that is unset, with the socket's existence and a successful connect as the claim. Per user, with no network port and no elevated rights. Record the mechanism, and why it beat a generalised rendezvous directory, in a bones ADR.
7. **Foreground on Windows.** Before sending, the handing-off process calls `AllowSetForegroundWindow` with the primary's process id, which the primary returns in the handshake. Otherwise Windows refuses to raise the primary's window.

### `bones-module-instance`

1. Constructed from a `Primary`. On every frame it drains `incoming()` without blocking, publishes `instance/launched` for each handoff, and then raises the window.
2. **Raising the window.** Add a `raise()` method to the kernel's `window-surface` service: un-minimise, raise and focus, where the platform allows. The module calls it through the service registry. This is the only kernel change.
3. **Startup arguments are not republished.** The app reads its own startup arguments as it does today. The module carries only later launches.

### Composition

`bones-engine` gains an `instance` feature and `Engine::single_instance(primary: bones_instance::Primary)`. The example below goes in the crate README and in a new example, `examples/embedding/single-instance-app`:

```rust
let handoff = Handoff { args: std::env::args().skip(1).collect(), cwd: std::env::current_dir()? };
match bones_instance::claim("myapp", handoff, Duration::from_secs(2))? {
    Claim::HandedOff => return Ok(()),
    Claim::Primary(primary) => Engine::new().window("myapp", 800, 600).web().single_instance(primary).run()?,
}
```

### Where it runs

The claim happens in the app (`*-app`), not in the launcher. Users and tools can start either one, and the launcher may be older than the app. The launcher keeps forwarding arguments unchanged and stays fast.

## Acceptance

- **Second launch:** with the example running, starting it again as `app --x y` from another shell exits 0 within 2 s. The first instance publishes `instance/launched` with `["--x", "y"]` and the caller's working directory, and its window comes to the front.
- **Isolation:** two different keys run side by side without interfering.
- **Crash recovery:** killing the running instance and starting again gives a normal start, even with the stale claim left behind.
- **Hang fallback:** while the primary's listener is blocked, a new start becomes its own instance after the timeout.
- **Tests:** unit tests in `bones-instance` for claim, handoff, stale-claim recovery and timeout, with no window. A module test with a fake `window-surface` checks that a handoff publishes the message and calls `raise()`. `bones-messages` gets codec tests and wire-format vectors for `instance/launched`, like its other messages.
- **Docs:** a README for each new crate, the ADR, `docs/architecture/structure.md` updated with both crates and their edges, and `docs/design/modules.md` listing the `raise` method on `window-surface`.
- **Platforms:** works on Windows and Linux. macOS compiles; it is not a release target.

## Out of scope

- What an app does with the handed-off arguments. That is brief 10 for Commits and brief 16 for DevPanel.
- Replacing Commits' askpass rendezvous with this channel.
