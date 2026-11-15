//! Runs the real `git` against throwaway repositories: the parser tests
//! cannot show that the commands themselves are right.

use std::process::Command;

use devpanel_core::domain::{Health, Operation, SubmoduleState};
use devpanel_core::ports::Silent;

use super::*;

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "git {args:?} failed");
}

/// A bare "server" and one clone of it with an initial commit pushed.
fn upstream_and_clone() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let server = root.path().join("server.git");
    let work = root.path().join("work");
    git(root.path(), &["init", "--bare", "-b", "main", server.to_str().unwrap()]);
    git(root.path(), &["clone", server.to_str().unwrap(), work.to_str().unwrap()]);
    std::fs::write(work.join("a.txt"), "a").unwrap();
    git(&work, &["add", "."]);
    git(&work, &["commit", "-m", "first"]);
    git(&work, &["push", "-u", "origin", "main"]);
    (root, server, work)
}

#[test]
fn missing_and_plain_folders() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(Health::of(&GitCli.status(&root.path().join("nope"))), Health::Missing);
    assert_eq!(Health::of(&GitCli.status(root.path())), Health::NotARepo);
}

#[test]
fn subfolder_of_a_repository_is_not_a_repository() {
    let (_root, _server, work) = upstream_and_clone();
    std::fs::create_dir(work.join("sub")).unwrap();
    assert!(!GitCli.status(&work.join("sub")).is_repo);
}

#[test]
fn clean_clone_reports_branch_upstream_and_last_commit() {
    let (_root, _server, work) = upstream_and_clone();
    let status = GitCli.status(&work);
    assert_eq!(Health::of(&status), Health::Clean);
    assert_eq!(status.branch.as_deref(), Some("main"));
    assert_eq!(status.upstream.as_deref(), Some("origin/main"));
    assert_eq!(status.last_commit.unwrap().subject, "first");
    assert!(status.remote_url.unwrap().ends_with("server.git"));
}

#[test]
fn ahead_then_push_then_clean() {
    let (_root, _server, work) = upstream_and_clone();
    std::fs::write(work.join("b.txt"), "b").unwrap();
    assert_eq!(Health::of(&GitCli.status(&work)), Health::Dirty);
    git(&work, &["add", "."]);
    git(&work, &["-c", "user.name=t", "commit", "-m", "second"]);
    assert_eq!(Health::of(&GitCli.status(&work)), Health::Ahead);
    GitCli.push(&work, false, &Silent).unwrap();
    assert_eq!(Health::of(&GitCli.status(&work)), Health::Clean);
}

#[test]
fn behind_after_fetch_and_pull_catches_up() {
    let (root, server, work) = upstream_and_clone();
    let other = root.path().join("other");
    git(root.path(), &["clone", server.to_str().unwrap(), other.to_str().unwrap()]);
    std::fs::write(other.join("c.txt"), "c").unwrap();
    git(&other, &["add", "."]);
    git(&other, &["commit", "-m", "from elsewhere"]);
    git(&other, &["push"]);

    GitCli.fetch(&work, &Silent).unwrap();
    let status = GitCli.status(&work);
    assert_eq!((Health::of(&status), status.behind), (Health::Behind, 1));
    GitCli.pull(&work, false, &Silent).unwrap();
    assert_eq!(Health::of(&GitCli.status(&work)), Health::Clean);
}

#[test]
fn new_branch_is_unpublished_until_pushed_with_upstream() {
    let (_root, _server, work) = upstream_and_clone();
    git(&work, &["switch", "-c", "feature"]);
    assert_eq!(Health::of(&GitCli.status(&work)), Health::Unpublished);
    GitCli.push(&work, true, &Silent).unwrap();
    assert_eq!(GitCli.status(&work).upstream.as_deref(), Some("origin/feature"));
}

#[test]
fn stash_cleans_the_tree_and_is_counted() {
    let (_root, _server, work) = upstream_and_clone();
    std::fs::write(work.join("scratch.txt"), "x").unwrap();
    GitCli.stash(&work, &Silent).unwrap();
    let status = GitCli.status(&work);
    assert_eq!((status.changes(), status.stashes), (0, 1));
}

#[test]
fn merge_conflict_is_reported() {
    let (root, server, work) = upstream_and_clone();
    let other = root.path().join("other");
    git(root.path(), &["clone", server.to_str().unwrap(), other.to_str().unwrap()]);
    std::fs::write(other.join("a.txt"), "theirs").unwrap();
    git(&other, &["commit", "-am", "theirs"]);
    git(&other, &["push"]);
    std::fs::write(work.join("a.txt"), "ours").unwrap();
    git(&work, &["commit", "-am", "ours"]);
    git(&work, &["fetch"]);
    // The merge is expected to fail with a conflict; only the state matters.
    let _ = Command::new("git").arg("-C").arg(&work).args(["-c", "user.name=t", "-c", "user.email=t@t", "merge", "origin/main"]).output();
    let status = GitCli.status(&work);
    assert_eq!(status.operation, Some(Operation::Merge));
    assert_eq!(Health::of(&status), Health::Conflicted);
}

#[test]
fn failing_command_reports_git_output() {
    let root = tempfile::tempdir().unwrap();
    let error = GitCli.clone_repo("/definitely/not/here", &root.path().join("x"), &Silent).unwrap_err();
    assert!(error.to_string().contains("not") || error.to_string().contains("exist"), "{error}");
}

/// A repository with one commit, to add as a submodule.
fn library(root: &Path, name: &str) -> PathBuf {
    let lib = root.join(name);
    git(root, &["init", "-b", "main", lib.to_str().unwrap()]);
    std::fs::write(lib.join("lib.txt"), "lib").unwrap();
    git(&lib, &["add", "."]);
    git(&lib, &["commit", "-m", "lib"]);
    lib
}

fn add_submodule(parent: &Path, url: &Path, path: &str) {
    git(parent, &["-c", "protocol.file.allow=always", "submodule", "add", url.to_str().unwrap(), path]);
    git(parent, &["commit", "-m", &format!("add {path}")]);
}

#[test]
fn submodules_report_their_state_and_update_back_to_the_recorded_commit() {
    let (root, _server, work) = upstream_and_clone();
    let lib = library(root.path(), "lib");
    add_submodule(&work, &lib, "vendor/lib");

    let status = GitCli.status(&work);
    assert_eq!(status.submodules.len(), 1);
    let sub = &status.submodules[0];
    assert_eq!((sub.path.as_str(), sub.state), ("vendor/lib", SubmoduleState::Clean));

    // Changes inside, then a commit inside: modified, then moved.
    let inside = work.join("vendor/lib");
    std::fs::write(inside.join("lib.txt"), "changed").unwrap();
    assert_eq!(GitCli.status(&work).submodules[0].state, SubmoduleState::Modified);
    git(&inside, &["commit", "-am", "inside"]);
    let moved = GitCli.status(&work).submodules[0].clone();
    assert_eq!(moved.state, SubmoduleState::Moved);
    assert_ne!(moved.checked_out.as_deref(), Some(moved.recorded.as_str()));

    GitCli.submodule_update(&work, Some("vendor/lib"), &Silent).unwrap();
    assert_eq!(GitCli.status(&work).submodules[0].state, SubmoduleState::Clean);
    // The submodule is a repository of its own, detached as submodules are.
    let own = GitCli.status(&inside);
    assert!(own.is_repo);
    assert_eq!(own.branch, None);
}

#[test]
fn a_fresh_clone_lists_uninitialized_submodules_until_updated() {
    let (root, _server, work) = upstream_and_clone();
    let lib = library(root.path(), "lib");
    let inner = library(root.path(), "inner");
    add_submodule(&lib, &inner, "inner");
    add_submodule(&work, &lib, "lib");
    let copy = root.path().join("copy");
    git(root.path(), &["clone", work.to_str().unwrap(), copy.to_str().unwrap()]);

    assert_eq!(GitCli.status(&copy).submodules[0].state, SubmoduleState::Uninitialized);
    std::env::set_var("GIT_CONFIG_COUNT", "1");
    std::env::set_var("GIT_CONFIG_KEY_0", "protocol.file.allow");
    std::env::set_var("GIT_CONFIG_VALUE_0", "always");
    GitCli.submodule_update(&copy, None, &Silent).unwrap();
    assert_eq!(GitCli.status(&copy).submodules[0].state, SubmoduleState::Clean);
    // The nested submodule was cloned too, and reports in its own parent.
    assert_eq!(GitCli.status(&copy.join("lib")).submodules[0].state, SubmoduleState::Clean);
}
