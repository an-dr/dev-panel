//! Commits' repository-health conformance vectors, run against DevPanel's
//! own verdict function (see README.md beside this file).

use devpanel_core::domain::{Health, Operation, RepoStatus};
use serde_json::Value;

const VECTORS: &str = include_str!("repository-health.vectors.json");

/// Every verdict, in DevPanel's order, which must be the contract's.
const VERDICTS: [Health; 11] = [
    Health::Missing,
    Health::NotARepo,
    Health::Conflicted,
    Health::InProgress,
    Health::Diverged,
    Health::Behind,
    Health::Dirty,
    Health::Ahead,
    Health::Detached,
    Health::Unpublished,
    Health::Clean,
];

fn vectors() -> Value {
    serde_json::from_str(VECTORS).unwrap()
}

fn id(health: Health) -> String {
    serde_json::to_value(health).unwrap().as_str().unwrap().to_string()
}

/// The contract's facts as the status DevPanel's git adapter would report.
fn status(facts: &Value) -> RepoStatus {
    let flag = |name: &str| facts[name].as_bool().unwrap();
    let count = |name: &str| u32::try_from(facts[name].as_u64().unwrap()).unwrap();
    let operation = facts["operation"].as_str().map(|op| match op {
        "merge" => Operation::Merge,
        "rebase" => Operation::Rebase,
        "cherry-pick" => Operation::CherryPick,
        "revert" => Operation::Revert,
        "bisect" => Operation::Bisect,
        other => panic!("unknown operation {other}"),
    });
    RepoStatus {
        exists: flag("exists"),
        is_repo: flag("isRepo"),
        conflicted: count("conflicted"),
        operation,
        ahead: count("ahead"),
        behind: count("behind"),
        unstaged: count("changes"),
        branch: (!flag("detached")).then(|| "main".to_string()),
        has_commits: flag("hasCommits"),
        upstream: flag("hasUpstream").then(|| "origin/main".to_string()),
        remote_url: flag("hasRemote").then(|| "https://example.com/r.git".to_string()),
        ..RepoStatus::default()
    }
}

#[test]
fn every_case_gets_the_contract_verdict() {
    let vectors = vectors();
    assert_eq!(vectors["version"], 1);
    let cases = vectors["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let verdict = id(Health::of(&status(&case["facts"])));
        assert_eq!(verdict, case["verdict"].as_str().unwrap(), "case {}", case["name"]);
    }
}

#[test]
fn the_verdicts_and_their_order_are_the_contract() {
    let vectors = vectors();
    let contract: Vec<&str> = vectors["verdicts"].as_array().unwrap().iter().map(|v| v["id"].as_str().unwrap()).collect();
    let ours: Vec<String> = VERDICTS.iter().copied().map(id).collect();
    assert_eq!(ours, contract);
    let mut sorted = VERDICTS;
    sorted.sort();
    assert_eq!(sorted, VERDICTS, "Health's order is worst first, as the contract lists it");
}
