//! Entities and value objects. Pure data plus the rules that belong to it.

mod health;
mod install;
mod repo;
mod status;
mod tool;
mod workspace;

pub use health::{Fix, Health};
pub use install::{detect_install_recipes, InstallRecipe, Platform};
pub use repo::{repo_name_from_url, Repo, RepoExport, RepoId, RepoList};
pub use status::{CommitSummary, FileChange, Operation, RepoStatus};
pub use tool::{Launch, Tool, ToolId};
pub use workspace::{Settings, Workspace};
