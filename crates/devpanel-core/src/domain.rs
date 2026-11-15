//! Entities and value objects. Pure data plus the rules that belong to it.

mod commits_link;
mod health;
mod just;
mod platform;
mod repo;
mod status;
mod tool;
mod web_url;
mod workspace;

pub use commits_link::{CommitsLink, ResolvedTheme};
pub use health::{Fix, Health};
pub use just::{is_justfile, just_command, parse_just_dump, JustParam, JustRecipe, JUST_DUMP};
pub use platform::Platform;
pub use repo::{repo_name_from_url, NameStyle, Repo, RepoExport, RepoId, RepoList};
pub use status::{CommitSummary, FileChange, Operation, RepoStatus, Submodule, SubmoduleState};
pub use tool::{Launch, Tool, ToolId};
pub use web_url::web_url_from_remote;
pub use workspace::{Settings, Workspace, TOOL_DEFAULTS};
