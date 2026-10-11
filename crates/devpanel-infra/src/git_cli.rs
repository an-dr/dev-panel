use std::path::{Path, PathBuf};
use std::process::Command;

use devpanel_core::domain::{Operation, RepoStatus};
use devpanel_core::ports::{Git, Output};
use devpanel_core::{HubError, HubResult};

use crate::process::{capture, stream};

mod porcelain;

/// [`Git`] over the user's own `git` executable.
pub struct GitCli;

impl GitCli {
    /// A git invocation that can never stop to ask a question: there is no
    /// terminal to answer it, so a prompt would hang the job forever.
    /// Credential helpers still work, because they do not need the terminal.
    fn git(path: &Path) -> Command {
        let mut command = Command::new("git");
        command.arg("-C").arg(path).args(["-c", "color.ui=false"]);
        command.env("GIT_TERMINAL_PROMPT", "0");
        if std::env::var_os("GIT_SSH_COMMAND").is_none() {
            command.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
        }
        command
    }

    fn run(path: &Path, args: &[&str], out: &dyn Output) -> HubResult<()> {
        let mut command = Self::git(path);
        command.args(args);
        out.line(&format!("$ git {}", args.join(" ")));
        stream(command, out)
    }

    fn query(path: &Path, args: &[&str]) -> Option<String> {
        let mut command = Self::git(path);
        command.args(args);
        capture(command)
    }

    /// `origin`, else the first configured remote, with its URL.
    fn remote(path: &Path) -> Option<(String, String)> {
        let config = Self::query(path, &["config", "--get-regexp", r"^remote\..*\.url$"])?;
        porcelain::pick_remote(&config)
    }

    fn operation(git_dir: &Path) -> Option<Operation> {
        let has = |name: &str| git_dir.join(name).exists();
        if has("rebase-merge") || has("rebase-apply") {
            Some(Operation::Rebase)
        } else if has("MERGE_HEAD") {
            Some(Operation::Merge)
        } else if has("CHERRY_PICK_HEAD") {
            Some(Operation::CherryPick)
        } else if has("REVERT_HEAD") {
            Some(Operation::Revert)
        } else if has("BISECT_LOG") {
            Some(Operation::Bisect)
        } else {
            None
        }
    }
}

impl Git for GitCli {
    fn status(&self, path: &Path) -> RepoStatus {
        if !path.is_dir() {
            return RepoStatus::missing();
        }
        // A subfolder of some other repository is not itself a repository,
        // so the reported top level must be the folder we were given.
        let Some(dirs) = Self::query(path, &["rev-parse", "--absolute-git-dir", "--show-toplevel"]) else {
            return RepoStatus::not_a_repo();
        };
        let mut dirs = dirs.lines();
        let (Some(git_dir), Some(top)) = (dirs.next(), dirs.next()) else {
            return RepoStatus::not_a_repo();
        };
        if !same_path(Path::new(top), path) {
            return RepoStatus::not_a_repo();
        }

        let mut status = RepoStatus { exists: true, is_repo: true, ..RepoStatus::default() };
        if let Some(text) = Self::query(path, &["status", "--porcelain=v2", "--branch", "--show-stash"]) {
            porcelain::apply_status(&text, &mut status);
        }
        status.operation = Self::operation(Path::new(git_dir));
        status.remote_url = Self::remote(path).map(|(_, url)| url);
        if status.has_commits {
            status.last_commit = Self::query(path, &["log", "-1", "--format=%h%x1f%s%x1f%an%x1f%ct"])
                .and_then(|line| porcelain::parse_commit(&line));
        }
        status
    }

    fn clone_repo(&self, url: &str, dest: &Path, out: &dyn Output) -> HubResult<()> {
        let parent = dest.parent().ok_or_else(|| HubError::Invalid("invalid destination".into()))?;
        std::fs::create_dir_all(parent).map_err(|e| HubError::Command(format!("creating {}: {e}", parent.display())))?;
        let dest = dest.to_string_lossy();
        Self::run(parent, &["clone", "--progress", url, &dest], out)
    }

    fn fetch(&self, path: &Path, out: &dyn Output) -> HubResult<()> {
        Self::run(path, &["fetch", "--all", "--prune", "--progress"], out)
    }

    fn pull(&self, path: &Path, rebase: bool, out: &dyn Output) -> HubResult<()> {
        // A plain pull only fast-forwards: anything else needs a decision
        // the dashboard should not take silently.
        let mode = if rebase { "--rebase" } else { "--ff-only" };
        Self::run(path, &["pull", mode, "--progress"], out)
    }

    fn push(&self, path: &Path, set_upstream: bool, out: &dyn Output) -> HubResult<()> {
        if !set_upstream {
            return Self::run(path, &["push", "--progress"], out);
        }
        let (remote, _) = Self::remote(path).ok_or_else(|| HubError::Invalid("the repository has no remote".into()))?;
        Self::run(path, &["push", "--progress", "-u", &remote, "HEAD"], out)
    }

    fn commit_all(&self, path: &Path, message: &str, out: &dyn Output) -> HubResult<()> {
        Self::run(path, &["add", "--all"], out)?;
        Self::run(path, &["commit", "-m", message], out)
    }

    fn stash(&self, path: &Path, out: &dyn Output) -> HubResult<()> {
        Self::run(path, &["stash", "push", "--include-untracked"], out)
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    let canonical = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p));
    canonical(a) == canonical(b)
}

#[cfg(test)]
mod tests;
