# 07 — bones: loopback control channel

- **Repository:** bones
- **Depends on:** nothing
- **Needed by:** optional; both apps' end-to-end tests, and agents checking UI changes against the real app
- **Priority:** optional

## Why

Both apps' interfaces are web pages that talk to the host only through bus messages. Scripting the real app, for an end-to-end test or a screenshot of a real state, is hard: synthetic clicks fail under Xwayland, and pixel positions drift. Commits solved this with an opt-in local HTTP channel. It posts page messages and records the bus, and it needs no repository.

## What exists

Commits' `apps/commits/host/src/control.rs`:

- **Off by default.** Enabled only by `COMMITS_CONTROL_PORT=<port>`. It binds to loopback and has no authentication.
- **Endpoints:**
  - `GET /health`
  - `POST /page-message?owner=…&panel=…`, which injects a page message
  - `GET /messages[?topic=prefix&limit=n]`, which returns recorded bus traffic, oldest first

DevPanel's `scripts/screenshot.ps1` can only screenshot a browser preview backed by a mock host. It cannot drive the real app.

## Architecture

This is genuinely bus work, so it is a **native module**, `crates/bones-engine/bones-module-control`, depending on `bones-kernel` and `bones-messages` only. `bones-engine` gains a `control` feature and `Engine::control_channel_from_env(var: &str)`.

The variable name is passed by the composition root, for example `"COMMITS_CONTROL_PORT"`. The module does not derive it from `AppIdentity`, so it does not depend on `bones-upgrader`.

## What to build

1. **Move it.** Move the channel into the module with the same endpoints and behaviour. Keep its tests.
2. **Inert unless asked.** With the variable unset or not a port number, the module registers nothing and opens no socket.
3. **A screenshot endpoint,** only if wry can capture a panel on both Windows and Linux: `GET /screenshot?panel=…` returning a PNG. If either platform cannot, leave the endpoint out and record why in the README.
4. **Loud security notes.** The README says plainly that the channel is unauthenticated, loopback only, never enabled in a shipped launch, and why.

## Acceptance

- With the variable unset, nothing listens.
- With it set, tests inject a page message and read it back from `/messages`.
- Commits can delete `control.rs` and get the same behaviour from bones (brief 10).
- `docs/architecture/structure.md` lists the module.

## Out of scope

- Authentication or remote access.
