# Plan 36: the account menu and the language choice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An account area at the bottom of the sidebar with a menu (Settings, Usage, Language, Get help, Learn more, Get API key), a Korean / English choice that redraws the app at once and is remembered, and the translation structure the next five stages fill in.

**Architecture:** `Language` lives in `forge-domain` next to `Theme` and is stored in `AppConfig.language` (`None` = follow the system). `forge-shared::i18n` holds a pure `translate` / `fill` pair over one sorted Korean table, a global Dioxus signal with the current language, and two macros: `tr!("English text")` (translate now) and `msg!("English text")` (mark a string that is translated later, at render time). A test scans every crate's sources for these macros and checks the table. `forge-features` gets the menu as plain data (`account_rows`) plus the language save in `Settings`; `forge-widgets` draws it.

**Tech Stack:** Rust workspace, Dioxus 0.7.10 desktop (`GlobalSignal`), `serde`, `sys-locale` (new, read in `forge-app` only).

**Spec:** `docs/superpowers/specs/2026-10-09-account-menu-language-design.md`

## Global Constraints

- Keys of the translation are the English sentences. A string with values is written `tr!("{n} sessions", n = count)`; the Korean text has the same placeholders. Placeholders are replaced by name; a literal `{` in a sentence is not supported.
- `tr!` and `msg!` take a string literal only (the macro refuses anything else), so the scan test can see every key.
- The Korean table is one static slice in `forge-shared/src/ko.rs`, sorted by key (byte order), no duplicates.
- Language names are written in their own language (`English`, `한국어`) and are never translated.
- An untranslated sentence shows in English. No screen may break because a table entry is missing.
- Stage 1 translates only the account menu and the account area's tooltip. Other screens stay English.
- The stored value follows the other choice enums: the config file holds `"english"` / `"korean"` (serde `snake_case`), `value()` gives `"en"` / `"ko"`. An unreadable or unknown value reads as `None`.
- First start: Korean when the system's first preferred language has the language code `ko` (`ko`, `ko-KR`, `ko_KR.UTF-8`; not `kok-IN`), English otherwise or when unreadable.
- "Shown only" rows (Usage, Get help, Get API key, Usage policy, Privacy policy) look like normal rows and only close the menu.
- Match surrounding code style and comment density; tests next to the code in `mod tests`. Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Known unrelated failures to ignore: `forge-adapter-claude-plugins --test round_trip`, the duplicate `SessionUpdate::PromptSuggestion` arm warning in `live_session_actor.rs`, flaky plugin tests (Plan 32).

## Review Focus

1. A config with no `language`, with `"language": null`, or with `"language": "klingon"` must load, with `None`, and the rest of the config intact. Task 1.
2. System language strings: `ko`, `ko-KR`, `ko_KR.UTF-8`, `KO`, `kok-IN` (Konkani, English), `en-US`, `ja-JP`, empty, garbage. Task 1.
3. A Korean value with a placeholder the key lacks (or the other way round), a key missing from the table, a table entry nobody uses, an unsorted table. Task 2.
4. The OS user name unset, empty, only spaces, or non-ASCII (`제임스`): the area still shows a name and an avatar letter. Task 5.
5. Choosing the language that is already current, and choosing while the config file cannot be saved: nothing panics; the screen follows the choice. Task 4.

---

## File Structure

- `crates/forge-domain/src/config.rs` — `Language`, `AppConfig.language`, `Language::from_locale`.
- `crates/forge-domain/src/keys.rs` — `KEY_SHORTCUTS`.
- `crates/forge-shared/Cargo.toml`, `src/lib.rs` — depend on `forge-domain`; export `i18n`, `tr!`, `msg!`.
- `crates/forge-shared/src/i18n.rs` — `translate`, `fill`, the signal, the macros, the scan test.
- `crates/forge-shared/src/ko.rs` — the Korean table.
- `crates/forge-shared/src/theme.css`, `theme.rs` — account classes.
- `crates/forge-app/Cargo.toml`, `src/main.rs` — read the system language at start.
- `crates/forge-features/src/settings.rs` — `CodeChange::Language`, `Settings::set_language`, apply on load and save.
- `crates/forge-features/src/account_menu.rs` — the menu as data; `user_name`.
- `crates/forge-widgets/src/account_menu.rs` — the component.
- `crates/forge-widgets/src/sidebar.rs` — the footer shows the account area.
- `docs/superpowers/roadmap/backlog.md` — PG-08 row, a new row for the translation.

---

### Task 1: `Language` in the domain

**Files:**
- Modify: `crates/forge-domain/src/config.rs` (add enum after `MotionPref`, field in `AppConfig`, `Default`, tests)
- Modify: `crates/forge-domain/src/lib.rs:30` (export `Language`)
- Modify: `crates/forge-domain/src/keys.rs`, `lib.rs:34` (`KEY_SHORTCUTS`)

**Interfaces:**
- Produces: `forge_domain::Language { English, Korean }` with `Language::ALL`, `value()`, `label()`, `from_value()`, `Default = English`, and `Language::from_locale(tag: &str) -> Language`; `AppConfig.language: Option<Language>`; `forge_domain::KEY_SHORTCUTS: &str` (`⌘/` / `Ctrl+/`).

- [ ] **Step 1: Write the failing tests** in `config.rs` `mod tests`:

```rust
    #[test]
    fn a_config_without_a_language_follows_the_system() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":"https://x","api_key":null}"#).unwrap();
        assert_eq!(config.language, None);
        assert_eq!(config.base_url.as_deref(), Some("https://x"));
    }

    #[test]
    fn a_null_or_unknown_language_reads_as_none_and_the_rest_still_loads() {
        for text in [r#""language": null"#, r#""language": "klingon""#, r#""language": 7"#] {
            let config: AppConfig = serde_json::from_str(&format!(r#"{{"base_url":"https://x",{text}}}"#)).unwrap();
            assert_eq!(config.language, None, "{text}");
            assert_eq!(config.base_url.as_deref(), Some("https://x"), "{text}");
        }
    }

    #[test]
    fn a_chosen_language_is_stored_and_read_back() {
        let config = AppConfig { language: Some(Language::Korean), ..AppConfig::default() };
        let text = serde_json::to_string(&config).unwrap();
        assert!(text.contains(r#""language":"korean""#), "{text}");
        let back: AppConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(back.language, Some(Language::Korean));
    }

    #[test]
    fn the_system_language_is_korean_only_for_the_language_code_ko() {
        for tag in ["ko", "ko-KR", "ko_KR", "ko_KR.UTF-8", "KO", "ko-Kore-KR", " ko-KR "] {
            assert_eq!(Language::from_locale(tag), Language::Korean, "{tag}");
        }
        for tag in ["en-US", "en", "ja-JP", "kok-IN", "", "   ", "😀", "korean"] {
            assert_eq!(Language::from_locale(tag), Language::English, "{tag}");
        }
    }

    #[test]
    fn languages_are_named_in_their_own_language() {
        assert_eq!(Language::English.label(), "English");
        assert_eq!(Language::Korean.label(), "한국어");
        assert_eq!(Language::from_value("ko"), Some(Language::Korean));
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-domain language`
Expected: FAIL to compile (`Language` not found).

- [ ] **Step 3: Implement.** After the `MotionPref` line add:

```rust
// The language of the interface. Each language is named in its own language and never translated.
choice_enum!(Language, default English, { English => ("en", "English"), Korean => ("ko", "한국어") });

impl Language {
    /// The language for a system locale tag such as `ko-KR` or `ko_KR.UTF-8`: Korean when the
    /// language code is `ko`, English for everything else (and for text that is not a tag).
    pub fn from_locale(tag: &str) -> Language {
        let code = tag.trim().split(|c: char| matches!(c, '-' | '_' | '.' | '@')).next().unwrap_or("");
        if code.eq_ignore_ascii_case("ko") {
            Language::Korean
        } else {
            Language::English
        }
    }
}
```

In `AppConfig` after `attention_bounce`:

```rust
    /// The language the person chose; `None` follows the system language.
    #[serde(default, deserialize_with = "lenient")]
    pub language: Option<Language>,
```

In `impl Default for AppConfig` add `language: None,` (and in any full `AppConfig { .. }` literal in this file's tests that lacks `..Default::default()`; the compiler lists them). In `lib.rs:30` add `Language` to the `pub use config::{...}` list. In `keys.rs` add `pub const KEY_SHORTCUTS: &str = command_label!("/");` and export it in `lib.rs:34`.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-domain`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain
git commit -m "feat: Language in the config, and the rule for the system language

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: the translation core

**Files:**
- Modify: `crates/forge-shared/Cargo.toml` (add `forge-domain = { path = "../forge-domain" }`)
- Create: `crates/forge-shared/src/i18n.rs`
- Create: `crates/forge-shared/src/ko.rs`
- Modify: `crates/forge-shared/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::Language`.
- Produces (all in `forge_shared::i18n`):
  - `translate(language: Language, english: &'static str) -> &'static str`
  - `fill(template: &str, args: &[(&str, String)]) -> String`
  - `text(english: &'static str) -> &'static str` (translate into the current language)
  - `current() -> Language`, `set_current(Language)`
  - `set_system_language(Language)`, `system_language() -> Language`, `effective_language(chosen: Option<Language>) -> Language`
  - macros `forge_shared::tr!` and `forge_shared::msg!`

- [ ] **Step 1: Write the table and the failing tests.** Create `ko.rs`:

```rust
//! The Korean text for each English sentence, sorted by the English text (byte order).
//! Keep it in step with the `tr!` and `msg!` calls: a test in `i18n.rs` checks both ways.

pub static KO: &[(&str, &str)] = &[
    ("Account menu", "계정 메뉴"),
    ("Get API key", "API 키 받기"),
    ("Get help", "도움 받기"),
    ("Keyboard shortcuts", "단축키"),
    ("Language", "언어"),
    ("Learn more", "더 알아보기"),
    ("Privacy policy", "개인정보 처리방침"),
    ("Settings", "설정"),
    ("Usage", "사용량"),
    ("Usage policy", "이용 정책"),
];
```

Create `i18n.rs` with the tests first (the functions come in step 3):

```rust
//! Text in the person's language. The English sentence is the key; the Korean table lives in `ko.rs`.

use std::sync::OnceLock;

use dioxus::prelude::*;
use forge_domain::Language;

use crate::ko::KO;

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
    fn the_chosen_language_wins_over_the_system_one() {
        assert_eq!(effective_language(Some(Language::English)), Language::English);
        assert_eq!(effective_language(Some(Language::Korean)), Language::Korean);
        // Nothing sets the system language in this test binary, so it is English.
        assert_eq!(effective_language(None), Language::English);
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
            if file.ends_with("forge-shared/src/i18n.rs") {
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
```

Add to `lib.rs`: `pub mod i18n;` and `mod ko;`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-shared i18n`
Expected: FAIL to compile (`translate`, `fill`, `effective_language` not found).

- [ ] **Step 3: Implement** above the `tests` module in `i18n.rs`:

```rust
/// The Korean text for `english`, or `english` itself when the language is English or the table
/// has no entry for it.
pub fn translate(language: Language, english: &'static str) -> &'static str {
    match language {
        Language::English => english,
        Language::Korean => KO
            .binary_search_by(|(key, _)| (*key).cmp(english))
            .map(|index| KO[index].1)
            .unwrap_or(english),
    }
}

/// Replaces each `{name}` in `template` with its value. A placeholder with no value stays as
/// written, and the text of a value is never searched again.
pub fn fill(template: &str, args: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let value = after
            .find('}')
            .and_then(|end| args.iter().find(|(name, _)| *name == &after[1..end]).map(|(_, value)| (end, value)));
        match value {
            Some((end, value)) => {
                out.push_str(value);
                rest = &after[end + 1..];
            }
            None => {
                out.push('{');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

static SYSTEM: OnceLock<Language> = OnceLock::new();

/// Tells the module what the system language is. Called once, before the window opens.
pub fn set_system_language(language: Language) {
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

/// The language on screen. Reading it in a component redraws that component when it changes.
pub fn current() -> Language {
    *LANGUAGE.read()
}

pub fn set_current(language: Language) {
    if *LANGUAGE.peek() != language {
        *LANGUAGE.write() = language;
    }
}

/// `english` in the current language (see `translate`).
pub fn text(english: &'static str) -> &'static str {
    translate(current(), english)
}

/// The text in the current language: `tr!("Settings")`, or with values `tr!("{n} sessions", n = count)`.
/// Takes a string literal only, so a test can list every sentence.
#[macro_export]
macro_rules! tr {
    ($text:literal) => {
        $crate::i18n::text($text)
    };
    ($text:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::fill($crate::i18n::text($text), &[$((stringify!($name), ($value).to_string())),+])
    };
}

/// Marks a sentence that is translated later, where it is drawn (`i18n::text(label)`). It is the
/// sentence itself; the scan test counts it like `tr!`.
#[macro_export]
macro_rules! msg {
    ($text:literal) => {
        $text
    };
}
```

Note: `fill` compares `*name == &after[1..end]`; if the borrow checker objects, write `*name == after[1..end].to_string()`-free form `*name == &after[1..end]` → `**name == after[1..end]`. Fix to what compiles; keep the behavior the tests pin.

- [ ] **Step 4: Run to verify the unit tests pass; the two scan tests still fail**

Run: `cargo test -p forge-shared i18n`
Expected: PASS except `every_korean_entry_is_used_in_the_sources` (nothing uses the entries yet). That test goes green in Task 5. To keep each commit green, in this task mark that single test `#[ignore = "until the account menu uses the entries (Task 5)"]`, and remove the attribute in Task 5.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-shared Cargo.lock
git commit -m "feat: translation core, English sentence as the key, Korean table, scan test

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: read the system language at start

**Files:**
- Modify: `crates/forge-app/Cargo.toml` (`sys-locale = "0.3"`, `forge-domain = { path = "../forge-domain" }` if not a dependency)
- Modify: `crates/forge-app/src/main.rs` (in `main`, next to the UTC offset)

**Interfaces:**
- Consumes: `Language::from_locale` (Task 1), `forge_shared::i18n::set_system_language` (Task 2).

- [ ] **Step 1: Spike, 5 minutes.** Add `sys-locale` and in a scratch `fn main` (scratchpad, not the repo) print `sys_locale::get_locale()`. Run it from Terminal and expect `Some("ko-KR")` on this Mac (`defaults read -g AppleLanguages` prints `ko-KR`). The point is that macOS answers from the system settings, not from `LANG` (which is `en_US.UTF-8` here and unset for an app started from the Dock). If it prints `en-US`, stop and report: the crate reads the wrong source and a `defaults read -g AppleLanguages` call is the fallback.

- [ ] **Step 2: Implement.** In `main()`, after the UTC offset block:

```rust
    // The first start follows the system language; a language the person chose is read later from the config.
    if let Some(tag) = sys_locale::get_locale() {
        forge_shared::i18n::set_system_language(forge_domain::Language::from_locale(&tag));
    }
```

- [ ] **Step 3: Build**

Run: `cargo build -p forge-app`
Expected: builds. (The rule itself is tested in Task 1.)

- [ ] **Step 4: Commit**

```bash
git add crates/forge-app Cargo.lock
git commit -m "feat: the system language is read once at start

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: choosing a language

**Files:**
- Modify: `crates/forge-features/src/settings.rs` (`CodeChange`, `apply_code_change`, `Settings`, `show`, `after_save_code`; tests next to the existing `apply_code_change` tests)

**Interfaces:**
- Consumes: `forge_domain::Language`, `forge_shared::i18n::{set_current, effective_language}`.
- Produces: `Settings::set_language(&self, Language)`; `Settings::language(&self) -> Language` (the one on screen).

- [ ] **Step 1: Write the failing tests** where the other `apply_code_change` tests are:

```rust
    #[test]
    fn choosing_a_language_stores_it() {
        let mut config = AppConfig::default();
        apply_code_change(&mut config, CodeChange::Language(Language::Korean));
        assert_eq!(config.language, Some(Language::Korean));
        apply_code_change(&mut config, CodeChange::Language(Language::English));
        assert_eq!(config.language, Some(Language::English));
    }

    #[test]
    fn choosing_the_current_language_again_changes_nothing_else() {
        let mut config = AppConfig { language: Some(Language::Korean), model: Some("m".into()), ..AppConfig::default() };
        apply_code_change(&mut config, CodeChange::Language(Language::Korean));
        assert_eq!(config.language, Some(Language::Korean));
        assert_eq!(config.model.as_deref(), Some("m"));
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-features choosing_a_language`
Expected: FAIL to compile (`CodeChange::Language`).

- [ ] **Step 3: Implement.**
  - `CodeChange`: add `Language(Language)`; `apply_code_change`: `CodeChange::Language(value) => config.language = Some(value),`.
  - Import `Language` from `forge_domain`.
  - In `impl Settings`:

```rust
    /// The language on screen.
    pub fn language(&self) -> Language {
        forge_shared::i18n::current()
    }

    /// Switches the interface language at once and remembers the choice. The screen follows the
    /// choice even if the config cannot be written (the error shows as for any other setting).
    pub fn set_language(&self, value: Language) {
        forge_shared::i18n::set_current(value);
        self.change_code(CodeChange::Language(value));
    }
```

  - In `show(config)`: before `saved.set(config)` add `forge_shared::i18n::set_current(forge_shared::i18n::effective_language(config.language));`.
  - In `after_save_code`: inside the block add `current.language = config.language;`.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-features`
Expected: PASS (ignore the known unrelated failures).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: Settings chooses and remembers the interface language

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: the menu as data

**Files:**
- Create: `crates/forge-features/src/account_menu.rs`
- Modify: `crates/forge-features/src/lib.rs` (`mod account_menu;` and `pub use account_menu::{account_rows, user_initial, user_name, AccountAction, AccountRow};`)

**Interfaces:**
- Consumes: `Language`, `KEY_SETTINGS`, `KEY_SHORTCUTS`, `forge_shared::msg!`.
- Produces:

```rust
pub enum AccountAction { OpenSettings, SetLanguage(Language), ShowShortcuts, Close }   // Debug, Clone, Copy, PartialEq, Eq
pub enum AccountRow {                                                                    // Debug, Clone, PartialEq
    Heading(String),
    Divider,
    Item { label: &'static str, keys: Option<&'static str>, action: AccountAction, check: Option<bool> },
    Submenu { label: &'static str, rows: Vec<AccountRow> },
}
pub fn account_rows(user: &str, current: Language) -> Vec<AccountRow>
pub fn user_name(user: Option<&str>, username: Option<&str>) -> String
pub fn user_initial(name: &str) -> String
```

`label` and `keys` are English / literal text; the widget draws `i18n::text(label)`. `check: Some(true)` is the current language, `Some(false)` another language (it keeps the column), `None` has no check column.

- [ ] **Step 1: Write the failing tests** in `account_menu.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // One line per row: heading text, `—` for a divider, `label [keys]`, `label >` then its rows indented.
    fn lines(rows: &[AccountRow], indent: &str, out: &mut Vec<String>) {
        for row in rows {
            match row {
                AccountRow::Heading(text) => out.push(format!("{indent}# {text}")),
                AccountRow::Divider => out.push(format!("{indent}—")),
                AccountRow::Item { label, keys, check, .. } => {
                    let mark = match check { Some(true) => " ✓", _ => "" };
                    let keys = keys.map(|k| format!(" [{k}]")).unwrap_or_default();
                    out.push(format!("{indent}{label}{keys}{mark}"));
                }
                AccountRow::Submenu { label, rows } => {
                    out.push(format!("{indent}{label} >"));
                    lines(rows, &format!("{indent}  "), out);
                }
            }
        }
    }

    fn shown(current: Language) -> Vec<String> {
        let mut out = Vec::new();
        lines(&account_rows("james", current), "", &mut out);
        out
    }

    #[test]
    fn the_menu_has_the_rows_in_order() {
        assert_eq!(
            shown(Language::English),
            [
                "# james".to_string(),
                "—".into(),
                format!("Settings [{}]", forge_domain::KEY_SETTINGS),
                "Usage".into(),
                "Language >".into(),
                "  한국어".into(),
                "  English ✓".into(),
                "Get help".into(),
                "—".into(),
                "Learn more >".into(),
                "  Usage policy".into(),
                "  Privacy policy".into(),
                "  —".into(),
                format!("  Keyboard shortcuts [{}]", forge_domain::KEY_SHORTCUTS),
                "—".into(),
                "Get API key".into(),
            ]
        );
    }

    #[test]
    fn the_check_follows_the_current_language() {
        assert!(shown(Language::Korean).contains(&"  한국어 ✓".to_string()));
        assert!(!shown(Language::Korean).contains(&"  English ✓".to_string()));
    }

    fn find(rows: &[AccountRow], wanted: &str) -> AccountAction {
        for row in rows {
            match row {
                AccountRow::Item { label, action, .. } if *label == wanted => return *action,
                AccountRow::Submenu { rows, .. } => {
                    if let found @ (AccountAction::OpenSettings | AccountAction::SetLanguage(_) | AccountAction::ShowShortcuts) = find(rows, wanted) {
                        return found;
                    }
                }
                _ => {}
            }
        }
        AccountAction::Close
    }

    #[test]
    fn the_rows_do_what_the_spec_says() {
        let rows = account_rows("james", Language::English);
        assert_eq!(find(&rows, "Settings"), AccountAction::OpenSettings);
        assert_eq!(find(&rows, "Keyboard shortcuts"), AccountAction::ShowShortcuts);
        assert_eq!(find(&rows, "한국어"), AccountAction::SetLanguage(Language::Korean));
        assert_eq!(find(&rows, "English"), AccountAction::SetLanguage(Language::English));
        for shown_only in ["Usage", "Get help", "Get API key", "Usage policy", "Privacy policy"] {
            assert_eq!(find(&rows, shown_only), AccountAction::Close, "{shown_only}");
        }
    }

    #[test]
    fn the_user_name_comes_from_the_environment_with_a_fallback() {
        assert_eq!(user_name(Some("james"), Some("other")), "james");
        assert_eq!(user_name(None, Some("james")), "james");
        assert_eq!(user_name(Some(""), Some("  ")), "Forge");
        assert_eq!(user_name(None, None), "Forge");
        assert_eq!(user_name(Some("  제임스 "), None), "제임스");
    }

    #[test]
    fn the_avatar_letter_is_the_first_character_in_capitals() {
        assert_eq!(user_initial("james"), "J");
        assert_eq!(user_initial("제임스"), "제");
        assert_eq!(user_initial("ñandú"), "Ñ");
        assert_eq!(user_initial(""), "F");
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-features account_menu`
Expected: FAIL to compile.

- [ ] **Step 3: Implement** above the tests:

```rust
//! The account menu at the bottom of the sidebar, as data the widget draws.

use forge_domain::{Language, KEY_SETTINGS, KEY_SHORTCUTS};
use forge_shared::msg;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountAction {
    OpenSettings,
    SetLanguage(Language),
    ShowShortcuts,
    /// A row that is shown but does nothing yet: it only closes the menu.
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccountRow {
    Heading(String),
    Divider,
    /// `label` and `keys` are text for `i18n::text`; `check` is `Some` when the item has a check column.
    Item { label: &'static str, keys: Option<&'static str>, action: AccountAction, check: Option<bool> },
    Submenu { label: &'static str, rows: Vec<AccountRow> },
}

fn item(label: &'static str, action: AccountAction) -> AccountRow {
    AccountRow::Item { label, keys: None, action, check: None }
}

fn language_item(language: Language, current: Language) -> AccountRow {
    // Each language is named in its own language; the label is never translated.
    AccountRow::Item { label: language.label(), keys: None, action: AccountAction::SetLanguage(language), check: Some(language == current) }
}

/// The menu for `user`, with the check on `current`.
pub fn account_rows(user: &str, current: Language) -> Vec<AccountRow> {
    vec![
        AccountRow::Heading(user.to_string()),
        AccountRow::Divider,
        AccountRow::Item { label: msg!("Settings"), keys: Some(KEY_SETTINGS), action: AccountAction::OpenSettings, check: None },
        item(msg!("Usage"), AccountAction::Close),
        AccountRow::Submenu {
            label: msg!("Language"),
            rows: vec![language_item(Language::Korean, current), language_item(Language::English, current)],
        },
        item(msg!("Get help"), AccountAction::Close),
        AccountRow::Divider,
        AccountRow::Submenu {
            label: msg!("Learn more"),
            rows: vec![
                item(msg!("Usage policy"), AccountAction::Close),
                item(msg!("Privacy policy"), AccountAction::Close),
                AccountRow::Divider,
                AccountRow::Item { label: msg!("Keyboard shortcuts"), keys: Some(KEY_SHORTCUTS), action: AccountAction::ShowShortcuts, check: None },
            ],
        },
        AccountRow::Divider,
        item(msg!("Get API key"), AccountAction::Close),
    ]
}

/// The name shown in the account area: the OS user (`USER`, on Windows `USERNAME`), else "Forge".
pub fn user_name(user: Option<&str>, username: Option<&str>) -> String {
    [user, username]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|name| !name.is_empty())
        .unwrap_or("Forge")
        .to_string()
}

/// The letter in the avatar circle.
pub fn user_initial(name: &str) -> String {
    name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "F".to_string())
}
```

Do not touch `i18n.rs` here: the `#[ignore]` from Task 2 is removed in Task 6, once `tr!("Account menu")` exists.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-features account_menu`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: the account menu as data, and the user name

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: the widget and the sidebar footer

**Files:**
- Create: `crates/forge-widgets/src/account_menu.rs`
- Modify: `crates/forge-widgets/src/lib.rs` (`mod account_menu;`, `pub use account_menu::AccountMenu;` if the other widgets are exported so; `Sidebar` uses it inside the crate, so `mod account_menu;` is enough)
- Modify: `crates/forge-widgets/src/sidebar.rs:201-207` (footer) and the unused `settings` / `use_settings` import
- Modify: `crates/forge-shared/src/theme.css` (next to `.sidebar-footer-button`), `theme.rs` test list
- Modify: `crates/forge-shared/src/i18n.rs` (remove the Task 2 `#[ignore]`)

**Interfaces:**
- Consumes: Task 5's `account_rows`, `user_name`, `user_initial`, `AccountAction`, `AccountRow`; `Settings::{open, set_language}`; `Workspace::open_shortcuts`; `forge_shared::{tr, i18n}`.

- [ ] **Step 1: Write the failing test** in `theme.rs` (add to the class list in `the_stylesheet_defines_the_layout_classes_the_components_use`): `".account"`, `".account-button"`, `".account-menu"`, `".picker-check"`.

Run: `cargo test -p forge-shared the_stylesheet_defines`
Expected: FAIL (classes missing).

- [ ] **Step 2: CSS.** After the `.sidebar-footer-button .keys` line in `theme.css`:

```css
.account { position: relative; }
.account-avatar { flex: none; display: flex; align-items: center; justify-content: center; width: 22px; height: 22px; border-radius: 50%; background: var(--bg-hover); font-size: 12px; font-weight: 600; }
.account-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.account-chevron { flex: none; color: var(--fg-muted); font-size: 11px; }
.account-menu { min-width: 240px; }
.account-heading { padding: 6px 8px; color: var(--fg-muted); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.picker-check { flex: none; width: 14px; }
```

The test needs `.account-button` to exist, so add `.account-button { cursor: pointer; }` as well.

- [ ] **Step 3: The component** `account_menu.rs`:

```rust
use dioxus::prelude::*;
use forge_features::{account_rows, use_settings, use_workspace, user_initial, user_name, AccountAction, AccountRow};
use forge_shared::{i18n, tr};

#[component]
fn Row(row: AccountRow, on_select: EventHandler<AccountAction>) -> Element {
    match row {
        AccountRow::Divider => rsx! { div { class: "picker-divider" } },
        AccountRow::Heading(text) => rsx! { div { class: "account-heading", "{text}" } },
        AccountRow::Item { label, keys, action, check } => rsx! {
            button {
                class: "picker-item",
                r#type: "button",
                role: "menuitem",
                onclick: move |_| on_select.call(action),
                if let Some(checked) = check {
                    span { class: "picker-check", if checked { "✓" } }
                }
                span { class: "picker-text", "{i18n::text(label)}" }
                if let Some(keys) = keys {
                    span { class: "picker-hint", "{keys}" }
                }
            }
        },
        AccountRow::Submenu { .. } => rsx! {},
    }
}

/// The account area at the bottom of the sidebar: the OS user's name, and the menu it opens.
#[component]
pub fn AccountMenu() -> Element {
    let settings = use_settings();
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let mut open_sub = use_signal(|| None::<usize>);
    // Read here so the whole menu is drawn again when the language changes.
    let language = i18n::current();
    let name = user_name(std::env::var("USER").ok().as_deref(), std::env::var("USERNAME").ok().as_deref());
    let initial = user_initial(&name);
    let rows = account_rows(&name, language);

    let on_select = move |action: AccountAction| {
        open.set(false);
        open_sub.set(None);
        match action {
            AccountAction::OpenSettings => settings.open(),
            AccountAction::SetLanguage(language) => settings.set_language(language),
            AccountAction::ShowShortcuts => workspace.open_shortcuts(),
            AccountAction::Close => {}
        }
    };

    rsx! {
        div { class: "account",
            button {
                class: "sidebar-footer-button account-button",
                r#type: "button",
                title: "{tr!(\"Account menu\")}",
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onclick: move |_| {
                    open_sub.set(None);
                    open.toggle();
                },
                span { class: "account-avatar", "{initial}" }
                span { class: "account-name", "{name}" }
                span { class: "account-chevron", "⌃" }
            }
            if open() {
                div {
                    class: "picker-backdrop",
                    onclick: move |_| {
                        open.set(false);
                        open_sub.set(None);
                    },
                }
                div {
                    class: "picker-menu account-menu",
                    role: "menu",
                    tabindex: "0",
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            open.set(false);
                            open_sub.set(None);
                        }
                    },
                    for (index, row) in rows.into_iter().enumerate() {
                        match row {
                            AccountRow::Submenu { label, rows: sub_rows } => rsx! {
                                div {
                                    key: "{index}",
                                    class: "context-sub",
                                    onmouseenter: move |_| open_sub.set(Some(index)),
                                    button {
                                        class: "picker-item",
                                        r#type: "button",
                                        role: "menuitem",
                                        "aria-haspopup": "menu",
                                        onclick: move |_| open_sub.set(Some(index)),
                                        span { class: "picker-text", "{i18n::text(label)}" }
                                        span { class: "picker-hint", "›" }
                                    }
                                    if open_sub() == Some(index) {
                                        div { class: "picker-menu context-submenu", role: "menu",
                                            for (sub_index, sub) in sub_rows.into_iter().enumerate() {
                                                Row { key: "{sub_index}", row: sub, on_select }
                                            }
                                        }
                                    }
                                }
                            },
                            other => rsx! {
                                div { key: "{index}", onmouseenter: move |_| open_sub.set(None),
                                    Row { row: other, on_select }
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}
```

Escaped quotes inside an rsx attribute string may not compile; if so, compute `let account_title = tr!("Account menu");` before `rsx!` and use `title: "{account_title}"`. The scan test must still see `tr!("Account menu")`.

- [ ] **Step 4: The sidebar.** In `sidebar.rs` replace the footer `div` with:

```rust
            div { class: "sidebar-footer",
                AccountMenu {}
            }
```

Add `use crate::account_menu::AccountMenu;`, remove `let settings = use_settings();` and `use_settings` from the `forge_features` import if nothing else in the file uses them.

- [ ] **Step 5: Remove the `#[ignore]`** from `every_korean_entry_is_used_in_the_sources` in `i18n.rs`.

- [ ] **Step 6: Run the whole suite**

Run: `cargo test --workspace`
Expected: PASS (ignore the known unrelated failures). Run `cargo build -p forge-app` too; there must be no new warnings.

- [ ] **Step 7: Commit**

```bash
git add crates
git commit -m "feat: the account menu at the bottom of the sidebar

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: check in the real window, then the notes

Follow the memory note "Driving the Forge app visually" (screencapture + Quartz clicks + `FORGE_CLAUDE_HOME` fixture) and use a throwaway config (`FORGE_CONFIG` or the harness's own fixture) so the person's real config is never written. Capture before each click.

- [ ] **Step 1: Run the checks of the spec** and record each as seen / not seen:
  1. The account area shows the OS user and an avatar letter; the menu opens above it with the rows of Task 5.
  2. Click outside and Escape close it; Escape does not close anything else (the session behind stays).
  3. Language submenu: opens on hover and on click; choosing 한국어 turns the menu Korean at once (no restart), and the check moves.
  4. Quit and start again: the choice is kept. Delete the field from the throwaway config and start with the system language set to Korean, then English (System Settings, or `-AppleLanguages '(ko)'` on the command line).
  5. Settings and Keyboard shortcuts open from the menu; ⌘, and ⌘/ still work; the menu is closed after.
  6. Usage, Get help, Get API key, Usage policy, Privacy policy close the menu and do nothing else.
  7. A narrow window (700 pt and below): the sidebar overlay shows the account area and the menu fits.
  8. Dark theme: the menu and the avatar read well.
  9. A long user name is cut with an ellipsis, not wrapped.

- [ ] **Step 2: Fix what the checks show,** each fix with a test where the logic allows, and its own commit.

- [ ] **Step 3: Notes.** In `docs/superpowers/roadmap/backlog.md`: PG-08 row — Forge `partial`, Plan `Plan 36 (built: account area, Settings, Language, Learn more > Keyboard shortcuts; shown only: Usage, Get help, Get API key, Usage policy, Privacy policy; not built: the other rows and the icon beside the name)`. Add a row `OTH-08 | Translate the whole app into Korean and English | Plan 36 | Plan 36 stage 1 (built: structure, account menu); stages 2 to 6 in the spec "Stages of the translation"`. In the spec, change the line that says the serialised value is `"en"` / `"ko"` to: the config file holds `"english"` / `"korean"`, and `value()` is `"en"` / `"ko"`. Add the memory note `plan-36-account-menu-language.md` and a line in `MEMORY.md`.

- [ ] **Step 4: Commit**

```bash
git add docs
git commit -m "docs: Plan 36 stage 1 done, notes and roadmap

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Self-review

**Spec coverage.** Translation structure, `Language` and the config field, system language: Tasks 1 to 3. Saving and the live switch: Task 4. Menu rows, shown-only rows, submenus, shortcuts, the user name: Tasks 5 and 6. Table guard test: Task 2. Real-window list of the spec: Task 7 step 1 (all nine lines of the spec's "Real window" paragraph, plus dark theme and a long name). "Not in this stage" is respected: no other screen gets `tr!`.

**Placeholders.** None: the only open item is the spike in Task 3 step 1, which has a stated stop condition. Two spots say "if it does not compile, write it this way" (the `fill` borrow, the escaped quotes in rsx) and give the alternative.

**Types.** `Language`, `AccountAction`, `AccountRow::Item { label, keys, action, check }`, `account_rows(&str, Language)`, `user_name(Option<&str>, Option<&str>)`, `user_initial(&str)`, `Settings::set_language(Language)`, `i18n::{text, current, set_current, set_system_language, effective_language}` are used with the same names and shapes in every task.

**Order of the ignored test.** Task 2 ignores `every_korean_entry_is_used_in_the_sources`; Task 6 un-ignores it once both `msg!` (Task 5) and `tr!("Account menu")` (Task 6) exist, so every commit between is green.
