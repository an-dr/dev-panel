//! The use cases: everything the dashboard can do, as one service.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::domain::{
    detect_install_recipes, repo_name_from_url, Fix, InstallRecipe, Platform, Repo, RepoExport, RepoId, RepoList,
    RepoStatus, Settings, Tool, ToolId, Workspace,
};
use crate::ports::{FileSystem, Git, Launcher, Output, Shell, Store};
use crate::{HubError, HubResult};

/// Name of the file an export writes and an import reads, inside the folder
/// the user chooses.
pub const EXPORT_FILE: &str = "devpanel-repos.json";

/// The adapters a [`Hub`] works through.
#[derive(Clone)]
pub struct Ports {
    pub git: Arc<dyn Git>,
    pub shell: Arc<dyn Shell>,
    pub launcher: Arc<dyn Launcher>,
    pub fs: Arc<dyn FileSystem>,
    pub store: Arc<dyn Store>,
}

/// User-editable fields of a tracked repository.
#[derive(Debug, Clone, Default)]
pub struct RepoEdit {
    pub name: String,
    pub group: Option<String>,
    pub install_command: Option<String>,
}

/// Application service. Thread-safe: the workspace sits behind a mutex that
/// is never held while git or a tool runs, so a long pull in one repository
/// does not block reading or editing another.
pub struct Hub {
    ports: Ports,
    platform: Platform,
    workspace: Mutex<Workspace>,
}

impl Hub {
    pub fn open(ports: Ports, platform: Platform) -> HubResult<Self> {
        let workspace = ports.store.load()?;
        Ok(Self { ports, platform, workspace: Mutex::new(workspace) })
    }

    pub fn workspace(&self) -> Workspace {
        self.lock().clone()
    }

    pub fn repo(&self, id: &RepoId) -> HubResult<Repo> {
        self.lock().repos.iter().find(|r| &r.id == id).cloned().ok_or_else(|| not_found(id))
    }

    // ---- tracking -------------------------------------------------------

    /// Starts tracking the working tree at `path`.
    pub fn add_existing(&self, path: &Path) -> HubResult<Repo> {
        let path = self.ports.fs.expand(path);
        let status = self.ports.git.status(&path);
        if !status.exists {
            return Err(HubError::Invalid(format!("{} does not exist", path.display())));
        }
        if !status.is_repo {
            return Err(HubError::Invalid(format!("{} is not a git repository", path.display())));
        }
        self.track(path, status.remote_url)
    }

    /// Tracks every repository under `root` that is not tracked yet.
    pub fn scan(&self, root: &Path) -> HubResult<Vec<Repo>> {
        let root = self.ports.fs.expand(root);
        if !self.ports.fs.exists(&root) {
            return Err(HubError::Invalid(format!("{} does not exist", root.display())));
        }
        let depth = self.lock().settings.scan_depth;
        let mut added = Vec::new();
        for path in self.ports.fs.find_repos(&root, depth) {
            if self.is_tracked(&path) {
                continue;
            }
            let remote = self.ports.git.status(&path).remote_url;
            added.push(self.track(path, remote)?);
        }
        Ok(added)
    }

    /// Clones `url` into `parent` (the configured clone root when `None`)
    /// and tracks the result.
    pub fn clone_repo(&self, url: &str, parent: Option<&Path>, out: &dyn Output) -> HubResult<Repo> {
        let url = url.trim();
        let name = repo_name_from_url(url).ok_or_else(|| HubError::Invalid("enter a repository URL".into()))?;
        let parent = match parent {
            Some(parent) => self.ports.fs.expand(parent),
            None => self.ports.fs.expand(&self.lock().settings.clone_root),
        };
        let dest = parent.join(&name);
        if self.ports.fs.exists(&dest) && !self.ports.fs.is_empty_dir(&dest) {
            return Err(HubError::Invalid(format!("{} already exists and is not empty", dest.display())));
        }
        self.ports.git.clone_repo(url, &dest, out)?;
        self.track(dest, Some(url.to_string()))
    }

    pub fn remove(&self, id: &RepoId) -> HubResult<()> {
        self.mutate(|ws| {
            let before = ws.repos.len();
            ws.repos.retain(|r| &r.id != id);
            if ws.repos.len() == before {
                return Err(not_found(id));
            }
            Ok(())
        })
    }

    pub fn update_repo(&self, id: &RepoId, edit: RepoEdit) -> HubResult<Repo> {
        let name = edit.name.trim();
        if name.is_empty() {
            return Err(HubError::Invalid("the name cannot be empty".into()));
        }
        self.mutate(|ws| {
            let repo = ws.repos.iter_mut().find(|r| &r.id == id).ok_or_else(|| not_found(id))?;
            repo.name = name.to_string();
            repo.group = non_empty(edit.group);
            repo.install_command = non_empty(edit.install_command);
            Ok(repo.clone())
        })
    }

    /// Marks a repository as a favourite (a main one the user works with) or
    /// clears the mark. Nothing git can report changes, so only the workspace
    /// is touched.
    pub fn set_favorite(&self, id: &RepoId, favorite: bool) -> HubResult<Repo> {
        self.mutate(|ws| {
            let repo = ws.repos.iter_mut().find(|r| &r.id == id).ok_or_else(|| not_found(id))?;
            repo.favorite = favorite;
            Ok(repo.clone())
        })
    }

    // ---- transfer --------------------------------------------------------

    /// Writes a portable list of every tracked repository to
    /// `devpanel-repos.json` in `folder`, and returns the file's path. The
    /// list carries each repository's remote, name, group and favourite mark,
    /// but no machine-specific path, so it can be imported on another machine.
    pub fn export_repos(&self, folder: &Path) -> HubResult<PathBuf> {
        let folder = self.ports.fs.expand(folder);
        let repos: Vec<RepoExport> = self.lock().repos.iter().map(RepoExport::from).collect();
        if repos.is_empty() {
            return Err(HubError::Invalid("there are no repositories to export".into()));
        }
        let list = RepoList { version: RepoList::VERSION, repos };
        let path = folder.join(EXPORT_FILE);
        self.ports.fs.write_repo_list(&path, &list)?;
        Ok(path)
    }

    /// Reads `devpanel-repos.json` from `folder` and clones every repository
    /// it lists into `dest` (the configured clone root when `None`), restoring
    /// each one's name, group, install command and favourite mark. Entries
    /// without a remote, or already tracked, are skipped; a single clone that
    /// fails is reported and does not stop the rest. Returns how many were
    /// newly cloned.
    pub fn import_repos(&self, folder: &Path, dest: Option<&Path>, out: &dyn Output) -> HubResult<usize> {
        let file = self.ports.fs.expand(folder).join(EXPORT_FILE);
        let list = self.ports.fs.read_repo_list(&file)?;
        if list.version > RepoList::VERSION {
            return Err(HubError::Invalid(format!(
                "{} was written by a newer DevPanel (format {}); update to import it",
                file.display(),
                list.version
            )));
        }
        let dest = match dest {
            Some(dest) => self.ports.fs.expand(dest),
            None => self.ports.fs.expand(&self.lock().settings.clone_root),
        };
        out.line(&format!("{} repositories in the list", list.repos.len()));
        let mut cloned = 0;
        for entry in &list.repos {
            match self.clone_from_export(entry, &dest, out) {
                Ok(Some(repo)) => {
                    out.line(&format!("  + {} ({})", repo.name, repo.path.display()));
                    cloned += 1;
                }
                Ok(None) => out.line(&format!("  skip {} (already tracked)", entry.name)),
                Err(error) => out.line(&format!("  ! {} — {error}", entry.name)),
            }
        }
        out.line(&format!("cloned {cloned} of {}", list.repos.len()));
        Ok(cloned)
    }

    /// Clones one exported entry into `parent` and tracks it, carrying over its
    /// saved fields. `Ok(None)` means it was already tracked and left alone.
    fn clone_from_export(&self, entry: &RepoExport, parent: &Path, out: &dyn Output) -> HubResult<Option<Repo>> {
        let remote = entry.remote.as_deref().filter(|r| !r.trim().is_empty());
        let remote = remote.ok_or_else(|| HubError::Invalid("it has no remote to clone from".into()))?;
        let folder = repo_name_from_url(remote).unwrap_or_else(|| entry.name.clone());
        let dest = parent.join(&folder);
        if self.is_tracked(&dest) {
            return Ok(None);
        }
        if self.ports.fs.exists(&dest) && !self.ports.fs.is_empty_dir(&dest) {
            return Err(HubError::Invalid(format!("{} already exists and is not empty", dest.display())));
        }
        self.ports.git.clone_repo(remote, &dest, out)?;
        let repo = self.mutate(|ws| {
            if ws.repos.iter().any(|r| r.path == dest) {
                return Err(HubError::Invalid("it is already tracked".into()));
            }
            let name = non_empty(Some(entry.name.clone())).unwrap_or(folder.clone());
            let repo = Repo {
                id: next_id(&ws.repos),
                name,
                path: dest.clone(),
                remote: Some(remote.to_string()),
                group: non_empty(entry.group.clone()),
                install_command: non_empty(entry.install_command.clone()),
                favorite: entry.favorite,
            };
            ws.repos.push(repo.clone());
            Ok(repo)
        })?;
        Ok(Some(repo))
    }

    // ---- condition -------------------------------------------------------

    /// Reads the repository's status, and remembers its remote URL when git
    /// reports one, so a later [`Fix::Reclone`] knows where to clone from.
    pub fn status(&self, id: &RepoId) -> HubResult<RepoStatus> {
        let repo = self.repo(id)?;
        let status = self.ports.git.status(&repo.path);
        if status.remote_url.is_some() && status.remote_url != repo.remote {
            self.mutate(|ws| {
                if let Some(stored) = ws.repos.iter_mut().find(|r| &r.id == id) {
                    stored.remote = status.remote_url.clone();
                }
                Ok(())
            })?;
        }
        Ok(status)
    }

    pub fn fetch(&self, id: &RepoId, out: &dyn Output) -> HubResult<()> {
        self.ports.git.fetch(&self.repo(id)?.path, out)
    }

    pub fn pull(&self, id: &RepoId, out: &dyn Output) -> HubResult<()> {
        self.ports.git.pull(&self.repo(id)?.path, false, out)
    }

    pub fn push(&self, id: &RepoId, out: &dyn Output) -> HubResult<()> {
        self.ports.git.push(&self.repo(id)?.path, false, out)
    }

    /// Pull with rebase, then push: brings a diverged branch level with its
    /// upstream in both directions.
    pub fn sync(&self, id: &RepoId, out: &dyn Output) -> HubResult<()> {
        let path = self.repo(id)?.path;
        self.ports.git.pull(&path, true, out)?;
        self.ports.git.push(&path, false, out)
    }

    pub fn commit_all(&self, id: &RepoId, message: &str, out: &dyn Output) -> HubResult<()> {
        let message = message.trim();
        if message.is_empty() {
            return Err(HubError::Invalid("a commit message is required".into()));
        }
        self.ports.git.commit_all(&self.repo(id)?.path, message, out)
    }

    /// Runs one [`Fix`]. `message` is the commit message for
    /// [`Fix::CommitAll`] and ignored otherwise.
    pub fn apply_fix(&self, id: &RepoId, fix: Fix, message: Option<&str>, out: &dyn Output) -> HubResult<()> {
        let repo = self.repo(id)?;
        match fix {
            Fix::Reclone => {
                let url = repo.remote.ok_or_else(|| HubError::Invalid("no remote URL is known for it".into()))?;
                self.ports.git.clone_repo(&url, &repo.path, out)
            }
            Fix::Pull => self.ports.git.pull(&repo.path, false, out),
            Fix::Sync => self.sync(id, out),
            Fix::Push => self.ports.git.push(&repo.path, false, out),
            Fix::Publish => self.ports.git.push(&repo.path, true, out),
            Fix::CommitAll => self.commit_all(id, message.unwrap_or_default(), out),
            Fix::Stash => self.ports.git.stash(&repo.path, out),
            Fix::OpenInTool => self.open_in(id, None),
        }
    }

    // ---- tools -------------------------------------------------------------

    /// Opens the repository in `tool`, or in the default tool when `None`.
    pub fn open_in(&self, id: &RepoId, tool: Option<&ToolId>) -> HubResult<()> {
        let repo = self.repo(id)?;
        let tool = self.tool(tool)?;
        if !self.ports.fs.exists(&repo.path) {
            return Err(HubError::Invalid(format!("{} does not exist", repo.path.display())));
        }
        self.ports.launcher.launch(&tool, &repo.path)
    }

    fn tool(&self, id: Option<&ToolId>) -> HubResult<Tool> {
        let ws = self.lock();
        let id = id.or(ws.settings.default_tool.as_ref());
        let found = match id {
            Some(id) => ws.tools.iter().find(|t| &t.id == id),
            None => ws.tools.first(),
        };
        found.cloned().ok_or_else(|| HubError::NotFound("no tool is configured".into()))
    }

    // ---- install -----------------------------------------------------------

    /// The repository's own install command first, then whatever its files
    /// suggest.
    pub fn install_recipes(&self, id: &RepoId) -> HubResult<Vec<InstallRecipe>> {
        let repo = self.repo(id)?;
        let mut recipes = Vec::new();
        if let Some(command) = repo.install_command {
            recipes.push(InstallRecipe { label: "Custom".into(), command, source: "repository settings".into() });
        }
        let files = self.ports.fs.list_files(&repo.path, 2);
        recipes.extend(detect_install_recipes(&files, self.platform));
        Ok(recipes)
    }

    pub fn run_command(&self, id: &RepoId, command_line: &str, out: &dyn Output) -> HubResult<()> {
        let command_line = command_line.trim();
        if command_line.is_empty() {
            return Err(HubError::Invalid("the command is empty".into()));
        }
        let path = self.repo(id)?.path;
        out.line(&format!("$ {command_line}"));
        self.ports.shell.run(command_line, &path, out)
    }

    // ---- configuration -----------------------------------------------------

    pub fn save_settings(&self, settings: Settings) -> HubResult<()> {
        self.mutate(|ws| {
            ws.settings = settings;
            Ok(())
        })
    }

    pub fn save_tools(&self, tools: Vec<Tool>) -> HubResult<()> {
        if tools.iter().any(|t| t.name.trim().is_empty() || t.id.0.trim().is_empty()) {
            return Err(HubError::Invalid("every tool needs a name".into()));
        }
        self.mutate(|ws| {
            ws.tools = tools;
            Ok(())
        })
    }

    // ---- internals -------------------------------------------------------

    fn lock(&self) -> MutexGuard<'_, Workspace> {
        // A panic while holding the lock leaves plain data behind, never a
        // broken invariant worth refusing to read.
        self.workspace.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Applies `change` and persists the result; the in-memory workspace is
    /// rolled back when saving fails, so memory never claims what disk lacks.
    fn mutate<T>(&self, change: impl FnOnce(&mut Workspace) -> HubResult<T>) -> HubResult<T> {
        let mut ws = self.lock();
        let mut next = ws.clone();
        let value = change(&mut next)?;
        self.ports.store.save(&next)?;
        *ws = next;
        Ok(value)
    }

    fn is_tracked(&self, path: &Path) -> bool {
        self.lock().repos.iter().any(|r| r.path == path)
    }

    fn track(&self, path: PathBuf, remote: Option<String>) -> HubResult<Repo> {
        self.mutate(|ws| {
            if let Some(existing) = ws.repos.iter().find(|r| r.path == path) {
                return Err(HubError::Invalid(format!("{} is already tracked", existing.name)));
            }
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "repository".into());
            let repo =
                Repo { id: next_id(&ws.repos), name, path, remote, group: None, install_command: None, favorite: false };
            ws.repos.push(repo.clone());
            Ok(repo)
        })
    }
}

fn next_id(repos: &[Repo]) -> RepoId {
    let max = repos.iter().filter_map(|r| r.id.0.strip_prefix('r')?.parse::<u64>().ok()).max().unwrap_or(0);
    RepoId(format!("r{}", max + 1))
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn not_found(id: &RepoId) -> HubError {
    HubError::NotFound(format!("repository {}", id.0))
}

#[cfg(test)]
mod tests;
