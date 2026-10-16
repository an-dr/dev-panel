//! DevPanel: a dashboard for the git repositories you work on.
//!
//! This file is the composition root. It is the one place that knows every
//! concrete type: the infra adapters go into the core [`Hub`], the hub into
//! the [`Controller`], and the controller into the bones [`Dashboard`]
//! module that the engine runs.

// A release build is a desktop app and must not open a console on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commits_theme;
mod controller;
mod dashboard;
mod install;
mod installation;
mod jobs;
mod page;
mod protocol;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bones_kernel::instance::{self, Claim, Handoff};

use bones_kernel::logging::{Level, LogSink, Logger};
use devpanel_core::domain::Platform;
use devpanel_core::Hub;

use controller::Controller;
use dashboard::Dashboard;

fn main() {
    prefer_x11();
    // An install never claims the instance: it must work while DevPanel runs.
    let result = match std::env::args().nth(1).as_deref() {
        Some("--install") => install::run(),
        _ => claim_and_run(),
    };
    if let Err(error) = result {
        eprintln!("fatal: {error}");
        std::process::exit(1);
    }
}

/// One DevPanel per user: a later `devpanel [<path>]` hands its arguments to
/// the running window and exits.
fn claim_and_run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cwd = std::env::current_dir()?;
    let startup_path = args.first().map(|arg| cwd.join(arg));
    let handoff = Handoff { args, cwd };
    match instance::claim(installation::identity().app_stem, handoff, Duration::from_secs(2)) {
        Ok(Claim::HandedOff) => Ok(()),
        Ok(Claim::Primary(primary)) => run(Some(primary), startup_path),
        // No claim possible, e.g. an unusable runtime folder: a window of its
        // own is still better than none.
        Err(error) => {
            eprintln!("[WARN] devpanel: running without single instance: {error}");
            run(None, startup_path)
        }
    }
}

fn run(primary: Option<instance::Primary>, startup_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let logger = Logger::new(Arc::new(ConsoleSink { debug: std::env::var_os("DEVPANEL_DEBUG").is_some() }));
    refresh_launcher(&logger);
    let hub = Arc::new(Hub::open(devpanel_infra::system_ports(), Platform::current())?);
    let theme_file = commits_theme::write(&devpanel_infra::JsonStore::default_location().dir());
    let controller = Controller::new(hub).with_startup_path(startup_path).with_commits_theme(theme_file);
    let dashboard = Dashboard::new(controller, logger.clone(), page::html());

    let mut kernel = bones_kernel::Kernel::new()
        .logger(logger)
        .window("DevPanel", 1280, 820)
        .min_window_size(760, 520)
        .web()
        .os()
        .desktop(installation::desktop_integration().ok_or("no home folder to install into")?)
        .upgrade(installation::config());
    if let Some(primary) = primary {
        kernel = kernel.single_instance(primary);
    }
    kernel.module(dashboard).run()?;
    Ok(())
}

/// Installs the launcher that shipped with this version as the entry point
/// when it differs from the installed one: the launcher cannot replace
/// itself, so the app does it. Outside an install this is a no-op, and a
/// failure leaves the old launcher working, so it is only logged.
fn refresh_launcher(logger: &Logger) {
    match bones_kernel::upgrade::UpgradeModule::new(installation::config()).refresh_launcher() {
        Ok(true) => logger.info("devpanel", "installed this version's launcher as the entry point"),
        Ok(false) => {}
        Err(error) => logger.warn("devpanel", &format!("could not refresh the launcher: {error}")),
    }
}

/// Linux web panels are GTK webviews inside an X11 window. bones asks GTK
/// for X11, but GTK ignores that when `GDK_BACKEND` is set, as Wayland
/// sessions such as COSMIC set it, and then refuses to start. So DevPanel
/// sets it for itself, before any thread starts; programs it launches get the
/// user's own value back.
///
/// `SDL_APP_ID` names the window's class `devpanel` instead of the
/// executable's `devpanel-app`, so a taskbar or dock matches the window to
/// the `devpanel.desktop` entry that `--install` registers, with its name
/// and icon.
fn prefer_x11() {
    #[cfg(target_os = "linux")]
    devpanel_infra::override_own_env(&[("GDK_BACKEND", "x11"), ("SDL_APP_ID", "devpanel")]);
}

/// Info and above to stderr. The engine logs every frame at debug level,
/// which is noise for an app; `DEVPANEL_DEBUG=1` brings it back.
struct ConsoleSink {
    debug: bool,
}

impl LogSink for ConsoleSink {
    fn log(&self, level: Level, category: &str, message: &str) {
        let tag = match level {
            Level::Debug if !self.debug => return,
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        };
        eprintln!("[{tag}] {category}: {message}");
    }
}
