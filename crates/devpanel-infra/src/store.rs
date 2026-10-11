use std::path::PathBuf;

use devpanel_core::domain::Workspace;
use devpanel_core::ports::Store;
use devpanel_core::{HubError, HubResult};

/// The workspace as one pretty-printed JSON file, readable and hand-editable.
pub struct JsonStore {
    path: PathBuf,
}

impl JsonStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// `<config dir>/devpanel/workspace.json`, e.g. `~/.config/devpanel` on
    /// Linux and `%APPDATA%\devpanel` on Windows. `DEVPANEL_CONFIG` overrides it,
    /// which is how tests and a second profile stay apart from real data.
    pub fn default_location() -> Self {
        let path = match std::env::var_os("DEVPANEL_CONFIG") {
            Some(path) => PathBuf::from(path),
            None => dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("devpanel").join("workspace.json"),
        };
        Self::new(path)
    }
}

impl Store for JsonStore {
    fn load(&self) -> HubResult<Workspace> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| HubError::Storage(format!("{} is not valid: {e}", self.path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Workspace::default()),
            Err(e) => Err(HubError::Storage(format!("reading {}: {e}", self.path.display()))),
        }
    }

    /// Writes to a sibling file and renames it over the old one, so a crash
    /// mid-write never leaves a truncated workspace behind.
    fn save(&self, workspace: &Workspace) -> HubResult<()> {
        let storage = |e: std::io::Error| HubError::Storage(format!("writing {}: {e}", self.path.display()));
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(storage)?;
        }
        let text = serde_json::to_string_pretty(workspace).map_err(|e| HubError::Storage(e.to_string()))?;
        let temp = self.path.with_extension("json.tmp");
        std::fs::write(&temp, text).map_err(storage)?;
        std::fs::rename(&temp, &self.path).map_err(storage)
    }
}

#[cfg(test)]
mod tests {
    use devpanel_core::domain::{Repo, RepoId};

    use super::*;

    #[test]
    fn missing_file_loads_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonStore::new(dir.path().join("none.json"));
        assert_eq!(store.load().unwrap(), Workspace::default());
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonStore::new(dir.path().join("sub/ws.json"));
        let mut workspace = Workspace::default();
        workspace.repos.push(Repo {
            id: RepoId("r1".into()),
            name: "bones".into(),
            path: "/code/bones".into(),
            remote: Some("https://x".into()),
            group: Some("engine".into()),
            install_command: None,
            favorite: false,
        });
        store.save(&workspace).unwrap();
        assert_eq!(store.load().unwrap(), workspace);
    }

    #[test]
    fn older_files_missing_new_fields_still_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ws.json");
        std::fs::write(&path, r#"{"repos":[{"id":"r1","name":"a","path":"/a"}]}"#).unwrap();
        let workspace = JsonStore::new(path).load().unwrap();
        assert_eq!(workspace.repos[0].remote, None);
        assert!(!workspace.tools.is_empty());
    }

    #[test]
    fn corrupt_file_is_an_error_not_an_empty_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ws.json");
        std::fs::write(&path, "{").unwrap();
        assert!(JsonStore::new(path).load().is_err());
    }
}
