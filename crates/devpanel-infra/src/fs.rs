use std::path::{Path, PathBuf};

use devpanel_core::ports::FileSystem;

/// Folders never worth descending into while looking for repositories or
/// install scripts: dependency caches and build output.
const SKIPPED_DIRS: &[&str] = &["node_modules", "target", ".venv", "venv", "build", "dist", "vendor", ".cache"];

pub struct LocalFileSystem;

impl FileSystem for LocalFileSystem {
    fn expand(&self, path: &Path) -> PathBuf {
        let expanded = match path.strip_prefix("~") {
            Ok(rest) => dirs::home_dir().map(|home| home.join(rest)).unwrap_or_else(|| path.to_path_buf()),
            Err(_) => path.to_path_buf(),
        };
        if expanded.is_absolute() {
            return expanded;
        }
        std::env::current_dir().map(|cwd| cwd.join(&expanded)).unwrap_or(expanded)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_empty_dir(&self, path: &Path) -> bool {
        std::fs::read_dir(path).map(|mut entries| entries.next().is_none()).unwrap_or(false)
    }

    fn list_files(&self, root: &Path, depth: u32) -> Vec<String> {
        let mut files = Vec::new();
        collect_files(root, "", depth, &mut files);
        files.sort();
        files
    }

    fn find_repos(&self, root: &Path, depth: u32) -> Vec<PathBuf> {
        let mut repos = Vec::new();
        collect_repos(root, depth, &mut repos);
        repos.sort();
        repos
    }
}

fn collect_files(dir: &Path, prefix: &str, depth: u32, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_file() {
            files.push(format!("{prefix}{name}"));
        } else if kind.is_dir() && depth > 1 && !name.starts_with('.') && !SKIPPED_DIRS.contains(&name.as_str()) {
            collect_files(&entry.path(), &format!("{prefix}{name}/"), depth - 1, files);
        }
    }
}

/// A folder with a `.git` entry (directory, or file for worktrees and
/// submodules) is a repository; its contents are not searched further.
fn collect_repos(dir: &Path, depth: u32, repos: &mut Vec<PathBuf>) {
    if dir.join(".git").exists() {
        repos.push(dir.to_path_buf());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // `file_type` does not follow symlinks, so a link back up the tree
        // cannot send the scan in circles.
        if entry.file_type().is_ok_and(|t| t.is_dir()) && !name.starts_with('.') && !SKIPPED_DIRS.contains(&&*name) {
            collect_repos(&entry.path(), depth - 1, repos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_repositories_without_descending_into_them() {
        let root = tempfile::tempdir().unwrap();
        for dir in ["a/.git", "a/nested/.git", "group/b/.git", "node_modules/c/.git", "deep/1/2/3/d/.git"] {
            std::fs::create_dir_all(root.path().join(dir)).unwrap();
        }
        let found: Vec<_> = LocalFileSystem
            .find_repos(root.path(), 3)
            .into_iter()
            .map(|p| p.strip_prefix(root.path()).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(found, ["a", "group/b"]);
    }

    #[test]
    fn lists_files_to_the_given_depth_with_forward_slashes() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("scripts/deeper")).unwrap();
        for file in ["install.ps1", "scripts/install.sh", "scripts/deeper/x.sh"] {
            std::fs::write(root.path().join(file), "").unwrap();
        }
        assert_eq!(LocalFileSystem.list_files(root.path(), 2), ["install.ps1", "scripts/install.sh"]);
    }

    #[test]
    fn tilde_expands_to_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(LocalFileSystem.expand(Path::new("~/Code")), home.join("Code"));
    }
}
