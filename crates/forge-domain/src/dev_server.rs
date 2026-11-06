//! The Preview panel's `개발 서버 감지`, as the desktop app does it (app.asar, 2026-11-06): the
//! `dev`, `start`, `serve` and `develop` scripts of the project's package.json, run with the package
//! manager its lock file names, on the port of the framework its config file names. A chosen one is
//! kept in the project's `.claude/launch.json`, the file the desktop app's `preview_start` reads.

use serde_json::{json, Value};

/// Where the choice is kept, relative to the project folder.
pub const LAUNCH_JSON: &str = ".claude/launch.json";

const SCRIPTS: [&str; 4] = ["dev", "start", "serve", "develop"];

/// Config files that name a framework, and the port its dev server uses.
const FRAMEWORKS: &[(&[&str], &str, u16)] = &[
    (&["next.config.js", "next.config.ts", "next.config.mjs"], "Next.js", 3000),
    (&["vite.config.js", "vite.config.ts", "vite.config.mjs"], "Vite", 5173),
    (&["nuxt.config.js", "nuxt.config.ts"], "Nuxt", 3000),
    (&["astro.config.js", "astro.config.ts", "astro.config.mjs"], "Astro", 4321),
    (&["svelte.config.js", "svelte.config.ts"], "SvelteKit", 5173),
    (&["angular.json"], "Angular", 4200),
    (&["remix.config.js", "remix.config.ts"], "Remix", 3000),
    (&["gatsby-config.js", "gatsby-config.ts"], "Gatsby", 8000),
];

/// The files the detection looks at, for the caller to say which exist.
pub fn files_to_check() -> Vec<&'static str> {
    let mut files = vec!["package.json", "yarn.lock", "pnpm-lock.yaml", "package-lock.json"];
    files.extend(FRAMEWORKS.iter().flat_map(|(names, _, _)| names.iter().copied()));
    files
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerCandidate {
    pub name: String,
    pub runtime_executable: String,
    pub runtime_args: Vec<String>,
    pub port: u16,
    /// `None` when no config file names one.
    pub framework: Option<&'static str>,
    /// Already in `.claude/launch.json`.
    pub saved: bool,
}

impl ServerCandidate {
    pub fn command_line(&self) -> String {
        std::iter::once(self.runtime_executable.as_str()).chain(self.runtime_args.iter().map(String::as_str)).collect::<Vec<_>>().join(" ")
    }

    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }
}

// The desktop app only offers a script name made of these characters, so the command needs no quoting.
fn safe_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphanumeric()) && name.len() <= 64 && chars.all(|c| c.is_ascii_alphanumeric() || ":_.-".contains(c))
}

/// The servers the project offers: the saved ones first, then the scripts of package.json.
/// `exists` says whether a file (from [`files_to_check`]) is in the project folder.
pub fn candidates(exists: &dyn Fn(&str) -> bool, package_json: Option<&str>, launch_json: Option<&str>) -> Vec<ServerCandidate> {
    let mut out = launch_json.map(saved_servers).unwrap_or_default();
    let (framework, port) = FRAMEWORKS
        .iter()
        .find(|(names, _, _)| names.iter().any(|name| exists(name)))
        .map(|(_, framework, port)| (Some(*framework), *port))
        .unwrap_or((None, 3000));
    let runner = if exists("yarn.lock") {
        "yarn"
    } else if exists("pnpm-lock.yaml") {
        "pnpm"
    } else {
        "npm"
    };
    let scripts = package_json.and_then(|text| serde_json::from_str::<Value>(text).ok()).and_then(|value| value.get("scripts").cloned());
    for script in SCRIPTS {
        if scripts.as_ref().and_then(|scripts| scripts.get(script)).is_none() || !safe_name(script) {
            continue;
        }
        let candidate = ServerCandidate { name: script.to_string(), runtime_executable: runner.to_string(), runtime_args: vec!["run".to_string(), script.to_string()], port, framework, saved: false };
        if !out.iter().any(|saved| saved.command_line() == candidate.command_line()) {
            out.push(candidate);
        }
    }
    out
}

/// The entries of a `.claude/launch.json` that name a command and a port.
pub fn saved_servers(text: &str) -> Vec<ServerCandidate> {
    let Ok(value) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    value
        .get("configurations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str()?.to_string();
            let runtime_executable = entry.get("runtimeExecutable")?.as_str()?.to_string();
            let runtime_args = entry.get("runtimeArgs").and_then(Value::as_array).map(|args| args.iter().filter_map(|arg| arg.as_str().map(str::to_string)).collect()).unwrap_or_default();
            let port = entry.get("port").and_then(Value::as_u64).filter(|port| (1..=65535).contains(port))? as u16;
            Some(ServerCandidate { name, runtime_executable, runtime_args, port, framework: None, saved: true })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchJsonError {
    /// An entry by that name is there already.
    NameExists,
    /// The file is there but is not the JSON object the format needs.
    Unreadable,
}

/// `.claude/launch.json` with `candidate` added, in the desktop app's format (`version` 0.0.1 and
/// `configurations` with name, runtimeExecutable, runtimeArgs and port). Other entries are kept.
pub fn with_server(existing: Option<&str>, candidate: &ServerCandidate) -> Result<String, LaunchJsonError> {
    let mut value = match existing {
        Some(text) if !text.trim().is_empty() => serde_json::from_str::<Value>(text).map_err(|_| LaunchJsonError::Unreadable)?,
        _ => json!({ "version": "0.0.1", "configurations": [] }),
    };
    let object = value.as_object_mut().ok_or(LaunchJsonError::Unreadable)?;
    let configurations = object.entry("configurations").or_insert_with(|| json!([])).as_array_mut().ok_or(LaunchJsonError::Unreadable)?;
    if configurations.iter().any(|entry| entry.get("name").and_then(Value::as_str).is_some_and(|name| name.eq_ignore_ascii_case(&candidate.name))) {
        return Err(LaunchJsonError::NameExists);
    }
    configurations.push(json!({
        "name": candidate.name,
        "runtimeExecutable": candidate.runtime_executable,
        "runtimeArgs": candidate.runtime_args,
        "port": candidate.port,
    }));
    Ok(serde_json::to_string_pretty(&value).unwrap_or_default() + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PACKAGE: &str = r#"{"scripts":{"dev":"vite","build":"vite build","start":"node server.js","weird":"x"}}"#;

    #[test]
    fn a_vite_project_with_npm_offers_dev_and_start_on_5173() {
        let exists = |name: &str| ["package.json", "package-lock.json", "vite.config.ts"].contains(&name);
        let found = candidates(&exists, Some(PACKAGE), None);
        assert_eq!(found.iter().map(ServerCandidate::command_line).collect::<Vec<_>>(), ["npm run dev", "npm run start"]);
        assert!(found.iter().all(|c| c.port == 5173 && c.framework == Some("Vite")));
        assert_eq!(found[0].url(), "http://localhost:5173");
    }

    #[test]
    fn the_lock_file_names_the_runner_and_no_framework_means_port_3000() {
        let exists = |name: &str| ["package.json", "yarn.lock"].contains(&name);
        let found = candidates(&exists, Some(PACKAGE), None);
        assert_eq!(found[0].command_line(), "yarn run dev");
        assert_eq!((found[0].port, found[0].framework), (3000, None));
        let pnpm = |name: &str| name == "pnpm-lock.yaml";
        assert_eq!(candidates(&pnpm, Some(PACKAGE), None)[0].runtime_executable, "pnpm");
    }

    #[test]
    fn no_package_json_offers_nothing_and_saved_servers_come_first() {
        assert!(candidates(&|_| false, None, None).is_empty());
        let saved = with_server(None, &ServerCandidate { name: "web".into(), runtime_executable: "npm".into(), runtime_args: vec!["run".into(), "dev".into()], port: 1420, framework: None, saved: false }).unwrap();
        let found = candidates(&|name| name == "package.json", Some(PACKAGE), Some(&saved));
        assert_eq!(found[0].name, "web");
        assert!(found[0].saved);
        assert_eq!(found[0].port, 1420);
        assert_eq!(found.iter().filter(|c| c.command_line() == "npm run dev").count(), 1, "a saved command is not offered twice");
    }

    #[test]
    fn launch_json_keeps_other_entries_and_refuses_a_name_twice() {
        let first = ServerCandidate { name: "dev".into(), runtime_executable: "npm".into(), runtime_args: vec!["run".into(), "dev".into()], port: 5173, framework: Some("Vite"), saved: false };
        let text = with_server(Some(r#"{"version":"0.0.1","configurations":[{"name":"api","url":"http://localhost:8080"}]}"#), &first).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["configurations"][0]["name"], "api");
        assert_eq!(value["configurations"][1], json!({"name":"dev","runtimeExecutable":"npm","runtimeArgs":["run","dev"],"port":5173}));
        assert_eq!(with_server(Some(&text), &first), Err(LaunchJsonError::NameExists));
        assert_eq!(with_server(Some("[1]"), &first), Err(LaunchJsonError::Unreadable));
        assert_eq!(serde_json::from_str::<Value>(&with_server(None, &first).unwrap()).unwrap()["version"], "0.0.1");
    }
}
