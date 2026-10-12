# TODO

## Ship Commits with DevPanel

Bundle a pinned [Commits](https://github.com/an-dr/commits) release in every DevPanel release, so the default "Commits" tool works right after installing DevPanel.

- Pin the Commits version in one place in this repository (currently 1.8.1).
- In the release build, download `commits-<version>-<platform>.zip` for each platform, verify it against the `sha256` in Commits' `commits-<platform>.json` manifest, and place it inside the DevPanel zip.
- Have `scripts/dist.ps1` fetch the same pinned zip, so a local install matches a release.
- On `devpanel-app --install` and `scripts/install.ps1`, install the bundled Commits into `~/.commits/app` only when it is absent. Never downgrade or overwrite an existing install. Check whether Commits updates itself after install; its release manifests suggest it does.
- If installing Commits fails, warn and still install DevPanel.
- Leave Commits in place when DevPanel is removed; it is a standalone app.

Cost: about 25 MB more per platform download (roughly 11 MB to 36 MB).

Considered instead: downloading Commits on demand at install or on first "Open in Commits". It keeps the zip small but needs the network and a download-and-verify path in DevPanel. Embedding Commits as a bones extension was set aside: two git layers, coupled releases and the WASM toolchain for one panel.
