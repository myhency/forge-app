use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A saved set of folders and variables a new session can start from. `folders[0]` is the working
/// folder; the others are added directories. `branch` and `worktree` belong to the first folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub folders: Vec<PathBuf>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub worktree: bool,
    /// `.env` text, as typed.
    #[serde(default)]
    pub env_vars: String,
    #[serde(default)]
    pub pinned: bool,
    /// Epoch milliseconds.
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub last_used_at: i64,
}

impl Environment {
    /// The environment as it is stored: trimmed name, each folder once (the first spelling wins), and
    /// no branch or worktree choice when there is no folder to apply it to.
    pub fn normalized(mut self) -> Environment {
        self.name = self.name.trim().to_string();
        let mut seen: Vec<PathBuf> = Vec::new();
        for folder in std::mem::take(&mut self.folders) {
            if !seen.contains(&folder) {
                seen.push(folder);
            }
        }
        self.folders = seen;
        if self.folders.is_empty() {
            self.branch = None;
            self.worktree = false;
        }
        self
    }
}

/// `seed` is any number that differs from call to call (the clock's nanoseconds).
pub fn new_environment_id(seed: u64) -> String {
    format!("env-{seed:016x}")
}

/// Variables for a session's process. Their values may be secrets, so `Debug` shows names only.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct EnvVars(pub Vec<(String, String)>);

impl std::fmt::Debug for EnvVars {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.iter().map(|(name, _)| name)).finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvParseReason {
    NotAPair,
    BadKey,
    UnclosedQuote,
}

/// A line of the variables text that is not valid; `line` counts from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvParseError {
    pub line: usize,
    pub reason: EnvParseReason,
}

fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Reads `.env` text: `KEY=value` lines, `#` comment lines, blank lines, an optional `export `,
/// a value in `"` or `'` that may run over several lines, a trailing ` #` comment on an unquoted
/// value. A later duplicate replaces the earlier one.
pub fn parse_env(text: &str) -> Result<Vec<(String, String)>, EnvParseError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<(String, String)> = Vec::new();
    let mut next = 0;
    while next < lines.len() {
        let number = next + 1;
        let line = lines[next].trim();
        next += 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").map_or(line, str::trim_start);
        let fail = |reason| EnvParseError { line: number, reason };
        let Some((key, rest)) = line.split_once('=') else {
            return Err(fail(EnvParseReason::NotAPair));
        };
        let key = key.trim();
        if !valid_key(key) {
            return Err(fail(EnvParseReason::BadKey));
        }
        let rest = rest.trim_start();
        let value = match rest.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                let mut value = String::new();
                let mut tail = &rest[1..];
                loop {
                    if let Some(end) = tail.find(quote) {
                        value.push_str(&tail[..end]);
                        break;
                    }
                    value.push_str(tail);
                    if next >= lines.len() {
                        return Err(fail(EnvParseReason::UnclosedQuote));
                    }
                    value.push('\n');
                    tail = lines[next];
                    next += 1;
                }
                value
            }
            _ => rest.split(" #").next().unwrap_or("").trim_end().to_string(),
        };
        out.retain(|(existing, _)| existing != key);
        out.push((key.to_string(), value));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(text: &str) -> Vec<(String, String)> {
        parse_env(text).unwrap()
    }

    fn pair(key: &str, value: &str) -> (String, String) {
        (key.to_string(), value.to_string())
    }

    #[test]
    fn plain_pairs_comments_blank_lines_and_export_are_read() {
        let text = "# a comment\n\nA=1\nexport B=two words\nC=\n";
        assert_eq!(env(text), vec![pair("A", "1"), pair("B", "two words"), pair("C", "")]);
    }

    #[test]
    fn a_quoted_value_keeps_its_spaces_and_may_run_over_lines() {
        assert_eq!(env("D=\"  x y \""), vec![pair("D", "  x y ")]);
        assert_eq!(env("CONFIG=\"key1=val1\nkey2=val2\"\nE='it'"), vec![pair("CONFIG", "key1=val1\nkey2=val2"), pair("E", "it")]);
    }

    #[test]
    fn an_equals_sign_inside_a_value_and_a_trailing_comment_are_handled() {
        assert_eq!(env("URL=https://x.test/?a=b # note"), vec![pair("URL", "https://x.test/?a=b")]);
    }

    #[test]
    fn windows_line_endings_a_bom_and_spaces_around_the_equals_sign_read_like_unix_text() {
        let unix = env("A=1\nB=\"x\ny\"\nC = 3");
        let windows = env("\u{feff}A=1\r\nB=\"x\r\ny\"\r\nC = 3\r\n");
        assert_eq!(unix, vec![pair("A", "1"), pair("B", "x\ny"), pair("C", "3")]);
        assert_eq!(windows, unix);
    }

    #[test]
    fn a_later_duplicate_wins() {
        assert_eq!(env("A=1\nB=x\nA=2"), vec![pair("B", "x"), pair("A", "2")]);
    }

    #[test]
    fn errors_name_the_line_and_the_reason() {
        assert_eq!(parse_env("A=1\nnot a pair").unwrap_err(), EnvParseError { line: 2, reason: EnvParseReason::NotAPair });
        assert_eq!(parse_env("1A=x").unwrap_err(), EnvParseError { line: 1, reason: EnvParseReason::BadKey });
        assert_eq!(parse_env("A=1\nB=\"open\nstill open").unwrap_err(), EnvParseError { line: 2, reason: EnvParseReason::UnclosedQuote });
    }

    #[test]
    fn env_vars_debug_shows_names_and_never_values() {
        let vars = EnvVars(vec![pair("TOKEN", "s3cret"), pair("NODE_ENV", "production")]);
        let shown = format!("{vars:?}");
        assert!(shown.contains("TOKEN") && shown.contains("NODE_ENV"));
        assert!(!shown.contains("s3cret") && !shown.contains("production"));
    }

    #[test]
    fn normalized_trims_the_name_drops_repeated_folders_and_clears_the_first_folder_choices_when_there_is_none() {
        let mut environment = Environment {
            id: "env-1".into(),
            name: "  work  ".into(),
            folders: vec!["/a".into(), "/b".into(), "/a".into()],
            branch: Some("dev".into()),
            worktree: true,
            env_vars: String::new(),
            pinned: false,
            created_at: 1,
            last_used_at: 2,
        };
        let kept = environment.clone().normalized();
        assert_eq!(kept.name, "work");
        assert_eq!(kept.folders, vec![std::path::PathBuf::from("/a"), "/b".into()]);
        assert_eq!(kept.branch.as_deref(), Some("dev"));
        environment.folders.clear();
        let empty = environment.normalized();
        assert_eq!((empty.branch, empty.worktree), (None, false));
    }

    #[test]
    fn an_id_is_sixteen_hex_digits_after_the_prefix() {
        assert_eq!(new_environment_id(255), "env-00000000000000ff");
    }
}
