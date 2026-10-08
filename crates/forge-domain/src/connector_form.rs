use std::fmt;

use crate::connector::{mask_url, ConnectorKind};

/// What the Add dialog holds. Values (headers, env) live here only until the CLI is called.
#[derive(Clone, PartialEq, Eq)]
pub struct ConnectorDraft {
    pub name: String,
    /// `Http`, `Sse` or `Stdio`.
    pub kind: ConnectorKind,
    pub url: String,
    pub command: String,
    /// One text field; split like a shell would split quotes (see `split_args`).
    pub args: String,
    pub headers: Vec<(String, String)>,
    pub env: Vec<(String, String)>,
}

impl Default for ConnectorDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: ConnectorKind::Http,
            url: String::new(),
            command: String::new(),
            args: String::new(),
            headers: Vec::new(),
            env: Vec::new(),
        }
    }
}

impl fmt::Debug for ConnectorDraft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectorDraft")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("headers", &self.headers.len())
            .field("env", &self.env.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum NewTransport {
    Http { url: String, headers: Vec<(String, String)> },
    Sse { url: String, headers: Vec<(String, String)> },
    Stdio { command: String, args: Vec<String>, env: Vec<(String, String)> },
}

impl fmt::Debug for NewTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NewTransport::Http { url, headers } => write!(f, "Http({}, {} headers)", mask_url(url), headers.len()),
            NewTransport::Sse { url, headers } => write!(f, "Sse({}, {} headers)", mask_url(url), headers.len()),
            NewTransport::Stdio { command, args, env } => write!(f, "Stdio({command}, {} args, {} env)", args.len(), env.len()),
        }
    }
}

/// A connector that passed `validate_new`: what the CLI is asked to add.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NewConnector {
    pub name: String,
    pub transport: NewTransport,
}

impl NewConnector {
    /// The header and env values, longest first, so an error text can have them taken out.
    pub fn secrets(&self) -> Vec<String> {
        let rows = match &self.transport {
            NewTransport::Http { headers, .. } | NewTransport::Sse { headers, .. } => headers,
            NewTransport::Stdio { env, .. } => env,
        };
        let mut values: Vec<String> = Vec::new();
        for (_, value) in rows.iter().filter(|(_, value)| !value.is_empty()) {
            values.push(value.clone());
            // The CLI prints a value as JSON in places: with the quotes and backslashes escaped.
            let escaped = serde_json::to_string(value).unwrap_or_default();
            let escaped = escaped.trim_matches('"').to_string();
            if escaped != *value && !escaped.is_empty() {
                values.push(escaped);
            }
        }
        values.sort_by_key(|value| std::cmp::Reverse(value.len()));
        values
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 64
        && !name.starts_with('-')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn valid_url(url: &str) -> bool {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"));
    rest.is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/') && !url.chars().any(char::is_whitespace))
}

fn valid_header_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_graphic() && c != ':')
}

fn valid_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn blank(row: &(String, String)) -> bool {
    row.0.trim().is_empty() && row.1.trim().is_empty()
}

fn clean_headers(rows: &[(String, String)]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for row in rows.iter().filter(|row| !blank(row)) {
        let name = row.0.trim();
        if !valid_header_name(name) {
            return Err(format!("\"{name}\" is not a header name: no spaces or colons, and it cannot be empty."));
        }
        if row.1.contains(['\n', '\r', '\0']) {
            return Err(format!("The value of {name} cannot have a line break."));
        }
        out.push((name.to_string(), row.1.trim().to_string()));
    }
    Ok(out)
}

fn clean_env(rows: &[(String, String)]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for row in rows.iter().filter(|row| !blank(row)) {
        let key = row.0.trim();
        if !valid_env_key(key) {
            return Err(format!("\"{key}\" is not a variable name: use letters, digits and _, not starting with a digit."));
        }
        if row.1.contains('\0') {
            return Err(format!("The value of {key} has a character that cannot be used."));
        }
        out.push((key.to_string(), row.1.clone()));
    }
    Ok(out)
}

/// Splits on spaces; text inside '…' or "…" stays one argument (an empty pair makes an empty argument).
pub fn split_args(text: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in text.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => current.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                started = true;
            }
            None if c.is_whitespace() => {
                if started || !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            None => current.push(c),
        }
    }
    if quote.is_some() {
        return Err("Close the quote in the arguments.".to_string());
    }
    if started || !current.is_empty() {
        out.push(current);
    }
    Ok(out)
}

/// Checks the form. `taken` is every connector name already in use (any scope but plugins).
pub fn validate_new(draft: &ConnectorDraft, taken: &[String]) -> Result<NewConnector, String> {
    let name = draft.name.trim().to_string();
    if name.is_empty() {
        return Err("Give the connector a name.".to_string());
    }
    if !valid_name(&name) {
        return Err("A name can use letters, numbers, - and _ only (up to 64), and cannot start with -.".to_string());
    }
    if taken.iter().any(|existing| *existing == name) {
        return Err(format!("A connector named {name} already exists."));
    }
    let transport = match draft.kind {
        ConnectorKind::Http | ConnectorKind::Sse => {
            let url = draft.url.trim().to_string();
            if !valid_url(&url) {
                return Err("The address must start with https:// or http:// and have no spaces.".to_string());
            }
            let headers = clean_headers(&draft.headers)?;
            if draft.kind == ConnectorKind::Http {
                NewTransport::Http { url, headers }
            } else {
                NewTransport::Sse { url, headers }
            }
        }
        ConnectorKind::Stdio => {
            let command = draft.command.trim().to_string();
            if command.is_empty() {
                return Err("Enter the command to run.".to_string());
            }
            NewTransport::Stdio { command, args: split_args(&draft.args)?, env: clean_env(&draft.env)? }
        }
        ConnectorKind::Unknown => return Err("Choose a type.".to_string()),
    };
    Ok(NewConnector { name, transport })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http() -> ConnectorDraft {
        ConnectorDraft { name: "sentry".into(), kind: ConnectorKind::Http, url: "https://mcp.sentry.dev/mcp".into(), ..ConnectorDraft::default() }
    }

    fn local() -> ConnectorDraft {
        ConnectorDraft { name: "files".into(), kind: ConnectorKind::Stdio, command: "npx".into(), args: "-y pkg \"two words\"".into(), ..ConnectorDraft::default() }
    }

    #[test]
    fn a_good_remote_draft_becomes_a_new_connector() {
        let mut draft = http();
        draft.headers = vec![("Authorization".into(), "Bearer abc".into()), (" ".into(), " ".into())];
        let made = validate_new(&draft, &[]).unwrap();
        assert_eq!(made.name, "sentry");
        assert_eq!(
            made.transport,
            NewTransport::Http { url: "https://mcp.sentry.dev/mcp".into(), headers: vec![("Authorization".into(), "Bearer abc".into())] }
        );
        assert_eq!(made.secrets(), vec!["Bearer abc".to_string()]);
    }

    #[test]
    fn a_good_local_draft_splits_the_args_and_keeps_quoted_words_together() {
        let mut draft = local();
        draft.env = vec![("API_KEY".into(), "k 1".into())];
        let made = validate_new(&draft, &[]).unwrap();
        assert_eq!(
            made.transport,
            NewTransport::Stdio { command: "npx".into(), args: vec!["-y".into(), "pkg".into(), "two words".into()], env: vec![("API_KEY".into(), "k 1".into())] }
        );
    }

    #[test]
    fn the_name_rules() {
        for bad in ["", "  ", "-x", "a b", "a.b", "한글", &"x".repeat(65)] {
            let mut draft = http();
            draft.name = bad.to_string();
            assert!(validate_new(&draft, &[]).is_err(), "{bad:?}");
        }
        for good in ["a", "my-server_2", &"x".repeat(64)] {
            let mut draft = http();
            draft.name = good.to_string();
            assert!(validate_new(&draft, &[]).is_ok(), "{good:?}");
        }
    }

    #[test]
    fn a_name_that_is_taken_is_refused_with_its_name_in_the_message() {
        let error = validate_new(&http(), &["sentry".to_string()]).unwrap_err();
        assert!(error.contains("sentry") && error.contains("already"), "{error}");
    }

    #[test]
    fn the_address_must_be_http_or_https_without_spaces() {
        for bad in ["", "mcp.sentry.dev", "ftp://x/y", "https://", "https://a b", "javascript:alert(1)"] {
            let mut draft = http();
            draft.url = bad.to_string();
            assert!(validate_new(&draft, &[]).is_err(), "{bad:?}");
        }
        let mut draft = http();
        draft.url = "  http://localhost:3000/sse  ".into();
        assert!(validate_new(&draft, &[]).is_ok());
    }

    #[test]
    fn header_and_env_names_are_checked_and_blank_rows_are_dropped() {
        let mut draft = http();
        draft.headers = vec![("Bad:Name".into(), "v".into())];
        assert!(validate_new(&draft, &[]).is_err());
        draft.headers = vec![("Has Space".into(), "v".into())];
        assert!(validate_new(&draft, &[]).is_err());
        draft.headers = vec![("X-Ok".into(), "line\nbreak".into())];
        assert!(validate_new(&draft, &[]).is_err());
        let mut draft = local();
        draft.env = vec![("1BAD".into(), "v".into())];
        assert!(validate_new(&draft, &[]).is_err());
        draft.env = vec![("".into(), "".into())];
        assert!(validate_new(&draft, &[]).is_ok());
    }

    #[test]
    fn a_local_connector_needs_a_command_and_balanced_quotes() {
        let mut draft = local();
        draft.command = "  ".into();
        assert!(validate_new(&draft, &[]).is_err());
        let mut draft = local();
        draft.args = "-y \"open".into();
        assert!(validate_new(&draft, &[]).is_err());
    }

    #[test]
    fn unknown_kind_is_refused() {
        let mut draft = http();
        draft.kind = ConnectorKind::Unknown;
        assert!(validate_new(&draft, &[]).is_err());
    }

    #[test]
    fn split_args_handles_quotes_and_empty_strings() {
        assert_eq!(split_args("").unwrap(), Vec::<String>::new());
        assert_eq!(split_args("  a   b ").unwrap(), vec!["a", "b"]);
        assert_eq!(split_args("a 'b c' \"d e\" \"\"").unwrap(), vec!["a", "b c", "d e", ""]);
        assert_eq!(split_args("--flag=\"x y\"").unwrap(), vec!["--flag=x y"]);
        assert!(split_args("'open").is_err());
    }

    #[test]
    fn secrets_also_list_the_json_escaped_form_of_a_value() {
        let mut draft = http();
        draft.headers = vec![("X".into(), "a \"q\" b\\c".into())];
        let secrets = validate_new(&draft, &[]).unwrap().secrets();
        assert!(secrets.contains(&"a \"q\" b\\c".to_string()));
        assert!(secrets.contains(&"a \\\"q\\\" b\\\\c".to_string()), "{secrets:?}");
    }

    #[test]
    fn debug_hides_a_token_in_the_address_too() {
        let mut draft = http();
        draft.url = "https://h/p?token=SECRETQ".into();
        let made = validate_new(&draft, &[]).unwrap();
        assert!(!format!("{made:?}").contains("SECRETQ"));
    }

    #[test]
    fn debug_never_prints_a_value() {
        let mut draft = http();
        draft.headers = vec![("Authorization".into(), "TOP-SECRET".into())];
        assert!(!format!("{draft:?}").contains("TOP-SECRET"));
        let made = validate_new(&draft, &[]).unwrap();
        assert!(!format!("{made:?}").contains("TOP-SECRET"));
    }
}
