# 17 — DevPanel: bundle Commits

- **Repository:** dev-panel
- **Depends on:** 16 (bones with the companion install from 03) and 15 (the Commits release to pin)
- **Needed by:** 20

## Why

DevPanel's default tools already include Commits (`~/.commits/app/commits {path}`), but it only works if the user installed Commits separately. Shipping a pinned Commits release inside every DevPanel release makes the integration work right after installing DevPanel, offline, and the same way every time. Commits updates itself afterwards, so the bundled version only matters for a first install.

## Architecture

- **Commits is a companion, not a dependency.** DevPanel carries Commits' unmodified release archive and asks bones to install it. No Commits code is linked or vendored. Record this in DevPanel's first ADR on the subject, `docs/adr/ADR-003-commits-is-a-bundled-companion.md`: DevPanel reaches Commits only through Commits' contracts (00-contracts), bundles its release archive, and never depends on its code. It also records why bundling beat downloading on demand and embedding Commits as an extension, from `TODO.md` § Set aside.
- **One source of truth for the bundled version.** The version is pinned once. The manifest that travels with the archive carries the version and checksum to the installer, so `install.rs` hard-codes neither.

## What to build

1. **Pin the version** as `COMMITS_VERSION` in `.cargo/config.toml`'s `[env]`, beside the `UPGRADER_*` values, with a comment. Set it to the release from brief 15.
2. **Layout inside a version folder:**

   ```text
   <version>/
     companions/
       commits-<platform>.json              # Commits' manifest, unchanged
       commits-<cversion>-<platform>.zip    # Commits' update payload, unchanged
   ```

3. **Fetch in CI.** The release job in `.github/workflows/build.yml`, per platform:
   - downloads `https://github.com/an-dr/commits/releases/download/v<COMMITS_VERSION>/commits-<platform>.json`
   - reads `url` and `sha256` from it, downloads the zip, and verifies the checksum, failing the build on a mismatch
   - places both files in `companions/` inside the version folder before the release zip is made

   Platform names are the same in both projects: `linux-x64`, `windows-x64`, `windows-arm64`.
4. **Fetch locally.** Put the download-and-verify in one script, `scripts/fetch-commits.ps1 -Platform <platform> -Destination <folder>`, which CI and `scripts/dist.ps1` both call, so the logic exists once. It caches downloads under `.artifacts/commits/<version>/` so repeated builds don't refetch.
5. **Install.** In `install.rs`, after DevPanel's own install:
   - Find the one `companions/commits-*.json` beside the running executable. None found, as in a `cargo run` build, means skip silently.
   - Read `version` and `sha256` from it, and call `bones_upgrader::install_companion` with the zip, that version and checksum, `Policy::IfAbsent`, and Commits' identity as a literal: `AppIdentity { install_dir_name: ".commits", install_env_var: "COMMITS_INSTALL_DIR", launcher_stem: "commits", app_stem: "commits-app" }`. Check those four values against Commits' `.cargo/config.toml` at the pinned tag.
   - Print one line: installed, already present with its version, or skipped and why. A failure prints a warning, and DevPanel's install still exits 0.
6. **Docs:**
   - README "Getting it": DevPanel now installs Commits too, if it is not installed, and the download is larger. State the size measured from the first CI build.
   - `docs/installation.md`: the bundled layout, the `IfAbsent` rule, and that removing DevPanel leaves Commits installed.
   - `docs/ci.md`: the fetch step and the pin.
   - `TODO.md`: remove the bundling item.

## Acceptance

- On a machine without Commits, unzipping a DevPanel release and running `--install` gives a working `~/.commits/app/commits`, and DevPanel's "Commits" tool opens a repository in it.
- With Commits already installed at another version, `--install` leaves it untouched and says so.
- A wrong checksum fails the CI build, and `fetch-commits.ps1` has a test or a dry-run mode showing it.
- CI is green on all three platforms, and every release zip contains `companions/` with both files.
