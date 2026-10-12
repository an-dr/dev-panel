# 15 — Commits: release

- **Repository:** commits
- **Depends on:** 10, 11 and 12, plus whatever of 13 and 14 is done
- **Needed by:** 17 (DevPanel bundles this release), 18 and 19

## What to do

1. **Release.** Follow Commits' `AGENTS.md` § Bump the app version and § Release by tag, and `docs/updating.md` § Publishing a release. Bump `apps/commits/host/Cargo.toml` and `UPGRADER_LAUNCHER_VERSION` together, as a minor version, for example 1.9.0. Push to `main`, then push the tag `v<version>`; CI publishes.
2. **Assets.** The release keeps publishing, per platform, `commits-<version>-<platform>.zip` and `commits-<platform>.json`, a manifest with `version`, `url` and `sha256`, for `linux-x64`, `windows-x64` and `windows-arm64`. DevPanel's build downloads exactly these names (brief 17).
3. **Release notes.** List:
   - single instance
   - each flag in `docs/command-line.md`
   - the health line, and the repository-health contract with its vectors
   - repository sources in the selector, if included
   - theme flags and theme files, if included
4. **Hand over the version.** Put the released version in the release notes' first line, so DevPanel's briefs 17 and 19 can pin it.

## Acceptance

- The GitHub release has all six assets, and each manifest's `sha256` matches its zip.
- On a machine with the previous Commits installed, the in-app updater offers and applies the new version.
- The contract pages `docs/command-line.md`, `docs/repository-health.md`, `docs/repository-health.vectors.json`, and, if included, `docs/repository-sources.md` and `docs/themes.md` exist at the release tag.
