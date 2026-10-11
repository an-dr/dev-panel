//! Parsing git's machine-readable output. Kept apart from process handling so
//! it can be tested against literal text.

use devpanel_core::domain::{CommitSummary, FileChange, RepoStatus};

/// Fills branch, tracking and change counts from
/// `git status --porcelain=v2 --branch --show-stash`.
pub fn apply_status(text: &str, status: &mut RepoStatus) {
    status.has_commits = true;
    for line in text.lines() {
        if let Some(header) = line.strip_prefix("# ") {
            apply_header(header, status);
            continue;
        }
        let kind = line.split(' ').next().unwrap_or_default();
        let xy = line.split(' ').nth(1).unwrap_or_default();
        match kind {
            "1" | "2" => {
                if xy.as_bytes().first().is_some_and(|&x| x != b'.') {
                    status.staged += 1;
                }
                if xy.as_bytes().get(1).is_some_and(|&y| y != b'.') {
                    status.unstaged += 1;
                }
            }
            "u" => status.conflicted += 1,
            "?" => status.untracked += 1,
            _ => continue,
        }
        if status.files.len() < RepoStatus::LISTED_CHANGES {
            if let Some(path) = changed_path(kind, line) {
                let code = if kind == "?" { "??" } else { xy };
                status.files.push(FileChange { code: code.to_string(), path });
            }
        }
    }
}

/// The path of one porcelain v2 entry. Ordinary (`1`), renamed (`2`) and
/// unmerged (`u`) entries put it after a fixed number of fields; a rename
/// adds the original after a tab, shown as `new <- old`.
fn changed_path(kind: &str, line: &str) -> Option<String> {
    let fields_before_path = match kind {
        "1" => 8,
        "2" => 9,
        "u" => 10,
        "?" => 1,
        _ => return None,
    };
    let path = line.splitn(fields_before_path + 1, ' ').nth(fields_before_path)?;
    Some(match path.split_once('\t') {
        Some((new, old)) => format!("{new} <- {old}"),
        None => path.to_string(),
    })
}

fn apply_header(header: &str, status: &mut RepoStatus) {
    let (key, value) = header.split_once(' ').unwrap_or((header, ""));
    match key {
        "branch.oid" => status.has_commits = value != "(initial)",
        "branch.head" => status.branch = (value != "(detached)").then(|| value.to_string()),
        "branch.upstream" => status.upstream = Some(value.to_string()),
        "branch.ab" => {
            for part in value.split_whitespace() {
                if let Some(n) = part.strip_prefix('+') {
                    status.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix('-') {
                    status.behind = n.parse().unwrap_or(0);
                }
            }
        }
        "stash" => status.stashes = value.parse().unwrap_or(0),
        _ => {}
    }
}

/// Reads `git config --get-regexp ^remote\..*\.url$` output and returns
/// `origin` if present, else the first remote, as `(name, url)`.
pub fn pick_remote(config: &str) -> Option<(String, String)> {
    let remotes: Vec<(String, String)> = config
        .lines()
        .filter_map(|line| {
            let (key, url) = line.split_once(' ')?;
            let name = key.strip_prefix("remote.")?.strip_suffix(".url")?;
            Some((name.to_string(), url.trim().to_string()))
        })
        .collect();
    let origin = remotes.iter().position(|(name, _)| name == "origin");
    remotes.into_iter().nth(origin.unwrap_or(0))
}

/// Parses `git log -1 --format=%h%x1f%s%x1f%an%x1f%ct`.
pub fn parse_commit(line: &str) -> Option<CommitSummary> {
    let mut fields = line.split('\u{1f}');
    Some(CommitSummary {
        hash: fields.next()?.to_string(),
        subject: fields.next()?.to_string(),
        author: fields.next()?.to_string(),
        time: fields.next()?.trim().parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = "\
# branch.oid 1234abcd
# branch.head main
# branch.upstream origin/main
# branch.ab +2 -1
# stash 3
1 M. N... 100644 100644 100644 a b src/staged.rs
1 .M N... 100644 100644 100644 a b src/unstaged.rs
1 MM N... 100644 100644 100644 a b src/both.rs
2 R. N... 100644 100644 100644 a b R100 new.rs\told.rs
u UU N... 100644 100644 100644 100644 a b c conflict.rs
? notes.txt
? more.txt
! ignored.log";

    #[test]
    fn counts_every_kind_of_change() {
        let mut status = RepoStatus::default();
        apply_status(STATUS, &mut status);
        assert_eq!(status.branch.as_deref(), Some("main"));
        assert_eq!(status.upstream.as_deref(), Some("origin/main"));
        assert_eq!((status.ahead, status.behind), (2, 1));
        assert_eq!(status.stashes, 3);
        assert_eq!(status.staged, 3);
        assert_eq!(status.unstaged, 2);
        assert_eq!(status.conflicted, 1);
        assert_eq!(status.untracked, 2);
        assert!(status.has_commits);
        let files: Vec<_> = status.files.iter().map(|f| format!("{} {}", f.code, f.path)).collect();
        assert_eq!(
            files,
            [
                "M. src/staged.rs",
                ".M src/unstaged.rs",
                "MM src/both.rs",
                "R. new.rs <- old.rs",
                "UU conflict.rs",
                "?? notes.txt",
                "?? more.txt",
            ]
        );
    }

    #[test]
    fn paths_with_spaces_survive() {
        let mut status = RepoStatus::default();
        apply_status("1 .M N... 100644 100644 100644 a b my file.txt\n? new dir/x y.txt", &mut status);
        let paths: Vec<_> = status.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["my file.txt", "new dir/x y.txt"]);
    }

    #[test]
    fn fresh_and_detached_heads() {
        let mut fresh = RepoStatus::default();
        apply_status("# branch.oid (initial)\n# branch.head main", &mut fresh);
        assert!(!fresh.has_commits);

        let mut detached = RepoStatus::default();
        apply_status("# branch.oid abc\n# branch.head (detached)", &mut detached);
        assert_eq!(detached.branch, None);
    }

    #[test]
    fn origin_is_preferred_over_the_first_remote() {
        let config = "remote.fork.url git@x:me/a\nremote.origin.url https://x/a.git";
        assert_eq!(pick_remote(config), Some(("origin".into(), "https://x/a.git".into())));
        assert_eq!(pick_remote("remote.up.url u"), Some(("up".into(), "u".into())));
        assert_eq!(pick_remote(""), None);
    }

    #[test]
    fn commit_line_with_separators() {
        let commit = parse_commit("abc1234\u{1f}Fix: a thing\u{1f}Ann\u{1f}1700000000").unwrap();
        assert_eq!(commit.subject, "Fix: a thing");
        assert_eq!(commit.time, 1_700_000_000);
        assert_eq!(parse_commit("broken"), None);
    }
}
