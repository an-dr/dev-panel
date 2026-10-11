# Development

## Layout

```text
apps/devpanel/          the application
  src/                  main (composition root), dashboard (bones), controller, jobs, protocol, page, install
  src/bin/              the launcher
  web/                  index.html, style.css, icons.js, app.js — the whole UI
  web/dev/mock-host.js  a fake host for working on the UI in a browser
  assets/               the app icon
crates/devpanel-core/   domain, use cases and ports
crates/devpanel-infra/  adapters
vendor/bones/           the engine, as a git submodule
scripts/                dist.ps1, package-release.ps1, install.ps1, preview.ps1
.github/workflows/      CI: build, test and release on every platform
.cargo/config.toml      the install identity bones-upgrader is built with
docs/                   this documentation
```

## Build and test

```sh
cargo build -p devpanel
cargo test --workspace
```

The first build compiles SDL3 from source, which takes a few minutes. The tests run the real `git` against throwaway repositories, so `git` must be on `PATH`; nothing outside the system temp folder is touched.

| Suite | Covers |
| --- | --- |
| `devpanel-core` | Health verdicts, fixes, install detection, every use case against fakes |
| `devpanel-infra` | Porcelain parsing, and status, fetch, pull, push, stash and conflicts against real repositories; filesystem scans; the JSON store; program resolution; child environment |
| `devpanel` | The protocol's JSON shape; the controller driving real adapters, including job output, failures and the busy guard |

## Run

```sh
cargo run -p devpanel
```

This runs the app binary, `devpanel-app`, straight from `target/`. Installing is covered in [installation.md](installation.md).

| Variable | Effect |
| --- | --- |
| `DEVPANEL_CONFIG` | Path of the workspace file to use instead of the default — keeps experiments away from your real list |
| `DEVPANEL_WEB_DIR` | Reads the page from this folder at start instead of the copy built into the binary, so UI edits need a restart, not a rebuild |
| `DEVPANEL_DEBUG` | Also prints the engine's per-frame debug log |
| `DEVPANEL_INSTALL_DIR` | Where `install.ps1` and the launcher look instead of `~/.devpanel/app` |

## Working on the UI

```sh
pwsh scripts/preview.ps1
```

writes `.artifacts/preview/index.html`: the page with `web/dev/mock-host.js` standing in for the host, with sample repositories and simulated jobs. Serve or open it in any browser. The mock speaks the same [protocol](design/protocol.md); keep it in step when the protocol changes.

The page is plain JavaScript with no build step and no dependencies, because a panel's page is loaded from a string and cannot rely on the network. `render()` rebuilds the visible view from state after every host batch; menus and modals live in their own roots, so a status update never closes one under the user's cursor.

## Platform notes

- **Linux** needs WebKitGTK (`libwebkit2gtk-4.1-dev` on Debian and Ubuntu) and an X server or Xwayland. DevPanel selects X11 itself; see [architecture.md](architecture.md#linux-display).
- **Windows** builds with MSVC. Release builds have no console window.
- **macOS** is untested.

## Releasing

The version is `[workspace.package] version` in `Cargo.toml`. Move `UPGRADER_LAUNCHER_VERSION` in `.cargo/config.toml` with it: `devpanel --version` reports both, so a mismatch shows. Releases are built and published by CI from a `v<version>` tag; see [ci.md](ci.md#releasing).

## Updating bones

bones is pinned as a submodule. To move it:

```sh
git -C vendor/bones fetch --tags
git -C vendor/bones checkout <tag or commit>
cargo test --workspace
git add vendor/bones
```
