//! Adapters that implement `devpanel-core`'s ports against the real machine:
//! the `git` CLI, the platform shell, terminals and editors, the filesystem,
//! and a JSON file for state.
//!
//! Git is driven through its command line rather than a library binding so
//! that DevPanel sees exactly what the user's own `git` sees — the same
//! credential helpers, hooks, config and version.

mod fs;
mod git_cli;
mod launcher;
mod process;
mod shell;
mod store;

pub use fs::LocalFileSystem;
pub use process::override_own_env;
pub use git_cli::GitCli;
pub use launcher::SystemLauncher;
pub use shell::SystemShell;
pub use store::JsonStore;

use std::sync::Arc;

use devpanel_core::hub::Ports;

/// The production wiring of every port.
pub fn system_ports() -> Ports {
    Ports {
        git: Arc::new(GitCli),
        shell: Arc::new(SystemShell),
        launcher: Arc::new(SystemLauncher),
        fs: Arc::new(LocalFileSystem),
        store: Arc::new(JsonStore::default_location()),
    }
}
