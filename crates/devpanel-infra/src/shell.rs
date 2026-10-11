use std::path::Path;
use std::process::Command;

use devpanel_core::ports::{Output, Shell};
use devpanel_core::HubResult;

use crate::process::stream;

/// `sh -c` on Unix, `cmd /C` on Windows: the shell an install script's
/// author expects on that platform.
pub struct SystemShell;

impl Shell for SystemShell {
    fn run(&self, command_line: &str, cwd: &Path, out: &dyn Output) -> HubResult<()> {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.arg("/C").arg(command_line);
            command
        } else {
            let mut command = Command::new("sh");
            command.arg("-c").arg(command_line);
            command
        };
        command.current_dir(cwd);
        stream(command, out)
    }
}
