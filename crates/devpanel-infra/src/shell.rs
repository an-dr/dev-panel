use std::path::Path;
use std::process::Command;

use devpanel_core::ports::{Output, Shell};
use devpanel_core::HubResult;

use crate::process::{capture_stdout, stream};

/// `sh -c` on Unix, `cmd /C` on Windows: the shell an install script's
/// author expects on that platform.
pub struct SystemShell;

impl Shell for SystemShell {
    fn run(&self, command_line: &str, cwd: &Path, out: &dyn Output) -> HubResult<()> {
        let mut command = platform_shell(command_line);
        command.current_dir(cwd);
        stream(command, out)
    }

    fn capture(&self, command_line: &str, cwd: &Path) -> HubResult<String> {
        let mut command = platform_shell(command_line);
        command.current_dir(cwd);
        capture_stdout(command)
    }
}

fn platform_shell(command_line: &str) -> Command {
    if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(command_line);
        command
    } else {
        let mut command = Command::new("sh");
        command.arg("-c").arg(command_line);
        command
    }
}
