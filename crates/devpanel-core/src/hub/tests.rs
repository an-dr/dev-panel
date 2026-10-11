use std::collections::HashMap;
use std::sync::Mutex;

use super::*;
use crate::domain::Launch;
use crate::ports::Silent;

/// Records every git call and answers status from a fixed table.
#[derive(Default)]
struct FakeGit {
    statuses: Mutex<HashMap<PathBuf, RepoStatus>>,
    calls: Mutex<Vec<String>>,
}

impl FakeGit {
    fn with_repo(self, path: &str, remote: Option<&str>) -> Self {
        let status = RepoStatus {
            exists: true,
            is_repo: true,
            branch: Some("main".into()),
            remote_url: remote.map(String::from),
            has_commits: true,
            ..RepoStatus::default()
        };
        self.statuses.lock().unwrap().insert(PathBuf::from(path), status);
        self
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, call: String) -> HubResult<()> {
        self.calls.lock().unwrap().push(call);
        Ok(())
    }
}

impl Git for FakeGit {
    fn status(&self, path: &Path) -> RepoStatus {
        self.statuses.lock().unwrap().get(path).cloned().unwrap_or_else(RepoStatus::missing)
    }
    fn clone_repo(&self, url: &str, dest: &Path, _: &dyn Output) -> HubResult<()> {
        self.record(format!("clone {url} {}", dest.display()))
    }
    fn fetch(&self, path: &Path, _: &dyn Output) -> HubResult<()> {
        self.record(format!("fetch {}", path.display()))
    }
    fn pull(&self, path: &Path, rebase: bool, _: &dyn Output) -> HubResult<()> {
        self.record(format!("pull rebase={rebase} {}", path.display()))
    }
    fn push(&self, path: &Path, set_upstream: bool, _: &dyn Output) -> HubResult<()> {
        self.record(format!("push upstream={set_upstream} {}", path.display()))
    }
    fn commit_all(&self, path: &Path, message: &str, _: &dyn Output) -> HubResult<()> {
        self.record(format!("commit {message} {}", path.display()))
    }
    fn stash(&self, path: &Path, _: &dyn Output) -> HubResult<()> {
        self.record(format!("stash {}", path.display()))
    }
}

#[derive(Default)]
struct FakeFs {
    files: Vec<String>,
    repos_under: Vec<PathBuf>,
    /// Path → list, for read_repo_list; write_repo_list records here too.
    stored: Mutex<HashMap<PathBuf, RepoList>>,
}

impl FileSystem for FakeFs {
    fn expand(&self, path: &Path) -> PathBuf {
        match path.strip_prefix("~") {
            Ok(rest) => Path::new("/home/u").join(rest),
            Err(_) => path.to_path_buf(),
        }
    }
    fn exists(&self, path: &Path) -> bool {
        path.starts_with("/home/u")
    }
    fn is_empty_dir(&self, _: &Path) -> bool {
        true
    }
    fn list_files(&self, _: &Path, _: u32) -> Vec<String> {
        self.files.clone()
    }
    fn find_repos(&self, _: &Path, _: u32) -> Vec<PathBuf> {
        self.repos_under.clone()
    }
    fn read_repo_list(&self, path: &Path) -> HubResult<RepoList> {
        self.stored
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| HubError::Storage(format!("no such file {}", path.display())))
    }
    fn write_repo_list(&self, path: &Path, list: &RepoList) -> HubResult<()> {
        self.stored.lock().unwrap().insert(path.to_path_buf(), list.clone());
        Ok(())
    }
}

#[derive(Default)]
struct FakeStore {
    saved: Mutex<Option<Workspace>>,
    fail: bool,
}

impl Store for FakeStore {
    fn load(&self) -> HubResult<Workspace> {
        Ok(Workspace::default())
    }
    fn save(&self, workspace: &Workspace) -> HubResult<()> {
        if self.fail {
            return Err(HubError::Storage("disk full".into()));
        }
        *self.saved.lock().unwrap() = Some(workspace.clone());
        Ok(())
    }
}

#[derive(Default)]
struct FakeLauncher(Mutex<Vec<String>>);

impl Launcher for FakeLauncher {
    fn launch(&self, tool: &Tool, path: &Path) -> HubResult<()> {
        self.0.lock().unwrap().push(format!("{} {}", tool.id.0, path.display()));
        Ok(())
    }
}

#[derive(Default)]
struct FakeShell(Mutex<Vec<String>>);

impl Shell for FakeShell {
    fn run(&self, command_line: &str, cwd: &Path, _: &dyn Output) -> HubResult<()> {
        self.0.lock().unwrap().push(format!("{command_line} @ {}", cwd.display()));
        Ok(())
    }
}

struct Fixture {
    hub: Hub,
    git: Arc<FakeGit>,
    launcher: Arc<FakeLauncher>,
    shell: Arc<FakeShell>,
    store: Arc<FakeStore>,
    fs: Arc<FakeFs>,
}

fn fixture_with(git: FakeGit, fs: FakeFs, store: FakeStore) -> Fixture {
    let git = Arc::new(git);
    let launcher = Arc::new(FakeLauncher::default());
    let shell = Arc::new(FakeShell::default());
    let store = Arc::new(store);
    let fs = Arc::new(fs);
    let ports = Ports {
        git: git.clone(),
        shell: shell.clone(),
        launcher: launcher.clone(),
        fs: fs.clone(),
        store: store.clone(),
    };
    Fixture { hub: Hub::open(ports, Platform::Unix).unwrap(), git, launcher, shell, store, fs }
}

fn fixture(git: FakeGit) -> Fixture {
    fixture_with(git, FakeFs::default(), FakeStore::default())
}

#[test]
fn adding_an_existing_repository_tracks_and_persists_it() {
    let f = fixture(FakeGit::default().with_repo("/home/u/bones", Some("https://x/bones.git")));
    let repo = f.hub.add_existing(Path::new("~/bones")).unwrap();
    assert_eq!(repo.name, "bones");
    assert_eq!(repo.id, RepoId("r1".into()));
    assert_eq!(repo.remote.as_deref(), Some("https://x/bones.git"));
    assert_eq!(f.store.saved.lock().unwrap().as_ref().unwrap().repos.len(), 1);
}

#[test]
fn adding_twice_is_refused() {
    let f = fixture(FakeGit::default().with_repo("/home/u/a", None));
    f.hub.add_existing(Path::new("/home/u/a")).unwrap();
    let error = f.hub.add_existing(Path::new("/home/u/a")).unwrap_err();
    assert!(matches!(error, HubError::Invalid(_)));
}

#[test]
fn adding_a_plain_folder_is_refused() {
    let f = fixture(FakeGit::default());
    assert!(f.hub.add_existing(Path::new("/nowhere")).is_err());
}

#[test]
fn clone_goes_under_the_clone_root_and_is_tracked() {
    let f = fixture(FakeGit::default());
    let repo = f.hub.clone_repo("git@github.com:an-dr/commits.git", None, &Silent).unwrap();
    let dest = Path::new("/home/u").join("Code").join("commits");
    assert_eq!(repo.path, dest);
    assert_eq!(f.git.calls(), [format!("clone git@github.com:an-dr/commits.git {}", dest.display())]);
}

#[test]
fn scan_skips_repositories_already_tracked() {
    let git = FakeGit::default().with_repo("/home/u/a", None).with_repo("/home/u/b", None);
    let fs = FakeFs { repos_under: vec!["/home/u/a".into(), "/home/u/b".into()], ..FakeFs::default() };
    let f = fixture_with(git, fs, FakeStore::default());
    f.hub.add_existing(Path::new("/home/u/a")).unwrap();
    let added = f.hub.scan(Path::new("/home/u")).unwrap();
    assert_eq!(added.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["b"]);
}

#[test]
fn failed_save_leaves_memory_unchanged() {
    let git = FakeGit::default().with_repo("/home/u/a", None);
    let f = fixture_with(git, FakeFs::default(), FakeStore { fail: true, ..FakeStore::default() });
    assert!(f.hub.add_existing(Path::new("/home/u/a")).is_err());
    assert!(f.hub.workspace().repos.is_empty());
}

#[test]
fn ids_restart_once_every_repository_is_removed() {
    let git = FakeGit::default().with_repo("/home/u/a", None).with_repo("/home/u/b", None);
    let f = fixture(git);
    let a = f.hub.add_existing(Path::new("/home/u/a")).unwrap();
    let b = f.hub.add_existing(Path::new("/home/u/b")).unwrap();
    f.hub.remove(&a.id).unwrap();
    f.hub.remove(&b.id).unwrap();
    let again = f.hub.add_existing(Path::new("/home/u/a")).unwrap();
    // Only the highest live id is known, so after removing every repository
    // numbering restarts; ids need only be unique among tracked repositories.
    assert_eq!(again.id, RepoId("r1".into()));
}

#[test]
fn status_learns_the_remote_url() {
    let f = fixture(FakeGit::default().with_repo("/home/u/a", None));
    let repo = f.hub.add_existing(Path::new("/home/u/a")).unwrap();
    f.git.statuses.lock().unwrap().get_mut(Path::new("/home/u/a")).unwrap().remote_url = Some("https://x/a".into());
    f.hub.status(&repo.id).unwrap();
    assert_eq!(f.hub.repo(&repo.id).unwrap().remote.as_deref(), Some("https://x/a"));
}

#[test]
fn fixes_map_to_the_right_git_calls() {
    let f = fixture(FakeGit::default().with_repo("/home/u/a", Some("https://x/a")));
    let id = f.hub.add_existing(Path::new("/home/u/a")).unwrap().id;
    f.hub.apply_fix(&id, Fix::Publish, None, &Silent).unwrap();
    f.hub.apply_fix(&id, Fix::Sync, None, &Silent).unwrap();
    f.hub.apply_fix(&id, Fix::CommitAll, Some("wip"), &Silent).unwrap();
    f.hub.apply_fix(&id, Fix::Reclone, None, &Silent).unwrap();
    assert_eq!(
        f.git.calls(),
        [
            "push upstream=true /home/u/a",
            "pull rebase=true /home/u/a",
            "push upstream=false /home/u/a",
            "commit wip /home/u/a",
            "clone https://x/a /home/u/a",
        ]
    );
}

#[test]
fn commit_without_message_is_refused_before_git_runs() {
    let f = fixture(FakeGit::default().with_repo("/home/u/a", None));
    let id = f.hub.add_existing(Path::new("/home/u/a")).unwrap().id;
    assert!(f.hub.commit_all(&id, "  ", &Silent).is_err());
    assert!(f.git.calls().is_empty());
}

#[test]
fn open_uses_the_default_tool_unless_one_is_named() {
    let f = fixture(FakeGit::default().with_repo("/home/u/a", None));
    let id = f.hub.add_existing(Path::new("/home/u/a")).unwrap().id;
    f.hub.open_in(&id, None).unwrap();
    f.hub.open_in(&id, Some(&ToolId("claude".into()))).unwrap();
    assert_eq!(*f.launcher.0.lock().unwrap(), ["vscode /home/u/a", "claude /home/u/a"]);
}

#[test]
fn custom_install_command_comes_first() {
    let git = FakeGit::default().with_repo("/home/u/a", None);
    let fs = FakeFs { files: vec!["install.sh".into()], ..FakeFs::default() };
    let f = fixture_with(git, fs, FakeStore::default());
    let id = f.hub.add_existing(Path::new("/home/u/a")).unwrap().id;
    let edit = RepoEdit { name: "a".into(), group: None, install_command: Some("just install".into()) };
    f.hub.update_repo(&id, edit).unwrap();
    let labels: Vec<_> = f.hub.install_recipes(&id).unwrap().into_iter().map(|r| r.label).collect();
    assert_eq!(labels, ["Custom", "install.sh"]);
    f.hub.run_command(&id, "just install", &Silent).unwrap();
    assert_eq!(*f.shell.0.lock().unwrap(), ["just install @ /home/u/a"]);
}

#[test]
fn tools_without_names_are_refused() {
    let f = fixture(FakeGit::default());
    let tool = Tool { id: ToolId("x".into()), name: " ".into(), launch: Launch::FileManager };
    assert!(f.hub.save_tools(vec![tool]).is_err());
}

#[test]
fn favouriting_persists_the_mark() {
    let git = FakeGit::default().with_repo("/home/u/a", Some("https://x/a.git"));
    let f = fixture(git);
    let repo = f.hub.add_existing(Path::new("/home/u/a")).unwrap();
    assert!(!repo.favorite);
    let marked = f.hub.set_favorite(&repo.id, true).unwrap();
    assert!(marked.favorite);
    assert!(f.store.saved.lock().unwrap().as_ref().unwrap().repos[0].favorite);
    f.hub.set_favorite(&repo.id, false).unwrap();
    assert!(!f.store.saved.lock().unwrap().as_ref().unwrap().repos[0].favorite);
}

#[test]
fn export_captures_every_repository_without_local_paths() {
    let git = FakeGit::default().with_repo("/home/u/a", Some("https://x/a.git"));
    let f = fixture(git);
    let repo = f.hub.add_existing(Path::new("/home/u/a")).unwrap();
    f.hub.set_favorite(&repo.id, true).unwrap();
    let path = f.hub.export_repos(Path::new("~/backup")).unwrap();
    assert_eq!(path, Path::new("/home/u/backup/devpanel-repos.json"));
    let stored = f.fs.stored.lock().unwrap();
    let list = stored.get(&path).unwrap();
    assert_eq!(list.version, RepoList::VERSION);
    assert_eq!(list.repos.len(), 1);
    assert_eq!(list.repos[0].name, "a");
    assert_eq!(list.repos[0].remote.as_deref(), Some("https://x/a.git"));
    assert!(list.repos[0].favorite);
}

#[test]
fn exporting_an_empty_workspace_is_refused() {
    let f = fixture(FakeGit::default());
    assert!(matches!(f.hub.export_repos(Path::new("~/backup")), Err(HubError::Invalid(_))));
}

#[test]
fn import_clones_each_listed_repository_and_restores_its_marks() {
    let f = fixture(FakeGit::default());
    let list = RepoList {
        version: RepoList::VERSION,
        repos: vec![
            RepoExport {
                name: "bones".into(),
                remote: Some("https://x/bones.git".into()),
                group: Some("engine".into()),
                install_command: Some("just build".into()),
                favorite: true,
            },
            RepoExport { name: "nolocal".into(), remote: None, group: None, install_command: None, favorite: false },
        ],
    };
    f.fs.stored.lock().unwrap().insert(PathBuf::from("/home/u/backup/devpanel-repos.json"), list);
    let cloned = f.hub.import_repos(Path::new("~/backup"), None, &Silent).unwrap();
    // The entry without a remote cannot be cloned and is reported, not tracked.
    assert_eq!(cloned, 1);
    let repos = f.hub.workspace().repos;
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].name, "bones");
    let dest = Path::new("/home/u").join("Code").join("bones");
    assert_eq!(repos[0].path, dest);
    assert_eq!(repos[0].group.as_deref(), Some("engine"));
    assert_eq!(repos[0].install_command.as_deref(), Some("just build"));
    assert!(repos[0].favorite);
    assert_eq!(f.git.calls(), [format!("clone https://x/bones.git {}", dest.display())]);
}

#[test]
fn import_skips_repositories_already_tracked() {
    let git = FakeGit::default().with_repo("/home/u/Code/bones", Some("https://x/bones.git"));
    let fs = FakeFs::default();
    let f = fixture_with(git, fs, FakeStore::default());
    f.hub.add_existing(Path::new("/home/u/Code/bones")).unwrap();
    let list = RepoList {
        version: RepoList::VERSION,
        repos: vec![RepoExport {
            name: "bones".into(),
            remote: Some("https://x/bones.git".into()),
            group: None,
            install_command: None,
            favorite: false,
        }],
    };
    f.fs.stored.lock().unwrap().insert(PathBuf::from("/home/u/backup/devpanel-repos.json"), list);
    let cloned = f.hub.import_repos(Path::new("~/backup"), None, &Silent).unwrap();
    assert_eq!(cloned, 0);
    assert_eq!(f.hub.workspace().repos.len(), 1);
}

#[test]
fn import_rejects_a_newer_format() {
    let f = fixture(FakeGit::default());
    let list = RepoList { version: RepoList::VERSION + 1, repos: vec![] };
    f.fs.stored.lock().unwrap().insert(PathBuf::from("/home/u/backup/devpanel-repos.json"), list);
    assert!(matches!(f.hub.import_repos(Path::new("~/backup"), None, &Silent), Err(HubError::Invalid(_))));
}
