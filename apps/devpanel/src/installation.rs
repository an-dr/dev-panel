//! DevPanel's installation identity, release source and desktop entry: what
//! the bones upgrade and desktop modules install, update and register.
//! Supplied here, at composition, because bones reads none of it itself.

use std::path::{Path, PathBuf};

use bones_kernel::desktop::Integration;
use bones_kernel::upgrade::{AppIdentity, ReleaseSource, UpgradeConfig, UpgradeModule};

/// The install folder (`~/.devpanel/app`), the variable that redirects it,
/// and the two executable names. The launcher's own build reads the same
/// stems from `.cargo/config.toml`, so the two always agree.
pub fn identity() -> AppIdentity {
    AppIdentity {
        install_dir_name: env!("UPGRADER_INSTALL_DIR_NAME"),
        install_env_var: env!("UPGRADER_INSTALL_ENV_VAR"),
        launcher_stem: env!("UPGRADER_LAUNCHER_STEM"),
        app_stem: env!("UPGRADER_APP_STEM"),
    }
}

/// Updates come from the latest GitHub release of this repository.
pub fn config() -> UpgradeConfig {
    let source = ReleaseSource::GitHub { owner: "an-dr".into(), repo: "dev-panel".into(), app: "devpanel".into() };
    UpgradeConfig::new(identity(), source, env!("CARGO_PKG_VERSION"))
}

/// The menu entry and the "Open in DevPanel" folder action. Both start the
/// installed launcher, so they keep working across updates; `None` when there
/// is no home folder to install into.
pub fn desktop_integration() -> Option<Integration<'static>> {
    let launcher = UpgradeModule::new(config()).install_dir()?.join(identity().launcher_exe());
    Some(integration(launcher))
}

/// Where DevPanel stands: what Settings → Installation shows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Status {
    /// The newest installed version, `None` with nothing installed.
    pub installed: Option<String>,
    /// Whether this process runs out of a version folder of that install.
    pub current: bool,
    /// The install folder, `None` with no home folder to install into.
    pub dir: Option<PathBuf>,
}

pub fn status() -> Status {
    let upgrade = UpgradeModule::new(config());
    let dir = upgrade.install_dir();
    let running = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf));
    let current = matches!((&running, &dir), (Some(running), Some(dir)) if runs_from(running, dir));
    Status { installed: upgrade.installed_version(), current, dir }
}

/// Whether `running_dir` is one of `install_dir`'s version folders, however
/// either path is spelled.
fn runs_from(running_dir: &Path, install_dir: &Path) -> bool {
    let (Ok(running), Ok(install)) = (running_dir.canonicalize(), install_dir.canonicalize()) else { return false };
    running.parent() == Some(install.as_path())
        && running.file_name().and_then(|name| name.to_str()).and_then(bones_versioning::parse_folder_name).is_some()
}

fn integration(executable: PathBuf) -> Integration<'static> {
    Integration {
        id: "devpanel",
        name: "DevPanel",
        comment: "Dashboard for your git repositories",
        open_label: "Open in DevPanel",
        categories: "Development;RevisionControl;",
        executable,
        icon_png: include_bytes!("../assets/icons/devpanel-256.png"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_version_folder_of_the_install_runs_from_it() {
        let root = tempfile::tempdir().unwrap();
        let install = root.path().join("app");
        for dir in ["app/0.4.1", "app/0.4.1-bdfbbcec", "app/notes", "download/0.4.1"] {
            std::fs::create_dir_all(root.path().join(dir)).unwrap();
        }
        assert!(runs_from(&install.join("0.4.1"), &install));
        assert!(runs_from(&install.join("0.4.1-bdfbbcec"), &install));
        assert!(runs_from(&install.join("../app/0.4.1"), &install));
        assert!(!runs_from(&install.join("notes"), &install));
        assert!(!runs_from(&install, &install));
        assert!(!runs_from(&root.path().join("download/0.4.1"), &install));
        assert!(!runs_from(&install.join("0.5.0"), &install), "a folder that does not exist");
    }
}
