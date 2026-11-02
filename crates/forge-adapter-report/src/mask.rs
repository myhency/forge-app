//! Hides what is personal in text that leaves the computer: the home folder and the user name, API keys and tokens,
//! passwords in URLs, and e-mail addresses. Project folder names stay, because a report is hard to follow without them.

use std::sync::OnceLock;

use regex::Regex;

pub struct Masker {
    home: Option<String>,
    user: Option<String>,
}

struct Rule {
    pattern: Regex,
    replacement: &'static str,
}

fn rules() -> &'static [Rule] {
    static RULES: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES.get_or_init(|| {
        let rule = |pattern: &str, replacement: &'static str| Rule { pattern: Regex::new(pattern).expect("a fixed pattern"), replacement };
        vec![
            // A password inside a URL: https://name:secret@host
            rule(r"(?i)([a-z][a-z0-9+.-]*://)[^\s/:@]+:[^\s/@]+@", "${1}<hidden>@"),
            // An Authorization header or a bearer token.
            rule(r"(?i)\b(bearer|basic)\s+[A-Za-z0-9._~+/=-]{8,}", "${1} <hidden>"),
            // key: value or key=value for names that hold a secret.
            rule(r#"(?i)\b([a-z0-9_.-]*(?:api[_-]?key|token|secret|password|passwd|authorization|credential)[a-z0-9_.-]*)(["']?\s*[:=]\s*["']?)[^\s"',;}&]+"#, "${1}${2}<hidden>"),
            // Well-known key shapes, wherever they appear.
            rule(r"\bsk-[A-Za-z0-9_-]{12,}", "<hidden-key>"),
            rule(r"\b(?:ghp|gho|ghu|ghs|ghr|github_pat)_[A-Za-z0-9_]{12,}", "<hidden-key>"),
            rule(r"\bxox[abprs]-[A-Za-z0-9-]{10,}", "<hidden-key>"),
            rule(r"\bAKIA[0-9A-Z]{16}\b", "<hidden-key>"),
            rule(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{4,}", "<hidden-token>"),
            rule(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}", "<hidden-email>"),
        ]
    })
}

impl Masker {
    /// `home` is the person's home folder; the user name is its last part.
    pub fn new(home: Option<&std::path::Path>) -> Self {
        let home_text = home.map(|path| path.to_string_lossy().trim_end_matches(['/', '\\']).to_string()).filter(|text| text.len() > 1);
        let user = home.and_then(|path| path.file_name()).map(|name| name.to_string_lossy().into_owned()).filter(|name| name.len() >= 3);
        Self { home: home_text, user }
    }

    /// The text with every personal detail replaced, and how many were.
    pub fn mask(&self, text: &str) -> (String, usize) {
        let mut out = text.to_string();
        let mut count = 0;
        if let Some(home) = &self.home {
            count += out.matches(home.as_str()).count();
            out = out.replace(home.as_str(), "~");
            // The same folder written with the other slash (a Windows path in a JSON string, escaped or not).
            let flipped = home.replace('\\', "/");
            if flipped != *home {
                count += out.matches(flipped.as_str()).count();
                out = out.replace(flipped.as_str(), "~");
            }
        }
        for rule in rules() {
            count += rule.pattern.find_iter(&out).count();
            out = rule.pattern.replace_all(&out, rule.replacement).into_owned();
        }
        // The bare user name, such as in `/Users/<name>` of another volume or in a process owner column.
        if let Some(user) = &self.user {
            let pattern = Regex::new(&format!(r"\b{}\b", regex::escape(user))).expect("an escaped name");
            count += pattern.find_iter(&out).count();
            out = pattern.replace_all(&out, "<user>").into_owned();
        }
        (out, count)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn masker() -> Masker {
        Masker::new(Some(Path::new("/Users/jamesyeo")))
    }

    #[test]
    fn the_home_folder_becomes_a_tilde() {
        let (text, count) = masker().mask("opened /Users/jamesyeo/Developer/forge/src/main.rs");
        assert_eq!(text, "opened ~/Developer/forge/src/main.rs");
        assert_eq!(count, 1);
    }

    #[test]
    fn a_windows_home_folder_is_hidden_in_both_slash_forms() {
        let masker = Masker::new(Some(Path::new(r"C:\Users\kim")));
        assert_eq!(masker.mask(r"C:\Users\kim\work and C:/Users/kim/work").0, r"~\work and ~/work");
    }

    #[test]
    fn the_bare_user_name_is_hidden_but_not_inside_other_words() {
        let (text, _) = masker().mask("owner jamesyeo group staff; jamesyeo2 stays");
        assert_eq!(text, "owner <user> group staff; jamesyeo2 stays");
    }

    #[test]
    fn api_keys_and_tokens_are_hidden() {
        let (text, count) = masker().mask("key sk-ant-api03-AbCdEf123456_xyz and ghp_abcdefghijklmnop1234 and Bearer abcdef1234567890");
        assert!(!text.contains("AbCdEf") && !text.contains("ghp_abc") && !text.contains("abcdef1234567890"), "{text}");
        assert!(count >= 3);
    }

    #[test]
    fn a_secret_assigned_to_a_named_field_is_hidden_but_the_name_stays() {
        let (text, _) = masker().mask(r#"{"apiKey": "hunter2hunter2", "password":"p@ss", ANTHROPIC_AUTH_TOKEN=abc123xyz}"#);
        assert!(text.contains("apiKey") && text.contains("password") && text.contains("ANTHROPIC_AUTH_TOKEN"));
        assert!(!text.contains("hunter2") && !text.contains("p@ss") && !text.contains("abc123xyz"), "{text}");
    }

    #[test]
    fn a_password_in_a_url_is_hidden() {
        let (text, _) = masker().mask("clone https://me:secret@git.example.com/a.git");
        assert_eq!(text, "clone https://<hidden>@git.example.com/a.git");
    }

    #[test]
    fn email_addresses_are_hidden() {
        let (text, _) = masker().mask("by hency.yeo@gmail.com today");
        assert_eq!(text, "by <hidden-email> today");
    }

    #[test]
    fn plain_text_is_left_alone() {
        let (text, count) = masker().mask("sidecar exited with code 1 after 3.2s");
        assert_eq!(text, "sidecar exited with code 1 after 3.2s");
        assert_eq!(count, 0);
    }

    #[test]
    fn with_no_home_only_the_patterns_apply() {
        let masker = Masker::new(None);
        assert_eq!(masker.mask("/Users/x/a sk-abcdefghijklmnop").0, "/Users/x/a <hidden-key>");
    }
}
