use serde::{Deserialize, Serialize};

use super::RepoStatus;

/// The one-word verdict on a repository, worst first.
///
/// A repository can be several of these at once (dirty *and* behind); the
/// dashboard shows the most urgent, and [`Health::fixes`] lists every action
/// that would improve it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Health {
    Missing,
    NotARepo,
    Conflicted,
    InProgress,
    Diverged,
    Behind,
    Dirty,
    Ahead,
    Unpublished,
    Detached,
    Clean,
}

/// An action that moves a repository towards [`Health::Clean`]. The
/// presentation decides how to show it; the hub knows how to run it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fix {
    /// Clone the remote back into the missing folder.
    Reclone,
    Pull,
    /// Pull with rebase, then push: the usual way out of divergence.
    Sync,
    Push,
    /// First push of a branch with no upstream (`push -u origin HEAD`).
    Publish,
    /// Stage everything and commit with a message the user gives.
    CommitAll,
    Stash,
    /// Nothing automatic is safe: hand the repository to a tool.
    OpenInTool,
}

impl Health {
    pub fn of(status: &RepoStatus) -> Health {
        if !status.exists {
            return Health::Missing;
        }
        if !status.is_repo {
            return Health::NotARepo;
        }
        if status.conflicted > 0 {
            return Health::Conflicted;
        }
        if status.operation.is_some() {
            return Health::InProgress;
        }
        if status.ahead > 0 && status.behind > 0 {
            return Health::Diverged;
        }
        if status.behind > 0 {
            return Health::Behind;
        }
        if status.changes() > 0 {
            return Health::Dirty;
        }
        if status.ahead > 0 {
            return Health::Ahead;
        }
        if status.branch.is_none() && status.has_commits {
            return Health::Detached;
        }
        if status.upstream.is_none() && status.remote_url.is_some() && status.has_commits {
            return Health::Unpublished;
        }
        Health::Clean
    }

    /// Every fix that applies to `status`, most useful first. Not limited to
    /// the headline verdict: a repository that is behind *and* dirty offers
    /// both a commit and a pull.
    pub fn fixes(status: &RepoStatus, has_remote_url: bool) -> Vec<Fix> {
        let mut fixes = Vec::new();
        match Health::of(status) {
            Health::Missing => {
                if has_remote_url {
                    fixes.push(Fix::Reclone);
                }
                return fixes;
            }
            Health::NotARepo => return fixes,
            Health::Conflicted | Health::InProgress => {
                fixes.push(Fix::OpenInTool);
                return fixes;
            }
            _ => {}
        }
        let dirty = status.changes() > 0;
        if dirty {
            fixes.push(Fix::CommitAll);
        }
        if status.ahead > 0 && status.behind > 0 {
            if !dirty {
                fixes.push(Fix::Sync);
            }
        } else if status.behind > 0 {
            fixes.push(Fix::Pull);
        } else if status.ahead > 0 {
            fixes.push(Fix::Push);
        }
        if status.upstream.is_none() && status.branch.is_some() && status.remote_url.is_some() && status.has_commits {
            fixes.push(Fix::Publish);
        }
        if dirty {
            fixes.push(Fix::Stash);
        }
        fixes
    }

    /// Whether the dashboard should flag this repository for attention.
    pub fn needs_attention(self) -> bool {
        self != Health::Clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean() -> RepoStatus {
        RepoStatus {
            exists: true,
            is_repo: true,
            branch: Some("main".into()),
            upstream: Some("origin/main".into()),
            has_commits: true,
            remote_url: Some("u".into()),
            ..RepoStatus::default()
        }
    }

    #[test]
    fn clean_repository_needs_nothing() {
        assert_eq!(Health::of(&clean()), Health::Clean);
        assert!(Health::fixes(&clean(), true).is_empty());
    }

    #[test]
    fn missing_folder_offers_reclone_only_with_a_remote() {
        let status = RepoStatus::missing();
        assert_eq!(Health::of(&status), Health::Missing);
        assert_eq!(Health::fixes(&status, true), vec![Fix::Reclone]);
        assert!(Health::fixes(&status, false).is_empty());
    }

    #[test]
    fn conflicts_outrank_everything_and_are_not_automated() {
        let status = RepoStatus { conflicted: 1, behind: 3, ..clean() };
        assert_eq!(Health::of(&status), Health::Conflicted);
        assert_eq!(Health::fixes(&status, true), vec![Fix::OpenInTool]);
    }

    #[test]
    fn behind_and_dirty_offers_commit_then_pull() {
        let status = RepoStatus { behind: 2, unstaged: 1, ..clean() };
        assert_eq!(Health::of(&status), Health::Behind);
        assert_eq!(Health::fixes(&status, true), vec![Fix::CommitAll, Fix::Pull, Fix::Stash]);
    }

    #[test]
    fn diverged_clean_tree_offers_sync() {
        let status = RepoStatus { ahead: 1, behind: 1, ..clean() };
        assert_eq!(Health::of(&status), Health::Diverged);
        assert_eq!(Health::fixes(&status, true), vec![Fix::Sync]);
    }

    #[test]
    fn ahead_offers_push() {
        let status = RepoStatus { ahead: 4, ..clean() };
        assert_eq!(Health::of(&status), Health::Ahead);
        assert_eq!(Health::fixes(&status, true), vec![Fix::Push]);
    }

    #[test]
    fn branch_without_upstream_offers_publish() {
        let status = RepoStatus { upstream: None, ..clean() };
        assert_eq!(Health::of(&status), Health::Unpublished);
        assert_eq!(Health::fixes(&status, true), vec![Fix::Publish]);
    }

    #[test]
    fn local_only_repository_is_clean() {
        let status = RepoStatus { upstream: None, remote_url: None, ..clean() };
        assert_eq!(Health::of(&status), Health::Clean);
    }

    #[test]
    fn fresh_repository_without_commits_is_not_detached() {
        let status = RepoStatus { branch: None, upstream: None, has_commits: false, ..clean() };
        assert_eq!(Health::of(&status), Health::Clean);
    }
}
