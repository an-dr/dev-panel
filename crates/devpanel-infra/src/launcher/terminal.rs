//! Opening a terminal at a folder, optionally running a command in it.
//!
//! Every platform and every terminal spells this differently, so this is a
//! table of known spellings rather than a general mechanism. When nothing in
//! the table is installed the error says so instead of guessing.

use std::path::Path;
use std::process::Command;

use devpanel_core::{HubError, HubResult};

use super::resolve_program;

pub fn command(path: &Path, run: Option<&str>) -> HubResult<Command> {
    if cfg!(windows) {
        Ok(windows(path, run))
    } else if cfg!(target_os = "macos") {
        Ok(macos(path, run))
    } else {
        linux(path, run)
    }
}

fn windows(path: &Path, run: Option<&str>) -> Command {
    let mut command;
    if resolve_program("wt").is_some() {
        command = Command::new("wt");
        command.arg("-d").arg(path);
        if let Some(run) = run {
            command.args(["cmd", "/K", run]);
        }
    } else {
        command = Command::new("cmd");
        command.args(["/C", "start", "", "/D"]).arg(path).arg("cmd");
        if let Some(run) = run {
            command.args(["/K", run]);
        }
    }
    command
}

fn macos(path: &Path, run: Option<&str>) -> Command {
    let mut command;
    match run {
        Some(run) => {
            let script = format!("cd {} && {run}", sh_quote(&path.to_string_lossy()));
            command = Command::new("osascript");
            command
                .arg("-e")
                .arg(format!("tell application \"Terminal\" to do script \"{}\"", applescript_escape(&script)))
                .args(["-e", "tell application \"Terminal\" to activate"]);
        }
        None => {
            command = Command::new("open");
            command.args(["-a", "Terminal"]).arg(path);
        }
    }
    command
}

/// How one Linux terminal takes a working directory and a command.
struct Known {
    program: &'static str,
    dir_flag: &'static [&'static str],
    exec_flag: &'static [&'static str],
}

/// Order is preference when several are installed and `$TERMINAL` names none.
const KNOWN: &[Known] = &[
    Known { program: "cosmic-term", dir_flag: &["-w"], exec_flag: &["-e"] },
    Known { program: "gnome-terminal", dir_flag: &["--working-directory"], exec_flag: &["--"] },
    Known { program: "ptyxis", dir_flag: &["--new-window", "--working-directory"], exec_flag: &["--"] },
    Known { program: "konsole", dir_flag: &["--workdir"], exec_flag: &["-e"] },
    Known { program: "kitty", dir_flag: &["--directory"], exec_flag: &[] },
    Known { program: "alacritty", dir_flag: &["--working-directory"], exec_flag: &["-e"] },
    Known { program: "wezterm", dir_flag: &["start", "--cwd"], exec_flag: &["--"] },
    Known { program: "foot", dir_flag: &["-D"], exec_flag: &[] },
    Known { program: "xfce4-terminal", dir_flag: &["--working-directory"], exec_flag: &["-x"] },
    // No directory flag: it inherits the working directory set on spawn.
    Known { program: "xterm", dir_flag: &[], exec_flag: &["-e"] },
];

fn linux(path: &Path, run: Option<&str>) -> HubResult<Command> {
    let preferred = std::env::var("TERMINAL").ok().and_then(|t| basename(&t));
    let alternative = resolve_program("x-terminal-emulator")
        .and_then(|p| std::fs::canonicalize(p).ok())
        .and_then(|p| basename(&p.to_string_lossy()));
    let candidates = preferred.iter().chain(alternative.iter()).map(String::as_str).chain(KNOWN.iter().map(|k| k.program));
    let known = candidates
        .filter_map(|name| KNOWN.iter().find(|k| k.program == name))
        .find(|k| resolve_program(k.program).is_some())
        .ok_or_else(|| HubError::Invalid("no supported terminal emulator was found; set $TERMINAL".into()))?;

    let mut command = Command::new(known.program);
    command.current_dir(path);
    if !known.dir_flag.is_empty() {
        command.args(known.dir_flag).arg(path);
    }
    if let Some(run) = run {
        // Leave the user in a shell after the command exits, so the
        // terminal does not vanish with its output.
        command.args(known.exec_flag).args(["sh", "-c", &format!("{run}; exec \"${{SHELL:-sh}}\"")]);
    }
    Ok(command)
}

fn basename(path: &str) -> Option<String> {
    Path::new(path.trim()).file_name().map(|n| n.to_string_lossy().into_owned())
}

fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn applescript_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_survives_awkward_paths() {
        assert_eq!(sh_quote("/a b/it's"), r"'/a b/it'\''s'");
        assert_eq!(applescript_escape(r#"cd "x" \ y"#), r#"cd \"x\" \\ y"#);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_terminal_runs_the_command_then_a_shell() {
        if KNOWN.iter().all(|k| resolve_program(k.program).is_none()) {
            return; // A headless CI box has no terminal to find.
        }
        let command = linux(Path::new("/tmp"), Some("claude")).unwrap();
        let args: Vec<_> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(args.contains(&"sh".to_string()), "{args:?}");
        assert!(args.last().unwrap().starts_with("claude; exec"), "{args:?}");
    }
}
