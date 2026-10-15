//! DevPanel's repositories as a Commits repository source (Commits'
//! docs/repository-sources.md): one JSON file Commits reads for its
//! repository selector and never writes.

use std::path::{Path, PathBuf};

use devpanel_core::domain::Repo;
use devpanel_core::ports::{FileSystem, RepoSource};
use devpanel_core::{HubError, HubResult};
use serde::Serialize;

use crate::LocalFileSystem;

/// The source id, which names the file.
const FILE_NAME: &str = "devpanel.json";

/// [`RepoSource`] over `devpanel.json` in Commits' sources folder.
pub struct CommitsSourceFile {
    /// `None` when there is no home folder: publishing then does nothing.
    dir: Option<PathBuf>,
}

#[derive(Serialize)]
struct Source<'a> {
    version: u32,
    name: &'a str,
    repositories: Vec<Entry>,
}

#[derive(Serialize)]
struct Entry {
    name: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<String>,
    favorite: bool,
}

impl CommitsSourceFile {
    /// `~/.commits/sources`, or `COMMITS_SOURCES_DIR` when set, as Commits
    /// itself resolves it.
    pub fn default_location() -> Self {
        let dir = match std::env::var_os("COMMITS_SOURCES_DIR") {
            Some(dir) => Some(PathBuf::from(dir)),
            None => dirs::home_dir().map(|home| home.join(".commits").join("sources")),
        };
        Self { dir }
    }

    pub fn in_dir(dir: PathBuf) -> Self {
        Self { dir: Some(dir) }
    }

    fn source(repos: &[Repo]) -> Source<'static> {
        let fs = LocalFileSystem;
        let repositories = repos
            .iter()
            .map(|repo| Entry {
                name: repo.name.clone(),
                path: fs.expand(&repo.path).to_string_lossy().into_owned(),
                group: repo.group.clone(),
                favorite: repo.favorite,
            })
            .collect();
        Source { version: 1, name: "DevPanel", repositories }
    }
}

impl RepoSource for CommitsSourceFile {
    /// Writes a sibling file and renames it over `devpanel.json`, so Commits
    /// never reads half a list.
    fn publish(&self, repos: &[Repo]) -> HubResult<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        let json = serde_json::to_string_pretty(&Self::source(repos)).map_err(|e| HubError::Storage(e.to_string()))?;
        let result = write_atomically(dir, &json);
        if let Err(error) = &result {
            eprintln!("[WARN] devpanel: could not update Commits' repository list: {error}");
        }
        result
    }
}

fn write_atomically(dir: &Path, json: &str) -> HubResult<()> {
    let storage = |e: std::io::Error| HubError::Storage(format!("{}: {e}", dir.display()));
    std::fs::create_dir_all(dir).map_err(storage)?;
    let temporary = dir.join(format!("{FILE_NAME}.tmp"));
    std::fs::write(&temporary, json).map_err(storage)?;
    std::fs::rename(&temporary, dir.join(FILE_NAME)).map_err(storage)
}

#[cfg(test)]
mod tests {
    use devpanel_core::domain::RepoId;

    use super::*;

    fn repo(name: &str, path: &str, group: Option<&str>, favorite: bool) -> Repo {
        Repo {
            id: RepoId(name.into()),
            name: name.into(),
            path: PathBuf::from(path),
            remote: None,
            group: group.map(String::from),
            install_command: None,
            favorite,
        }
    }

    #[test]
    fn writes_every_repository_in_commits_format() {
        let dir = tempfile::tempdir().unwrap();
        let sources = dir.path().join("sources");
        let absolute = dir.path().join("bones");
        let repos = [repo("bones", &absolute.to_string_lossy(), Some("engine"), true), repo("notes", "~/notes", None, false)];

        CommitsSourceFile::in_dir(sources.clone()).publish(&repos).unwrap();

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(sources.join("devpanel.json")).unwrap()).unwrap();
        assert_eq!(written["version"], 1);
        assert_eq!(written["name"], "DevPanel");
        let entries = written["repositories"].as_array().unwrap();
        assert_eq!(entries[0]["name"], "bones");
        assert_eq!(entries[0]["path"], absolute.to_string_lossy().as_ref());
        assert_eq!(entries[0]["group"], "engine");
        assert_eq!(entries[0]["favorite"], true);
        let home = dirs::home_dir().unwrap().join("notes");
        assert_eq!(entries[1]["path"], home.to_string_lossy().as_ref(), "~ is expanded");
        assert!(entries[1].get("group").is_none());
        assert!(!sources.join("devpanel.json.tmp").exists());
    }

    #[test]
    fn publishing_again_replaces_the_list() {
        let dir = tempfile::tempdir().unwrap();
        let source = CommitsSourceFile::in_dir(dir.path().to_path_buf());
        source.publish(&[repo("a", "/a", None, false), repo("b", "/b", None, false)]).unwrap();
        source.publish(&[repo("b", "/b", None, false)]).unwrap();
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join("devpanel.json")).unwrap()).unwrap();
        assert_eq!(written["repositories"].as_array().unwrap().len(), 1);
    }
}
