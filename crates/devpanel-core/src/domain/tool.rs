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
}

/// Something a repository can be opened in: an editor, an agent, a git
/// client, a terminal. Fully user-editable; [`Tool::defaults`] is only the
/// starting set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    pub id: ToolId,
    pub name: String,
    pub launch: Launch,
}

impl Tool {
    /// The tools the user named: VS Code, Claude Code and Commits, plus a
    /// terminal and the file manager, which every repository needs.
    pub fn defaults() -> Vec<Tool> {
        let tool = |id: &str, name: &str, launch| Tool { id: ToolId(id.into()), name: name.into(), launch };
        vec![
            tool("vscode", "VS Code", Launch::Program { program: "code".into(), args: vec!["{path}".into()] }),
            tool("claude", "Claude Code", Launch::Terminal { command: Some("claude".into()) }),
            tool(
                ToolId::COMMITS,
                "Commits",
                Launch::Program { program: "~/.commits/app/commits".into(), args: vec!["{path}".into()] },
            ),
            tool("terminal", "Terminal", Launch::Terminal { command: None }),
            tool("files", "File manager", Launch::FileManager),
        ]
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
        ids.dedup();
        assert_eq!(ids.len(), tools.len());
    }
}
