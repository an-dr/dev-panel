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
    /// Pull, fast-forward only, every repository not pulled for this many
    /// hours; `0` disables it.
    pub auto_pull_hours: u32,
    /// How deep "Scan folder" looks for repositories.
    pub scan_depth: u32,
    /// `system`, `light` or `dark`. Interpreted by the presentation only.
    pub theme: String,
    /// The page DevPanel opens on: `last` for wherever the user was, or
    /// `overview`, `favorites`, `repos`, `activity` or `group:<name>`.
    /// Interpreted by the presentation only.
    pub start_page: String,
    /// The user's order of the sidebar's items, by the same keys as
    /// `start_page`; items it does not name keep their default place after
    /// those it does. Interpreted by the presentation only.
    pub sidebar_order: Vec<String>,
    /// Newly tracked repositories get the plain name style instead of one the
    /// page picks, for a user who styles each repository by hand.
    pub plain_names_for_new_repos: bool,
    /// `compact`, `comfortable` or `spacious`. Interpreted by the
    /// presentation only.
    pub density: String,
    /// The newest [`TOOL_DEFAULTS`] set already offered to this workspace,
    /// so tools added to the defaults later reach existing users once, and a
    /// default the user removed does not return. Missing in a workspace
    /// saved before the field existed, hence `0` rather than the current set.
    #[serde(default)]
    pub tool_defaults: u32,
}

/// Version of [`Tool::defaults`]; raised whenever a default tool is added.
pub const TOOL_DEFAULTS: u32 = 1;

impl Default for Settings {
    fn default() -> Self {
        Self {
            clone_root: PathBuf::from("~/Code"),
            default_tool: Some(ToolId("vscode".into())),
            auto_fetch_minutes: 0,
            auto_pull_hours: 0,
            scan_depth: 3,
            theme: "system".into(),
            start_page: "last".into(),
            sidebar_order: Vec::new(),
            plain_names_for_new_repos: false,
            density: "comfortable".into(),
            tool_defaults: TOOL_DEFAULTS,
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

impl Workspace {
    /// Adds the default tools introduced since this workspace last saw the
    /// defaults, before the terminal and file manager that end the list.
    /// Returns whether anything changed.
    pub fn adopt_new_default_tools(&mut self) -> bool {
        if self.settings.tool_defaults >= TOOL_DEFAULTS {
            return false;
        }
        let missing: Vec<Tool> =
            Tool::defaults().into_iter().filter(|d| !self.tools.iter().any(|t| t.id == d.id)).collect();
        let at = self.tools.iter().position(|t| matches!(t.id.0.as_str(), "commits" | "terminal" | "files"));
        let at = at.unwrap_or(self.tools.len());
        self.tools.splice(at..at, missing);
        self.settings.tool_defaults = TOOL_DEFAULTS;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_default_tools_are_added_once_before_the_terminal() {
        let mut ws: Workspace = serde_json::from_str(
            r#"{"tools":[{"id":"vscode","name":"VS Code","launch":{"kind":"program","program":"code","args":[]}},
                        {"id":"terminal","name":"Terminal","launch":{"kind":"terminal","command":null}}],
              "settings":{"clone_root":"~/Code"}}"#,
        )
        .unwrap();

        assert!(ws.adopt_new_default_tools());
        let ids: Vec<&str> = ws.tools.iter().map(|t| t.id.0.as_str()).collect();
        assert_eq!(ids.iter().filter(|id| **id == "vscode").count(), 1);
        let at = |id: &str| ids.iter().position(|i| *i == id).unwrap();
        assert!(at("vscode") < at("zed") && at("zed") < at("terminal"));

        ws.tools.retain(|t| t.id.0 != "zed");
        assert!(!ws.adopt_new_default_tools());
        assert!(!ws.tools.iter().any(|t| t.id.0 == "zed"));
    }

    #[test]
    fn a_new_workspace_already_has_the_current_defaults() {
        assert!(!Workspace::default().adopt_new_default_tools());
    }
}
