use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Stable identity of a tracked repository. Survives renames and moves,
/// which is why it is not derived from the path.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoId(pub String);

/// A repository the user asked DevPanel to track.
///
/// Only what the user chose is stored here. Everything git can tell us
/// (branch, changes, upstream) is [`RepoStatus`](super::RepoStatus) and is
/// read fresh, never persisted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repo {
    pub id: RepoId,
    /// Display name; defaults to the folder name.
    pub name: String,
    /// Working tree on disk. May not exist yet (a clone pending) or any more.
    pub path: PathBuf,
    /// Where the repository came from, used to re-clone a missing one.
    #[serde(default)]
    pub remote: Option<String>,
    /// Free-form group shown in the sidebar, e.g. "work" or "tools".
    #[serde(default)]
    pub group: Option<String>,
    /// Overrides the detected install recipe when set. Run through the shell.
    #[serde(default)]
    pub install_command: Option<String>,
    /// Pinned by the user as a main repository; shown first and in Favorites.
    #[serde(default)]
    pub favorite: bool,
    /// The repository's web page when the one derived from the remote is
    /// wrong or missing; see [`web_url_from_remote`](super::web_url_from_remote).
    #[serde(default)]
    pub web_url: Option<String>,
    /// How the name is drawn, when the user chose it; `None` leaves the
    /// choice to the page, which derives one from the id.
    #[serde(default)]
    pub name_style: Option<NameStyle>,
    /// When DevPanel last pulled it, or tried to, in seconds since the Unix
    /// epoch; what the background pull measures staleness from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_pulled: Option<u64>,
}

/// The typeface of a repository's name. Interpreted by the presentation only:
/// the core neither knows the fonts nor checks the values, so a font the page
/// no longer ships falls back to its plain one rather than failing a load.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NameStyle {
    /// A font id from the page's catalogue; empty for the plain UI font.
    pub font: String,
    /// CSS weight, e.g. 400 or 700; `0` for the font's regular weight.
    pub weight: u16,
    pub italic: bool,
}

impl NameStyle {
    /// The interface's own font, upright and regular.
    pub fn plain() -> Self {
        Self::default()
    }
}

/// A repository as it travels between machines: everything needed to clone it
/// again and restore how the user had it, but nothing tied to this machine
/// (no absolute path, no local id). See [`Repo`] for the tracked form.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoExport {
    pub name: String,
    pub remote: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub install_command: Option<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub web_url: Option<String>,
    #[serde(default)]
    pub name_style: Option<NameStyle>,
}

impl From<&Repo> for RepoExport {
    fn from(repo: &Repo) -> Self {
        Self {
            name: repo.name.clone(),
            remote: repo.remote.clone(),
            group: repo.group.clone(),
            install_command: repo.install_command.clone(),
            favorite: repo.favorite,
            web_url: repo.web_url.clone(),
            name_style: repo.name_style.clone(),
        }
    }
}

/// A portable repository list, written by an export and read by an import.
/// `version` lets a later format be recognised and rejected cleanly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoList {
    pub version: u32,
    pub repos: Vec<RepoExport>,
}

impl RepoList {
    /// The version an export writes and an import understands.
    pub const VERSION: u32 = 1;
}

/// Derives a folder name from a clone URL:
/// `https://github.com/an-dr/bones.git` and `git@github.com:an-dr/bones` both
/// give `bones`. Returns `None` when nothing usable is left.
pub fn repo_name_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/');
    let last = trimmed.rsplit(['/', ':', '\\']).next()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    (!name.is_empty() && name != "." && name != "..").then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_from_https_and_ssh_urls() {
        assert_eq!(repo_name_from_url("https://github.com/an-dr/bones.git").as_deref(), Some("bones"));
        assert_eq!(repo_name_from_url("git@github.com:an-dr/commits").as_deref(), Some("commits"));
        assert_eq!(repo_name_from_url("https://host/a/b/").as_deref(), Some("b"));
        assert_eq!(repo_name_from_url("C:\\code\\local-repo").as_deref(), Some("local-repo"));
    }

    #[test]
    fn name_from_empty_url_is_none() {
        assert_eq!(repo_name_from_url(""), None);
        assert_eq!(repo_name_from_url("https://host/.git"), None);
    }
}
