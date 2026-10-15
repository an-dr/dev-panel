//! Noticing that a repository changed outside DevPanel: a commit in a
//! terminal, a checkout in an editor.
//!
//! The mechanism is bones' `bones-kernel-watch`; this adapter decides what a
//! change means. Only git's own state is watched — the top of `.git`, where
//! `index` and `HEAD` live, and `.git/refs` — so editing a file in the
//! working tree costs nothing until git sees it.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bones_kernel_watch::{Change, WatchId, Watcher};
use devpanel_core::ports::{RepoWatch, WatchHandle};

/// How long `.git` must be quiet before a change is reported: one commit or
/// checkout writes several files, and they should cost one status read.
const QUIET: Duration = Duration::from_millis(500);

type Callback = Arc<Mutex<Box<dyn Fn() + Send>>>;

/// [`RepoWatch`] over one bones watcher thread shared by every repository.
pub struct GitWatch {
    inner: Option<Arc<Inner>>,
}

struct Inner {
    watcher: Mutex<Watcher>,
    callbacks: Arc<Mutex<HashMap<WatchId, Callback>>>,
}

impl GitWatch {
    /// Without a watcher thread — the system refused one — every watch is
    /// inactive and DevPanel falls back to its focus and interval refreshes.
    pub fn new() -> Self {
        let callbacks: Arc<Mutex<HashMap<WatchId, Callback>>> = Arc::default();
        let sinks = Arc::clone(&callbacks);
        let watcher = Watcher::new(QUIET, move |id, change| {
            if !is_git_state(&change) {
                return;
            }
            let callback = sinks.lock().unwrap().get(&id).cloned();
            if let Some(callback) = callback {
                (callback.lock().unwrap())();
            }
        });
        let inner = watcher.ok().map(|watcher| Arc::new(Inner { watcher: Mutex::new(watcher), callbacks }));
        Self { inner }
    }
}

impl Default for GitWatch {
    fn default() -> Self {
        Self::new()
    }
}

impl RepoWatch for GitWatch {
    fn watch(&self, path: &Path, on_change: Box<dyn Fn() + Send>) -> WatchHandle {
        let Some(inner) = &self.inner else { return WatchHandle::inactive() };
        // A worktree or submodule has a `.git` file pointing elsewhere; it is
        // left to the other refreshes rather than followed.
        let git_dir = path.join(".git");
        if !git_dir.is_dir() {
            return WatchHandle::inactive();
        }
        let callback: Callback = Arc::new(Mutex::new(on_change));
        let mut ids = Vec::new();
        {
            let mut watcher = inner.watcher.lock().unwrap();
            for (dir, recursive) in [(git_dir.clone(), false), (git_dir.join("refs"), true)] {
                if let Ok(id) = watcher.watch(&dir, recursive) {
                    inner.callbacks.lock().unwrap().insert(id, Arc::clone(&callback));
                    ids.push(id);
                }
            }
        }
        if ids.is_empty() {
            return WatchHandle::inactive();
        }
        WatchHandle::new(Unwatch { inner: Arc::clone(inner), ids })
    }
}

/// Stops a repository's watches when its [`WatchHandle`] is dropped.
struct Unwatch {
    inner: Arc<Inner>,
    ids: Vec<WatchId>,
}

impl Drop for Unwatch {
    fn drop(&mut self) {
        let mut watcher = self.inner.watcher.lock().unwrap();
        for id in self.ids.drain(..) {
            watcher.unwatch(id);
            self.inner.callbacks.lock().unwrap().remove(&id);
        }
    }
}

/// Lock files come and go around every git command, including the reads of
/// other tools; on their own they change nothing a status would show.
fn is_git_state(change: &Change) -> bool {
    change.paths.iter().any(|path| path.extension().is_none_or(|ext| ext != "lock"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::mpsc;

    use bones_kernel_watch::ChangeKind;

    use super::*;

    #[test]
    fn lock_files_alone_are_not_a_change() {
        let change = |paths: &[&str]| Change {
            paths: paths.iter().map(PathBuf::from).collect(),
            kinds: vec![ChangeKind::Modified],
        };
        assert!(!is_git_state(&change(&["/r/.git/index.lock"])));
        assert!(is_git_state(&change(&["/r/.git/index.lock", "/r/.git/index"])));
        assert!(is_git_state(&change(&["/r/.git/HEAD"])));
    }

    #[test]
    fn a_commit_is_reported_and_a_dropped_handle_reports_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let ok = Command::new("git").arg("-C").arg(dir.path()).args(args).status().unwrap().success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        let watch = GitWatch::new();
        let (sender, changes) = mpsc::channel();
        // Kept here as well, so the channel outlives the handle and silence
        // after the drop means no call rather than a closed channel.
        let _keep = sender.clone();
        let sender = Mutex::new(sender);
        let handle = watch.watch(dir.path(), Box::new(move || {
            let _ = sender.lock().unwrap().send(());
        }));

        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "one"]);
        changes.recv_timeout(Duration::from_secs(10)).expect("a commit is a change");

        drop(handle);
        while changes.recv_timeout(QUIET * 2).is_ok() {}
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "two"]);
        assert!(changes.recv_timeout(QUIET * 3).is_err());
    }

    #[test]
    fn a_folder_without_a_git_directory_is_not_watched() {
        let dir = tempfile::tempdir().unwrap();
        let handle = GitWatch::new().watch(dir.path(), Box::new(|| {}));
        assert!(!handle.is_active());
    }
}
