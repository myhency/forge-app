//! What the report says about how Forge is set up: its settings without the secrets, the versions of the programs
//! it runs, and which proxy and account variables are set. A problem often comes from a setting, not from code.

use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where to look; the app fills it in because only it knows the paths.
#[derive(Default, Clone)]
pub struct Details {
    /// Forge's `config.json`.
    pub config_path: Option<PathBuf>,
    /// Programs Forge runs, as (label, program): each is asked for `--version`.
    pub tools: Vec<(String, PathBuf)>,
    /// Claude's `projects` folder, where the conversations are kept.
    pub transcripts_dir: Option<PathBuf>,
}

/// Variables whose value helps and holds no secret of its own (a proxy URL's password is hidden later).
const SHOWN: &[&str] = &["HTTPS_PROXY", "HTTP_PROXY", "ALL_PROXY", "NO_PROXY", "https_proxy", "http_proxy", "all_proxy", "no_proxy", "ANTHROPIC_BASE_URL", "CLAUDE_CONFIG_DIR", "SHELL", "LANG", "LC_ALL", "FORGE_CLAUDE_HOME", "FORGE_CONFIG_PATH", "FORGE_SIDECAR_SCRIPT"];
/// Variables that hold a secret: only whether they are set is said.
const PRESENCE_ONLY: &[&str] = &["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "CLAUDE_CODE_OAUTH_TOKEN"];

pub fn describe(details: &Details) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "[Settings]");
    match details.config_path.as_deref().map(read_settings) {
        Some(Ok(text)) => out.push_str(&text),
        Some(Err(why)) => {
            let _ = writeln!(out, "Could not read the settings: {why}");
        }
        None => {
            let _ = writeln!(out, "No settings file.");
        }
    }
    let _ = writeln!(out, "\n[Programs]");
    for (label, program) in &details.tools {
        let version = super::oslogs::run(&program.to_string_lossy(), &["--version"], Duration::from_secs(10)).map(|text| text.trim().lines().next().unwrap_or("").to_string()).unwrap_or_else(|why| format!("not run: {why}"));
        let _ = writeln!(out, "{label}: {version}  ({})", program.display());
    }
    let _ = writeln!(out, "\n[Environment variables]");
    for name in SHOWN {
        if let Ok(value) = std::env::var(name) {
            let _ = writeln!(out, "{name}={value}");
        }
    }
    for name in PRESENCE_ONLY {
        let _ = writeln!(out, "{name} ({})", if std::env::var_os(name).is_some_and(|value| !value.is_empty()) { "set" } else { "not set" });
    }
    out
}

fn read_settings(path: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|err| format!("the file is not valid JSON ({err})"))?;
    let mut out = String::new();
    summarize(&value, 0, &mut out);
    Ok(out)
}

/// Writes the settings as indented lines: scalars as they are, a list as its length, a secret as set or not.
fn summarize(value: &serde_json::Value, depth: usize, out: &mut String) {
    let Some(map) = value.as_object() else { return };
    for (key, value) in map {
        let pad = "  ".repeat(depth);
        let lower = key.to_lowercase();
        let secret = ["key", "token", "secret", "password"].iter().any(|word| lower.contains(word));
        match value {
            _ if secret => {
                // Written without a colon so the masking that follows does not take "set" for the secret itself.
                let _ = writeln!(out, "{pad}{key} ({})", if value.is_null() || value.as_str() == Some("") { "not set" } else { "set" });
            }
            serde_json::Value::Array(items) => {
                let _ = writeln!(out, "{pad}{key}: {} items", items.len());
            }
            serde_json::Value::Object(_) if depth < 2 => {
                let _ = writeln!(out, "{pad}{key}:");
                summarize(value, depth + 1, out);
            }
            serde_json::Value::Object(map) => {
                let _ = writeln!(out, "{pad}{key}: {} entries", map.len());
            }
            other => {
                let _ = writeln!(out, "{pad}{key}: {other}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_only_said_to_be_set_and_lists_are_counted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, r#"{"base_url":"https://gw.example.com","api_key":"sk-ant-REAL","model":null,"archived":[1,2,3],"sidebar":{"sort":"recent","pinned":[1]}}"#).unwrap();
        let text = describe(&Details { config_path: Some(path), tools: vec![], ..Default::default() });
        assert!(text.contains("base_url: \"https://gw.example.com\"") && text.contains("api_key (set)") && text.contains("archived: 3 items"), "{text}");
        assert!(text.contains("  sort: \"recent\"") && !text.contains("REAL"), "{text}");
    }

    #[test]
    fn a_broken_settings_file_is_named_not_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "{ nope").unwrap();
        assert!(describe(&Details { config_path: Some(path), tools: vec![], ..Default::default() }).contains("not valid JSON"));
    }

    #[cfg(unix)]
    #[test]
    fn a_program_is_asked_for_its_version() {
        let text = describe(&Details { config_path: None, tools: vec![("echo".into(), PathBuf::from("echo"))], ..Default::default() });
        assert!(text.contains("echo: --version"), "{text}");
    }

    #[test]
    fn a_missing_program_is_reported() {
        let text = describe(&Details { config_path: None, tools: vec![("node".into(), PathBuf::from("/no/such/node"))], ..Default::default() });
        assert!(text.contains("not run"), "{text}");
    }
}
