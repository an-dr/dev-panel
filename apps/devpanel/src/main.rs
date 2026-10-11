//! DevPanel: a dashboard for the git repositories you work on.
//!
//! This file is the composition root. It is the one place that knows every
//! concrete type: the infra adapters go into the core [`Hub`], the hub into
//! the [`Controller`], and the controller into the bones [`Dashboard`]
//! module that the engine runs.

// A release build is a desktop app and must not open a console on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod controller;
mod dashboard;
mod install;
mod jobs;
mod page;
mod protocol;

use std::sync::Arc;

use bones_engine::logging::{Level, LogSink, Logger};
use devpanel_core::domain::Platform;
use devpanel_core::Hub;

use controller::Controller;
use dashboard::Dashboard;

fn main() {
    let result = match std::env::args().nth(1).as_deref() {
        Some("--install") => install::run(),
        _ => run(),
    };
    if let Err(error) = result {
        eprintln!("fatal: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    prefer_x11();
    let logger = Logger::new(Arc::new(ConsoleSink { debug: std::env::var_os("DEVPANEL_DEBUG").is_some() }));
    refresh_launcher(&logger);
    let hub = Arc::new(Hub::open(devpanel_infra::system_ports(), Platform::current())?);
    let dashboard = Dashboard::new(Controller::new(hub), logger.clone(), page::html());

    bones_engine::Engine::new()
        .logger(logger)
        .window("DevPanel", 1280, 820)
        .min_window_size(760, 520)
        .web()
        .os()
        .module(dashboard)
        .run()?;
    Ok(())
}

/// Installs the launcher that shipped with this version as the entry point
/// when it differs from the installed one: the launcher cannot replace
/// itself, so the app does it. Outside an install this is a no-op, and a
/// failure leaves the old launcher working, so it is only logged.
fn refresh_launcher(logger: &Logger) {
    match bones_upgrader::refresh_launcher_from_running_exe(&bones_upgrader::host_identity()) {
        Ok(true) => logger.info("devpanel", "installed this version's launcher as the entry point"),
        Ok(false) => {}
        Err(error) => logger.warn("devpanel", &format!("could not refresh the launcher: {error}")),
    }
}

/// Linux web panels are GTK webviews embedded in the SDL window, which only
/// works on X11 (Xwayland on a Wayland desktop), so both SDL and GTK are
/// pointed at it — overriding a session-wide `GDK_BACKEND=wayland`, which
/// would otherwise make GTK refuse to start. Programs DevPanel launches still
/// get the user's original values.
fn prefer_x11() {
    #[cfg(target_os = "linux")]
    devpanel_infra::override_own_env(&[("SDL_VIDEO_DRIVER", "x11"), ("GDK_BACKEND", "x11")]);
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
