# 20 — DevPanel: release

- **Repository:** dev-panel
- **Depends on:** 16, 17 and 18, plus 19 if it is done

## What to do

1. **Bump the version.** Follow `docs/ci.md` § Releasing: bump `version` in `Cargo.toml` and `UPGRADER_LAUNCHER_VERSION` in `.cargo/config.toml` together, as a minor version.
2. **Publish manifests.** If DevPanel adopted self-update in brief 16, CI also publishes `devpanel-<platform>.json` manifests (`version`, `url`, `sha256`) next to the zips, following the release convention in bones-upgrader's README (brief 04). Extend `scripts/package-release.ps1` for it, using Commits' `scripts/package-release.ps1` as the model, and update `docs/ci.md`.
3. **Release.** Push to `main`, wait for CI, then tag and push `v<version>`.
4. **Release notes.** Describe:
   - Commits now ships with DevPanel, and installs alongside it when it is not already installed. Give the bundled Commits version.
   - "Open in Commits" on every card and drawer, with problems opening the matching view, in DevPanel's colours.
   - DevPanel's repositories appear in Commits' repository selector, if 19 shipped.
   - Single window, "Open in DevPanel" on folders, desktop registration, and self-update, if included.
5. **Install locally.** Build and install with `pwsh scripts/install.ps1`, and confirm that DevPanel and Commits both start and work together.

## Acceptance

- The GitHub release has the three zips, each with `companions/` holding the pinned Commits archive and manifest, plus `SHA256SUMS`, plus the DevPanel manifests if self-update shipped.
- On a clean machine, installing DevPanel and clicking "Open in Commits" on a dirty repository opens Commits on its changes, in one window, in DevPanel's colours.
