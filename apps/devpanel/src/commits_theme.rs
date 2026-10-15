//! DevPanel's palette as a Commits theme file (Commits' docs/themes.md), so
//! a repository opened in Commits looks like part of the same product. The
//! values come from docs/design/colours.md; a test holds the urgency colours
//! to the page's own tokens.

use std::path::{Path, PathBuf};

const THEME: &str = include_str!("../assets/commits-theme.json");
const FILE_NAME: &str = "commits-theme.json";

/// Writes the theme file into `dir` when it is missing or differs, and
/// returns its path; `None` when it cannot be written, in which case Commits
/// opens in DevPanel's mode but its own colours.
pub fn write(dir: &Path) -> Option<PathBuf> {
    let path = dir.join(FILE_NAME);
    if std::fs::read_to_string(&path).ok().as_deref() != Some(THEME) {
        std::fs::create_dir_all(dir).ok()?;
        std::fs::write(&path, THEME).ok()?;
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every colour key Commits' theme-file format requires (docs/themes.md
    /// in the Commits repository, at the pinned release).
    const KEYS: [&str; 17] = [
        "canvas",
        "surface",
        "elevated",
        "text",
        "border",
        "input",
        "hover",
        "selection",
        "selectionText",
        "accent",
        "positive",
        "negative",
        "warning",
        "shadow",
        "urgencyAdvisory",
        "urgencyCaution",
        "urgencyWarning",
    ];

    fn theme() -> serde_json::Value {
        serde_json::from_str(THEME).unwrap()
    }

    /// `--name: value` from the CSS block that starts with `selector {`.
    fn token(selector: &str, name: &str) -> String {
        let css = include_str!("../web/style.css");
        let block = &css[css.find(&format!("{selector} {{")).unwrap()..];
        let block = &block[..block.find("\n}").unwrap()];
        let start = block.find(&format!("--{name}:")).unwrap() + name.len() + 3;
        block[start..].split(';').next().unwrap().trim().to_string()
    }

    #[test]
    fn the_theme_has_every_key_commits_requires() {
        let theme = theme();
        assert_eq!(theme["version"], 1);
        for mode in ["light", "dark"] {
            for key in KEYS {
                assert!(theme[mode]["colours"][key].is_string(), "{mode}.colours.{key} is missing");
            }
        }
    }

    #[test]
    fn urgency_colours_match_the_page() {
        let theme = theme();
        for (mode, selector) in [("light", ":root"), ("dark", ":root[data-theme=\"dark\"]")] {
            let colours = &theme[mode]["colours"];
            assert_eq!(colours["urgencyAdvisory"], token(selector, "info"), "{mode} advisory");
            assert_eq!(colours["urgencyCaution"], token(selector, "warn"), "{mode} caution");
            assert_eq!(colours["urgencyWarning"], token(selector, "danger"), "{mode} warning");
        }
    }

    #[test]
    fn writing_twice_leaves_one_current_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path()).unwrap();
        std::fs::write(&path, "stale").unwrap();
        assert_eq!(write(dir.path()), Some(path.clone()));
        assert_eq!(std::fs::read_to_string(path).unwrap(), THEME);
    }
}
