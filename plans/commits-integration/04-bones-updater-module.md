# 04 — bones: updater module and GitHub release source

- **Repository:** bones
- **Depends on:** nothing
- **Needed by:** 10 (Commits deletes its wiring), 16 (DevPanel gains self-update)
- **Priority:** recommended. It is not on the integration's critical path, but without it DevPanel never updates itself.

## Why

Commits updates itself from its GitHub releases: check, download, verify, stage, apply on next launch, roll back on failure. The pieces that don't name Commits already live in `bones-upgrader`: manifests, checksums and version folders. The bus module that drives them, the HTTPS download, and the "latest GitHub release, per build type" convention are still Commits code. DevPanel has no self-update at all; you update it by re-running `install.ps1`.

## What exists

- **`bones-upgrader`:** `fetch_manifest`, `parse_manifest`, `download_asset_verified`, `verify_checksum`, `is_newer`, the install and version-folder functions, and the `Fetch` trait. In bones, only a test stub implements `Fetch`.
- **Commits' `apps/commits/host/src/updater.rs`:** a bus module on `updater/request` and `updater/completed`, with actions CHECK, STAGE, INSTALL, REGISTER and UNREGISTER. REGISTER and UNREGISTER move to brief 02's module, not this one. It records the running version through kernel persistence (`state/updater.bin`) for an "Updated to X" banner; see Commits' `docs/updating.md` § Confirming an update took effect.
- **Commits' `apps/commits/host/src/release.rs`:** a compile-time `PLATFORM` (`windows-x64`, `windows-arm64`, `linux-x64`), the URL `https://github.com/<owner>/<repo>/releases/latest/download/<app>-<platform>.json` with a settings override, `ReleaseDownload` (an HTTPS `Fetch` with a 256 MB cap, built on `ureq` and `native-tls`), and `mark_executables` for Unix.
- **The manifest format:** Commits' `docs/updating.md` § The manifest:

  ```json
  { "version": "1.8.1", "url": "https://…/commits-1.8.1-windows-x64.zip", "sha256": "…" }
  ```

## Architecture

| Piece | Crate | Depends on | Responsibility |
| --- | --- | --- | --- |
| Release source | `bones-upgrader`, new `release_source.rs` | — | `PLATFORM` and `ReleaseSource::GitHub { owner, repo, app }` producing the manifest URL; `ReleaseSource::Url(String)` for an override |
| Update flow | `bones-upgrader`, new `updater.rs` | `Fetch` | `check(fetch, source, current)`, `stage(fetch, manifest, install_dir)`, `install(...)`: plain functions returning values, no threads, no bus |
| HTTPS fetch | the new module crate | `ureq`, `native-tls` | `HttpsFetch`, moved from Commits' `ReleaseDownload` with its limits |
| Messages | `bones-messages::upgrader` | — | `Request::{Check, Stage, Install, LastApplied}` on `upgrader/request`; `Result::{Available { version }, UpToDate, Staged { version }, Installed, LastApplied { previous: Option<String>, current: String }, Failed { reason }}` on `upgrader/result`, correlated |
| Bus adapter | new module `crates/bones-engine/bones-module-upgrader` | `bones-kernel`, `bones-messages`, `bones-upgrader` | Runs each request off the frame thread and publishes the result |
| Composition | `bones-engine`, feature `upgrader` | — | `Engine::upgrader(UpgraderConfig { source, current_version })` |

- **`bones-upgrader` stays a leaf.** It gains no HTTP client and no bus. `Fetch` remains its one host requirement, as ADR-033 states.
- **The module does not depend on `bones-module-os`** (ADR-031). It owns `HttpsFetch` itself. `UpgraderConfig` takes an optional `Arc<dyn Fetch + Send + Sync>`, so tests and embedders can substitute one.
- **Identity** comes from `bones_upgrader::host_identity()`, as everywhere else.

## What to build

1. **The pieces in the table.** Move logic from Commits' `updater.rs` and `release.rs` rather than rewriting it: the staging layout, the rollback and `mark_executables` keep their current behaviour.
2. **"Updated to X".** On `init`, the module records the running version through kernel persistence under its own endpoint name, and answers `LastApplied` with the previous and current versions. Keep Commits' rule: no banner on a first run ever.
3. **Same behaviour as Commits.** Same manifest format, same staging layout, same rollback, same override semantics: an empty override means the GitHub source.
4. **The release convention, documented** in `bones-upgrader`'s README: per platform, CI publishes `<app>-<version>-<platform>.zip`, whose entries are one version folder's contents at the zip root, and `<app>-<platform>.json`. Commits' `scripts/package-release.ps1` is the reference; link it.
5. **An ADR** in bones: the update flow is functions in the leaf crate, and the bus adapter is a module that owns its HTTPS fetch.

## Acceptance

- Tests in `bones-upgrader` with a fake `Fetch` cover:
  - a newer version available
  - already up to date
  - a checksum mismatch, which fails without staging anything
  - staging followed by applying on next launch
  - rollback when the new version fails to start
- Module tests with a fake `Fetch` check that a request publishes one correlated result and never blocks the frame thread.
- `bones-messages` has codec tests and wire-format vectors for `upgrader/*`.
- `examples/embedding/self-updating-app` uses the module and checks for an update against a local fake manifest.

## Out of scope

- Any user interface. Apps decide how to offer the update.
- Background or automatic update scheduling. Apps decide when to check.
- Desktop registration. That is brief 02.
