//! Turns page actions into hub use cases and hub results into page events.
//!
//! Knows nothing about bones: it takes [`Action`]s and queues [`Event`]s,
//! and the [`crate::dashboard`] module moves them across the web panel. That
//! is what lets the controller be tested without a window.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use devpanel_core::domain::{Fix, RepoId};
use devpanel_core::hub::RepoEdit;
use devpanel_core::ports::Output;
use devpanel_core::{Hub, HubError, HubResult};

use crate::jobs::{self, Pool};
use crate::protocol::{Action, Event, JobState};

/// Concurrent `git status` reads.
const STATUS_WORKERS: usize = 4;
/// Concurrent fetches during "Fetch all".
const NETWORK_WORKERS: usize = 4;

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
        }
    }

    /// Events produced since the last call, in order.
    pub fn drain(&self) -> Vec<Event> {
        self.inbox.try_iter().collect()
    }

    /// Called every frame; runs the periodic fetch when it is due.
    pub fn tick(&mut self, now: Instant) {
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
            }
            Action::Refresh { repo: Some(repo) } => self.refresh(repo),
            Action::Refresh { repo: None } => self.refresh_all(),
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
            Action::Edit { repo, name, group, install_command } => {
                self.after_workspace_change(self.hub.update_repo(&repo, RepoEdit { name, group, install_command }).map(drop));
            }
            Action::Favorite { repo, favorite } => {
                self.after_workspace_change(self.hub.set_favorite(&repo, favorite).map(drop));
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
            // Desktop capabilities belong to the bones `os` module; the
            // dashboard module answers these before they reach here.
            Action::PickFolder { .. } | Action::OpenUrl { .. } | Action::Copy { .. } => {}
        }
    }

    pub fn reply(&self, request: u64, result: HubResult<serde_json::Value>) {
        let event = match result {
            Ok(data) => Event::Reply { request, ok: true, data, error: None },
            Err(error) => Event::Reply { request, ok: false, data: serde_json::Value::Null, error: Some(error.to_string()) },
        };
        self.emit(event);
    }

    // ---- actions -------------------------------------------------------

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
        self.emit(workspace_event(&self.hub));
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
            let has_url = hub.repo(repo).is_ok_and(|r| r.remote.is_some());
            Event::status(repo.clone(), status, has_url)
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
    Event::Workspace { workspace: hub.workspace(), platform, version: env!("CARGO_PKG_VERSION") }
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
