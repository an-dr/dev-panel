//! Runs the real `git` against throwaway repositories: the parser tests
//! cannot show that the commands themselves are right.

use std::process::Command;

use devpanel_core::domain::{Health, Operation};
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
