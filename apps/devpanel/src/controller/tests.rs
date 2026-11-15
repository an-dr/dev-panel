//! Drives the controller with real adapters against a throwaway repository,
//! the way the page would, and checks the events that come back.

use std::process::Command;
use std::time::{Duration, Instant};

use devpanel_core::domain::{Health, Platform};
use devpanel_infra::{CommitsSourceFile, GitCli, GitWatch, JsonStore, LocalFileSystem, SystemLauncher, SystemShell};

use super::*;

fn controller(config: &Path) -> Controller {
    let ports = devpanel_core::hub::Ports {
        git: Arc::new(GitCli),
        shell: Arc::new(SystemShell),
        launcher: Arc::new(SystemLauncher),
        fs: Arc::new(LocalFileSystem),
        store: Arc::new(JsonStore::new(config.join("workspace.json"))),
        watch: Arc::new(GitWatch::new()),
        source: Arc::new(CommitsSourceFile::in_dir(config.join("sources"))),
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
    assert!(events.iter().any(|e| matches!(e, Event::Workspace { version, .. } if *version == env!("CARGO_PKG_VERSION"))));
    // Whether Commits is installed on the machine running the tests is not
    // fixed, so only the field's presence is checked here; the hub tests
    // cover what decides it.
    let json = serde_json::to_value(events.iter().find(|e| matches!(e, Event::Workspace { .. })).unwrap()).unwrap();
    assert!(json["commits"].is_boolean());
    let Some(Event::Status { health, .. }) = events.last() else { unreachable!() };
    assert_eq!(*health, Health::Clean);
}

#[test]
fn a_startup_folder_is_opened_and_selected_once_the_page_is_ready() {
    let root = tempfile::tempdir().unwrap();
    init_repo(&root.path().join("clones").join("one"));
    init_repo(&root.path().join("clones").join("two"));
    let controller = controller(root.path()).with_startup_path(Some(root.path().join("clones")));

    controller.handle(Action::Ready);
    let events = wait_for(&controller, |e| matches!(e, Event::Select { .. }));

    let Some(Event::Workspace { workspace, .. }) = events.iter().rev().find(|e| matches!(e, Event::Workspace { .. })) else {
        panic!("no workspace in {events:?}");
    };
    assert_eq!(workspace.repos.len(), 2);
    let Some(Event::Select { repo }) = events.last() else { unreachable!() };
    assert!(workspace.repos.iter().any(|r| &r.id == repo));
}

#[test]
fn a_commit_made_outside_devpanel_refreshes_that_repository() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("project");
    init_repo(&repo);
    let mut controller = controller(root.path());
    controller.handle(Action::AddExisting { path: repo.to_string_lossy().into_owned() });
    wait_for(&controller, |e| matches!(e, Event::Status { .. }));
    controller.tick(Instant::now());

    let ok = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "outside"])
        .status()
        .unwrap()
        .success();
    assert!(ok);

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        controller.tick(Instant::now());
        let events = controller.drain();
        let seen = events.iter().any(|e| {
            matches!(e, Event::Status { status, .. } if status.last_commit.as_ref().is_some_and(|c| c.subject == "outside"))
        });
        if seen {
            break;
        }
        assert!(Instant::now() < deadline, "no refreshed status after the outside commit");
        std::thread::sleep(Duration::from_millis(20));
    }
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

    controller.job(Some(id.clone()), "echo".into(), Afterwards::Status, Runner::OwnThread, |_, out| {
        out.line("hello");
        Ok(())
    });
    let events = wait_for(&controller, |e| matches!(e, Event::Job { state: JobState::Succeeded, .. }));
    assert!(events.iter().any(|e| matches!(e, Event::Log { line, .. } if line == "hello")));

    controller.job(Some(id), "fail".into(), Afterwards::Status, Runner::OwnThread, |_, _| Err(HubError::Command("exit 3".into())));
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

    controller.job(Some(id.clone()), "slow".into(), Afterwards::Status, Runner::OwnThread, |_, _| {
        std::thread::sleep(Duration::from_secs(1));
        Ok(())
    });
    controller.job(Some(id), "second".into(), Afterwards::Status, Runner::OwnThread, |_, _| Ok(()));
    let events = wait_for(&controller, |e| matches!(e, Event::Job { state: JobState::Succeeded, .. }));
    let started = events.iter().filter(|e| matches!(e, Event::Job { state: JobState::Running, .. })).count();
    assert_eq!(started, 1);
    assert!(events.iter().any(|e| matches!(e, Event::Toast { level: "info", message } if message.contains("busy"))));
}

#[test]
fn a_repository_without_a_justfile_answers_not_found() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("project");
    init_repo(&repo);
    let controller = controller(root.path());
    controller.handle(Action::AddExisting { path: repo.to_string_lossy().into_owned() });

    controller.handle(Action::JustRecipes { request: 7, repo: RepoId("r1".into()) });
    let events = wait_for(&controller, |e| matches!(e, Event::Reply { .. }));
    let Some(Event::Reply { request, ok, data, .. }) = events.last() else { unreachable!() };
    assert_eq!((*request, *ok), (7, true));
    assert_eq!(data["found"], false);
    assert_eq!(data["recipes"], serde_json::json!([]));
}

#[test]
fn status_says_whether_the_repository_has_a_justfile() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("project");
    init_repo(&repo);
    std::fs::write(repo.join("justfile"), "test:\n    echo ok\n").unwrap();
    let controller = controller(root.path());
    controller.handle(Action::AddExisting { path: repo.to_string_lossy().into_owned() });
    let events = wait_for(&controller, |e| matches!(e, Event::Status { .. }));
    assert!(matches!(events.last(), Some(Event::Status { justfile: true, .. })));
}
