# 10 — Commits: adopt the new bones, and the launch request

- **Repository:** commits
- **Depends on:** 09 (bones release)
- **Needed by:** 11, 12, 13, 14 and 15
- **Contracts:** publishes contracts 1 (path only) and 2, owned by Commits; drafts in [00-contracts.md](00-contracts.md#1-launching-commits)

## Why

bones now provides capabilities Commits either lacked (single instance) or carried itself (desktop registration, and possibly the updater module, the control channel, the watcher core and program launching). Commits' ADR-003 and ADR-012 require taking them from bones and deleting the local copies. A capability must never exist in both places.

This brief also lays the seam that briefs 11 and 14 extend: one parsed **launch request** that arrives the same way at startup and on a handoff. After this brief, a new flag is a new field in one parser and one handler, not a change to how launches travel (open–closed principle).

## Architecture

| Concern | Where | Notes |
| --- | --- | --- |
| Claiming the instance | `apps/commits/host/src/main.rs` | Before `Engine::new()`, after `diagnostics::install()` |
| Parsing arguments | new `apps/commits/host/src/launch_request.rs` | A pure function from `(args, cwd)` to `LaunchRequest`; no filesystem |
| Checking the path | `launch.rs`, existing `check` | Unchanged: canonicalise, find repositories |
| Holding and publishing launches | `launch.rs`, `LaunchModule` | Startup request at construction; handoffs from `instance/launched` |
| Applying a launch | `apps/commits/bones-adapter` | The guest opens or switches repository, as at startup |
| Wire format | `ipc/` with fixtures | Per ADR-002, both language bindings and a shared fixture |

`packages/core` is not touched by this brief.

## What to do

1. **Bump the pin.** Point `vendor/bones` at `v1.2.0` from brief 09, and read `docs/architecture/upgrading.md` there.
2. **The launch request.** Add `LaunchRequest { path: Option<String>, sequence: u32 }` and `parse(args: &[String]) -> LaunchRequest`. The parser follows contract 1's rules from day one: flags in any order after the path, `--name=value` accepted, unknown flags and values ignored. Today it fills only `path`; briefs 11 and 14 add fields. `sequence` starts at 0 for the startup request and increases with each handoff.
3. **Single instance (required).** In `main.rs`, call `bones_instance::claim(host_identity().app_stem, handoff, Duration::from_secs(2))`. On `HandedOff`, exit 0 at once. On `Primary`, pass it to `Engine::single_instance`.
4. **Later launches in `LaunchModule`.** Today it reads the arguments once, at construction, and answers the guest's question on the `launch` endpoint. Keep that for the startup request. Also subscribe to `instance/launched`. For each one: parse, run the same `check` against the handed-off working directory, store it as the current request with the next `sequence`, and publish it on a new topic `launch/request` in the same encoding the endpoint answers with. Extend the encoding in `ipc/` with `sequence`, and add fixtures for both languages.
5. **The guest applies it.** `bones-adapter` subscribes to `launch/request`. On each one it opens that repository, or switches to it when it is already open, and adds it to the recent list, as at startup. A request that arrives while a repository is still loading waits for that load to finish and then applies; only the highest `sequence` is applied.
6. **Desktop registration (required).** Delete `crates/desktop-integration`. Compose `Engine::desktop(integration)` with the values in today's `desktop.rs`, and delete REGISTER and UNREGISTER from `updater.rs`. Point the page's "Register with system" at `desktop/request`. Close BUG-006 in `docs/techdebt.md`, and remove the exception paragraph from `crates/README.md`.
7. **Each of these only if bones' release includes it:**
   - **Updater (brief 04):** delete `updater.rs` and `release.rs`. Compose `Engine::upgrader` with `ReleaseSource::GitHub { owner: "an-dr", repo: "commits", app: "commits" }`, or `ReleaseSource::Url` when `app.updateManifestUrl` is set. Point the page at `upgrader/*`. Update `docs/updating.md`.
   - **Control channel (brief 07):** delete `control.rs` and compose `control_channel_from_env("COMMITS_CONTROL_PORT")`.
   - **Watcher (brief 06):** rebuild `crates/watcher` on `bones-watch`, keeping only the `.git` path classification and the `repo/*` topics.
   - **Program launching (brief 05):** `run_tool` in `crates/os` calls `bones_launch::launch`.
   - **Panel serving (brief 08 A):** retire `page.rs`'s local server, serve the splash and page through `PanelSource::Html`, and close BUG-005.
8. **Publish the contract.** Create `docs/command-line.md`. Move `docs/updating.md` § The command line into it, leaving a link behind. Add the path argument as it works today, and contract 2 from the draft. Link the page from `docs/index.md` and the README. From now on this page is the authority for Commits' command line.
9. **Behaviour.** No visible change except single instance.

## Acceptance

- **Single instance:** with Commits open on repository A, `commits <B>` from a terminal exits 0. The running window switches to B and comes to the front, and B is in the recent list.
- **Folder argument:** with Commits open, `commits <folder of clones>` behaves like the same argument at startup.
- **During a load:** handing off C while A is still loading opens C once A's load finishes, with no error.
- **No duplicates:** no deleted crate or module remains beside its bones replacement. `rg desktop-integration` finds only docs history.
- **Tests:** all existing tests pass. New tests cover the parser, including unknown flags and `--name=value`, `LaunchModule` receiving a handoff after startup, and the `ipc/` fixtures in both languages.

## Out of scope

- New flags. Those are briefs 11 and 14.
