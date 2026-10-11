//! Starting tools: GUI programs, terminals and the file manager.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use devpanel_core::domain::{Launch, Tool};
use devpanel_core::ports::Launcher;
use devpanel_core::{HubError, HubResult};

mod terminal;

/// Starts tools as independent processes: closing DevPanel never closes the
/// editor it opened.
pub struct SystemLauncher;

impl Launcher for SystemLauncher {
    fn launch(&self, tool: &Tool, path: &Path) -> HubResult<()> {
        let command = match &tool.launch {
            Launch::Program { program, args } => program_command(program, args, path)?,
            Launch::Terminal { command } => terminal::command(path, command.as_deref().filter(|c| !c.trim().is_empty()))?,
            Launch::FileManager => file_manager_command(path),
        };
        spawn_detached(command, &tool.name)
    }
}

fn program_command(program: &str, args: &[String], path: &Path) -> HubResult<Command> {
    let resolved = resolve_program(program)
        .ok_or_else(|| HubError::Invalid(format!("`{program}` was not found; check the tool in Settings")))?;
    let args = Tool::expand_args(args, &path.to_string_lossy());
    let mut command = if is_batch_file(&resolved) {
        // Windows runs `.cmd` shims such as VS Code's `code.cmd` only
        // through the command interpreter.
        let mut command = Command::new("cmd");
        command.arg("/C").arg(&resolved);
        crate::process::prepare_hidden(&mut command);
        command
    } else {
        Command::new(&resolved)
    };
    command.args(args).current_dir(path);
    Ok(command)
}

fn file_manager_command(path: &Path) -> Command {
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let mut command = Command::new(opener);
    command.arg(path);
    command
}

fn spawn_detached(mut command: Command, name: &str) -> HubResult<()> {
    crate::process::prepare(&mut command);
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Own process group: a Ctrl+C or signal aimed at DevPanel stays with it.
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|e| HubError::Command(format!("could not start {name}: {e}")))?;
    // Reaped in the background so finished children do not linger as zombies.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn is_batch_file(path: &Path) -> bool {
    cfg!(windows)
        && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
}

/// Finds `program` the way a shell would: `~` expanded, an explicit path
/// taken as-is, a bare name searched on `PATH` — with `PATHEXT` extensions
/// on Windows, so `code` finds `code.cmd`.
pub(crate) fn resolve_program(program: &str) -> Option<PathBuf> {
    let program = program.trim();
    if program.is_empty() {
        return None;
    }
    let expanded = match program.strip_prefix("~/").or_else(|| program.strip_prefix("~\\")) {
        Some(rest) => dirs::home_dir()?.join(rest),
        None => PathBuf::from(program),
    };
    let extensions = executable_extensions();
    let with_extension = |base: &Path| -> Option<PathBuf> {
        extensions.iter().map(|ext| append_extension(base, ext)).find(|candidate| candidate.is_file())
    };
    if expanded.components().count() > 1 || expanded.is_absolute() {
        return with_extension(&expanded);
    }
    std::env::split_paths(&std::env::var_os("PATH")?).find_map(|dir| with_extension(&dir.join(&expanded)))
}

fn executable_extensions() -> Vec<String> {
    if !cfg!(windows) {
        return vec![String::new()];
    }
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    std::iter::once(String::new()).chain(pathext.split(';').map(str::to_lowercase)).collect()
}

fn append_extension(base: &Path, ext: &str) -> PathBuf {
    let mut name = base.as_os_str().to_owned();
    name.push(ext);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_programs_on_path_and_by_absolute_path() {
        let git = resolve_program("git").expect("git is on PATH");
        assert!(git.is_file());
        assert_eq!(resolve_program(&git.to_string_lossy()), Some(git));
    }

    #[test]
    fn unknown_programs_are_not_found() {
        assert_eq!(resolve_program("devpanel-no-such-program"), None);
        assert_eq!(resolve_program("  "), None);
        assert_eq!(resolve_program("~/devpanel-no-such/program"), None);
    }
}
