//! Running child processes: capture for quick queries, streaming for long
//! operations whose output the user watches.

use std::collections::VecDeque;
use std::io::{BufReader, Read};
use std::process::{Command, Stdio};
use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};

use devpanel_core::ports::Output;
use devpanel_core::{HubError, HubResult};

/// Lines kept to explain a failure.
const TAIL_LINES: usize = 12;

/// Environment variables this process changed for itself, with the values
/// they had before, so children can be given the originals back.
static ORIGINAL_ENV: OnceLock<Vec<(String, Option<OsString>)>> = OnceLock::new();

/// Sets environment variables for this process only: every child started
/// through this crate sees the values from before the call. Must run before
/// any other thread starts.
///
/// The host needs X11 for its web panel even on a Wayland desktop, but an
/// editor it opens should start the way it would from the user's own shell.
pub fn override_own_env(vars: &[(&str, &str)]) {
    let originals = vars.iter().map(|(name, _)| (name.to_string(), std::env::var_os(name))).collect();
    if ORIGINAL_ENV.set(originals).is_err() {
        return;
    }
    for (name, value) in vars {
        std::env::set_var(name, value);
    }
}

/// The variables [`override_own_env`] changed, with their values from before.
pub(crate) fn original_env() -> &'static [(String, Option<OsString>)] {
    ORIGINAL_ENV.get().map(Vec::as_slice).unwrap_or_default()
}

/// Readies a background child: the user's original environment, and on
/// Windows no console window flashing up.
pub fn prepare_hidden(command: &mut Command) -> &mut Command {
    for (name, value) in original_env() {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Runs `command` and returns its trimmed stdout, or `None` on any failure.
pub fn capture(mut command: Command) -> Option<String> {
    let output = prepare_hidden(&mut command).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim_end().to_string())
}

/// Runs `command` and returns its stdout. A failure carries what the command
/// wrote to stderr, or says it could not start.
pub fn capture_stdout(mut command: Command) -> HubResult<String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = prepare_hidden(&mut command)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| HubError::Command(format!("could not start {program}: {error}")))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let reason = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(HubError::Command(if reason.is_empty() { format!("{program} exited with {}", output.status) } else { reason }))
}

/// Runs `command` to completion, forwarding stdout and stderr to `out` line
/// by line. Progress output that redraws with `\r` is split there too, so a
/// clone reports progress instead of one line at the end.
pub fn stream(mut command: Command, out: &dyn Output) -> HubResult<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let mut child = prepare_hidden(&mut command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| HubError::Command(format!("could not start {program}: {error}")))?;

    let tail = Mutex::new(VecDeque::with_capacity(TAIL_LINES));
    let forward = |line: &str| {
        out.line(line);
        let mut tail = tail.lock().unwrap();
        if tail.len() == TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(line.to_string());
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    std::thread::scope(|scope| {
        if let Some(stdout) = stdout {
            scope.spawn(|| for_each_line(stdout, &forward));
        }
        if let Some(stderr) = stderr {
            scope.spawn(|| for_each_line(stderr, &forward));
        }
    });

    let status = child.wait().map_err(|error| HubError::Command(error.to_string()))?;
    if status.success() {
        return Ok(());
    }
    let tail: Vec<String> = tail.into_inner().unwrap().into_iter().collect();
    let reason = match tail.last() {
        Some(_) => tail.join("\n"),
        None => format!("{program} exited with {status}"),
    };
    Err(HubError::Command(reason))
}

fn for_each_line(source: impl Read, mut emit: impl FnMut(&str)) {
    let mut reader = BufReader::new(source);
    let mut buffer = [0u8; 4096];
    let mut pending = Vec::new();
    while let Ok(read) = reader.read(&mut buffer) {
        if read == 0 {
            break;
        }
        for &byte in &buffer[..read] {
            if byte == b'\n' || byte == b'\r' {
                flush(&mut pending, &mut emit);
            } else {
                pending.push(byte);
            }
        }
    }
    flush(&mut pending, &mut emit);
}

fn flush(pending: &mut Vec<u8>, emit: &mut impl FnMut(&str)) {
    if pending.is_empty() {
        return;
    }
    let line = strip_ansi(&String::from_utf8_lossy(pending));
    let line = line.trim_end();
    if !line.is_empty() {
        emit(line);
    }
    pending.clear();
}

/// Removes terminal escape sequences (colours, cursor moves, window titles)
/// that programs such as pwsh, cargo and just write even into a pipe, so the
/// log shows only their text.
fn strip_ansi(text: &str) -> String {
    const ESC: char = '\x1b';
    const BEL: char = '\x07';
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != ESC {
            out.push(c);
            continue;
        }
        match chars.next() {
            // CSI: parameters and intermediates up to a final byte in @..~.
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            // OSC: up to BEL or ESC \.
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == BEL || (c == ESC && chars.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                }
            }
            // Character-set selection: ESC ( B and the like take one more character.
            Some('(' | ')' | '*' | '+') => {
                chars.next();
            }
            // Any other two-character escape.
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_codes_are_removed_from_output() {
        let mut lines = Vec::new();
        let pwsh = b"\x1b[31;1mRemove-Item: \x1b[0mdist.ps1:44\x1b[0m\n\x1b]0;title\x07done\x1b(B";
        for_each_line(&pwsh[..], |l| lines.push(l.to_string()));
        assert_eq!(lines, ["Remove-Item: dist.ps1:44", "done"]);
    }

    #[test]
    fn carriage_returns_and_newlines_both_end_a_line() {
        let mut lines = Vec::new();
        for_each_line(&b"a\r\nprogress 1%\rprogress 2%\nlast"[..], |l| lines.push(l.to_string()));
        assert_eq!(lines, ["a", "progress 1%", "progress 2%", "last"]);
    }
}
