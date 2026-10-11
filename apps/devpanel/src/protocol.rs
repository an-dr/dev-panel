//! The JSON vocabulary between the page and the host. Every message is an
//! object with a `type`; requests that expect an answer carry a `request`
//! number that the matching [`Event::Reply`] echoes.

use devpanel_core::domain::{Fix, Health, RepoId, RepoStatus, Settings, Tool, ToolId, Workspace};
use serde::{Deserialize, Serialize};

/// Page → host.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Action {
    /// The page has loaded and wants the full state.
    Ready,
    /// Re-read status; every repository when `repo` is absent.
    Refresh { repo: Option<RepoId> },
    Fetch { repo: Option<RepoId> },
    Pull { repo: RepoId },
    Push { repo: RepoId },
    Sync { repo: RepoId },
    Stash { repo: RepoId },
    Fix { repo: RepoId, fix: Fix, message: Option<String> },
    Commit { repo: RepoId, message: String },
    Open { repo: RepoId, tool: Option<ToolId> },
    AddExisting { path: String },
    Scan { path: String },
    Clone { url: String, parent: Option<String> },
    Remove { repo: RepoId },
    Edit { repo: RepoId, name: String, group: Option<String>, install_command: Option<String> },
    InstallRecipes { request: u64, repo: RepoId },
    /// Runs a shell command in the repository, e.g. an install recipe.
    Run { repo: RepoId, command: String, title: Option<String> },
    SaveSettings { settings: Settings },
    SaveTools { tools: Vec<Tool> },
    PickFolder { request: u64 },
    OpenUrl { url: String },
    Copy { text: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobState {
    Running,
    Succeeded,
    Failed,
}

/// Host → page.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Event {
    Workspace { workspace: Workspace, platform: &'static str },
    Status { repo: RepoId, status: RepoStatus, health: Health, fixes: Vec<Fix> },
    Job { job: u64, repo: Option<RepoId>, title: String, state: JobState, error: Option<String> },
    Log { job: u64, line: String },
    Reply { request: u64, ok: bool, data: serde_json::Value, error: Option<String> },
    Toast { level: &'static str, message: String },
}

impl Event {
    pub fn error(message: impl Into<String>) -> Self {
        Event::Toast { level: "error", message: message.into() }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Event::Toast { level: "info", message: message.into() }
    }

    pub fn status(repo: RepoId, status: RepoStatus, has_remote_url: bool) -> Self {
        let health = Health::of(&status);
        let fixes = Health::fixes(&status, has_remote_url);
        Event::Status { repo, status, health, fixes }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_decode_from_page_json() {
        let action: Action = serde_json::from_str(r#"{"type":"fix","repo":"r1","fix":"commit-all","message":"wip"}"#).unwrap();
        assert!(matches!(action, Action::Fix { fix: Fix::CommitAll, .. }));
        let action: Action = serde_json::from_str(r#"{"type":"refresh"}"#).unwrap();
        assert!(matches!(action, Action::Refresh { repo: None }));
        let action: Action = serde_json::from_str(r#"{"type":"add-existing","path":"~/x"}"#).unwrap();
        assert!(matches!(action, Action::AddExisting { .. }));
    }

    #[test]
    fn events_encode_with_a_type_tag() {
        let json = serde_json::to_value(Event::info("hi")).unwrap();
        assert_eq!(json["type"], "toast");
        assert_eq!(json["message"], "hi");
    }
}
