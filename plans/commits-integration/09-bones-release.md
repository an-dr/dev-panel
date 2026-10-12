# 09 — bones: release

- **Repository:** bones
- **Depends on:** 01, 02 and 03, plus whatever of 04–08 is done
- **Needed by:** 10 (Commits), 16 (DevPanel)

## Why

Both apps pin bones as a git submodule. They should pin a published tag, not a bare commit. Today both pin `ff90d95` ("chore(release): bump engine to 1.1.1"), but no `v1.1.1` tag exists, locally or on GitHub; the newest tags are `v1.1.0` and `abi-v1.1.0`.

## What to do

1. **Tag the old pin.** Create the annotated tag `v1.1.1` on `ff90d95` and push it, so the current pin is reachable by name. This creates a missing tag; it moves none, so the immutable-tags rule in `docs/contributing/releasing.md` holds.
2. **Release both version lines.** Follow `docs/contributing/releasing.md` and ADR-029:
   - **Engine line:** a minor bump, `v1.2.0`. The new crates, modules, builder methods and the `window-surface` `raise()` method are additive.
   - **ABI line:** a minor bump, `abi-v1.2.0`. `bones-messages` gains `instance`, `desktop` and, if included, `upgrader`. `wit/` is unchanged.
   - If a change turns out to break an existing signature or message, the matching line takes a major bump instead, and the upgrade notes say what breaks.
3. **Changelog.** In `CHANGELOG.md`, one entry for each line, listing each feature from 01–08 that is in the release and what an app must do to adopt it.
4. **Upgrade notes** for hosts, in `docs/architecture/upgrading.md`: for each feature, the Cargo feature to enable, the builder call or leaf-crate call to add, and which `bones-messages` types are new. Include the single-instance `main` snippet from brief 01.
5. **Tag and push** `v1.2.0` and `abi-v1.2.0` on the same commit.

## Acceptance

- `v1.1.1`, `v1.2.0` and `abi-v1.2.0` are on GitHub.
- From a fresh clone of each app, checking out `vendor/bones` at `v1.2.0` and building works.
- `CHANGELOG.md` names every included brief by feature, and `docs/architecture/structure.md` matches the crates that exist.
