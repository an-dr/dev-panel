//! DevPanel's installation identity, release source and desktop entry: what
//! the bones upgrade and desktop modules install, update and register.
//! Supplied here, at composition, because bones reads none of it itself.

use std::path::PathBuf;

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
