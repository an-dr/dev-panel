use std::path::Path;

use serde::Deserialize;

use super::{Health, RepoStatus};

/// The theme the page is showing, with "follow the system" already resolved:
/// only the page knows which one that is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResolvedTheme {
    Light,
    Dark,
}

/// Which Commits view a repository's problem opens in, as Commits' command
/// line spells it (contract 1 in plans/commits-integration/00-contracts.md,
/// owned by Commits' docs/command-line.md).
pub struct CommitsLink;

impl CommitsLink {
    /// The arguments after the repository path, or `None` when there is
    /// nothing to open: the folder is gone or holds no repository.
    ///
    /// - Conflicts and unfinished operations open the conflict view; a dirty
    ///   tree opens its changes.
    /// - A branch out of step with its upstream is selected in the graph; a
    ///   detached HEAD selects its commit, when it is known.
    /// - Every link carries DevPanel's mode, and its palette when the theme
    ///   file was written, so Commits looks like part of the same product.
    pub fn for_status(status: &RepoStatus, theme: ResolvedTheme, theme_file: Option<&Path>) -> Option<Vec<String>> {
        let mut args: Vec<String> = match Health::of(status) {
            Health::Missing | Health::NotARepo => return None,
            Health::Conflicted | Health::InProgress => vec!["--show".into(), "conflicts".into()],
            Health::Dirty => vec!["--show".into(), "changes".into()],
            Health::Diverged | Health::Behind | Health::Ahead | Health::Unpublished => match &status.branch {
                Some(branch) => vec!["--branch".into(), branch.clone()],
                None => Vec::new(),
            },
            Health::Detached => match &status.last_commit {
                Some(commit) if !commit.hash.is_empty() => vec!["--commit".into(), commit.hash.clone()],
                _ => Vec::new(),
            },
            Health::Clean => Vec::new(),
        };
        let mode = match theme {
            ResolvedTheme::Light => "light",
            ResolvedTheme::Dark => "dark",
        };
        args.extend(["--theme".into(), mode.into()]);
        if let Some(file) = theme_file {
            args.extend(["--theme-file".into(), file.to_string_lossy().into_owned()]);
        }
        Some(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CommitSummary, Operation};

    fn clean() -> RepoStatus {
        RepoStatus {
            exists: true,
            is_repo: true,
            branch: Some("main".into()),
            upstream: Some("origin/main".into()),
            has_commits: true,
            remote_url: Some("u".into()),
            last_commit: Some(CommitSummary { hash: "abc123".into(), ..CommitSummary::default() }),
            ..RepoStatus::default()
        }
    }

    fn link(status: RepoStatus) -> Option<Vec<String>> {
        CommitsLink::for_status(&status, ResolvedTheme::Dark, None)
    }

    fn args(list: &[&str]) -> Option<Vec<String>> {
        Some(list.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn problems_that_need_resolving_open_the_conflict_view() {
        assert_eq!(link(RepoStatus { conflicted: 2, ..clean() }), args(&["--show", "conflicts", "--theme", "dark"]));
        assert_eq!(
            link(RepoStatus { operation: Some(Operation::Rebase), ..clean() }),
            args(&["--show", "conflicts", "--theme", "dark"])
        );
    }

    #[test]
    fn uncommitted_changes_open_the_changes_view() {
        assert_eq!(link(RepoStatus { unstaged: 1, ..clean() }), args(&["--show", "changes", "--theme", "dark"]));
    }

    #[test]
    fn a_branch_out_of_step_selects_the_branch() {
        let expected = args(&["--branch", "main", "--theme", "dark"]);
        assert_eq!(link(RepoStatus { ahead: 1, behind: 1, ..clean() }), expected);
        assert_eq!(link(RepoStatus { behind: 1, ..clean() }), expected);
        assert_eq!(link(RepoStatus { ahead: 1, ..clean() }), expected);
        assert_eq!(link(RepoStatus { upstream: None, ..clean() }), expected);
    }

    #[test]
    fn a_detached_head_selects_its_commit_when_known() {
        let detached = RepoStatus { branch: None, upstream: None, ..clean() };
        assert_eq!(link(detached.clone()), args(&["--commit", "abc123", "--theme", "dark"]));
        assert_eq!(link(RepoStatus { last_commit: None, ..detached }), args(&["--theme", "dark"]));
    }

    #[test]
    fn a_clean_repository_opens_plainly() {
        assert_eq!(link(clean()), args(&["--theme", "dark"]));
    }

    #[test]
    fn nothing_opens_without_a_repository() {
        assert_eq!(link(RepoStatus::missing()), None);
        assert_eq!(link(RepoStatus::not_a_repo()), None);
    }

    #[test]
    fn the_theme_and_its_file_close_every_link() {
        let file = Path::new("/cfg/commits-theme.json");
        assert_eq!(
            CommitsLink::for_status(&RepoStatus { unstaged: 1, ..clean() }, ResolvedTheme::Light, Some(file)),
            args(&["--show", "changes", "--theme", "light", "--theme-file", "/cfg/commits-theme.json"])
        );
    }
}
