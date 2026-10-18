//! The web page of a repository, derived from its git remote.

/// Turns a clone URL into the address of the repository's web page, or
/// `None` when the remote is not on a web host (a local path, a `file://`
/// URL, nothing usable).
///
/// Every common forge serves a repository's page at `https://host/path`,
/// where `path` is the remote's path without `.git`: GitHub, GitLab with its
/// nested groups, Bitbucket, Codeberg, Gitea, Forgejo, sourcehut. So the rule
/// is generic, and only hosts whose clone path differs from the page path are
/// special cases. A user whose host fits neither sets the address on the
/// repository instead.
///
/// ```
/// # use devpanel_core::domain::web_url_from_remote;
/// assert_eq!(web_url_from_remote("git@github.com:an-dr/bones.git").as_deref(), Some("https://github.com/an-dr/bones"));
/// ```
pub fn web_url_from_remote(remote: &str) -> Option<String> {
    let remote = remote.trim();
    let (host, path) = split_remote(remote)?;
    let host = host.rsplit('@').next()?.to_ascii_lowercase();
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path).trim_end_matches('/');
    if host.is_empty() || path.is_empty() {
        return None;
    }
    // Azure DevOps clones over SSH from ssh.dev.azure.com:v3/org/project/repo
    // but shows the page at dev.azure.com/org/project/_git/repo.
    if host == "ssh.dev.azure.com" || host == "vs-ssh.visualstudio.com" {
        let parts: Vec<&str> = path.strip_prefix("v3/").unwrap_or(path).split('/').collect();
        if let [org, project, repo] = parts[..] {
            return Some(format!("https://dev.azure.com/{org}/{project}/_git/{repo}"));
        }
        return None;
    }
    Some(format!("https://{host}/{path}"))
}

/// Splits a remote into its host (possibly with `user@` and `:port`) and its
/// path. Accepts `scheme://host/path` and scp-like `user@host:path`.
fn split_remote(remote: &str) -> Option<(String, &str)> {
    if let Some((scheme, rest)) = remote.split_once("://") {
        if !matches!(scheme, "https" | "http" | "ssh" | "git" | "git+ssh" | "ssh+git") {
            return None;
        }
        let (authority, path) = rest.split_once('/')?;
        let authority = authority.rsplit('@').next()?;
        // An SSH port is never part of the web address; an HTTP one is.
        let host = match (scheme, authority.split_once(':')) {
            ("https" | "http", _) | (_, None) => authority,
            (_, Some((host, _port))) => host,
        };
        return Some((host.to_string(), path));
    }
    // scp-like: [user@]host:path, where the host has no slash before the colon.
    let (host, path) = remote.split_once(':')?;
    if host.is_empty() || host.contains('/') || host.contains('\\') || host.len() == 1 {
        // `C:\path` or a relative path, not a host.
        return None;
    }
    Some((host.to_string(), path))
}

#[cfg(test)]
mod tests {
    use super::web_url_from_remote as web;

    fn is(remote: &str, expected: &str) {
        assert_eq!(web(remote).as_deref(), Some(expected), "{remote}");
    }

    #[test]
    fn github_in_every_clone_form() {
        is("https://github.com/an-dr/bones.git", "https://github.com/an-dr/bones");
        is("https://github.com/an-dr/bones", "https://github.com/an-dr/bones");
        is("git@github.com:an-dr/bones.git", "https://github.com/an-dr/bones");
        is("ssh://git@github.com/an-dr/bones.git", "https://github.com/an-dr/bones");
        is("git://github.com/an-dr/bones.git", "https://github.com/an-dr/bones");
        is("https://token@GitHub.com/an-dr/bones.git/", "https://github.com/an-dr/bones");
    }

    #[test]
    fn credentials_never_reach_the_address() {
        is("https://user:secret@gitlab.com/a/b.git", "https://gitlab.com/a/b");
    }

    #[test]
    fn gitlab_nested_groups_and_self_hosted() {
        is("git@gitlab.com:group/sub/project.git", "https://gitlab.com/group/sub/project");
        is("ssh://git@git.example.org:2222/team/tool.git", "https://git.example.org/team/tool");
        is("https://git.example.org:8443/team/tool.git", "https://git.example.org:8443/team/tool");
    }

    #[test]
    fn bitbucket_and_other_forges() {
        is("git@bitbucket.org:team/repo.git", "https://bitbucket.org/team/repo");
        is("https://user@bitbucket.org/team/repo.git", "https://bitbucket.org/team/repo");
        is("git@codeberg.org:user/repo.git", "https://codeberg.org/user/repo");
        is("git@git.sr.ht:~user/repo", "https://git.sr.ht/~user/repo");
    }

    #[test]
    fn azure_devops_moves_the_repository_under_git() {
        is("git@ssh.dev.azure.com:v3/org/project/repo", "https://dev.azure.com/org/project/_git/repo");
        is("https://org@dev.azure.com/org/project/_git/repo", "https://dev.azure.com/org/project/_git/repo");
    }

    #[test]
    fn local_remotes_have_no_page() {
        for remote in ["/srv/git/repo.git", "../repo", "C:\\repos\\x", "C:/repos/x", "file:///srv/repo.git", "", "github.com"] {
            assert_eq!(web(remote), None, "{remote}");
        }
    }
}
