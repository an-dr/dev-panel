//! Running child processes: capture for quick queries, streaming for long
//! operations whose output the user watches.

use std::collections::VecDeque;
use std::io::{BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use devpanel_core::ports::Output;
use devpanel_core::{HubError, HubResult};

/// Lines kept to explain a failure.
const TAIL_LINES: usize = 12;

/// Hides the console window of a command that runs in the background.
/// No-op outside Windows.
pub fn prepare_hidden(command: &mut Command) -> &mut Command {
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
    let line = String::from_utf8_lossy(pending);
    let line = line.trim_end();
    if !line.is_empty() {
        emit(line);
    }
    pending.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carriage_returns_and_newlines_both_end_a_line() {
        let mut lines = Vec::new();
        for_each_line(&b"a\r\nprogress 1%\rprogress 2%\nlast"[..], |l| lines.push(l.to_string()));
        assert_eq!(lines, ["a", "progress 1%", "progress 2%", "last"]);
    }
}
