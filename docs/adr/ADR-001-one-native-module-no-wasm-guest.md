# ADR-001: The product is one native bones module, with no WASM guest

**Status:** Accepted

## Context

bones' intended shape is a native core plus product behaviour in sandboxed, hot-reloadable WASM extensions, with native modules only for capabilities the sandbox refuses. Commits follows it: a native host for git, a WASM component for product logic, and a web page.

Almost everything DevPanel does needs the host: run `git` and install scripts, launch editors and terminals, read arbitrary folders. A guest would forward nearly every action to a native module and wait for the answer.

## Decision

All behaviour lives in native Rust. One native module (`Dashboard`) owns the web panel and is the only code that speaks the bus. It opens the panel through the `Registry` service bones provides to modules, receives page messages as `web/page-message` events, and uses the engine's `os` module for dialogs, URLs and the clipboard. The product logic sits behind it in plain crates (`devpanel-core`, `devpanel-infra`) that know nothing about bones.

## Consequences

- No WASM toolchain, component build, or guest/host vocabulary to maintain; `cargo build` produces the whole app.
- No hot reload of product logic, and no sandbox around it. Neither would protect anything here: the logic exists to run local commands.
- bones still provides the window, web panel, frame loop, folder picker, browser and clipboard, and its platform fixes arrive with each submodule update.
- If a part of DevPanel should ever be sandboxed or scripted by users, it can move into an extension and talk to `Dashboard` over the bus without the core crates changing.
