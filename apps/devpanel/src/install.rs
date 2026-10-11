//! `devpanel-app --install`: copies the build this executable belongs to into
//! the user's install, the same way an update would.
//!
//! The mechanism is bones-upgrader's; this file only picks between its two
//! cases. With nothing installed, the launcher and a first version folder are
//! placed (`install_fresh`). With an install present, the build becomes a
//! new version folder beside the running one (`copy_version_from_dir`), so a
//! running DevPanel is never overwritten and the launcher switches to it on
//! its next start. Only the newest two version folders are kept.
//!
//! Run it from an assembled `dist/app/<version>/` folder (scripts/dist.ps1),
//! where the launcher sits one level up.

use std::error::Error;

pub fn run() -> Result<(), Box<dyn Error>> {
    let identity = bones_upgrader::host_identity();
    let exe = std::env::current_exe()?;
    let source = exe.parent().ok_or("the executable has no folder")?;
    let install_dir = bones_upgrader::default_install_dir(&identity).ok_or("no home directory to install into")?;
    let version = env!("CARGO_PKG_VERSION");

    if install_dir.join(identity.launcher_exe()).is_file() {
        let version_dir = bones_upgrader::copy_version_from_dir(&identity, source, &install_dir, version)?;
        println!("Pushed {} — it starts the next time DevPanel is launched.", version_dir.display());
    } else {
        bones_upgrader::install_fresh(&identity, source, &install_dir, version)?;
        println!("Installed DevPanel {version} to {}", install_dir.display());
    }
    Ok(())
}
