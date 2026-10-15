//! The launcher: installed at the install root as `devpanel`, the name users
//! run. It starts the app in the newest version folder and exits; that part
//! is `bones_launcher`. The one flag it answers itself is `--version`, since
//! bones leaves version reporting to the application.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;

use bones_launcher::report_error;

fn main() {
    let Some(install_dir) = std::env::current_exe().ok().and_then(|path| path.parent().map(Path::to_path_buf)) else {
        report_error("could not resolve the launcher's own directory\n");
        std::process::exit(1);
    };
    // Arguments belong to the app; the launcher answers only its own flags.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--version") {
        print(&format!("devpanel {}\nlauncher {}\n", installed_version(&install_dir), env!("UPGRADER_LAUNCHER_VERSION")));
        return;
    }
    let app = format!("{}{}", env!("UPGRADER_APP_STEM"), std::env::consts::EXE_SUFFIX);
    if let Err(error) = bones_launcher::run(&app, &install_dir, &args) {
        report_error(&format!("{error}\n"));
        std::process::exit(1);
    }
}

/// The version the launcher would start: the newest version folder's name.
fn installed_version(install_dir: &Path) -> String {
    let folders = std::fs::read_dir(install_dir).into_iter().flatten().filter_map(|entry| {
        let entry = entry.ok()?;
        entry.file_type().ok()?.is_dir().then_some(())?;
        Some((entry.path(), entry.metadata().ok()?.modified().ok()?))
    });
    bones_versioning::order_version_folders(folders)
        .first()
        .and_then(|path| bones_versioning::parse_folder_name(path.file_name()?.to_str()?))
        .map(|version| version.to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// A release launcher has no console of its own; on Windows it borrows the
/// caller's, so `devpanel --version` in a terminal prints there.
fn print(text: &str) {
    use std::io::Write;
    #[cfg(windows)]
    {
        extern "system" {
            fn GetStdHandle(which: u32) -> isize;
            fn AttachConsole(process: u32) -> i32;
        }
        // SAFETY: both are plain Win32 calls with no pointer arguments.
        let handle = unsafe { GetStdHandle(-11i32 as u32) };
        if (handle == 0 || handle == -1) && unsafe { AttachConsole(u32::MAX) } != 0 {
            if let Ok(mut console) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
                let _ = console.write_all(text.as_bytes());
                return;
            }
        }
    }
    let _ = std::io::stdout().write_all(text.as_bytes());
}
