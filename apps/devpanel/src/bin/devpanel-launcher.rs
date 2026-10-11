//! The launcher: installed at the install root as `devpanel`, the name users
//! run. It starts the app in the newest version folder and exits; everything
//! it does is `bones_upgrader::launcher`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;

use bones_upgrader::launcher::report_error;

fn main() {
    let Some(install_dir) = std::env::current_exe().ok().and_then(|path| path.parent().map(Path::to_path_buf)) else {
        report_error("could not resolve the launcher's own directory\n");
        std::process::exit(1);
    };
    // Arguments belong to the app; the launcher answers only its own flags.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = bones_upgrader::launcher::run(&bones_upgrader::host_identity(), &install_dir, &args) {
        report_error(&format!("{error}\n"));
        std::process::exit(1);
    }
}
