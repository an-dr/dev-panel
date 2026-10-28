//! Turns page actions into hub use cases and hub results into page events.
//!
//! Knows nothing about bones: it takes [`Action`]s and queues [`Event`]s,
//! and the [`crate::dashboard`] module moves them across the web panel. That
//! is what lets the controller be tested without a window.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use devpanel_core::domain::{Fix, RepoId, ToolId};
use devpanel_core::hub::RepoEdit;
use devpanel_core::ports::{Output, WatchHandle};
use devpanel_core::{Hub, HubError, HubResult};

use crate::jobs::{self, Pool};
use crate::protocol::{Action, Event, InstallationAction, JobState};

/// Concurrent `git status` reads.
const STATUS_WORKERS: usize = 4;
/// Concurrent fetches during "Fetch all".
const NETWORK_WORKERS: usize = 4;
/// How often the background pull looks for repositories gone stale.
const PULL_CHECK_INTERVAL: Duration = Duration::from_secs(60);

/// What a finished job invalidates on the page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Afterwards {
    /// The repository's status.
    Status,
    /// The tracked set changed: send the workspace and refresh everything.
    Workspace,
}

/// Where a job runs.
enum Runner<'a> {
    OwnThread,
    Pool(&'a Pool),
}

pub struct Controller {
    hub: Arc<Hub>,
    events: Sender<Event>,
    inbox: Receiver<Event>,
    status_pool: Pool,
    network_pool: Pool,
    /// Repositories with a job running; a second job on one is refused,
    /// since two git processes in one tree only fight over its lock.
    busy: Arc<Mutex<HashSet<RepoId>>>,
    /// Repositories with a status read queued or running.
    refreshing: Arc<Mutex<HashSet<RepoId>>>,
    next_job: AtomicU64,
    last_auto_fetch: Instant,
    /// When the background pull last looked for stale repositories.
    last_pull_check: Option<Instant>,
    /// The folder DevPanel was started with, opened once the page is ready.
    startup_path: Mutex<Option<PathBuf>>,
    /// DevPanel's palette as a Commits theme file, when it could be written.
    commits_theme: Option<PathBuf>,
    /// What the page was last told is installed: whether Commits is, and
    /// which tools are.
    installed: Mutex<(bool, Vec<ToolId>)>,
    /// One watch per tracked repository, with the path it watches.
    watches: HashMap<RepoId, (PathBuf, WatchHandle)>,
    /// The workspace revision the watches were last matched to.
    watched_revision: Option<u64>,
    /// Repositories a watch reported changed, waiting for the next tick.
    changed: (Sender<RepoId>, Receiver<RepoId>),
}

impl Controller {
    pub fn new(hub: Arc<Hub>) -> Self {
        let (events, inbox) = channel();
        Self {
            hub,
            events,
            inbox,
            status_pool: Pool::new(STATUS_WORKERS),
            network_pool: Pool::new(NETWORK_WORKERS),
            busy: Arc::default(),
            refreshing: Arc::default(),
            next_job: AtomicU64::new(0),
            last_auto_fetch: Instant::now(),
            last_pull_check: None,
            startup_path: Mutex::new(None),
            commits_theme: None,
            installed: Mutex::new((false, Vec::new())),
            watches: HashMap::new(),
            watched_revision: None,
            changed: channel(),
        }
    }

    /// The theme file passed to Commits with every "Open in Commits".
    pub fn with_commits_theme(mut self, path: Option<PathBuf>) -> Self {
        self.commits_theme = path;
        self
    }

    /// The folder from `devpanel <path>`, opened when the page first says
    /// it is ready, since a selection sent before then would be lost.
    pub fn with_startup_path(self, path: Option<PathBuf>) -> Self {
        *self.startup_path.lock().unwrap() = path;
        self
    }

    /// Events produced since the last call, in order.
    pub fn drain(&self) -> Vec<Event> {
        self.inbox.try_iter().collect()
    }

    /// Called every frame: keeps the watches in step with the workspace,
    /// re-reads repositories they report, and runs the periodic fetch when
    /// it is due.
    pub fn tick(&mut self, now: Instant) {
        if self.watched_revision != Some(self.hub.revision()) {
            self.sync_watches();
        }
        let changed: HashSet<RepoId> = self.changed.1.try_iter().collect();
        for repo in changed {
            self.refresh(repo);
        }
        if self.last_pull_check.is_none_or(|last| now.duration_since(last) >= PULL_CHECK_INTERVAL) {
            self.last_pull_check = Some(now);
            self.pull_stale();
        }
        let minutes = self.hub.workspace().settings.auto_fetch_minutes;
        if minutes == 0 {
            self.last_auto_fetch = now;
            return;
        }
        if now.duration_since(self.last_auto_fetch) >= Duration::from_secs(u64::from(minutes) * 60) {
            self.last_auto_fetch = now;
            self.fetch_all();
        }
    }

    pub fn handle(&self, action: Action) {
        match action {
            Action::Ready => {
                self.send_workspace();
                self.refresh_all();
                if let Some(path) = self.startup_path.lock().unwrap().take() {
                    self.open_path(path);
                }
            }
            Action::Refresh { repo: Some(repo) } => self.refresh(repo),
            Action::Refresh { repo: None } => {
                // The page asks this when its window regains focus, which is
                // when Commits or another tool may have been installed or
                // removed.
                let installed = (self.hub.commits_available(), self.hub.available_tools());
                if installed != *self.installed.lock().unwrap() {
                    self.send_workspace();
                }
                self.refresh_all();
            }
            Action::Fetch { repo: Some(repo) } => self.git_job(repo, "Fetch", |hub, id, out| hub.fetch(id, out)),
            Action::Fetch { repo: None } => self.fetch_all(),
            Action::Pull { repo } => self.git_job(repo, "Pull", |hub, id, out| hub.pull(id, out)),
            Action::Push { repo } => self.git_job(repo, "Push", |hub, id, out| hub.push(id, out)),
            Action::Sync { repo } => self.git_job(repo, "Sync", |hub, id, out| hub.sync(id, out)),
            Action::Stash { repo } => self.git_job(repo, "Stash", |hub, id, out| hub.apply_fix(id, Fix::Stash, None, out)),
            Action::Commit { repo, message } => {
                self.git_job(repo, "Commit", move |hub, id, out| hub.commit_all(id, &message, out))
            }
            Action::Fix { repo, fix, message } => self.fix(repo, fix, message),
            Action::Open { repo, tool } => {
                let result = self.hub.open_in(&repo, tool.as_ref());
                self.report(result);
            }
            // Off the frame thread: choosing the view reads the status first.
            Action::OpenInCommits { repo, theme } => {
                let hub = Arc::clone(&self.hub);
                let events = self.events.clone();
                let theme_file = self.commits_theme.clone();
                jobs::spawn(move || {
                    if let Err(error) = hub.open_in_commits(&repo, theme, theme_file.as_deref()) {
                        let _ = events.send(Event::error(error.to_string()));
                    }
                });
            }
            Action::AddExisting { path } => {
                match self.hub.add_existing(Path::new(path.trim())) {
                    Ok(repo) => {
                        self.emit(Event::info(format!("Added {}", repo.name)));
                        // Only the new repository's status is unknown; refresh
                        // it alone rather than re-reading every tracked one.
                        self.send_workspace();
                        self.refresh(repo.id);
                    }
                    Err(error) => self.emit(Event::error(error.to_string())),
                }
            }
            Action::Scan { path } => self.scan(path),
            Action::Clone { url, parent } => self.clone_repo(url, parent),
            // Removing, editing and (un)favouriting change no repository's git
            // status, so they resend the workspace without a status storm —
            // which is what used to make the list flicker on every change.
            Action::Remove { repo } => self.after_workspace_change(self.hub.remove(&repo)),
            Action::Edit { repo, name, group, install_command, web_url, name_style } => {
                let edit = RepoEdit { name, group, install_command, web_url, name_style };
                self.after_workspace_change(self.hub.update_repo(&repo, edit).map(drop));
            }
            Action::Favorite { repo, favorite } => {
                self.after_workspace_change(self.hub.set_favorite(&repo, favorite).map(drop));
            }
            Action::SetGroup { repo, group } => {
                self.after_workspace_change(self.hub.set_group(&repo, group).map(drop));
            }
            Action::Export { folder } => {
                match self.hub.export_repos(Path::new(folder.trim())) {
                    Ok(path) => self.emit(Event::info(format!("Exported the repository list to {}", path.display()))),
                    Err(error) => self.emit(Event::error(error.to_string())),
                }
            }
            Action::Import { folder, dest } => self.import(folder, dest),
            Action::InstallRecipes { request, repo } => {
                let reply = self.hub.install_recipes(&repo).and_then(|r| to_json(&r));
                self.reply(request, reply);
            }
            Action::Run { repo, command, title } => {
                let title = title.unwrap_or_else(|| command.clone());
                self.job(Some(repo.clone()), title, Afterwards::Status, Runner::OwnThread, move |hub, out| {
                    hub.run_command(&repo, &command, out)
                });
            }
            Action::SaveSettings { settings } => {
                let result = self.hub.save_settings(settings);
                self.after_workspace_change(result);
            }
            Action::SaveTools { tools } => {
                let result = self.hub.save_tools(tools);
                self.after_workspace_change(result);
            }
            // Off the frame thread: installing copies the whole build.
            Action::Installation { request, action } => {
                let events = self.events.clone();
                jobs::spawn(move || {
                    let _ = events.send(reply_event(request, installation(action)));
                });
            }
            // Desktop capabilities belong to the bones `os`, `desktop` and `upgrader` modules; the
            // dashboard module answers these before they reach here.
            Action::PickFolder { .. } | Action::OpenUrl { .. } | Action::Copy { .. } | Action::Desktop { .. } | Action::Update { .. } => {}
        }
    }

    pub fn reply(&self, request: u64, result: HubResult<serde_json::Value>) {
        self.emit(reply_event(request, result));
    }

    // ---- actions -------------------------------------------------------

    /// Watches every tracked repository, and only those, at its current path.
    fn sync_watches(&mut self) {
        self.watched_revision = Some(self.hub.revision());
        let repos = self.hub.workspace().repos;
        self.watches.retain(|id, (path, _)| repos.iter().any(|r| &r.id == id && &r.path == path));
        for repo in repos {
            if self.watches.contains_key(&repo.id) {
                continue;
            }
            let id = repo.id.clone();
            let changed = Mutex::new(self.changed.0.clone());
            let on_change = Box::new(move || {
                let _ = changed.lock().unwrap().send(id.clone());
            });
            // An inactive watch (no `.git` yet) is retried at the next change.
            if let Ok(handle) = self.hub.watch(&repo.id, on_change) {
                if handle.is_active() {
                    self.watches.insert(repo.id, (repo.path, handle));
                }
            }
        }
    }

    /// Tracks the repository at `path`, or every one under it, then selects
    /// the first. Off the frame thread, since a scan walks the disk.
    pub fn open_path(&self, path: PathBuf) {
        let hub = Arc::clone(&self.hub);
        let events = self.events.clone();
        jobs::spawn(move || match hub.add_or_scan(&path) {
            Ok(repos) if repos.is_empty() => {
                let _ = events.send(Event::info(format!("No repositories in {}", path.display())));
            }
            Ok(repos) => {
                let _ = events.send(workspace_event(&hub));
                for repo in &repos {
                    let _ = events.send(status_event(&hub, &repo.id));
                }
                let _ = events.send(Event::Select { repo: repos[0].id.clone() });
            }
            Err(error) => {
                let _ = events.send(Event::error(error.to_string()));
            }
        });
    }

    fn fix(&self, repo: RepoId, fix: Fix, message: Option<String>) {
        if fix == Fix::OpenInTool {
            let result = self.hub.open_in(&repo, None);
            self.report(result);
            return;
        }
        let title = match fix {
            Fix::Reclone => "Re-clone",
            Fix::Pull => "Pull",
            Fix::Sync => "Sync",
            Fix::Push => "Push",
            Fix::Publish => "Publish branch",
            Fix::CommitAll => "Commit",
            Fix::Stash => "Stash",
            Fix::OpenInTool => unreachable!("handled above"),
        };
        self.git_job(repo, title, move |hub, id, out| hub.apply_fix(id, fix, message.as_deref(), out));
    }

    fn scan(&self, path: String) {
        self.job(None, format!("Scan {path}"), Afterwards::Workspace, Runner::OwnThread, move |hub, out| {
            let added = hub.scan(Path::new(path.trim()))?;
            out.line(&format!("{} new repositories", added.len()));
            for repo in &added {
                out.line(&format!("  + {} ({})", repo.name, repo.path.display()));
            }
            Ok(())
        });
    }

    fn clone_repo(&self, url: String, parent: Option<String>) {
        let title = format!("Clone {}", devpanel_core::domain::repo_name_from_url(&url).unwrap_or_else(|| url.clone()));
        self.job(None, title, Afterwards::Workspace, Runner::OwnThread, move |hub, out| {
            let parent = parent.filter(|p| !p.trim().is_empty()).map(PathBuf::from);
            hub.clone_repo(&url, parent.as_deref(), out).map(drop)
        });
    }

    fn import(&self, folder: String, dest: Option<String>) {
        self.job(None, "Import repositories".into(), Afterwards::Workspace, Runner::OwnThread, move |hub, out| {
            let dest = dest.filter(|d| !d.trim().is_empty()).map(PathBuf::from);
            hub.import_repos(Path::new(folder.trim()), dest.as_deref(), out).map(drop)
        });
    }

    fn fetch_all(&self) {
        for repo in self.hub.workspace().repos.into_iter().filter(|r| r.remote.is_some()) {
            let id = repo.id.clone();
            let title = format!("Fetch {}", repo.name);
            self.job(Some(repo.id), title, Afterwards::Status, Runner::Pool(&self.network_pool), move |hub, out| {
                hub.fetch(&id, out)
            });
        }
    }

    /// Pulls in the background every repository not pulled within the
    /// configured number of hours.
    fn pull_stale(&self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default();
        let due = match self.hub.take_due_pulls(now) {
            Ok(due) => due,
            Err(error) => return self.emit(Event::error(error.to_string())),
        };
        for id in due {
            let name = self.hub.repo(&id).map(|r| r.name).unwrap_or_else(|_| id.0.clone());
            let repo = id.clone();
            self.job(Some(id), format!("Auto-pull {name}"), Afterwards::Status, Runner::Pool(&self.network_pool), move |hub, out| {
                hub.auto_pull(&repo, out)
            });
        }
    }

    fn git_job(
        &self,
        repo: RepoId,
        verb: &str,
        work: impl FnOnce(&Hub, &RepoId, &dyn Output) -> HubResult<()> + Send + 'static,
    ) {
        let name = self.hub.repo(&repo).map(|r| r.name).unwrap_or_else(|_| repo.0.clone());
        let id = repo.clone();
        self.job(Some(repo), format!("{verb} {name}"), Afterwards::Status, Runner::OwnThread, move |hub, out| {
            work(hub, &id, out)
        });
    }

    // ---- jobs ------------------------------------------------------------

    fn job(
        &self,
        repo: Option<RepoId>,
        title: String,
        afterwards: Afterwards,
        runner: Runner<'_>,
        work: impl FnOnce(&Hub, &dyn Output) -> HubResult<()> + Send + 'static,
    ) {
        if let Some(repo) = &repo {
            if !self.busy.lock().unwrap().insert(repo.clone()) {
                self.emit(Event::info("That repository is busy; wait for its current job to finish"));
                return;
            }
        }
        let job = self.next_job.fetch_add(1, Ordering::Relaxed) + 1;
        self.emit(Event::Job { job, repo: repo.clone(), title: title.clone(), state: JobState::Running, error: None });

        let hub = Arc::clone(&self.hub);
        let events = self.events.clone();
        let busy = Arc::clone(&self.busy);
        let task = move || {
            let output = JobOutput { job, events: events.clone() };
            let result = work(&hub, &output);
            if let Some(repo) = &repo {
                busy.lock().unwrap().remove(repo);
            }
            let (state, error) = match &result {
                Ok(()) => (JobState::Succeeded, None),
                Err(error) => (JobState::Failed, Some(error.to_string())),
            };
            let _ = events.send(Event::Job { job, repo: repo.clone(), title, state, error });
            match (afterwards, &repo) {
                (Afterwards::Status, Some(repo)) => {
                    let _ = events.send(status_event(&hub, repo));
                }
                _ => {
                    let _ = events.send(workspace_event(&hub));
                    for repo in hub.workspace().repos {
                        let _ = events.send(status_event(&hub, &repo.id));
                    }
                }
            }
        };
        match runner {
            Runner::OwnThread => jobs::spawn(task),
            Runner::Pool(pool) => pool.submit(task),
        }
    }

    fn refresh(&self, repo: RepoId) {
        if !self.refreshing.lock().unwrap().insert(repo.clone()) {
            return;
        }
        let hub = Arc::clone(&self.hub);
        let events = self.events.clone();
        let refreshing = Arc::clone(&self.refreshing);
        self.status_pool.submit(move || {
            let event = status_event(&hub, &repo);
            refreshing.lock().unwrap().remove(&repo);
            let _ = events.send(event);
        });
    }

    fn refresh_all(&self) {
        for repo in self.hub.workspace().repos {
            self.refresh(repo.id);
        }
    }

    // ---- events ----------------------------------------------------------

    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
    }

    fn send_workspace(&self) {
        let event = workspace_event(&self.hub);
        if let Event::Workspace { commits, available, .. } = &event {
            *self.installed.lock().unwrap() = (*commits, available.clone());
        }
        self.emit(event);
    }

    fn report(&self, result: HubResult<()>) {
        if let Err(error) = result {
            self.emit(Event::error(error.to_string()));
        }
    }

    /// The workspace changed but no repository's git status did: resend it and
    /// stop, leaving the page's cached statuses in place.
    fn after_workspace_change(&self, result: HubResult<()>) {
        match result {
            Ok(()) => self.send_workspace(),
            Err(error) => self.emit(Event::error(error.to_string())),
        }
    }
}

fn status_event(hub: &Hub, repo: &RepoId) -> Event {
    match hub.status(repo) {
        Ok(status) => {
            let remote = hub.repo(repo).ok().and_then(|r| r.remote);
            Event::status(repo.clone(), status, remote.as_deref())
        }
        Err(error) => Event::error(error.to_string()),
    }
}

fn workspace_event(hub: &Hub) -> Event {
    let platform = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    Event::Workspace {
        workspace: hub.workspace(),
        platform,
        version: env!("CARGO_PKG_VERSION"),
        commits: hub.commits_available(),
        available: hub.available_tools(),
    }
}

fn reply_event(request: u64, result: HubResult<serde_json::Value>) -> Event {
    match result {
        Ok(data) => Event::Reply { request, ok: true, data, error: None },
        Err(error) => Event::Reply { request, ok: false, data: serde_json::Value::Null, error: Some(error.to_string()) },
    }
}

/// The installation status, after installing the running build for `install`.
fn installation(action: InstallationAction) -> HubResult<serde_json::Value> {
    let report = match action {
        InstallationAction::Status => None,
        InstallationAction::Install => Some(crate::install::install().map_err(HubError::Command)?),
    };
    let mut reply = to_json(&crate::installation::status())?;
    if let Some(report) = report {
        reply["report"] = serde_json::json!(report);
    }
    Ok(reply)
}

fn to_json(value: &impl serde::Serialize) -> HubResult<serde_json::Value> {
    serde_json::to_value(value).map_err(|e| HubError::Invalid(e.to_string()))
}

/// Streams a job's output to the page as `log` events.
struct JobOutput {
    job: u64,
    events: Sender<Event>,
}

impl Output for JobOutput {
    fn line(&self, text: &str) {
        let _ = self.events.send(Event::Log { job: self.job, line: text.to_string() });
    }
}

#[cfg(test)]
mod tests;
