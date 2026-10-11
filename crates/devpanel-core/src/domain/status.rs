use serde::{Deserialize, Serialize};

/// An operation git has left half-done in the working tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
}

/// One changed path. `code` is git's two-letter `XY` status: the first
/// letter is the index (staged) side, the second the working tree; `.` means
/// unchanged, `??` untracked, and `U` on either side a conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub code: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CommitSummary {
    pub hash: String,
    pub subject: String,
    pub author: String,
    /// Unix seconds.
    pub time: i64,
}

/// What git reports about a working tree at one moment.
///
/// `exists == false` means the folder is gone; `is_repo == false` means it is
/// there but holds no repository. All other fields are meaningful only when
/// both are true.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RepoStatus {
    pub exists: bool,
    pub is_repo: bool,
    /// `None` when HEAD is detached.
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
    pub stashes: u32,
    /// The changed paths, at most [`RepoStatus::LISTED_CHANGES`] of them;
    /// the counts above are always complete.
    pub files: Vec<FileChange>,
    pub operation: Option<Operation>,
    pub last_commit: Option<CommitSummary>,
    /// `false` for a freshly initialised repository with no commit yet.
    pub has_commits: bool,
    /// URL of `origin`, or of the first remote when there is no `origin`.
    pub remote_url: Option<String>,
}

impl RepoStatus {
    /// Enough to see what a repository holds without flooding the page when
    /// a build directory was never ignored.
    pub const LISTED_CHANGES: usize = 200;

    pub fn missing() -> Self {
        Self::default()
    }

    pub fn not_a_repo() -> Self {
        Self { exists: true, ..Self::default() }
    }

    pub fn changes(&self) -> u32 {
        self.staged + self.unstaged + self.untracked + self.conflicted
    }
}
