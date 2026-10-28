use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolId(pub String);

impl ToolId {
    /// The Commits tool: DevPanel's git client, which problems open in.
    pub const COMMITS: &'static str = "commits";
}

/// How a tool is started for a repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Launch {
    /// A GUI program. `{path}` in any argument is replaced by the repository
    /// path; the program also starts with the repository as its working
    /// directory.
    Program { program: String, args: Vec<String> },
    /// A terminal opened at the repository, optionally running `command`
    /// (e.g. `claude`) inside it.
    Terminal { command: Option<String> },
    /// The system file manager showing the repository folder.
    FileManager,
    /// An installed desktop app that does not take a folder, such as the
    /// Claude or ChatGPT app; opening it ignores the repository. `app` is
    /// the Windows AppUserModelID (`Package_family!App`) or the macOS
    /// application name.
    App { app: String },
}

/// Something a repository can be opened in: an editor, an agent, a git
/// client, a terminal. Fully user-editable; [`Tool::defaults`] is only the
/// starting set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    pub id: ToolId,
    pub name: String,
    pub launch: Launch,
    /// A disabled tool stays in Settings but is offered nowhere else.
    #[serde(default = "enabled")]
    pub enabled: bool,
}

fn enabled() -> bool {
    true
}

impl Tool {
    /// The starting set: popular editors, agents and desktop apps, plus a
    /// terminal and the file manager, which every repository needs. Most
    /// will not be installed; the page hides a tool whose program is missing.
    pub fn defaults() -> Vec<Tool> {
        let program = |program: &str| Launch::Program { program: program.into(), args: vec!["{path}".into()] };
        let terminal = |command: &str| Launch::Terminal { command: Some(command.into()) };
        let mut tools = vec![
            Tool::new("vscode", "VS Code", program("code")),
            Tool::new("cursor", "Cursor", program("cursor")),
            Tool::new("windsurf", "Windsurf", program("windsurf")),
            Tool::new("zed", "Zed", program("zed")),
            Tool::new("claude", "Claude Code", terminal("claude")),
            Tool::new("codex", "Codex CLI", terminal("codex")),
        ];
        let apps: &[(&str, &str, &str)] = if cfg!(windows) {
            &[("claude-app", "Claude", "Claude_pzs8sxrjxfjjc!Claude"), ("chatgpt-app", "ChatGPT", "OpenAI.Codex_2p2nqsd0c76g0!App")]
        } else if cfg!(target_os = "macos") {
            &[("claude-app", "Claude", "Claude"), ("chatgpt-app", "ChatGPT", "ChatGPT")]
        } else {
            &[]
        };
        tools.extend(apps.iter().map(|(id, name, app)| Tool::new(id, name, Launch::App { app: (*app).into() })));
        tools.extend([
            Tool::new(ToolId::COMMITS, "Commits", program("~/.commits/app/commits")),
            Tool::new("terminal", "Terminal", Launch::Terminal { command: None }),
            Tool::new("files", "File manager", Launch::FileManager),
        ]);
        tools
    }

    pub fn new(id: &str, name: &str, launch: Launch) -> Tool {
        Tool { id: ToolId(id.into()), name: name.into(), launch, enabled: true }
    }

    /// The program a launch needs on `PATH` or at a path, if any: a program
    /// tool's program, or the first word of a terminal tool's command.
    pub fn required_program(&self) -> Option<&str> {
        match &self.launch {
            Launch::Program { program, .. } => Some(program),
            Launch::Terminal { command } => command.as_deref().and_then(|c| c.split_whitespace().next()),
            Launch::FileManager | Launch::App { .. } => None,
        }
    }

    /// Arguments with `{path}` substituted.
    pub fn expand_args(args: &[String], path: &str) -> Vec<String> {
        args.iter().map(|arg| arg.replace("{path}", path)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_placeholder_is_substituted_everywhere() {
        let args = vec!["--folder={path}".to_string(), "{path}".to_string(), "-n".to_string()];
        assert_eq!(Tool::expand_args(&args, "/r"), vec!["--folder=/r", "/r", "-n"]);
    }

    #[test]
    fn defaults_have_unique_ids() {
        let tools = Tool::defaults();
        let mut ids: Vec<_> = tools.iter().map(|t| &t.id.0).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), tools.len());
    }

    #[test]
    fn a_tool_saved_before_the_enabled_flag_is_enabled() {
        let tool: Tool = serde_json::from_str(r#"{"id":"x","name":"X","launch":{"kind":"file-manager"}}"#).unwrap();
        assert!(tool.enabled);
    }

    #[test]
    fn a_terminal_tool_needs_its_command_program() {
        let tool = Tool::new("c", "C", Launch::Terminal { command: Some("claude --resume".into()) });
        assert_eq!(tool.required_program(), Some("claude"));
        assert_eq!(Tool::new("t", "T", Launch::Terminal { command: None }).required_program(), None);
    }
}
