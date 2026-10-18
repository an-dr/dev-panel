//! The JSON vocabulary between the page and the host. Every message is an
//! object with a `type`; requests that expect an answer carry a `request`
//! number that the matching [`Event::Reply`] echoes.

use devpanel_core::domain::{web_url_from_remote, Fix, Health, NameStyle, RepoId, RepoStatus, ResolvedTheme, Settings, Tool, ToolId, Workspace};
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
    /// Opens the repository in Commits on the view its verdict calls for, in
    /// the theme the page is showing.
    OpenInCommits { repo: RepoId, theme: ResolvedTheme },
    AddExisting { path: String },
    Scan { path: String },
    Clone { url: String, parent: Option<String> },
    Remove { repo: RepoId },
    Edit {
        repo: RepoId,
        name: String,
        group: Option<String>,
        install_command: Option<String>,
        #[serde(default)]
        web_url: Option<String>,
        /// Absent or `null` hands the choice back to the page.
        #[serde(default)]
        name_style: Option<NameStyle>,
    },
    /// Pins a repository as a favourite, or clears the mark.
    Favorite { repo: RepoId, favorite: bool },
    /// Moves a repository into a group, or out of every group when absent.
    SetGroup { repo: RepoId, group: Option<String> },
    /// Writes a portable repository list into `folder`.
    Export { folder: String },
    /// Reads a repository list from `folder` and clones every repository it
    /// lists into `dest` (the configured clone folder when absent).
    Import { folder: String, dest: Option<String> },
    InstallRecipes { request: u64, repo: RepoId },
    /// Runs a shell command in the repository, e.g. an install recipe.
    Run { repo: RepoId, command: String, title: Option<String> },
    SaveSettings { settings: Settings },
    SaveTools { tools: Vec<Tool> },
    PickFolder { request: u64 },
    /// Registers DevPanel with the desktop, removes it, or asks which it is;
    /// answered with a `reply` whose data is whether it is registered.
    Desktop { request: u64, action: DesktopAction },
    /// Reports where DevPanel is installed, or installs the running build as
    /// `devpanel-app --install` does; answered with an installation status,
    /// plus the install's `report` lines after `install`.
    Installation { request: u64, action: InstallationAction },
    /// Looks for a newer release, or downloads it for the next start;
    /// answered with `{available}`, `{staged}` or `{}` when up to date.
    Update { request: u64, action: UpdateAction },
    OpenUrl { url: String },
    Copy { text: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopAction {
    Register,
    Unregister,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallationAction {
    Status,
    Install,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateAction {
    Check,
    Stage,
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
    /// `commits`: whether "Open in Commits" can work right now.
    Workspace { workspace: Workspace, platform: &'static str, version: &'static str, commits: bool },
    Status { repo: RepoId, status: RepoStatus, health: Health, fixes: Vec<Fix>, web_url: Option<String> },
    Job { job: u64, repo: Option<RepoId>, title: String, state: JobState, error: Option<String> },
    Log { job: u64, line: String },
    Reply { request: u64, ok: bool, data: serde_json::Value, error: Option<String> },
    Toast { level: &'static str, message: String },
    /// Select this repository and open its drawer: the result of
    /// `devpanel <path>`, at startup or from a later launch.
    Select { repo: RepoId },
}

impl Event {
    pub fn error(message: impl Into<String>) -> Self {
        Event::Toast { level: "error", message: message.into() }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Event::Toast { level: "info", message: message.into() }
    }

    /// `remote` is the URL DevPanel remembers for the repository, used for
    /// its web page when git reports no remote.
    pub fn status(repo: RepoId, status: RepoStatus, remote: Option<&str>) -> Self {
        let health = Health::of(&status);
        let fixes = Health::fixes(&status, remote.is_some());
        let web_url = status.remote_url.as_deref().or(remote).and_then(web_url_from_remote);
        Event::Status { repo, status, health, fixes, web_url }
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
        let action: Action = serde_json::from_str(r#"{"type":"desktop","request":3,"action":"unregister"}"#).unwrap();
        assert!(matches!(action, Action::Desktop { request: 3, action: DesktopAction::Unregister }));
        let action: Action = serde_json::from_str(r#"{"type":"open-in-commits","repo":"r2","theme":"dark"}"#).unwrap();
        assert!(matches!(action, Action::OpenInCommits { theme: ResolvedTheme::Dark, .. }));
        let action: Action = serde_json::from_str(r#"{"type":"update","request":4,"action":"stage"}"#).unwrap();
        assert!(matches!(action, Action::Update { request: 4, action: UpdateAction::Stage }));
        let action: Action = serde_json::from_str(r#"{"type":"installation","request":5,"action":"install"}"#).unwrap();
        assert!(matches!(action, Action::Installation { request: 5, action: InstallationAction::Install }));
    }

    /// The page's labels, in the page's order, are Commits' repository-health
    /// contract: the vectors file holds its verdict list.
    #[test]
    fn the_page_labels_every_verdict_as_the_contract_does() {
        let vectors: serde_json::Value =
            serde_json::from_str(include_str!("../../../crates/devpanel-core/tests/repository-health.vectors.json")).unwrap();
        let contract: Vec<(String, String)> = vectors["verdicts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| (v["id"].as_str().unwrap().to_string(), v["label"].as_str().unwrap().to_string()))
            .collect();
        let page = include_str!("../web/app.js");
        let table = &page[page.find("const HEALTH = {").unwrap()..];
        let table = &table[..table.find("\n};").unwrap()];
        let ours: Vec<(String, String)> = table
            .lines()
            .filter_map(|line| {
                let (key, rest) = line.trim().split_once(": { label: '")?;
                Some((key.trim_matches('\'').to_string(), rest.split('\'').next()?.to_string()))
            })
            .collect();
        assert_eq!(ours, contract);
    }

    #[test]
    fn events_encode_with_a_type_tag() {
        let json = serde_json::to_value(Event::info("hi")).unwrap();
        assert_eq!(json["type"], "toast");
        assert_eq!(json["message"], "hi");
    }
}
