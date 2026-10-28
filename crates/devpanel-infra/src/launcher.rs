//! Starting tools: GUI programs, terminals and the file manager.
//!
//! The mechanism is bones' `bones-kernel-launch`: resolving a program the way
//! a shell would, starting it detached, and the table of terminals. This
//! adapter only turns a [`Tool`] into one of its calls.

use std::ffi::OsString;
use std::path::Path;

use bones_kernel_launch::{Launched, Program};
use devpanel_core::domain::{Launch, Tool};
use devpanel_core::ports::Launcher;
use devpanel_core::{HubError, HubResult};

/// Starts tools as independent processes: closing DevPanel never closes the
/// editor it opened.
pub struct SystemLauncher;

impl Launcher for SystemLauncher {
    fn launch(&self, tool: &Tool, path: &Path, extra_args: &[String]) -> HubResult<()> {
        match &tool.launch {
            Launch::Program { program, args } => {
                let mut args = Tool::expand_args(args, &path.to_string_lossy());
                args.extend_from_slice(extra_args);
                start(program, args, path, &tool.name)
            }
            Launch::Terminal { command } => {
                let run = command.as_deref().filter(|c| !c.trim().is_empty());
                bones_kernel_launch::open_terminal(path, run).map_err(HubError::Command)
            }
            Launch::FileManager => start(file_manager(), vec![path.to_string_lossy().into_owned()], path, &tool.name),
            Launch::App { app } => start_app(app, path, &tool.name),
        }
    }

    fn is_available(&self, tool: &Tool) -> bool {
        if let Launch::App { app } = &tool.launch {
            return app_installed(app);
        }
        tool.required_program().is_none_or(|program| bones_kernel_launch::resolve(program).is_some())
    }
}

/// Opens a desktop app by its platform id ([`Launch::App`]): through the
/// shell's app folder on Windows, which starts packaged (Store) apps that
/// have no program on `PATH`, and `open -a` on macOS.
fn start_app(app: &str, cwd: &Path, name: &str) -> HubResult<()> {
    if cfg!(windows) {
        start("explorer", vec![format!("shell:AppsFolder\\{app}")], cwd, name)
    } else if cfg!(target_os = "macos") {
        start("open", vec!["-a".into(), app.into()], cwd, name)
    } else {
        Err(HubError::Invalid(format!("the {name} app cannot be opened on this system")))
    }
}

/// Whether the app [`start_app`] would open is installed: on Windows its
/// package has a data folder for this user, on macOS its bundle is in an
/// Applications folder.
fn app_installed(app: &str) -> bool {
    if cfg!(windows) {
        let family = app.split('!').next().unwrap_or_default();
        !family.is_empty() && dirs::data_local_dir().is_some_and(|dir| dir.join("Packages").join(family).is_dir())
    } else if cfg!(target_os = "macos") {
        let bundle = format!("{app}.app");
        let home_apps = dirs::home_dir().map(|home| home.join("Applications").join(&bundle));
        Path::new("/Applications").join(&bundle).is_dir() || home_apps.is_some_and(|path| path.is_dir())
    } else {
        false
    }
}

fn start(program: &str, args: Vec<String>, cwd: &Path, name: &str) -> HubResult<()> {
    let (program, args) = with_original_env(program, args, crate::process::original_env());
    let program = Program { program, args, cwd: cwd.to_path_buf(), env: Vec::new() };
    match bones_kernel_launch::launch(&program).map_err(HubError::Command)? {
        Launched::Started => Ok(()),
        Launched::NotFound { searched } => {
            Err(HubError::Invalid(format!("`{searched}` was not found; check the {name} tool in Settings")))
        }
    }
}

/// Gives a program the environment DevPanel overrode for itself back
/// ([`crate::override_own_env`]) by starting it through `env`: bones' launch
/// can set a variable but not remove one. The program is resolved first, so
/// a missing one is still reported by its own name.
fn with_original_env(program: &str, args: Vec<String>, originals: &[(String, Option<OsString>)]) -> (String, Vec<String>) {
    let resolved = bones_kernel_launch::resolve(program);
    let (Some(resolved), false) = (resolved, originals.is_empty() || cfg!(windows)) else {
        return (program.to_string(), args);
    };
    let mut wrapped = Vec::new();
    for (name, value) in originals {
        if value.is_none() {
            wrapped.extend(["-u".to_string(), name.clone()]);
        }
    }
    for (name, value) in originals {
        if let Some(value) = value {
            wrapped.push(format!("{name}={}", value.to_string_lossy()));
        }
    }
    wrapped.push(resolved.to_string_lossy().into_owned());
    wrapped.extend(args);
    ("env".to_string(), wrapped)
}

fn file_manager() -> &'static str {
    if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn overridden_variables_are_restored_through_env() {
        let originals = [("GDK_BACKEND".to_string(), Some(OsString::from("wayland"))), ("GONE".to_string(), None)];
        let (program, args) = with_original_env("sh", vec!["-c".into(), "true".into()], &originals);
        assert_eq!(program, "env");
        let sh = bones_kernel_launch::resolve("sh").unwrap().to_string_lossy().into_owned();
        assert_eq!(args, ["-u", "GONE", "GDK_BACKEND=wayland", sh.as_str(), "-c", "true"]);
    }

    #[test]
    fn nothing_overridden_starts_the_program_itself() {
        let (program, args) = with_original_env("git", vec!["x".into()], &[]);
        assert_eq!((program.as_str(), args), ("git", vec!["x".to_string()]));
    }

    #[test]
    fn a_missing_program_keeps_its_name_for_the_error() {
        let originals = [("GDK_BACKEND".to_string(), None)];
        let (program, _) = with_original_env("devpanel-no-such-program", Vec::new(), &originals);
        assert_eq!(program, "devpanel-no-such-program");
    }
}
