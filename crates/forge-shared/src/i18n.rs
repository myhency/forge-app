//! Text in the person's language. The English sentence is the key; the Korean table lives in `ko.rs`.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

use dioxus::core::Runtime;
use dioxus::prelude::*;
use forge_domain::Language;

use crate::ko::KO;

/// The Korean text for `english`, or `english` itself when the language is English or the table
/// has no entry for it. A key may carry a context after a bar (`"Created|tool"`): the context only
/// tells two meanings of one English word apart in the table, and is never shown.
pub fn translate(language: Language, english: &'static str) -> &'static str {
    let shown = english.split('|').next().unwrap_or(english);
    match language {
        Language::English => shown,
        Language::Korean => KO
            .binary_search_by(|(key, _)| (*key).cmp(english))
            .map(|index| KO[index].1)
            .unwrap_or(shown),
    }
}

pub use forge_domain::i18n::fill;

static SYSTEM: OnceLock<Language> = OnceLock::new();

/// Tells the module what the system language is. Called once, before the window opens.
pub fn set_system_language(language: Language) {
    register();
    let _ = SYSTEM.set(language);
}

/// The system language; English until `set_system_language` has been called.
pub fn system_language() -> Language {
    SYSTEM.get().copied().unwrap_or_default()
}

/// The language to use for a stored choice: the choice, or the system language when there is none.
pub fn effective_language(chosen: Option<Language>) -> Language {
    chosen.unwrap_or_else(system_language)
}

static LANGUAGE: GlobalSignal<Language> = Signal::global(system_language);

// The language as a plain value too, for code that runs outside a Dioxus runtime (the unit tests of
// the state machines). 0 is English, 1 is Korean. It follows `set_current`.
static MIRROR: AtomicU8 = AtomicU8::new(0);

/// The language on screen. Reading it in a component redraws that component when it changes.
/// Outside a Dioxus runtime it is the last language set (English until one is set).
pub fn current() -> Language {
    if Runtime::try_current().is_some() {
        *LANGUAGE.read()
    } else if MIRROR.load(Ordering::Relaxed) == 1 {
        Language::Korean
    } else {
        Language::English
    }
}

pub fn set_current(language: Language) {
    register();
    MIRROR.store(u8::from(language == Language::Korean), Ordering::Relaxed);
    if Runtime::try_current().is_some() && *LANGUAGE.peek() != language {
        *LANGUAGE.write() = language;
    }
}

/// Lets the domain (which cannot see this module) translate through `text`. Idempotent.
fn register() {
    forge_domain::i18n::set_translator(text);
}

/// `english` in the current language (see `translate`).
pub fn text(english: &'static str) -> &'static str {
    translate(current(), english)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    #[test]
    fn english_returns_the_sentence_and_korean_the_table_text() {
        assert_eq!(translate(Language::English, "Settings"), "Settings");
        assert_eq!(translate(Language::Korean, "Settings"), "설정");
    }

    #[test]
    fn a_sentence_missing_from_the_table_stays_english() {
        assert_eq!(translate(Language::Korean, "No such sentence in the table"), "No such sentence in the table");
    }

    #[test]
    fn values_are_filled_by_name() {
        let args = [("n", "3".to_string()), ("name", "forge".to_string())];
        assert_eq!(fill("{n} sessions in {name}", &args), "3 sessions in forge");
        assert_eq!(fill("{name}: {name}", &args), "forge: forge");
        assert_eq!(fill("{n}개 세션", &args), "3개 세션");
    }

    #[test]
    fn a_placeholder_without_a_value_is_left_as_written() {
        assert_eq!(fill("{n} of {total}", &[("n", "1".to_string())]), "1 of {total}");
    }

    #[test]
    fn a_value_that_looks_like_a_placeholder_is_not_filled_again() {
        let args = [("a", "{b}".to_string()), ("b", "x".to_string())];
        assert_eq!(fill("{a}", &args), "{b}");
    }

    #[test]
    fn a_context_after_a_bar_tells_two_meanings_of_one_word_apart() {
        assert_eq!(translate(Language::English, "Sample|context"), "Sample");
        assert_eq!(translate(Language::Korean, "Sample|context"), "Sample", "no table entry: the English word, without the context");
        assert_eq!(translate(Language::English, "Plain"), "Plain");
        assert_eq!(translate(Language::Korean, "Created|tool"), "생성");
        assert_eq!(translate(Language::Korean, "Created"), "생성 날짜");
    }

    #[test]
    fn the_chosen_language_wins_over_the_system_one() {
        assert_eq!(effective_language(Some(Language::English)), Language::English);
        assert_eq!(effective_language(Some(Language::Korean)), Language::Korean);
        // Nothing sets the system language in this test binary, so it is English.
        assert_eq!(effective_language(None), Language::English);
    }

    #[test]
    fn outside_a_runtime_the_text_follows_the_last_language_set() {
        assert_eq!(text("Settings"), "Settings");
        set_current(Language::Korean);
        assert_eq!(text("Settings"), "설정");
        assert_eq!(crate::tr!("Settings"), "설정");
        set_current(Language::English);
        assert_eq!(text("Settings"), "Settings");
    }

    #[test]
    fn the_table_is_sorted_and_has_no_duplicates() {
        for pair in KO.windows(2) {
            assert!(pair[0].0 < pair[1].0, "`{}` must come before `{}`", pair[0].0, pair[1].0);
        }
    }

    fn placeholders(text: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            let Some(end) = rest[start..].find('}') else { break };
            found.insert(rest[start + 1..start + end].to_string());
            rest = &rest[start + end + 1..];
        }
        found
    }

    #[test]
    fn a_korean_text_has_the_placeholders_of_its_key() {
        for (english, korean) in KO {
            assert_eq!(placeholders(english), placeholders(korean), "`{english}`");
        }
    }

    // Every string literal written as `tr!("...")` or `msg!("...")` in the sources of every crate.
    fn used_keys() -> BTreeSet<String> {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = Vec::new();
        collect(&crates, &mut files);
        let mut keys = BTreeSet::new();
        for file in files {
            if file.ends_with("forge-shared/src/i18n.rs") || file.ends_with("forge-domain/src/i18n.rs") {
                continue;
            }
            let source = std::fs::read_to_string(&file).unwrap();
            for name in ["tr!(", "msg!("] {
                let mut from = 0;
                while let Some(found) = source[from..].find(name) {
                    let at = from + found;
                    from = at + name.len();
                    let before = source[..at].chars().next_back();
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue; // `attr!(`, `str!(`...
                    }
                    let rest = source[from..].trim_start();
                    assert!(rest.starts_with('"'), "{}: `{name}` must be followed by a string literal", file.display());
                    let mut key = String::new();
                    let mut chars = rest[1..].chars();
                    while let Some(c) = chars.next() {
                        match c {
                            '"' => break,
                            '\\' => key.push(match chars.next() {
                                Some('n') => '\n',
                                Some(other) => other,
                                None => break,
                            }),
                            other => key.push(other),
                        }
                    }
                    keys.insert(key);
                }
            }
        }
        keys
    }

    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                if name != "target" && name != "node_modules" {
                    collect(&path, out);
                }
            } else if path.extension().is_some_and(|ext| ext == "rs") && path.components().any(|c| c.as_os_str() == "src") {
                out.push(path);
            }
        }
    }

    #[test]
    fn every_sentence_in_the_sources_has_a_korean_text() {
        let known: BTreeSet<&str> = KO.iter().map(|(english, _)| *english).collect();
        for key in used_keys() {
            assert!(known.contains(key.as_str()), "`{key}` is used in the sources but not in ko.rs");
        }
    }

    #[test]
    fn every_korean_entry_is_used_in_the_sources() {
        let used = used_keys();
        for (english, _) in KO {
            assert!(used.contains(*english), "`{english}` is in ko.rs but no `tr!` or `msg!` uses it");
        }
    }
}
