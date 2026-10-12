# 03 — bones: companion app install

- **Repository:** bones (`bones-upgrader`)
- **Depends on:** nothing
- **Needed by:** 17 (DevPanel bundles Commits)

## Why

DevPanel will ship a pinned Commits release inside its own release zip, so installing DevPanel also installs its git client. "Install another bones app from a release archive I carry, but only if the user does not have it already" needs no repository. It is an installer operation, so it belongs in `bones-upgrader`, not in DevPanel's scripts.

## What exists in `bones-upgrader`

- `install.rs`: `install_fresh(identity, source_dir, install_dir, version)`, `extract_version(archive_bytes, install_dir, version)`, `copy_version_from_dir`, `refresh_launcher` and `remove_version_dir`.
- `identity.rs`: `AppIdentity` has four public `&'static str` fields, so a host can already write a literal for an app it was not compiled as. `host_identity()` reads the compile-time `UPGRADER_*` values.
- `lib.rs`: `verify_checksum`, `is_newer`, `default_install_dir(identity)`.
- `versions.rs`: `current_version_dir(install_dir)`, `previous_version_dir`.

## The archive

The archive is an app's ordinary **update payload**, the zip its release publishes for self-update. For Commits that is `commits-<version>-<platform>.zip`, made by Commits' `scripts/package-release.ps1`. Its entries sit at the zip's root: the contents of one version folder, including the launcher payload `identity.launcher_payload_exe()`, for example `commits-launcher.exe`. It does **not** contain an install layout or a top-level launcher. The version is not inside the archive, so the caller passes it from the release manifest.

## What to build

One function in a new file, `companion.rs`, with its own types in their own files:

```rust
pub enum Policy { IfAbsent, IfNewer }
pub enum Outcome { Installed { version: String }, AlreadyPresent { version: String }, Skipped { reason: String } }

pub fn install_companion(
    identity: &AppIdentity,
    archive: &Path,
    version: &str,
    expected_sha256: Option<&str>,
    policy: Policy,
) -> Result<Outcome, String>
```

1. **Where.** The install directory is `default_install_dir(identity)`, which honours the identity's override variable.
2. **What counts as present.** The companion is present when its launcher, `identity.launcher_exe()`, exists in the install directory. Its version is the name of `current_version_dir`.
3. **Policy:**
   - `IfAbsent`, the one DevPanel uses: install only when the companion is not present. Never touch an existing install, newer or older, because the companion updates itself.
   - `IfNewer`: install only when `is_newer(current, version)`. An existing install gains a new version folder through `extract_version`, exactly as a self-update would; the launcher picks it on its next start.
4. **Fresh install.** Verify the checksum when one is given, before anything is written. Extract into a temporary folder, then call `install_fresh(identity, temp, install_dir, version)`, which places the launcher from its payload name and makes the first version folder. Remove the temporary folder afterwards, whatever the outcome.
5. **Errors are values.** Return `Err` for a corrupt archive, a checksum mismatch or a filesystem failure. Never panic: the host must finish its own install when the companion fails.
6. **No bus message.** The only caller is an app's `--install` path, which runs before any engine exists.

## Acceptance

- **Tests** use a scratch install directory through the identity's override variable, and an archive built in the test with the payload layout above. They cover:
  - a fresh install, after which the companion's launcher exists and `current_version_dir` names the given version
  - an existing install left untouched under `IfAbsent`
  - an older install gaining a version folder under `IfNewer`
  - a newer install left alone under `IfNewer`
  - a corrupt archive
  - a checksum mismatch, with nothing written
- After a fresh install in a real run, the companion's launcher starts its app exactly as if the companion had been installed by its own `--install`. Check this by hand with a Commits 1.8.1 zip.
- The crate README documents the archive layout it expects and the two policies.

## Out of scope

- Downloading the archive. The host's build or CI does that.
- Registering the companion with the desktop. The companion does that itself, from its own "Register with system".
