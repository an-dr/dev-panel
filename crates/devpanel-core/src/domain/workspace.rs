use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{Repo, Tool, ToolId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Where new clones go when the user does not pick a folder.
    pub clone_root: PathBuf,
    /// Tool used by the primary "Open" button.
    pub default_tool: Option<ToolId>,
    /// Fetch every repository this often; `0` disables it.
    pub auto_fetch_minutes: u32,
    /// How deep "Scan folder" looks for repositories.
    pub scan_depth: u32,
    /// `system`, `light` or `dark`. Interpreted by the presentation only.
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            clone_root: PathBuf::from("~/Code"),
            default_tool: Some(ToolId("vscode".into())),
            auto_fetch_minutes: 0,
            scan_depth: 3,
            theme: "system".into(),
        }
    }
}

/// Everything DevPanel persists: the tracked repositories, the tools and the
/// settings. One document, loaded at start and saved after every change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Workspace {
    pub repos: Vec<Repo>,
    pub tools: Vec<Tool>,
    pub settings: Settings,
}

impl Default for Workspace {
    fn default() -> Self {
        Self { repos: Vec::new(), tools: Tool::defaults(), settings: Settings::default() }
    }
}
