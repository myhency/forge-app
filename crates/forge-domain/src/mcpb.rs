//! Desktop extensions (`.mcpb`, formerly `.dxt`): a zip with a `manifest.json` that names a local MCP
//! server. The desktop app installs them in Settings › 확장 프로그램; Forge installs one as a connector.

use std::collections::BTreeMap;

use serde_json::Value;

/// One setting the extension asks the person for (`user_config` in the manifest).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleSetting {
    pub key: String,
    pub title: String,
    pub description: String,
    /// `string`, `number`, `boolean`, `directory` or `file`.
    pub kind: String,
    pub required: bool,
    /// Several values (one argument each when the whole argument is the placeholder).
    pub multiple: bool,
    pub sensitive: bool,
    pub default: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleManifest {
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub description: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub settings: Vec<BundleSetting>,
}

fn text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn values_of(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(|item| match item {
            Value::String(text) => Some(text.clone()),
            Value::Null => None,
            other => Some(other.to_string()),
        }).collect(),
        Some(Value::String(text)) => vec![text.clone()],
        Some(Value::Null) | None => Vec::new(),
        Some(other) => vec![other.to_string()],
    }
}

/// Reads a `manifest.json`. An error says what is missing, for the person.
pub fn read_manifest(json: &str) -> Result<BundleManifest, String> {
    let value: Value = serde_json::from_str(json).map_err(|_| "The extension's manifest.json is not valid JSON.".to_string())?;
    let name = text(&value, "name");
    if name.trim().is_empty() {
        return Err("The extension's manifest has no name.".to_string());
    }
    let config = value.get("server").and_then(|server| server.get("mcp_config")).ok_or_else(|| "The extension's manifest names no server to run (server.mcp_config).".to_string())?;
    let command = text(config, "command");
    if command.trim().is_empty() {
        return Err("The extension's manifest names no command to run.".to_string());
    }
    let args = config.get("args").and_then(Value::as_array).map(|args| args.iter().filter_map(|arg| arg.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let env = config.get("env").and_then(Value::as_object).map(|env| env.iter().map(|(key, value)| (key.clone(), value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string()))).collect()).unwrap_or_default();
    let settings = value
        .get("user_config")
        .and_then(Value::as_object)
        .map(|fields| {
            fields
                .iter()
                .map(|(key, field)| BundleSetting {
                    key: key.clone(),
                    title: Some(text(field, "title")).filter(|title| !title.is_empty()).unwrap_or_else(|| key.clone()),
                    description: text(field, "description"),
                    kind: Some(text(field, "type")).filter(|kind| !kind.is_empty()).unwrap_or_else(|| "string".to_string()),
                    required: field.get("required").and_then(Value::as_bool).unwrap_or(false),
                    multiple: field.get("multiple").and_then(Value::as_bool).unwrap_or(false),
                    sensitive: field.get("sensitive").and_then(Value::as_bool).unwrap_or(false),
                    default: values_of(field.get("default")),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(BundleManifest {
        display_name: Some(text(&value, "display_name")).filter(|shown| !shown.is_empty()).unwrap_or_else(|| name.clone()),
        version: text(&value, "version"),
        description: text(&value, "description"),
        name,
        command,
        args,
        env,
        settings,
    })
}

/// A connector name made from the extension's name: letters, digits, `-` and `_`, at most 64.
pub fn bundle_server_name(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    let trimmed = cleaned.trim_matches('-');
    let short: String = trimmed.chars().take(64).collect();
    if short.is_empty() { "extension".to_string() } else { short }
}

/// Where the placeholders of the manifest point on this computer.
#[derive(Debug, Clone)]
pub struct BundlePlaces {
    /// The folder the extension was unpacked into (`${__dirname}`).
    pub dir: String,
    pub home: String,
    pub separator: char,
    /// Forge's own Node, for a `node` command (an app opened from the Dock may not find one on PATH).
    pub node: Option<String>,
}

/// The command, arguments and variables to run the extension with the person's `values`
/// (`${user_config.KEY}`): a placeholder that is a whole argument becomes one argument per value; inside
/// other text the values are joined with commas. A required setting without a value is an error.
pub fn resolve_bundle(manifest: &BundleManifest, places: &BundlePlaces, values: &BTreeMap<String, Vec<String>>) -> Result<(String, Vec<String>, Vec<(String, String)>), String> {
    for setting in &manifest.settings {
        let given = values.get(&setting.key).map(|values| values.iter().any(|value| !value.trim().is_empty())).unwrap_or(false);
        if setting.required && !given && setting.default.is_empty() {
            return Err(format!("{} is needed.", setting.title));
        }
    }
    let value_of = |key: &str| -> Vec<String> {
        let given: Vec<String> = values.get(key).map(|values| values.iter().filter(|value| !value.trim().is_empty()).cloned().collect()).unwrap_or_default();
        if given.is_empty() {
            manifest.settings.iter().find(|setting| setting.key == key).map(|setting| setting.default.clone()).unwrap_or_default()
        } else {
            given
        }
    };
    let fill = |text: &str| -> String {
        let mut out = text
            .replace("${__dirname}", &places.dir)
            .replace("${HOME}", &places.home)
            .replace("${pathSeparator}", &places.separator.to_string())
            .replace("${/}", &places.separator.to_string());
        while let Some(start) = out.find("${user_config.") {
            let Some(length) = out[start..].find('}') else { break };
            let key = out[start + "${user_config.".len()..start + length].to_string();
            out.replace_range(start..start + length + 1, &value_of(&key).join(","));
        }
        out
    };
    let mut args = Vec::new();
    for arg in &manifest.args {
        let whole = arg.strip_prefix("${user_config.").and_then(|rest| rest.strip_suffix('}')).filter(|key| !key.contains('}'));
        match whole {
            Some(key) => args.extend(value_of(key)),
            None => args.push(fill(arg)),
        }
    }
    let command = match (&places.node, manifest.command.as_str()) {
        (Some(node), "node") => node.clone(),
        _ => fill(&manifest.command),
    };
    let env = manifest.env.iter().map(|(key, value)| (key.clone(), fill(value))).collect();
    Ok((command, args, env))
}

/// The arguments as one line that `split_args` reads back the same: an argument with a space or a
/// quote is quoted. `None` when one holds both kinds of quote.
pub fn join_args(args: &[String]) -> Option<String> {
    args.iter()
        .map(|arg| {
            if !arg.is_empty() && !arg.contains(|c: char| c.is_whitespace() || c == '"' || c == '\'') {
                Some(arg.clone())
            } else if !arg.contains('"') {
                Some(format!("\"{arg}\""))
            } else if !arg.contains('\'') {
                Some(format!("'{arg}'"))
            } else {
                None
            }
        })
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILESYSTEM: &str = r#"{"manifest_version":"0.3","name":"Filesystem","display_name":"Filesystem","version":"2026.7.10",
      "server":{"type":"node","entry_point":"dist/index.js","mcp_config":{"command":"node","args":["${__dirname}/dist/index.js","${user_config.allowed_directories}"]}},
      "user_config":{"allowed_directories":{"type":"directory","title":"Allowed Directories","description":"Select directories","multiple":true,"required":true,"default":[]}}}"#;

    fn places() -> BundlePlaces {
        BundlePlaces { dir: "/x/ext/Filesystem".into(), home: "/Users/me".into(), separator: '/', node: Some("/app/node".into()) }
    }

    #[test]
    fn the_filesystem_extension_reads_and_asks_for_its_folders() {
        let manifest = read_manifest(FILESYSTEM).unwrap();
        assert_eq!((manifest.name.as_str(), manifest.command.as_str()), ("Filesystem", "node"));
        assert_eq!(manifest.settings.len(), 1);
        assert!(manifest.settings[0].required && manifest.settings[0].multiple && manifest.settings[0].kind == "directory");
        assert_eq!(resolve_bundle(&manifest, &places(), &BTreeMap::new()), Err("Allowed Directories is needed.".to_string()));
        let values = BTreeMap::from([("allowed_directories".to_string(), vec!["/Users/me/a".to_string(), "/Users/me/b c".to_string()])]);
        let (command, args, env) = resolve_bundle(&manifest, &places(), &values).unwrap();
        assert_eq!(command, "/app/node", "Forge's own Node runs a node extension");
        assert_eq!(args, vec!["/x/ext/Filesystem/dist/index.js", "/Users/me/a", "/Users/me/b c"]);
        assert!(env.is_empty());
        assert_eq!(join_args(&args).unwrap(), "/x/ext/Filesystem/dist/index.js /Users/me/a \"/Users/me/b c\"");
        assert_eq!(crate::split_args(&join_args(&args).unwrap()).unwrap(), args);
    }

    #[test]
    fn placeholders_inside_text_and_env_are_filled_and_defaults_count() {
        let json = r#"{"name":"My Tool!","server":{"mcp_config":{"command":"${__dirname}/bin/tool","args":["--root=${user_config.root}","--home=${HOME}"],"env":{"TOKEN":"${user_config.token}"}}},
          "user_config":{"root":{"type":"string","default":"/tmp"},"token":{"type":"string","sensitive":true}}}"#;
        let manifest = read_manifest(json).unwrap();
        let values = BTreeMap::from([("token".to_string(), vec!["s3cret".to_string()])]);
        let (command, args, env) = resolve_bundle(&manifest, &places(), &values).unwrap();
        assert_eq!(command, "/x/ext/Filesystem/bin/tool");
        assert_eq!(args, vec!["--root=/tmp", "--home=/Users/me"]);
        assert_eq!(env, vec![("TOKEN".to_string(), "s3cret".to_string())]);
        assert_eq!(bundle_server_name(&manifest.name), "My-Tool");
    }

    #[test]
    fn a_manifest_without_a_server_is_refused() {
        assert!(read_manifest(r#"{"name":"x"}"#).unwrap_err().contains("server"));
        assert!(read_manifest("nope").is_err());
        assert_eq!(join_args(&["a\"b'c".to_string()]), None);
    }
}
