//! What the core needs from the outside world. `devpanel-infra` implements
//! each trait; tests implement them with fakes.
//!
//! Every port is `Send + Sync` because the presentation runs use cases on
//! worker threads so that a slow `git pull` never freezes the window.

use std::path::{Path, PathBuf};

use crate::domain::{Repo, RepoList, RepoStatus, Tool, Workspace};
use crate::HubResult;

/// Receives the output of a long-running command line by line, so the user
/// can watch a clone or an install as it happens.
pub trait Output: Send + Sync {
    fn line(&self, text: &str);
}

/// An [`Output`] that discards everything.
pub struct Silent;

impl Output for Silent {
    fn line(&self, _text: &str) {}
}

pub trait Git: Send + Sync {
    /// Never fails: a missing folder or a non-repository is a status, not an
    /// error.
    fn status(&self, path: &Path) -> RepoStatus;
    fn clone_repo(&self, url: &str, dest: &Path, out: &dyn Output) -> HubResult<()>;
    fn fetch(&self, path: &Path, out: &dyn Output) -> HubResult<()>;
    fn pull(&self, path: &Path, rebase: bool, out: &dyn Output) -> HubResult<()>;
    /// With `set_upstream`, pushes `HEAD` to `origin` and tracks it.
    fn push(&self, path: &Path, set_upstream: bool, out: &dyn Output) -> HubResult<()>;
    fn commit_all(&self, path: &Path, message: &str, out: &dyn Output) -> HubResult<()>;
    fn stash(&self, path: &Path, out: &dyn Output) -> HubResult<()>;
}

/// Runs a command line through the platform shell.
pub trait Shell: Send + Sync {
    fn run(&self, command_line: &str, cwd: &Path, out: &dyn Output) -> HubResult<()>;
}

/// Starts a tool for a repository and returns without waiting for it.
pub trait Launcher: Send + Sync {
    /// `extra_args` follow the tool's own arguments; only a program tool
    /// takes them.
    fn launch(&self, tool: &Tool, path: &Path, extra_args: &[String]) -> HubResult<()>;
    /// Whether `tool` could start: a program tool's program resolves.
    /// Starts nothing.
    fn is_available(&self, tool: &Tool) -> bool;
}

pub trait FileSystem: Send + Sync {
    /// Resolves `~` and makes the path absolute. Pure path arithmetic.
    fn expand(&self, path: &Path) -> PathBuf;
    fn exists(&self, path: &Path) -> bool;
    fn is_empty_dir(&self, path: &Path) -> bool;
    /// Files under `root` down to `depth` levels, relative and `/`-separated.
    fn list_files(&self, root: &Path, depth: u32) -> Vec<String>;
    /// Working trees under `root` down to `depth` levels, `root` included.
    /// Does not descend into a repository once found.
    fn find_repos(&self, root: &Path, depth: u32) -> Vec<PathBuf>;
    /// Reads an exported repository list from `path`, failing if the file is
    /// missing or is not a valid list. The storage format is the adapter's
    /// concern, not the core's.
    fn read_repo_list(&self, path: &Path) -> HubResult<RepoList>;
    /// Writes a repository list to `path`, creating parent folders as needed.
    fn write_repo_list(&self, path: &Path, list: &RepoList) -> HubResult<()>;
}

/// Keeps a [`RepoWatch`] watch alive; dropping it stops the watch.
pub struct WatchHandle(Option<Box<dyn Send>>);

impl WatchHandle {
    /// A live watch, stopped when `guard` is dropped.
    pub fn new(guard: impl Send + 'static) -> Self {
        Self(Some(Box::new(guard)))
    }

    /// No watch: the repository could not be watched.
    pub fn inactive() -> Self {
        Self(None)
    }

    pub fn is_active(&self) -> bool {
        self.0.is_some()
    }
}

/// Tells DevPanel that a repository changed outside it.
pub trait RepoWatch: Send + Sync {
    /// Calls `on_change`, on a thread of the adapter's, after the git state
    /// of the repository at `path` changed and settled. A repository that
    /// cannot be watched gets an inactive handle; the focus and interval
    /// refreshes still cover it.
    fn watch(&self, path: &Path, on_change: Box<dyn Fn() + Send>) -> WatchHandle;
}

/// Offers the tracked repositories to another program, such as Commits'
/// repository selector. A failure here never fails the change that caused it.
pub trait RepoSource: Send + Sync {
    fn publish(&self, repos: &[Repo]) -> HubResult<()>;
}

pub trait Store: Send + Sync {
    fn load(&self) -> HubResult<Workspace>;
    fn save(&self, workspace: &Workspace) -> HubResult<()>;
}
