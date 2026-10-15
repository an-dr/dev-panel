//! Starting tools: GUI programs, terminals and the file manager.
//!
//! The mechanism is bones' `bones-kernel-launch`: resolving a program the way
//! a shell would, starting it detached, and the table of terminals. This
//! adapter only turns a [`Tool`] into one of its calls.

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
        }
    }

    fn is_available(&self, tool: &Tool) -> bool {
        match &tool.launch {
            Launch::Program { program, .. } => bones_kernel_launch::resolve(program).is_some(),
            Launch::Terminal { .. } | Launch::FileManager => true,
        }
    }
}

fn start(program: &str, args: Vec<String>, cwd: &Path, name: &str) -> HubResult<()> {
    let program = Program { program: program.to_string(), args, cwd: cwd.to_path_buf(), env: Vec::new() };
    match bones_kernel_launch::launch(&program).map_err(HubError::Command)? {
        Launched::Started => Ok(()),
        Launched::NotFound { searched } => {
            Err(HubError::Invalid(format!("`{searched}` was not found; check the {name} tool in Settings")))
        }
    }
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
