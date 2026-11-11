//! GitHub CLI integration and the explicitly supported global Git settings.
use std::process::Command;
use devpanel_core::{HubError, HubResult};
use devpanel_core::domain::{Launch, Tool};
use devpanel_core::ports::Launcher;
use serde_json::{json, Value};

const KEYS: &[&str] = &["user.name", "user.email", "init.defaultBranch", "pull.rebase", "push.default", "core.editor", "core.autocrlf", "commit.gpgsign", "user.signingkey"];

fn run(program: &str, args: &[&str]) -> HubResult<String> {
    let output = Command::new(program).args(args).output().map_err(|e| HubError::Command(format!("Could not start {program}: {e}")))?;
    if !output.status.success() {
        return Err(HubError::Command(String::from_utf8_lossy(&output.stderr).trim().to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn github(action: &str) -> HubResult<Value> {
    match action {
        "status" => {
            run("gh", &["--version"])?;
            if run("gh", &["auth", "status", "--hostname", "github.com"]).is_err() { return Ok(json!({"login": null})); }
            let login = run("gh", &["api", "--hostname", "github.com", "user", "--jq", ".login"])?;
            Ok(json!({"login": login}))
        }
        "login" => {
            run("gh", &["--version"])?;
            let tool = Tool::new("github-login", "GitHub sign-in", Launch::Terminal { command: Some("gh auth login --hostname github.com --web --git-protocol https".into()) });
            let cwd = std::env::current_dir().map_err(|e| HubError::Command(e.to_string()))?;
            devpanel_infra::SystemLauncher.launch(&tool, &cwd, &[])?;
            Ok(json!({}))
        }
        "repos" => {
            let text = run("gh", &["api", "--hostname", "github.com", "--paginate", "/user/repos?per_page=100&affiliation=owner,collaborator,organization_member&sort=updated"])?;
            parse_repos(&text)
        }
        _ => Err(HubError::Invalid("Unknown GitHub action".into())),
    }
}

fn parse_repos(text: &str) -> HubResult<Value> {
    let mut pages = Vec::new();
    for page in serde_json::Deserializer::from_str(text).into_iter::<Vec<Value>>() {
        pages.extend(page.map_err(|e| HubError::Command(e.to_string()))?);
    }
    let repos: Vec<Value> = pages.into_iter().filter_map(|repo| Some(json!({
        "name": repo["full_name"].as_str()?, "url": repo["clone_url"].as_str()?,
        "private": repo["private"].as_bool().unwrap_or(false), "description": repo["description"].as_str().unwrap_or("")
    }))).collect();
    Ok(json!(repos))
}

pub fn git_config(values: Option<Value>) -> HubResult<Value> {
    git_config_at(values, None)
}

fn git_config_at(values: Option<Value>, path: Option<&std::path::Path>) -> HubResult<Value> {
    let options = match path { Some(path) => vec!["--file".to_string(), path.to_string_lossy().into_owned()], None => vec!["--global".to_string()] };
    if let Some(values) = values {
        let values = values.as_object().ok_or_else(|| HubError::Invalid("Expected Git settings".into()))?;
        if values.keys().any(|key| !KEYS.contains(&key.as_str())) { return Err(HubError::Invalid("Unsupported Git setting".into())); }
        for value in values.values() {
            let value = value.as_str().ok_or_else(|| HubError::Invalid("Git values must be text".into()))?;
            if value.contains(['\n', '\r', '\0']) { return Err(HubError::Invalid("Git values must be a single line".into())); }
        }
        for (key, value) in values {
            let value = value.as_str().unwrap();
            if value.is_empty() {
                let output = Command::new("git").arg("config").args(&options).args(["--unset-all", "--", key]).output().map_err(|e| HubError::Command(e.to_string()))?;
                if !output.status.success() && output.status.code() != Some(5) { return Err(HubError::Command(String::from_utf8_lossy(&output.stderr).into())); }
            } else {
                let output = Command::new("git").arg("config").args(&options).args(["--replace-all", "--", key, value]).output().map_err(|e| HubError::Command(e.to_string()))?;
                if !output.status.success() { return Err(HubError::Command(String::from_utf8_lossy(&output.stderr).into())); }
            }
        }
    }
    let mut result = serde_json::Map::new();
    for key in KEYS {
        let output = Command::new("git").arg("config").args(&options).args(["--get", "--", key]).output().map_err(|e| HubError::Command(e.to_string()))?;
        if !output.status.success() && output.status.code() != Some(1) { return Err(HubError::Command(String::from_utf8_lossy(&output.stderr).into())); }
        result.insert((*key).into(), json!(String::from_utf8_lossy(&output.stdout).trim_end().to_string()));
    }
    Ok(Value::Object(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn github_pages_are_flattened_without_unneeded_fields() {
        let repos = parse_repos(r#"[{"full_name":"a/one","clone_url":"https://github.com/a/one.git","private":true,"secret":"discard"}] [{"full_name":"b/two","clone_url":"https://github.com/b/two.git","description":"hi"}]"#).unwrap();
        assert_eq!(repos.as_array().unwrap().len(), 2);
        assert_eq!(repos[0]["private"], true);
        assert!(repos[0].get("secret").is_none());
        assert!(parse_repos("invalid").is_err());
    }
    #[test]
    fn git_settings_round_trip_and_reject_unsafe_input() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gitconfig");
        let written = git_config_at(Some(json!({"user.name":"-Alice", "user.email":"a@example.test"})), Some(&path)).unwrap();
        assert_eq!(written["user.name"], "-Alice");
        assert_eq!(written["user.email"], "a@example.test");
        assert!(git_config_at(Some(json!({"user.name":"changed", "user.email":"bad\nvalue"})), Some(&path)).is_err());
        assert_eq!(git_config_at(None, Some(&path)).unwrap()["user.name"], "-Alice");
        assert!(git_config_at(Some(json!({"credential.helper":"bad"})), Some(&path)).is_err());
        let cleared = git_config_at(Some(json!({"user.email":""})), Some(&path)).unwrap();
        assert_eq!(cleared["user.email"], "");
        assert_eq!(cleared["user.name"], "-Alice");
    }
}
