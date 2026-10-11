//! Drives the controller with real adapters against a throwaway repository,
//! the way the page would, and checks the events that come back.

use std::process::Command;
use std::time::{Duration, Instant};

use devpanel_core::domain::{Health, Platform};
use devpanel_infra::{GitCli, JsonStore, LocalFileSystem, SystemLauncher, SystemShell};

use super::*;

fn controller(config: &Path) -> Controller {
    let ports = devpanel_core::hub::Ports {
        git: Arc::new(GitCli),
        shell: Arc::new(SystemShell),
        launcher: Arc::new(SystemLauncher),
        fs: Arc::new(LocalFileSystem),
        store: Arc::new(JsonStore::new(config.join("workspace.json"))),
    };
    Controller::new(Arc::new(Hub::open(ports, Platform::current()).unwrap()))
}

fn init_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let ok = Command::new("git").arg("-C").arg(dir).args(["init", "-q"]).status().unwrap().success();
    assert!(ok);
}

/// Collects events until `done` matches one, or fails after a few seconds.
fn wait_for(controller: &Controller, done: impl Fn(&Event) -> bool) -> Vec<Event> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        for event in controller.drain() {
            let finished = done(&event);
            seen.push(event);
            if finished {
                return seen;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out; saw {seen:?}");
}

#[test]
fn adding_a_repository_sends_the_workspace_and_its_status() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("project");
    init_repo(&repo);
    let controller = controller(root.path());

    controller.handle(Action::AddExisting { path: repo.to_string_lossy().into_owned() });
    let events = wait_for(&controller, |e| matches!(e, Event::Status { .. }));

    assert!(events.iter().any(|e| matches!(e, Event::Workspace { workspace, .. } if workspace.repos.len() == 1)));
    let Some(Event::Status { health, .. }) = events.last() else { unreachable!() };
    assert_eq!(*health, Health::Clean);
}

#[test]
fn failures_reach_the_page_as_toasts() {
    let root = tempfile::tempdir().unwrap();
    let controller = controller(root.path());
    controller.handle(Action::AddExisting { path: root.path().join("missing").to_string_lossy().into_owned() });
    let events = wait_for(&controller, |e| matches!(e, Event::Toast { .. }));
    assert!(matches!(events.last(), Some(Event::Toast { level: "error", .. })));
}

#[test]
fn a_job_streams_output_then_reports_its_outcome() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("project");
    init_repo(&repo);
    let controller = controller(root.path());
    controller.handle(Action::AddExisting { path: repo.to_string_lossy().into_owned() });
    let id = RepoId("r1".into());

    controller.handle(Action::Run { repo: id.clone(), command: "echo hello".into(), title: None });
    let events = wait_for(&controller, |e| matches!(e, Event::Job { state: JobState::Succeeded, .. }));
    assert!(events.iter().any(|e| matches!(e, Event::Log { line, .. } if line == "hello")));

    controller.handle(Action::Run { repo: id, command: "exit 3".into(), title: None });
    let events = wait_for(&controller, |e| matches!(e, Event::Job { state: JobState::Failed, .. }));
    assert!(matches!(events.last(), Some(Event::Job { error: Some(_), .. })));
}

#[test]
fn a_second_job_on_a_busy_repository_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("project");
    init_repo(&repo);
    let controller = controller(root.path());
    controller.handle(Action::AddExisting { path: repo.to_string_lossy().into_owned() });
    let id = RepoId("r1".into());
    let slow = if cfg!(windows) { "ping -n 2 127.0.0.1 > nul" } else { "sleep 1" };

    controller.handle(Action::Run { repo: id.clone(), command: slow.into(), title: None });
    controller.handle(Action::Run { repo: id, command: "echo second".into(), title: None });
    let events = wait_for(&controller, |e| matches!(e, Event::Job { state: JobState::Succeeded, .. }));
    let started = events.iter().filter(|e| matches!(e, Event::Job { state: JobState::Running, .. })).count();
    assert_eq!(started, 1);
    assert!(events.iter().any(|e| matches!(e, Event::Toast { level: "info", message } if message.contains("busy"))));
}

#[test]
fn install_recipes_are_answered_to_the_request() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("project");
    init_repo(&repo);
    std::fs::write(repo.join("Cargo.toml"), "").unwrap();
    let controller = controller(root.path());
    controller.handle(Action::AddExisting { path: repo.to_string_lossy().into_owned() });

    controller.handle(Action::InstallRecipes { request: 42, repo: RepoId("r1".into()) });
    let events = wait_for(&controller, |e| matches!(e, Event::Reply { .. }));
    let Some(Event::Reply { request, ok, data, .. }) = events.last() else { unreachable!() };
    assert_eq!((*request, *ok), (42, true));
    assert_eq!(data[0]["label"], "cargo install");
}
