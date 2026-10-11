use serde::{Deserialize, Serialize};

/// The operating system family, which decides between `.ps1`/`.cmd` and
/// `.sh` scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Unix,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Unix
        }
    }
}

/// One way to install or build a repository, found from the files in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecipe {
    /// Short label for the button, e.g. "install.ps1" or "cargo install".
    pub label: String,
    /// Shell command line run in the repository root.
    pub command: String,
    /// The file that suggested it.
    pub source: String,
}

/// Proposes install recipes from the repository's file list (paths relative
/// to the root, `/`-separated), best first.
///
/// The repository's own install scripts win over ecosystem defaults: when an
/// author ships `install.ps1`, that is what they meant. Scripts for the other
/// platform are left out rather than offered and failing.
pub fn detect_install_recipes(files: &[String], platform: Platform) -> Vec<InstallRecipe> {
    let has = |name: &str| files.iter().any(|f| f.eq_ignore_ascii_case(name));
    let mut recipes = Vec::new();
    let mut push = |label: &str, command: String, source: &str| {
        recipes.push(InstallRecipe { label: label.into(), command, source: source.into() });
    };

    for dir in ["", "scripts/", "tools/"] {
        for stem in ["install", "setup"] {
            let ps1 = format!("{dir}{stem}.ps1");
            let sh = format!("{dir}{stem}.sh");
            let cmd = format!("{dir}{stem}.cmd");
            let bat = format!("{dir}{stem}.bat");
            if has(&ps1) {
                push(&ps1, format!("pwsh -NoProfile -ExecutionPolicy Bypass -File {ps1}"), &ps1);
            }
            match platform {
                Platform::Unix if has(&sh) => push(&sh, format!("sh {sh}"), &sh),
                Platform::Windows if has(&cmd) => push(&cmd, cmd.replace('/', "\\"), &cmd),
                Platform::Windows if has(&bat) => push(&bat, bat.replace('/', "\\"), &bat),
                _ => {}
            }
        }
    }

    if has("Cargo.toml") {
        push("cargo install", "cargo install --path .".into(), "Cargo.toml");
    }
    if has("package.json") {
        push("npm install", "npm install".into(), "package.json");
    }
    if has("pyproject.toml") {
        push("pip install", "pip install -e .".into(), "pyproject.toml");
    } else if has("requirements.txt") {
        push("pip requirements", "pip install -r requirements.txt".into(), "requirements.txt");
    }
    if has("go.mod") {
        push("go install", "go install ./...".into(), "go.mod");
    }
    if has("CMakeLists.txt") {
        push("cmake build", "cmake -B build && cmake --build build".into(), "CMakeLists.txt");
    }
    if platform == Platform::Unix && has("Makefile") {
        push("make install", "make install".into(), "Makefile");
    }
    recipes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn repository_scripts_come_before_ecosystem_defaults() {
        let recipes = detect_install_recipes(&files(&["Cargo.toml", "install.ps1"]), Platform::Unix);
        let labels: Vec<_> = recipes.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["install.ps1", "cargo install"]);
    }

    #[test]
    fn shell_scripts_only_on_unix() {
        let list = files(&["install.sh", "scripts/setup.cmd"]);
        let unix: Vec<_> = detect_install_recipes(&list, Platform::Unix).into_iter().map(|r| r.label).collect();
        let windows: Vec<_> = detect_install_recipes(&list, Platform::Windows).into_iter().map(|r| r.label).collect();
        assert_eq!(unix, ["install.sh"]);
        assert_eq!(windows, ["scripts/setup.cmd"]);
    }

    #[test]
    fn windows_paths_use_backslashes() {
        let recipes = detect_install_recipes(&files(&["scripts/install.cmd"]), Platform::Windows);
        assert_eq!(recipes[0].command, "scripts\\install.cmd");
    }

    #[test]
    fn nothing_recognised_gives_nothing() {
        assert!(detect_install_recipes(&files(&["README.md"]), Platform::Unix).is_empty());
    }
}
