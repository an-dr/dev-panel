//! `devpanel-app --install`, and Settings → Installation: copies the build
//! this executable belongs to into the user's install, the same way an
//! update would.
//!
//! The mechanism is the bones upgrade module's. With nothing installed, the
//! launcher and a first version folder are placed. With an install present,
//! the build becomes a new version folder beside the running one, so a
//! running DevPanel is never overwritten and the launcher switches to it on
//! its next start. Only the newest two version folders are kept.
//!
//! Next it installs Commits from the `companions/` folder beside it, unless
//! Commits is already installed: Commits updates itself, so an existing
//! install, newer or older, is left alone (docs/adr/ADR-003).
//!
//! Then it registers DevPanel with the desktop: a menu entry and "Open in
//! DevPanel" on folders, both pointing at the installed launcher. A failed
//! registration is only a warning; the install itself succeeded.
//!
//! Run it from an assembled `dist/app/<version>/` folder (scripts/dist.ps1),
//! where the launcher sits one level up, or from an unzipped release, which
//! carries it as `devpanel-launcher`.

use std::error::Error;
use std::path::{Path, PathBuf};

use bones_kernel::desktop::{self, Locations};
use bones_kernel::upgrade::{AppIdentity, CompanionOutcome, CompanionPolicy, UpgradeModule};

use crate::installation;

pub fn run() -> Result<(), Box<dyn Error>> {
    for line in install()? {
        if line.starts_with("warning:") {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    }
    Ok(())
}

/// Installs the running build, Commits beside it and the desktop entry, and
/// reports each step as a line; a step that failed after the install itself
/// is a `warning:` line, not an error. Settings → Installation runs this too.
pub fn install() -> Result<Vec<String>, String> {
    let upgrade = UpgradeModule::new(installation::config());
    let install_dir = upgrade.install_dir().ok_or("no home directory to install into")?;
    let version = env!("CARGO_PKG_VERSION");
    let mut report = vec![if upgrade.install()? {
        format!("Installed DevPanel {version} to {}", install_dir.display())
    } else {
        format!("Pushed DevPanel {version} to {} — it starts the next time DevPanel is launched.", install_dir.display())
    }];
    match install_commits(&upgrade) {
        Ok(Some(message)) => report.push(message),
        Ok(None) => {}
        Err(error) => report.push(format!("warning: could not install Commits: {error}")),
    }
    report.push(match register() {
        Ok(()) => "Registered DevPanel with the desktop".to_string(),
        Err(error) => format!("warning: could not register DevPanel with the desktop: {error}"),
    });
    Ok(report)
}

/// Commits' installation identity, as Commits itself declares it
/// (apps/commits/host/src/installation.rs in the Commits repository).
const COMMITS: AppIdentity = AppIdentity {
    install_dir_name: ".commits",
    install_env_var: "COMMITS_INSTALL_DIR",
    launcher_stem: "commits",
    app_stem: "commits-app",
};

/// `None` when this build carries no Commits, as a `cargo run` build does.
fn install_commits(upgrade: &UpgradeModule) -> Result<Option<String>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let Some(manifest) = exe.parent().and_then(|dir| bundled_manifest(&dir.join("companions"))) else {
        return Ok(None);
    };
    let bundle = Bundle::read(&manifest)?;
    let outcome = upgrade.install_companion(&COMMITS, &bundle.archive, &bundle.version, Some(&bundle.sha256), CompanionPolicy::IfAbsent)?;
    Ok(Some(match outcome {
        CompanionOutcome::Installed { version } => format!("Installed Commits {version}"),
        CompanionOutcome::AlreadyPresent { version } => format!("Commits {version} is already installed; left as it is"),
        CompanionOutcome::Skipped { reason } => format!("Skipped Commits: {reason}"),
    }))
}

/// The one `commits-<platform>.json` in `companions`.
fn bundled_manifest(companions: &Path) -> Option<PathBuf> {
    std::fs::read_dir(companions).ok()?.flatten().map(|entry| entry.path()).find(|path| {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        name.starts_with("commits-") && name.ends_with(".json")
    })
}

/// Commits' own update manifest, and the archive beside it that it names.
#[derive(Debug)]
struct Bundle {
    version: String,
    sha256: String,
    archive: PathBuf,
}

impl Bundle {
    fn read(manifest: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
        let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
        let field = |name: &str| {
            json.get(name).and_then(|v| v.as_str()).map(str::to_string).ok_or(format!("{} has no {name}", manifest.display()))
        };
        let url = field("url")?;
        let file = url.rsplit('/').next().filter(|f| !f.is_empty()).ok_or(format!("{url} names no file"))?;
        let archive = manifest.with_file_name(file);
        Ok(Self { version: field("version")?, sha256: field("sha256")?, archive })
    }
}

fn register() -> Result<(), String> {
    let integration = installation::desktop_integration().ok_or("no home folder")?;
    let locations = Locations::for_current_user().ok_or("this desktop is not supported")?;
    desktop::register(&integration, &locations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_manifest_names_its_archive_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("commits-linux-x64.json");
        std::fs::write(
            &manifest,
            r#"{"version":"1.9.1","url":"https://github.com/an-dr/commits/releases/download/v1.9.1/commits-1.9.1-linux-x64.zip","sha256":"ab"}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("notes.txt"), "").unwrap();
        assert_eq!(bundled_manifest(dir.path()), Some(manifest.clone()));
        let bundle = Bundle::read(&manifest).unwrap();
        assert_eq!(bundle.version, "1.9.1");
        assert_eq!(bundle.sha256, "ab");
        assert_eq!(bundle.archive, dir.path().join("commits-1.9.1-linux-x64.zip"));
    }

    #[test]
    fn a_build_without_companions_has_nothing_to_install() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(bundled_manifest(&dir.path().join("companions")), None);
    }

    #[test]
    fn a_manifest_without_a_checksum_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("commits-linux-x64.json");
        std::fs::write(&manifest, r#"{"version":"1.9.1","url":"https://x/commits.zip"}"#).unwrap();
        assert!(Bundle::read(&manifest).unwrap_err().contains("sha256"));
    }
}
