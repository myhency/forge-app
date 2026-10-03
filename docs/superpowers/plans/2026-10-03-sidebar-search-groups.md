# Sidebar search, filter menu and collapse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Forge's sidebar the desktop app's search palette (titles and message text), a sort / group / empty-group filter menu, collapsible groups, and a hideable, resizable sidebar (SES-07, SES-08, SES-11).

**Architecture:** Everything that decides something is a pure function with tests: `arrange` (sort, group, collapse), `FilterMenu` (menu keys), `merge_search` (palette rows), `find_snippet` (match and quote). The Claude fs adapter gets one new port method, `search_sessions`, that scans transcripts. `Workspace` (a signal bag) holds the saved `SidebarPrefs` and the palette state; widgets only draw. Preferences are saved in `AppConfig.sidebar`.

**Tech Stack:** Rust 2021, Dioxus 0.7.10 desktop, tokio, serde, `time` 0.3, mockall. Run tests per crate (`cargo test -p <crate>`), the whole workspace at the end (`cargo test --workspace`).

**Spec:** `docs/superpowers/specs/2026-10-03-sidebar-search-groups-design.md` (read it first; the as-built notes at its end are written in Task 11).

## Global Constraints

- UI text is English (the real app's Korean is evidence, not a copy target): "Sort by", "Group by", "Show empty groups", "Name", "Created", "Activity", "Folder", "Date", "None", "Search sessions", "No sessions match".
- Menu keys are matched by physical key (`event.code()`, e.g. `"ArrowDown"`), never by the typed character, so a Korean input method does not break them (Plan 15).
- Enter that ends an IME composition must not act (`event.is_composing()`), as in `session_row.rs`.
- A modifier (Ctrl, Meta, Alt) turns a menu key into something else; leave it alone.
- Saved settings must stay readable: every new `AppConfig` field is `#[serde(default)]`, and an old `config.json` loads unchanged.
- Search matches the title and the user and assistant text of the transcript. It never matches tool input, tool results or thinking.
- The sidebar width is 200 to 480 px; the default is 264 (today's CSS value).
- New dependencies: only `time` (feature `parsing`) and `tokio` (feature `time`) in `forge-features`, and `time` feature `parsing` in `forge-adapter-claude-fs`.
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

Differences from the spec, decided while planning (state them in the as-built notes, Task 11):

- The filter menu has no number keys: the real menu shows none.
- A session row in Date or None grouping shows `project · 3d ago` as its subtitle (the spec said "the project name"; the time stays).
- A session that is open but not yet on disk is searched by its title only (its title already is its first message).
- The palette runs the query while an input method is composing (the 200 ms debounce and the cancel make this harmless); the spec said it would not.
- Date buckets are cut on UTC calendar days, because the `time` crate has no local offset without an unsound feature.

## Review Focus

Inputs and conditions the spec implies but no task's main tests exercise; each has its own test in the task named.

1. A query that has JSON-special characters (`"`, `\`) still finds text, because the scan's raw-line prefilter must not drop it (Task 6).
2. A transcript with an invalid UTF-8 line or a half-written last line is skipped, never fails the whole search (Task 6).
3. A `config.json` with an unknown `sort_by` or `group_by` value, or a width of 0 or 9999, must not make the config unreadable or the sidebar unusable (Task 1).
4. Korean and other multibyte text: matching, snippets and bold parts never cut inside a character (Task 1).
5. Two sessions with the same title, and a new session that is not on disk yet (no time at all), in every sort and grouping: both appear, once, in a sensible place (Task 3).

---

### Task 1: Saved sidebar preferences and search types in the domain

**Files:**
- Modify: `crates/forge-domain/src/config.rs`
- Create: `crates/forge-domain/src/search.rs`
- Modify: `crates/forge-domain/src/lib.rs`
- Modify (compile fixes only): every `AppConfig { ... }` literal that now misses a field, found by the compiler (known: `crates/forge-features/src/settings_draft.rs`, `crates/forge-adapter-config/src/lib.rs`, `crates/forge-adapter-sidecar/tests/factory_test.rs`, `crates/forge-adapter-sidecar/src/factory.rs`, `crates/forge-domain/src/extension.rs`, `crates/forge-application/src/use_cases/extensions.rs`, `crates/forge-application/src/use_cases/settings.rs`)

**Interfaces:**
- Produces (`forge_domain`): `SortBy { Name, Created, Activity }`, `GroupBy { Folder, Date, Flat }`, `SidebarPrefs { sort_by, group_by, show_empty, collapsed: Vec<String>, width: u32, hidden: bool }` with `clamped_width() -> u32`, `is_collapsed(&str) -> bool`, `toggle_collapsed(&mut self, &str)`, constants `DEFAULT_SIDEBAR_WIDTH` (264), `MIN_SIDEBAR_WIDTH` (200), `MAX_SIDEBAR_WIDTH` (480); `AppConfig.sidebar: SidebarPrefs`; `Snippet { before, matched, after }`, `SearchHit { project: PathBuf, session: SessionSummary, snippet: Option<Snippet> }`, `find_snippet(&str, &str) -> Option<Snippet>`, `split_at_match(&str, &str) -> Option<(String, String, String)>`, `title_matches(&str, &str) -> bool`.

`SearchHit` needs `SessionSummary` to have `created_at`; Task 2 adds that field. In this task `SearchHit` already uses `SessionSummary` as it is today.

- [ ] **Step 1: Write the failing tests for the preferences**

Add to the `tests` module at the end of `crates/forge-domain/src/config.rs` (keep the existing tests; add `use super::*;` is already there):

```rust
    #[test]
    fn an_old_config_without_sidebar_preferences_loads_with_the_defaults() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();

        assert_eq!(config.sidebar, SidebarPrefs::default());
        assert_eq!(config.sidebar.sort_by, SortBy::Activity);
        assert_eq!(config.sidebar.group_by, GroupBy::Folder);
        assert!(!config.sidebar.show_empty);
        assert!(!config.sidebar.hidden);
        assert_eq!(config.sidebar.width, DEFAULT_SIDEBAR_WIDTH);
    }

    #[test]
    fn sidebar_preferences_survive_a_round_trip() {
        let prefs = SidebarPrefs {
            sort_by: SortBy::Name,
            group_by: GroupBy::Date,
            show_empty: true,
            collapsed: vec!["folder:/work/a".to_string()],
            width: 300,
            hidden: true,
        };
        let config = AppConfig { sidebar: prefs.clone(), ..AppConfig::default() };

        let json = serde_json::to_string(&config).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(back.sidebar, prefs);
    }

    #[test]
    fn an_unknown_sort_or_group_value_falls_back_instead_of_breaking_the_config() {
        let config: AppConfig =
            serde_json::from_str(r#"{"sidebar":{"sort_by":"relevance","group_by":"project"}}"#).unwrap();

        assert_eq!(config.sidebar.sort_by, SortBy::Activity);
        assert_eq!(config.sidebar.group_by, GroupBy::Folder);
    }

    #[test]
    fn a_stored_width_outside_the_range_is_clamped_for_use() {
        let narrow = SidebarPrefs { width: 0, ..SidebarPrefs::default() };
        let wide = SidebarPrefs { width: 9999, ..SidebarPrefs::default() };

        assert_eq!(narrow.clamped_width(), MIN_SIDEBAR_WIDTH);
        assert_eq!(wide.clamped_width(), MAX_SIDEBAR_WIDTH);
        assert_eq!(SidebarPrefs::default().clamped_width(), DEFAULT_SIDEBAR_WIDTH);
    }

    #[test]
    fn toggling_a_group_collapses_and_expands_it() {
        let mut prefs = SidebarPrefs::default();

        prefs.toggle_collapsed("folder:/a");
        assert!(prefs.is_collapsed("folder:/a"));
        assert!(!prefs.is_collapsed("folder:/b"));

        prefs.toggle_collapsed("folder:/a");
        assert!(!prefs.is_collapsed("folder:/a"));
        assert!(prefs.collapsed.is_empty());
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-domain`
Expected: compile error, `cannot find type SidebarPrefs`.

- [ ] **Step 3: Implement the preferences**

In `crates/forge-domain/src/config.rs`, add above `AppConfig` (the file already imports `serde::{Deserialize, Serialize}`; keep that import):

```rust
pub const DEFAULT_SIDEBAR_WIDTH: u32 = 264;
pub const MIN_SIDEBAR_WIDTH: u32 = 200;
pub const MAX_SIDEBAR_WIDTH: u32 = 480;

/// What the sidebar orders a group's sessions by. An unknown stored value reads as `Activity`, so a
/// config written by another version still loads (`other` must sit on the last variant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    Name,
    Created,
    #[default]
    #[serde(other)]
    Activity,
}

/// How the sidebar groups sessions. An unknown stored value reads as `Folder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    Date,
    Flat,
    #[default]
    #[serde(other)]
    Folder,
}

/// How the sidebar looks, kept in the config so it is the same after a restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SidebarPrefs {
    pub sort_by: SortBy,
    pub group_by: GroupBy,
    /// Whether a project with no sessions still gets a heading.
    pub show_empty: bool,
    /// Ids of the collapsed groups (`arrange` makes them: `folder:<path>`, `date:<bucket>`).
    pub collapsed: Vec<String>,
    pub width: u32,
    pub hidden: bool,
}

impl Default for SidebarPrefs {
    fn default() -> Self {
        Self {
            sort_by: SortBy::default(),
            group_by: GroupBy::default(),
            show_empty: false,
            collapsed: Vec::new(),
            width: DEFAULT_SIDEBAR_WIDTH,
            hidden: false,
        }
    }
}

impl SidebarPrefs {
    /// The width to draw: a hand-edited or corrupt value never makes the sidebar unusable.
    pub fn clamped_width(&self) -> u32 {
        self.width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH)
    }

    pub fn is_collapsed(&self, id: &str) -> bool {
        self.collapsed.iter().any(|collapsed| collapsed == id)
    }

    pub fn toggle_collapsed(&mut self, id: &str) {
        match self.collapsed.iter().position(|collapsed| collapsed == id) {
            Some(index) => {
                self.collapsed.remove(index);
            }
            None => self.collapsed.push(id.to_string()),
        }
    }
}
```

Add the field to `AppConfig`, after `enabled_mcp_servers`:

```rust
    #[serde(default)]
    pub sidebar: SidebarPrefs,
```

- [ ] **Step 4: Write the failing tests for search matching**

Create `crates/forge-domain/src/search.rs` with only the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_match_is_found_whatever_the_case() {
        let snippet = find_snippet("Please Explain the Borrow checker", "borrow").unwrap();

        assert_eq!(snippet.matched, "Borrow");
        assert_eq!(snippet.before, "Please Explain the ");
        assert_eq!(snippet.after, " checker");
    }

    #[test]
    fn korean_text_is_found_and_never_cut_inside_a_character() {
        let text = "안녕하세요 로컬 LLM 모델을 추천해 주세요";

        let snippet = find_snippet(text, "로컬").unwrap();

        assert_eq!(snippet.before, "안녕하세요 ");
        assert_eq!(snippet.matched, "로컬");
        assert_eq!(snippet.after, " LLM 모델을 추천해 주세요");
    }

    #[test]
    fn a_long_text_is_cut_to_a_window_with_ellipses() {
        let text = format!("{}needle{}", "a".repeat(100), "b".repeat(100));

        let snippet = find_snippet(&text, "needle").unwrap();

        assert_eq!(snippet.before, format!("…{}", "a".repeat(40)));
        assert_eq!(snippet.after, format!("{}…", "b".repeat(40)));
    }

    #[test]
    fn a_match_at_either_end_has_no_ellipsis_on_that_side() {
        let start = find_snippet("needle then more", "needle").unwrap();
        let end = find_snippet("more then needle", "needle").unwrap();

        assert_eq!(start.before, "");
        assert_eq!(end.after, "");
    }

    #[test]
    fn multibyte_characters_at_the_window_edge_are_not_split() {
        let text = format!("{}x{}", "😀".repeat(60), "😀".repeat(60));

        let snippet = find_snippet(&text, "x").unwrap();

        assert_eq!(snippet.before.chars().filter(|c| *c == '😀').count(), 40);
        assert!(snippet.before.starts_with('…'));
    }

    #[test]
    fn newlines_in_a_snippet_become_spaces() {
        let snippet = find_snippet("line one\nneedle\nline three", "needle").unwrap();

        assert_eq!(snippet.before, "line one ");
        assert_eq!(snippet.after, " line three");
    }

    #[test]
    fn no_match_and_an_empty_or_blank_query_give_nothing() {
        assert!(find_snippet("hello", "xyz").is_none());
        assert!(find_snippet("hello", "").is_none());
        assert!(find_snippet("hello", "   ").is_none());
        assert!(!title_matches("hello", ""));
    }

    #[test]
    fn titles_match_in_any_case_and_split_around_the_match() {
        assert!(title_matches("Debug and identify error cause", "ERROR"));
        assert!(!title_matches("Debug", "error"));
        assert_eq!(
            split_at_match("Debug and identify error cause", "error"),
            Some(("Debug and identify ".to_string(), "error".to_string(), " cause".to_string()))
        );
    }
}
```

- [ ] **Step 5: Implement the matching**

Put this at the top of `crates/forge-domain/src/search.rs`, above the tests:

```rust
use std::path::PathBuf;

use crate::SessionSummary;

/// How many characters of the text are kept on each side of a match.
const SNIPPET_CONTEXT: usize = 40;

/// A quote of the text around the first match: the match is shown in bold, the rest plain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub before: String,
    pub matched: String,
    pub after: String,
}

/// One session the search found. `snippet` is `None` when only the title matched.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub project: PathBuf,
    pub session: SessionSummary,
    pub snippet: Option<Snippet>,
}

// One character to one character, so positions in the folded text are positions in the original.
fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

// The first case-insensitive occurrence of the trimmed query, as a range of character positions.
fn locate(chars: &[char], query: &str) -> Option<(usize, usize)> {
    let needle: Vec<char> = query.trim().chars().map(fold).collect();
    if needle.is_empty() {
        return None;
    }
    let folded: Vec<char> = chars.iter().copied().map(fold).collect();
    let start = folded.windows(needle.len()).position(|window| window == needle.as_slice())?;
    Some((start, start + needle.len()))
}

fn flatten(chars: &[char]) -> String {
    chars.iter().map(|c| if matches!(c, '\n' | '\r') { ' ' } else { *c }).collect()
}

/// The text before, at and after the first match of `query`, for drawing the match in bold.
pub fn split_at_match(text: &str, query: &str) -> Option<(String, String, String)> {
    let chars: Vec<char> = text.chars().collect();
    let (start, end) = locate(&chars, query)?;
    Some((flatten(&chars[..start]), flatten(&chars[start..end]), flatten(&chars[end..])))
}

/// The first match of `query` in `text` with up to 40 characters around it; a cut side gets `…`.
pub fn find_snippet(text: &str, query: &str) -> Option<Snippet> {
    let chars: Vec<char> = text.chars().collect();
    let (start, end) = locate(&chars, query)?;
    let from = start.saturating_sub(SNIPPET_CONTEXT);
    let to = (end + SNIPPET_CONTEXT).min(chars.len());
    let mut before = flatten(&chars[from..start]);
    if from > 0 {
        before.insert(0, '…');
    }
    let mut after = flatten(&chars[end..to]);
    if to < chars.len() {
        after.push('…');
    }
    Some(Snippet { before, matched: flatten(&chars[start..end]), after })
}

pub fn title_matches(title: &str, query: &str) -> bool {
    let chars: Vec<char> = title.chars().collect();
    locate(&chars, query).is_some()
}
```

In `crates/forge-domain/src/lib.rs` add `mod search;` (keep the list alphabetical, after `project`) and change the `config` export line and add the search export:

```rust
pub use config::{
    normalize_setting, AppConfig, GroupBy, SidebarPrefs, SortBy, DEFAULT_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH,
    MIN_SIDEBAR_WIDTH,
};
pub use search::{find_snippet, split_at_match, title_matches, SearchHit, Snippet};
```

- [ ] **Step 6: Fix the `AppConfig` literals elsewhere**

Run: `cargo build --workspace --tests 2>&1 | grep -B2 -A6 "missing field"`

For each literal the compiler names, add `..AppConfig::default()` as the last line of the literal, or `sidebar: SidebarPrefs::default(),` where a `..` is not wanted (import `forge_domain::SidebarPrefs`). Do not change what a test asserts.

- [ ] **Step 7: Run all tests**

Run: `cargo test -p forge-domain && cargo build --workspace --tests`
Expected: all domain tests pass (including the 13 new ones); the workspace builds.

- [ ] **Step 8: Commit**

```bash
git add -A crates
git commit -m "feat: saved sidebar preferences and the matching rules of search" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: When a session began (`created_at`)

**Files:**
- Modify: `crates/forge-domain/src/session.rs` (struct `SessionSummary`)
- Modify: `crates/forge-adapter-claude-fs/src/sessions.rs` (list, a new helper, tests)
- Modify: `crates/forge-adapter-claude-fs/Cargo.toml` (`time` gets `parsing`)
- Modify: `crates/forge-features/src/sidebar.rs` (`SidebarEntry.created_at`, `merge_sidebar`, tests)

**Interfaces:**
- Consumes: nothing new.
- Produces: `SessionSummary.created_at: String` (RFC 3339), `SidebarEntry.created_at: Option<String>` (`None` for an open session that is not on disk yet), `pub(crate) async fn created_at_of(path: &Path, updated_at: &str) -> String` in the adapter (used again in Task 6).

- [ ] **Step 1: Write the failing adapter tests**

Add to the `tests` module of `crates/forge-adapter-claude-fs/src/sessions.rs`:

```rust
    #[tokio::test]
    async fn list_sessions_reports_when_a_session_began_from_its_first_timestamp() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "stamped",
            &[
                json!({"type": "summary"}),
                json!({"type": "user", "timestamp": "2026-01-02T03:04:05.000Z", "message": {"role": "user", "content": "hi"}}),
                json!({"type": "user", "timestamp": "2026-02-01T00:00:00.000Z", "message": {"role": "user", "content": "later"}}),
            ],
        );
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        assert_eq!(sessions[0].created_at, "2026-01-02T03:04:05.000Z");
    }

    #[tokio::test]
    async fn a_transcript_with_no_timestamp_falls_back_to_a_file_time() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "bare", &[user_text("hi")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        let created = &sessions[0].created_at;
        assert!(created.contains('T') && created.ends_with('Z'), "not RFC 3339 UTC: {created}");
    }

    #[tokio::test]
    async fn a_timestamp_that_is_not_a_time_is_ignored() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "odd",
            &[json!({"type": "user", "timestamp": "yesterday", "message": {"role": "user", "content": "hi"}})],
        );
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        assert_ne!(sessions[0].created_at, "yesterday");
    }
```

- [ ] **Step 2: Add the field and watch the build fail**

In `crates/forge-domain/src/session.rs` change `SessionSummary` to:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    pub id: SessionId,
    pub title: Option<String>,
    pub updated_at: String,
    /// RFC 3339 time the session began: its first transcript timestamp, else the file's creation
    /// time, else `updated_at`.
    pub created_at: String,
}
```

In `crates/forge-adapter-claude-fs/Cargo.toml` change the time line to `time = { version = "0.3.55", features = ["formatting", "parsing"] }`.

Run: `cargo test -p forge-adapter-claude-fs`
Expected: compile errors, `missing field created_at` in `sessions.rs`.

- [ ] **Step 3: Implement it in the adapter**

In `crates/forge-adapter-claude-fs/src/sessions.rs`, add near `read_title`:

```rust
// How many lines are read to find the first timestamp. A transcript starts with its first message
// within a few lines; the bound only stops a pathological file from being read in full.
const MAX_TIMESTAMP_SCAN_LINES: usize = 20;

async fn first_timestamp(path: &Path) -> Option<String> {
    let file = tokio::fs::File::open(path).await.ok()?;
    let mut lines = BufReader::new(file).split(b'\n');
    for _ in 0..MAX_TIMESTAMP_SCAN_LINES {
        let Ok(Some(line)) = lines.next_segment().await else {
            return None;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        if let Some(stamp) = value
            .get("timestamp")
            .and_then(Value::as_str)
            .filter(|stamp| time::OffsetDateTime::parse(stamp, &Rfc3339).is_ok())
        {
            return Some(stamp.to_string());
        }
    }
    None
}

/// When the session began: the first timestamp the transcript carries, else the file's creation
/// time (not every file system keeps one), else `updated_at`.
pub(crate) async fn created_at_of(path: &Path, updated_at: &str) -> String {
    if let Some(stamp) = first_timestamp(path).await {
        return stamp;
    }
    tokio::fs::metadata(path)
        .await
        .ok()
        .and_then(|metadata| metadata.created().ok())
        .and_then(|created| time::OffsetDateTime::from(created).format(&Rfc3339).ok())
        .unwrap_or_else(|| updated_at.to_string())
}
```

In `list_sessions`, after `let title = read_title(&path).await;` add `let created_at = created_at_of(&path, &updated_at).await;` and put `created_at,` in the `SessionSummary { ... }` literal (after `updated_at,`).

- [ ] **Step 4: Write the failing features tests**

In `crates/forge-features/src/sidebar.rs` tests: update the helper `summary` to set `created_at: format!("2026-09-28T00:00:0{}Z", id.len() % 10),` after `updated_at`, update the helper `entry` to add `created_at: None,`, and add a test:

```rust
    #[test]
    fn an_entry_keeps_when_its_session_began_and_a_new_session_has_no_time() {
        let past = vec![(project("/a"), vec![summary("s1", Some("One"))])];
        let live = vec![open(1, "/a", None, SessionMarker::Idle)];

        let groups = merge_sidebar(&past, &live);

        let on_disk = groups[0].entries.iter().find(|e| e.title == "One").unwrap();
        assert_eq!(on_disk.created_at.as_deref(), Some("2026-09-28T00:00:02Z"));
        let fresh = groups[0].entries.iter().find(|e| matches!(e.key, EntryKey::Live(_))).unwrap();
        assert_eq!(fresh.created_at, None);
    }
```

(`open(id, path, claude_id, marker)` is the existing helper of that test module that builds a `LiveSummary`; `summary("s1", ...)` has `created_at` `2026-09-28T00:00:02Z` because `"s1".len() % 10` is 2.)

- [ ] **Step 5: Implement it in the features**

In `SidebarEntry` add `pub created_at: Option<String>,` after `updated_at`. In `merge_sidebar` set `created_at: Some(summary.created_at.clone()),` in both entries built from a past summary, and `created_at: None,` in the entry built for an open session with no past entry.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p forge-adapter-claude-fs -p forge-features -p forge-application`
Expected: all pass. (`forge-application` is included because its tests build `SessionSummary` values through the registry's fakes; if one fails to compile, add `created_at` to that literal.)

- [ ] **Step 7: Commit**

```bash
git add -A crates
git commit -m "feat: sessions know when they began" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Sort, group and collapse (`arrange`)

**Files:**
- Create: `crates/forge-features/src/arrange.rs`
- Modify: `crates/forge-features/src/lib.rs`
- Modify: `crates/forge-features/Cargo.toml`

**Interfaces:**
- Consumes: `SidebarGroup`, `SidebarEntry`, `EntryKey` (`forge-features/src/sidebar.rs`); `SidebarPrefs`, `SortBy`, `GroupBy`, `ProjectSummary` (`forge_domain`); `LiveSessionId` (`forge_application`).
- Produces: `DateBucket` (`label()`, `slug()`), `Heading { Project(ProjectSummary), Bucket(DateBucket), None }`, `ArrangedEntry { project: ProjectSummary, entry: SidebarEntry }`, `ArrangedGroup { id: String, heading: Heading, collapsed: bool, entries: Vec<ArrangedEntry> }`, `arrange(&[SidebarGroup], &SidebarPrefs, OffsetDateTime, Option<LiveSessionId>) -> Vec<ArrangedGroup>`, `arranged_open_order(&[ArrangedGroup]) -> Vec<LiveSessionId>`, `folder_group_id(&Path) -> String`, `pub(crate) instant(Option<&str>) -> Option<OffsetDateTime>`, `pub(crate) newest_first(Option<OffsetDateTime>)` (sort key; used again by Task 7).

- [ ] **Step 1: Add the dependency**

In `crates/forge-features/Cargo.toml` under `[dependencies]` add:

```toml
time = { version = "0.3.55", features = ["parsing"] }
```

- [ ] **Step 2: Write the failing tests**

Create `crates/forge-features/src/arrange.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use forge_application::LiveSessionId;
    use forge_domain::{GroupBy, ProjectSummary, SessionId, SidebarPrefs, SortBy};
    use forge_entities::SessionMarker;
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    use super::*;
    use crate::sidebar::{EntryKey, SidebarEntry, SidebarGroup};

    fn now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-10-03T12:00:00Z", &Rfc3339).unwrap()
    }

    fn entry(title: &str, created: Option<&str>, updated: Option<&str>) -> SidebarEntry {
        SidebarEntry {
            key: EntryKey::Past { project: PathBuf::from("/p"), session: SessionId(title.to_string()) },
            session_id: Some(SessionId(title.to_string())),
            title: title.to_string(),
            updated_at: updated.map(str::to_string),
            created_at: created.map(str::to_string),
            marker: SessionMarker::Idle,
        }
    }

    fn group(path: &str, entries: Vec<SidebarEntry>) -> SidebarGroup {
        SidebarGroup {
            project: ProjectSummary {
                path: PathBuf::from(path),
                last_active: String::new(),
                session_count: entries.len(),
                exists: true,
            },
            entries,
        }
    }

    fn prefs(sort_by: SortBy, group_by: GroupBy) -> SidebarPrefs {
        SidebarPrefs { sort_by, group_by, ..SidebarPrefs::default() }
    }

    fn titles(group: &ArrangedGroup) -> Vec<&str> {
        group.entries.iter().map(|item| item.entry.title.as_str()).collect()
    }

    fn headings(groups: &[ArrangedGroup]) -> Vec<String> {
        groups
            .iter()
            .map(|group| match &group.heading {
                Heading::Project(project) => project.path.display().to_string(),
                Heading::Bucket(bucket) => bucket.label().to_string(),
                Heading::None => String::new(),
            })
            .collect()
    }

    #[test]
    fn activity_sort_puts_the_newest_first_and_a_session_not_on_disk_before_all() {
        let groups = vec![group(
            "/a",
            vec![
                entry("old", None, Some("2026-10-01T00:00:00Z")),
                entry("new", None, Some("2026-10-03T10:00:00Z")),
                entry("unsaved", None, None),
            ],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);

        assert_eq!(titles(&arranged[0]), ["unsaved", "new", "old"]);
    }

    #[test]
    fn a_fractional_second_is_still_in_order() {
        let groups = vec![group(
            "/a",
            vec![
                entry("whole", None, Some("2026-10-03T01:02:03Z")),
                entry("later", None, Some("2026-10-03T01:02:03.500Z")),
            ],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);

        assert_eq!(titles(&arranged[0]), ["later", "whole"]);
    }

    #[test]
    fn name_sort_ignores_case_and_puts_korean_after_latin() {
        let groups = vec![group(
            "/a",
            vec![
                entry("banana", None, None),
                entry("가나다", None, None),
                entry("Apple", None, None),
                entry("cherry", None, None),
            ],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Name, GroupBy::Folder), now(), None);

        assert_eq!(titles(&arranged[0]), ["Apple", "banana", "cherry", "가나다"]);
    }

    #[test]
    fn created_sort_differs_from_activity_sort_and_falls_back_to_the_last_activity() {
        let groups = vec![group(
            "/a",
            vec![
                entry("began-first", Some("2026-10-01T00:00:00Z"), Some("2026-10-03T00:00:00Z")),
                entry("began-second", Some("2026-10-02T00:00:00Z"), Some("2026-10-02T00:00:00Z")),
                entry("no-created", None, Some("2026-10-02T12:00:00Z")),
            ],
        )];

        let by_created = arrange(&groups, &prefs(SortBy::Created, GroupBy::Folder), now(), None);
        let by_activity = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);

        assert_eq!(titles(&by_created[0]), ["no-created", "began-second", "began-first"]);
        assert_eq!(titles(&by_activity[0]), ["began-first", "no-created", "began-second"]);
    }

    #[test]
    fn two_sessions_with_one_title_both_stay() {
        let mut second = entry("same", None, Some("2026-10-02T00:00:00Z"));
        second.key = EntryKey::Past { project: PathBuf::from("/p"), session: SessionId("other-id".to_string()) };
        let groups = vec![group("/a", vec![entry("same", None, Some("2026-10-03T00:00:00Z")), second])];

        for sort in [SortBy::Name, SortBy::Created, SortBy::Activity] {
            for by in [GroupBy::Folder, GroupBy::Date, GroupBy::Flat] {
                let arranged = arrange(&groups, &prefs(sort, by), now(), None);
                let count: usize = arranged.iter().map(|g| g.entries.len()).sum();
                assert_eq!(count, 2, "{sort:?} {by:?}");
            }
        }
    }

    #[test]
    fn folder_grouping_keeps_the_projects_in_their_order_and_hides_empty_ones() {
        let groups = vec![
            group("/empty", vec![]),
            group("/b", vec![entry("one", None, Some("2026-10-03T00:00:00Z"))]),
        ];

        let hidden = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);
        let shown = arrange(
            &groups,
            &SidebarPrefs { show_empty: true, ..prefs(SortBy::Activity, GroupBy::Folder) },
            now(),
            None,
        );

        assert_eq!(headings(&hidden), ["/b"]);
        assert_eq!(headings(&shown), ["/empty", "/b"]);
        assert_eq!(hidden[0].id, "folder:/b");
    }

    #[test]
    fn date_grouping_cuts_on_calendar_days_and_skips_empty_buckets() {
        let groups = vec![group(
            "/a",
            vec![
                entry("today", None, Some("2026-10-03T01:00:00Z")),
                entry("yesterday", None, Some("2026-10-02T23:59:00Z")),
                entry("seven", None, Some("2026-09-26T08:00:00Z")),
                entry("eight", None, Some("2026-09-25T08:00:00Z")),
                entry("thirty", None, Some("2026-09-03T08:00:00Z")),
                entry("thirty-one", None, Some("2026-09-02T08:00:00Z")),
                entry("unsaved", None, None),
            ],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Date), now(), None);

        assert_eq!(
            headings(&arranged),
            ["Today", "Yesterday", "Previous 7 days", "Previous 30 days", "Older"]
        );
        assert_eq!(titles(&arranged[0]), ["unsaved", "today"]);
        assert_eq!(titles(&arranged[1]), ["yesterday"]);
        assert_eq!(titles(&arranged[2]), ["seven"]);
        assert_eq!(titles(&arranged[3]), ["eight", "thirty"]);
        assert_eq!(titles(&arranged[4]), ["thirty-one"]);
    }

    #[test]
    fn date_grouping_by_created_uses_the_creation_day() {
        let groups = vec![group(
            "/a",
            vec![entry("old-but-active", Some("2026-08-01T00:00:00Z"), Some("2026-10-03T00:00:00Z"))],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Created, GroupBy::Date), now(), None);

        assert_eq!(headings(&arranged), ["Older"]);
    }

    #[test]
    fn date_grouping_mixes_projects_and_each_row_keeps_its_own() {
        let groups = vec![
            group("/a", vec![entry("from-a", None, Some("2026-10-03T02:00:00Z"))]),
            group("/b", vec![entry("from-b", None, Some("2026-10-03T03:00:00Z"))]),
        ];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Date), now(), None);

        assert_eq!(titles(&arranged[0]), ["from-b", "from-a"]);
        assert_eq!(arranged[0].entries[0].project.path, PathBuf::from("/b"));
        assert_eq!(arranged[0].entries[1].project.path, PathBuf::from("/a"));
    }

    #[test]
    fn name_sort_inside_date_buckets_keeps_the_buckets_in_time_order() {
        let groups = vec![group(
            "/a",
            vec![
                entry("zebra", None, Some("2026-10-03T01:00:00Z")),
                entry("apple", None, Some("2026-08-01T00:00:00Z")),
                entry("mango", None, Some("2026-10-03T02:00:00Z")),
            ],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Name, GroupBy::Date), now(), None);

        assert_eq!(headings(&arranged), ["Today", "Older"]);
        assert_eq!(titles(&arranged[0]), ["mango", "zebra"]);
    }

    #[test]
    fn no_grouping_gives_one_group_with_no_heading_and_nothing_when_there_is_nothing() {
        let groups = vec![
            group("/a", vec![entry("one", None, Some("2026-10-01T00:00:00Z"))]),
            group("/b", vec![entry("two", None, Some("2026-10-02T00:00:00Z"))]),
        ];

        let flat = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Flat), now(), None);
        let none = arrange(&[], &prefs(SortBy::Activity, GroupBy::Flat), now(), None);

        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].heading, Heading::None);
        assert_eq!(titles(&flat[0]), ["two", "one"]);
        assert!(none.is_empty());
    }

    #[test]
    fn a_collapsed_group_hides_its_rows_except_the_open_session() {
        let live_id = LiveSessionId(7);
        let mut live = entry("open", None, Some("2026-10-03T00:00:00Z"));
        live.key = EntryKey::Live(live_id);
        let groups = vec![group("/a", vec![live, entry("closed", None, Some("2026-10-02T00:00:00Z"))])];
        let collapsed = SidebarPrefs { collapsed: vec![folder_group_id(std::path::Path::new("/a"))], ..SidebarPrefs::default() };

        let with_open = arrange(&groups, &collapsed, now(), Some(live_id));
        let with_none = arrange(&groups, &collapsed, now(), None);

        assert!(with_open[0].collapsed);
        assert_eq!(titles(&with_open[0]), ["open"]);
        assert!(with_none[0].collapsed);
        assert!(with_none[0].entries.is_empty());
    }

    #[test]
    fn the_open_order_follows_the_arranged_rows() {
        let mut first = entry("first", None, Some("2026-10-01T00:00:00Z"));
        first.key = EntryKey::Live(LiveSessionId(1));
        let mut second = entry("second", None, Some("2026-10-02T00:00:00Z"));
        second.key = EntryKey::Live(LiveSessionId(2));
        let groups = vec![group("/a", vec![first, second])];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);

        assert_eq!(arranged_open_order(&arranged), [LiveSessionId(2), LiveSessionId(1)]);
    }
}
```

- [ ] **Step 3: Run the tests to see them fail**

Add `mod arrange;` to `crates/forge-features/src/lib.rs` (after `mod approve_permission;`). Run: `cargo test -p forge-features arrange`
Expected: compile errors (`cannot find function arrange`).

- [ ] **Step 4: Implement `arrange`**

Put this above the test module in `crates/forge-features/src/arrange.rs`:

```rust
use std::cmp::Reverse;
use std::path::Path;

use forge_application::LiveSessionId;
use forge_domain::{GroupBy, ProjectSummary, SidebarPrefs, SortBy};
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

use crate::sidebar::{EntryKey, SidebarEntry, SidebarGroup};

/// The age buckets of date grouping, newest first (the derived order is the drawing order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DateBucket {
    Today,
    Yesterday,
    Previous7Days,
    Previous30Days,
    Older,
}

impl DateBucket {
    pub fn label(self) -> &'static str {
        match self {
            DateBucket::Today => "Today",
            DateBucket::Yesterday => "Yesterday",
            DateBucket::Previous7Days => "Previous 7 days",
            DateBucket::Previous30Days => "Previous 30 days",
            DateBucket::Older => "Older",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            DateBucket::Today => "today",
            DateBucket::Yesterday => "yesterday",
            DateBucket::Previous7Days => "7d",
            DateBucket::Previous30Days => "30d",
            DateBucket::Older => "older",
        }
    }
}

/// What a group's heading says.
#[derive(Debug, Clone, PartialEq)]
pub enum Heading {
    Project(ProjectSummary),
    Bucket(DateBucket),
    /// No grouping: no heading at all.
    None,
}

/// A row, with the project it belongs to (a row in a date group needs it to open and to show it).
#[derive(Debug, Clone, PartialEq)]
pub struct ArrangedEntry {
    pub project: ProjectSummary,
    pub entry: SidebarEntry,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrangedGroup {
    /// Stable between draws; the key of the collapsed set in `SidebarPrefs`.
    pub id: String,
    pub heading: Heading,
    pub collapsed: bool,
    pub entries: Vec<ArrangedEntry>,
}

pub fn folder_group_id(path: &Path) -> String {
    format!("folder:{}", path.display())
}

fn bucket_group_id(bucket: DateBucket) -> String {
    format!("date:{}", bucket.slug())
}

pub(crate) fn instant(text: Option<&str>) -> Option<OffsetDateTime> {
    text.and_then(|text| OffsetDateTime::parse(text, &Rfc3339).ok())
}

/// A sort key that puts the newest first and a missing time (a session not on disk yet) before
/// everything. Times are compared as instants, not as text: `…03.500Z` is later than `…03Z`.
pub(crate) fn newest_first(at: Option<OffsetDateTime>) -> (bool, Reverse<Option<i128>>) {
    (at.is_some(), Reverse(at.map(|at| at.unix_timestamp_nanos())))
}

fn activity_of(entry: &SidebarEntry) -> Option<OffsetDateTime> {
    instant(entry.updated_at.as_deref())
}

fn created_of(entry: &SidebarEntry) -> Option<OffsetDateTime> {
    instant(entry.created_at.as_deref()).or_else(|| activity_of(entry))
}

// A stable sort: sessions that tie keep the order the adapter gave them (newest first, then id).
fn sort_entries(entries: &mut [ArrangedEntry], sort_by: SortBy) {
    match sort_by {
        SortBy::Name => entries.sort_by_key(|item| item.entry.title.to_lowercase()),
        SortBy::Created => entries.sort_by_key(|item| newest_first(created_of(&item.entry))),
        SortBy::Activity => entries.sort_by_key(|item| newest_first(activity_of(&item.entry))),
    }
}

fn bucket_of(at: Option<OffsetDateTime>, now: OffsetDateTime) -> DateBucket {
    let Some(at) = at else {
        return DateBucket::Today;
    };
    let days = (now.to_offset(UtcOffset::UTC).date() - at.to_offset(UtcOffset::UTC).date()).whole_days();
    match days {
        days if days <= 0 => DateBucket::Today,
        1 => DateBucket::Yesterday,
        2..=7 => DateBucket::Previous7Days,
        8..=30 => DateBucket::Previous30Days,
        _ => DateBucket::Older,
    }
}

fn finish(
    id: String,
    heading: Heading,
    mut entries: Vec<ArrangedEntry>,
    prefs: &SidebarPrefs,
    active: Option<LiveSessionId>,
) -> ArrangedGroup {
    let collapsed = prefs.is_collapsed(&id);
    if collapsed {
        // The row of the session on screen stays, so the person never loses sight of where they are.
        entries.retain(|item| matches!((&item.entry.key, active), (EntryKey::Live(open), Some(shown)) if *open == shown));
    }
    ArrangedGroup { id, heading, collapsed, entries }
}

fn all_entries(groups: &[SidebarGroup]) -> Vec<ArrangedEntry> {
    groups
        .iter()
        .flat_map(|group| {
            group
                .entries
                .iter()
                .map(|entry| ArrangedEntry { project: group.project.clone(), entry: entry.clone() })
        })
        .collect()
}

/// The sidebar as it is drawn: `groups` (from `merge_sidebar`) sorted, grouped and collapsed
/// as `prefs` says. `now` is passed in so the date buckets are testable; `active` is the open
/// session on screen, whose row stays visible in a collapsed group.
pub fn arrange(
    groups: &[SidebarGroup],
    prefs: &SidebarPrefs,
    now: OffsetDateTime,
    active: Option<LiveSessionId>,
) -> Vec<ArrangedGroup> {
    match prefs.group_by {
        GroupBy::Folder => groups
            .iter()
            .filter(|group| prefs.show_empty || !group.entries.is_empty())
            .map(|group| {
                let mut entries: Vec<ArrangedEntry> = group
                    .entries
                    .iter()
                    .map(|entry| ArrangedEntry { project: group.project.clone(), entry: entry.clone() })
                    .collect();
                sort_entries(&mut entries, prefs.sort_by);
                finish(folder_group_id(&group.project.path), Heading::Project(group.project.clone()), entries, prefs, active)
            })
            .collect(),
        GroupBy::Date => {
            let mut items = all_entries(groups);
            sort_entries(&mut items, prefs.sort_by);
            let mut buckets: Vec<(DateBucket, Vec<ArrangedEntry>)> = Vec::new();
            for item in items {
                let at = match prefs.sort_by {
                    SortBy::Created => created_of(&item.entry),
                    _ => activity_of(&item.entry),
                };
                let bucket = bucket_of(at, now);
                match buckets.iter_mut().find(|(known, _)| *known == bucket) {
                    Some((_, list)) => list.push(item),
                    None => buckets.push((bucket, vec![item])),
                }
            }
            // Sorting by name meets the buckets in any order; they are drawn newest first.
            buckets.sort_by_key(|(bucket, _)| *bucket);
            buckets
                .into_iter()
                .map(|(bucket, list)| finish(bucket_group_id(bucket), Heading::Bucket(bucket), list, prefs, active))
                .collect()
        }
        GroupBy::Flat => {
            let mut items = all_entries(groups);
            if items.is_empty() {
                return Vec::new();
            }
            sort_entries(&mut items, prefs.sort_by);
            vec![ArrangedGroup { id: "flat".to_string(), heading: Heading::None, collapsed: false, entries: items }]
        }
    }
}

/// The open sessions in the order the sidebar draws them; Ctrl+Tab cycles through this. Pass
/// groups arranged with no collapsed ids, so a session in a collapsed group can still be reached.
pub fn arranged_open_order(groups: &[ArrangedGroup]) -> Vec<LiveSessionId> {
    groups
        .iter()
        .flat_map(|group| group.entries.iter())
        .filter_map(|item| match item.entry.key {
            EntryKey::Live(id) => Some(id),
            EntryKey::Past { .. } => None,
        })
        .collect()
}
```

In `crates/forge-features/src/lib.rs` add:

```rust
pub use arrange::{arrange, arranged_open_order, folder_group_id, ArrangedEntry, ArrangedGroup, DateBucket, Heading};
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p forge-features arrange`
Expected: 13 tests pass. If `collapsed_group...` fails on the pattern guard, keep the behaviour (open row stays) and fix the syntax only.

- [ ] **Step 6: Commit**

```bash
git add -A crates
git commit -m "feat: sort, group and collapse the sidebar as pure functions" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Keep the preferences in the workspace and save them

**Files:**
- Modify: `crates/forge-application/src/use_cases/settings.rs` (`save_sidebar`, test)
- Modify: `crates/forge-features/src/workspace.rs` (signal, getters, `arranged_sidebar`, `cycle`)

**Interfaces:**
- Consumes: `SidebarPrefs` (Task 1), `arrange`, `arranged_open_order` (Task 3).
- Produces: `SettingsUseCase::save_sidebar(&self, &SidebarPrefs) -> Result<AppConfig, ConfigError>`; on `Workspace`: `prefs() -> SidebarPrefs`, `update_prefs(impl FnOnce(&mut SidebarPrefs))` (change and save), `change_prefs_unsaved(impl FnOnce(&mut SidebarPrefs))`, `save_prefs()`, `arranged_sidebar(OffsetDateTime) -> Vec<ArrangedGroup>`.

- [ ] **Step 1: Write the failing use-case test**

Add to the `tests` module of `crates/forge-application/src/use_cases/settings.rs`:

```rust
    #[tokio::test]
    async fn save_sidebar_replaces_only_the_sidebar_preferences() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save()
            .withf(|c: &AppConfig| {
                c.sidebar.sort_by == forge_domain::SortBy::Name
                    && c.sidebar.hidden
                    && c.base_url.as_deref() == Some("https://old.test")
                    && c.enabled_skills == vec!["brainstorming".to_string()]
            })
            .times(1)
            .returning(|_| Ok(()));
        let usecase = SettingsUseCase::new(Arc::new(mock));
        let prefs = forge_domain::SidebarPrefs {
            sort_by: forge_domain::SortBy::Name,
            hidden: true,
            ..Default::default()
        };

        let saved = usecase.save_sidebar(&prefs).await.unwrap();

        assert_eq!(saved.sidebar, prefs);
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p forge-application save_sidebar`
Expected: compile error, no method `save_sidebar`.

- [ ] **Step 3: Implement the use case**

In `crates/forge-application/src/use_cases/settings.rs`, change the import to `use forge_domain::{normalize_setting, AppConfig, SidebarPrefs};` and add inside `impl SettingsUseCase`, after `save_model`:

```rust
    pub async fn save_sidebar(&self, prefs: &SidebarPrefs) -> Result<AppConfig, ConfigError> {
        let mut config = self.load_for_update().await?;
        config.sidebar = prefs.clone();
        self.config.save(&config).await?;
        Ok(config)
    }
```

Run: `cargo test -p forge-application save_sidebar` → passes.

- [ ] **Step 4: Add the state to `Workspace`**

In `crates/forge-features/src/workspace.rs`:

Imports: add `use forge_domain::SidebarPrefs;` to the `forge_domain` import list, `use time::OffsetDateTime;`, and change the sidebar import to also bring the new items:

```rust
use crate::arrange::{arrange, arranged_open_order, ArrangedGroup};
use crate::sidebar::{cycle_target, merge_sidebar, replace_project_sessions, LiveSummary, SidebarGroup};
```

(`open_order` is no longer used here; leave it exported from `lib.rs`.)

Add a field to `struct Workspace`: `prefs: Signal<SidebarPrefs>,` and in `use_workspace_provider` the initialiser `prefs: Signal::new(SidebarPrefs::default()),`. After `use_hook(move || workspace.reload_past());` add `use_hook(move || workspace.reload_prefs());`.

Add to `impl Workspace`:

```rust
    /// How the sidebar is sorted, grouped and sized.
    pub fn prefs(&self) -> SidebarPrefs {
        self.prefs.read().clone()
    }

    /// Changes the preferences and saves them.
    pub fn update_prefs(&self, change: impl FnOnce(&mut SidebarPrefs)) {
        self.change_prefs_unsaved(change);
        self.save_prefs();
    }

    /// Changes the preferences on screen only (a drag changes the width many times); call
    /// `save_prefs` when it ends.
    pub fn change_prefs_unsaved(&self, change: impl FnOnce(&mut SidebarPrefs)) {
        let mut prefs = self.prefs;
        change(&mut prefs.write());
    }

    pub fn save_prefs(&self) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        let prefs = self.prefs();
        // Root scope: the save must finish even if the control that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            if let Err(error) = services.settings.save_sidebar(&prefs).await {
                this.report(format!("failed to save the sidebar settings: {error}"));
            }
        });
    }

    // A config that cannot be read keeps the defaults; the settings screen reports that file's
    // problems, so the sidebar does not repeat them.
    fn reload_prefs(&self) {
        let services = (*self.services.peek()).clone();
        let mut prefs = self.prefs;
        spawn(async move {
            if let Ok(config) = services.settings.load().await {
                prefs.set(config.sidebar);
            }
        });
    }

    /// The sidebar as drawn: sorted, grouped and collapsed as the preferences say.
    pub fn arranged_sidebar(&self, now: OffsetDateTime) -> Vec<ArrangedGroup> {
        let active = match self.active() {
            Active::Live(id) => Some(id),
            Active::NewSession => None,
        };
        arrange(&self.sidebar(), &self.prefs.read(), now, active)
    }
```

Replace the body of `cycle`'s first line `let order = open_order(&self.sidebar());` with:

```rust
        // Collapsed groups hide rows but not sessions: Ctrl+Tab still reaches them.
        let mut prefs = self.prefs();
        prefs.collapsed.clear();
        let order = arranged_open_order(&arrange(&self.sidebar(), &prefs, OffsetDateTime::now_utc(), None));
```

- [ ] **Step 5: Build and test**

Run: `cargo build -p forge-features && cargo test -p forge-features -p forge-application`
Expected: builds; all tests pass. (`Workspace` methods need a Dioxus runtime, so they are checked in Task 10.)

- [ ] **Step 6: Commit**

```bash
git add -A crates
git commit -m "feat: the workspace keeps the sidebar preferences and saves them" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The filter menu's keys (pure)

**Files:**
- Create: `crates/forge-features/src/filter_menu.rs`
- Modify: `crates/forge-features/src/lib.rs`

**Interfaces:**
- Consumes: `SidebarPrefs`, `SortBy`, `GroupBy` (Task 1).
- Produces: `SORT_ITEMS`, `GROUP_ITEMS`, `FILTER_ROWS`, `sort_label`, `group_label`, `FilterRow`, `FilterChoice` (`apply`), `FilterKey`, `filter_key(&str) -> Option<FilterKey>`, `FilterMenu { top: usize, sub: Option<usize> }` with `key(&mut self, FilterKey, &SidebarPrefs) -> FilterOutcome`, `FilterOutcome { Stay, Choose(FilterChoice), Close }`, `current_index(FilterRow, &SidebarPrefs) -> usize`.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-features/src/filter_menu.rs` with the test module only:

```rust
#[cfg(test)]
mod tests {
    use forge_domain::{GroupBy, SidebarPrefs, SortBy};

    use super::*;

    fn press(menu: &mut FilterMenu, keys: &[FilterKey], prefs: &SidebarPrefs) -> FilterOutcome {
        let mut last = FilterOutcome::Stay;
        for key in keys {
            last = menu.key(*key, prefs);
        }
        last
    }

    #[test]
    fn up_and_down_move_over_the_three_rows_and_stop_at_the_ends() {
        let prefs = SidebarPrefs::default();
        let mut menu = FilterMenu::default();

        press(&mut menu, &[FilterKey::Up], &prefs);
        assert_eq!(menu.top, 0);
        press(&mut menu, &[FilterKey::Down, FilterKey::Down, FilterKey::Down, FilterKey::Down], &prefs);
        assert_eq!(menu.top, 2);
    }

    #[test]
    fn right_opens_the_submenu_on_the_current_value() {
        let prefs = SidebarPrefs { sort_by: SortBy::Created, ..SidebarPrefs::default() };
        let mut menu = FilterMenu::default();

        press(&mut menu, &[FilterKey::Right], &prefs);

        assert_eq!(menu.sub, Some(1));
    }

    #[test]
    fn enter_in_a_submenu_chooses_and_the_choice_changes_the_preferences() {
        let mut prefs = SidebarPrefs::default();
        let mut menu = FilterMenu::default();

        let outcome = press(&mut menu, &[FilterKey::Down, FilterKey::Right, FilterKey::Down, FilterKey::Enter], &prefs);

        assert_eq!(outcome, FilterOutcome::Choose(FilterChoice::Group(GroupBy::Date)));
        if let FilterOutcome::Choose(choice) = outcome {
            choice.apply(&mut prefs);
        }
        assert_eq!(prefs.group_by, GroupBy::Date);
    }

    #[test]
    fn left_and_escape_close_the_submenu_first_and_then_escape_closes_the_menu() {
        let prefs = SidebarPrefs::default();
        let mut menu = FilterMenu::default();

        press(&mut menu, &[FilterKey::Right], &prefs);
        assert!(menu.sub.is_some());
        assert_eq!(press(&mut menu, &[FilterKey::Left], &prefs), FilterOutcome::Stay);
        assert_eq!(menu.sub, None);

        press(&mut menu, &[FilterKey::Right], &prefs);
        assert_eq!(press(&mut menu, &[FilterKey::Escape], &prefs), FilterOutcome::Stay);
        assert_eq!(menu.sub, None);
        assert_eq!(press(&mut menu, &[FilterKey::Escape], &prefs), FilterOutcome::Close);
    }

    #[test]
    fn enter_on_show_empty_toggles_it_and_right_does_nothing_there() {
        let mut prefs = SidebarPrefs::default();
        let mut menu = FilterMenu::default();

        press(&mut menu, &[FilterKey::Down, FilterKey::Down], &prefs);
        assert_eq!(press(&mut menu, &[FilterKey::Right], &prefs), FilterOutcome::Stay);
        assert_eq!(menu.sub, None);
        let outcome = press(&mut menu, &[FilterKey::Enter], &prefs);

        assert_eq!(outcome, FilterOutcome::Choose(FilterChoice::ToggleShowEmpty));
        if let FilterOutcome::Choose(choice) = outcome {
            choice.apply(&mut prefs);
        }
        assert!(prefs.show_empty);
    }

    #[test]
    fn keys_are_read_from_the_physical_key() {
        assert_eq!(filter_key("ArrowDown"), Some(FilterKey::Down));
        assert_eq!(filter_key("ArrowUp"), Some(FilterKey::Up));
        assert_eq!(filter_key("ArrowLeft"), Some(FilterKey::Left));
        assert_eq!(filter_key("ArrowRight"), Some(FilterKey::Right));
        assert_eq!(filter_key("Enter"), Some(FilterKey::Enter));
        assert_eq!(filter_key("NumpadEnter"), Some(FilterKey::Enter));
        assert_eq!(filter_key("Escape"), Some(FilterKey::Escape));
        assert_eq!(filter_key("KeyR"), None);
    }

    #[test]
    fn the_labels_are_the_ones_the_menu_shows() {
        assert_eq!(sort_label(SortBy::Activity), "Activity");
        assert_eq!(group_label(GroupBy::Flat), "None");
    }
}
```

- [ ] **Step 2: Run to see it fail**

Add `mod filter_menu;` to `crates/forge-features/src/lib.rs`. Run: `cargo test -p forge-features filter_menu`
Expected: compile errors.

- [ ] **Step 3: Implement**

Above the tests in `crates/forge-features/src/filter_menu.rs`:

```rust
use forge_domain::{GroupBy, SidebarPrefs, SortBy};

pub const SORT_ITEMS: [SortBy; 3] = [SortBy::Name, SortBy::Created, SortBy::Activity];
pub const GROUP_ITEMS: [GroupBy; 3] = [GroupBy::Folder, GroupBy::Date, GroupBy::Flat];

pub fn sort_label(sort_by: SortBy) -> &'static str {
    match sort_by {
        SortBy::Name => "Name",
        SortBy::Created => "Created",
        SortBy::Activity => "Activity",
    }
}

pub fn group_label(group_by: GroupBy) -> &'static str {
    match group_by {
        GroupBy::Folder => "Folder",
        GroupBy::Date => "Date",
        GroupBy::Flat => "None",
    }
}

/// The top-level rows of the filter menu, in drawing order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterRow {
    Sort,
    Group,
    ShowEmpty,
}

pub const FILTER_ROWS: [FilterRow; 3] = [FilterRow::Sort, FilterRow::Group, FilterRow::ShowEmpty];

/// What picking an item changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterChoice {
    Sort(SortBy),
    Group(GroupBy),
    ToggleShowEmpty,
}

impl FilterChoice {
    pub fn apply(self, prefs: &mut SidebarPrefs) {
        match self {
            FilterChoice::Sort(sort_by) => prefs.sort_by = sort_by,
            FilterChoice::Group(group_by) => prefs.group_by = group_by,
            FilterChoice::ToggleShowEmpty => prefs.show_empty = !prefs.show_empty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
}

/// The key a physical key code stands for (`event.code()`), if the menu uses it.
pub fn filter_key(code: &str) -> Option<FilterKey> {
    match code {
        "ArrowUp" => Some(FilterKey::Up),
        "ArrowDown" => Some(FilterKey::Down),
        "ArrowLeft" => Some(FilterKey::Left),
        "ArrowRight" => Some(FilterKey::Right),
        "Enter" | "NumpadEnter" => Some(FilterKey::Enter),
        "Escape" => Some(FilterKey::Escape),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOutcome {
    Stay,
    /// Apply it, then close the menu.
    Choose(FilterChoice),
    Close,
}

/// Which row is highlighted, and which item of an open submenu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FilterMenu {
    pub top: usize,
    pub sub: Option<usize>,
}

/// The position of the current value among the items of a row's submenu.
pub fn current_index(row: FilterRow, prefs: &SidebarPrefs) -> usize {
    match row {
        FilterRow::Sort => SORT_ITEMS.iter().position(|item| *item == prefs.sort_by).unwrap_or(0),
        FilterRow::Group => GROUP_ITEMS.iter().position(|item| *item == prefs.group_by).unwrap_or(0),
        FilterRow::ShowEmpty => 0,
    }
}

impl FilterMenu {
    pub fn key(&mut self, key: FilterKey, prefs: &SidebarPrefs) -> FilterOutcome {
        let row = FILTER_ROWS[self.top];
        match (self.sub, key) {
            (Some(index), FilterKey::Up) => self.sub = Some(index.saturating_sub(1)),
            (Some(index), FilterKey::Down) => self.sub = Some((index + 1).min(2)),
            (Some(_), FilterKey::Left | FilterKey::Escape) => self.sub = None,
            (Some(_), FilterKey::Right) => {}
            (Some(index), FilterKey::Enter) => {
                let choice = match row {
                    FilterRow::Sort => FilterChoice::Sort(SORT_ITEMS[index]),
                    FilterRow::Group => FilterChoice::Group(GROUP_ITEMS[index]),
                    FilterRow::ShowEmpty => FilterChoice::ToggleShowEmpty,
                };
                return FilterOutcome::Choose(choice);
            }
            (None, FilterKey::Up) => self.top = self.top.saturating_sub(1),
            (None, FilterKey::Down) => self.top = (self.top + 1).min(FILTER_ROWS.len() - 1),
            (None, FilterKey::Left) => {}
            (None, FilterKey::Right) => {
                if row != FilterRow::ShowEmpty {
                    self.sub = Some(current_index(row, prefs));
                }
            }
            (None, FilterKey::Enter) => {
                if row == FilterRow::ShowEmpty {
                    return FilterOutcome::Choose(FilterChoice::ToggleShowEmpty);
                }
                self.sub = Some(current_index(row, prefs));
            }
            (None, FilterKey::Escape) => return FilterOutcome::Close,
        }
        FilterOutcome::Stay
    }
}
```

In `lib.rs` add:

```rust
pub use filter_menu::{
    current_index, filter_key, group_label, sort_label, FilterChoice, FilterKey, FilterMenu, FilterOutcome, FilterRow,
    FILTER_ROWS, GROUP_ITEMS, SORT_ITEMS,
};
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-features filter_menu`
Expected: 7 pass.

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: the keys of the sidebar filter menu as a pure state machine" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Search the transcripts (port and adapter)

**Files:**
- Modify: `crates/forge-ports/src/session_port.rs`
- Modify: `crates/forge-application/src/registry.rs` (the test fake `SlowRepository` implements the trait)
- Modify: `crates/forge-adapter-claude-fs/src/sessions.rs`

**Interfaces:**
- Consumes: `SearchHit`, `Snippet`, `find_snippet`, `title_matches` (Task 1); `created_at_of` (Task 2); `assign_cwds`, `transcripts_newest_first`, `Transcript` (`transcript_cwd.rs`); `read_title`.
- Produces: `SessionRepositoryPort::search_sessions(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, SessionRepositoryError>`; `SessionRepositoryError::SearchFailed(String)`. A hit's `project` is the folder the transcript names as its `cwd`; hits are newest first; at most one per session; a title-only match has `snippet: None`.

- [ ] **Step 1: Extend the port and fix the fake**

In `crates/forge-ports/src/session_port.rs`: change the import to `use forge_domain::{SearchHit, Session, SessionId, SessionSummary};`, add the error variant after `DeleteFailed`:

```rust
    #[error("failed to search sessions: {0}")]
    SearchFailed(String),
```

and the method at the end of the trait:

```rust
    /// Sessions of every project whose title or message text contains `query` (any case), newest
    /// first, at most `limit`. A blank query finds nothing. Only what the user and the assistant
    /// wrote is searched, not tool input, tool results or thinking.
    async fn search_sessions(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, SessionRepositoryError>;
```

Add to its tests:

```rust
    #[test]
    fn search_failed_message_names_the_cause() {
        let error = SessionRepositoryError::SearchFailed("no access".to_string());
        assert_eq!(error.to_string(), "failed to search sessions: no access");
    }

    #[tokio::test]
    async fn mock_session_repository_port_can_record_a_search_call() {
        let mut mock = MockSessionRepositoryPort::new();
        mock.expect_search_sessions()
            .withf(|query: &str, limit: &usize| query == "borrow" && *limit == 50)
            .times(1)
            .returning(|_, _| Ok(vec![]));

        assert!(mock.search_sessions("borrow", 50).await.unwrap().is_empty());
    }
```

In `crates/forge-application/src/registry.rs`, in `impl SessionRepositoryPort for SlowRepository`, add (and add `SearchHit` to that test module's `forge_domain` import):

```rust
        async fn search_sessions(&self, _query: &str, _limit: usize) -> Result<Vec<SearchHit>, SessionRepositoryError> {
            Ok(Vec::new())
        }
```

Run: `cargo test -p forge-ports -p forge-application`
Expected: pass.

- [ ] **Step 2: Write the failing adapter tests**

Add to the `tests` module of `crates/forge-adapter-claude-fs/src/sessions.rs` (the existing helpers `write_session`, `user_text`, `assistant_block`, `ai_title`, `set_mtime`, `project`, `json!` are in scope):

```rust
    fn user_in(cwd: &str, text: &str) -> Value {
        json!({"type": "user", "cwd": cwd, "isSidechain": false, "message": {"role": "user", "content": text}})
    }

    fn assistant_text(text: &str) -> Value {
        assistant_block(json!({"type": "text", "text": text}))
    }

    async fn search(home: &Path, query: &str) -> Vec<SearchHit> {
        ClaudeSessionAdapter::new(home.to_path_buf()).search_sessions(query, 50).await.unwrap()
    }

    #[tokio::test]
    async fn search_finds_text_the_user_wrote_and_quotes_it() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "s1", &[user_in("/Users/test/project", "How do I fix the borrow checker error?")]);

        let hits = search(home.path(), "BORROW").await;

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].session.id, SessionId("s1".to_string()));
        assert_eq!(hits[0].project, PathBuf::from("/Users/test/project"));
        let snippet = hits[0].snippet.as_ref().unwrap();
        assert_eq!(snippet.matched, "borrow");
        assert_eq!(snippet.before, "How do I fix the ");
    }

    #[tokio::test]
    async fn search_finds_text_the_assistant_wrote() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[user_in("/Users/test/project", "hi"), assistant_text("Use an Arc<Mutex<T>> here.")],
        );

        let hits = search(home.path(), "arc<mutex").await;

        assert_eq!(hits.len(), 1);
    }

    #[tokio::test]
    async fn search_ignores_tool_input_tool_results_and_thinking() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[
                user_in("/Users/test/project", "hello"),
                assistant_block(json!({"type": "tool_use", "id": "t1", "name": "Bash", "input": {"command": "grep needle-word ."}})),
                user_blocks(json!([{"type": "tool_result", "tool_use_id": "t1", "content": "needle-word found"}])),
                assistant_block(json!({"type": "thinking", "thinking": "needle-word is on my mind"})),
            ],
        );

        assert!(search(home.path(), "needle-word").await.is_empty());
    }

    #[tokio::test]
    async fn search_finds_a_title_and_marks_the_hit_as_title_only() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[user_in("/Users/test/project", "hello"), ai_title("Debug and identify error cause")],
        );

        let hits = search(home.path(), "identify").await;

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].snippet, None);
        assert_eq!(hits[0].session.title.as_deref(), Some("Debug and identify error cause"));
    }

    #[tokio::test]
    async fn two_matches_in_one_session_make_one_hit_with_the_first() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[user_in("/Users/test/project", "first needle"), assistant_text("second needle")],
        );

        let hits = search(home.path(), "needle").await;

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].snippet.as_ref().unwrap().before, "first ");
    }

    #[tokio::test]
    async fn search_matches_korean_text() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "s1", &[user_in("/Users/test/project", "최적의 로컬 LLM 모델이 뭐야?")]);

        let hits = search(home.path(), "로컬").await;

        assert_eq!(hits[0].snippet.as_ref().unwrap().matched, "로컬");
    }

    #[tokio::test]
    async fn a_query_with_json_special_characters_still_finds_text() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[user_in("/Users/test/project", "say \"hello\" and use C:\\temp")],
        );

        assert_eq!(search(home.path(), "\"hello\"").await.len(), 1);
        assert_eq!(search(home.path(), "C:\\temp").await.len(), 1);
    }

    #[tokio::test]
    async fn a_bad_line_does_not_fail_the_search() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[user_in("/Users/test/project", "needle here")]);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.extend_from_slice(&[0xff, 0xfe, b'\n']);
        bytes.extend_from_slice(b"{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"te");
        std::fs::write(&path, bytes).unwrap();
        write_session(home.path(), &project(), "s2", &[user_in("/Users/test/project", "needle too")]);

        let hits = search(home.path(), "needle").await;

        assert_eq!(hits.len(), 2);
    }

    #[tokio::test]
    async fn hits_come_newest_first_across_projects_and_the_limit_applies() {
        let home = tempfile::tempdir().unwrap();
        let other = PathBuf::from("/Users/test/other");
        let old = write_session(home.path(), &project(), "old", &[user_in("/Users/test/project", "needle")]);
        let new = write_session(home.path(), &other, "new", &[user_in("/Users/test/other", "needle")]);
        let mid = write_session(home.path(), &project(), "mid", &[user_in("/Users/test/project", "needle")]);
        set_mtime(&old, 100);
        set_mtime(&mid, 200);
        set_mtime(&new, 300);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let all = adapter.search_sessions("needle", 50).await.unwrap();
        let two = adapter.search_sessions("needle", 2).await.unwrap();

        let ids: Vec<&str> = all.iter().map(|hit| hit.session.id.0.as_str()).collect();
        assert_eq!(ids, ["new", "mid", "old"]);
        assert_eq!(two.len(), 2);
        assert_eq!(two[0].project, PathBuf::from("/Users/test/other"));
    }

    #[tokio::test]
    async fn a_blank_query_or_no_projects_finds_nothing() {
        let home = tempfile::tempdir().unwrap();
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());
        assert!(adapter.search_sessions("needle", 50).await.unwrap().is_empty());

        write_session(home.path(), &project(), "s1", &[user_in("/Users/test/project", "needle")]);
        assert!(adapter.search_sessions("   ", 50).await.unwrap().is_empty());
        assert!(adapter.search_sessions("needle", 0).await.unwrap().is_empty());
    }
```

Add `SearchHit` to the test module's imports if it does not already `use super::*` it (it will once the production code imports it).

- [ ] **Step 3: Run to see them fail**

Run: `cargo test -p forge-adapter-claude-fs search`
Expected: compile error (the trait method is not implemented).

- [ ] **Step 4: Implement the search**

In `crates/forge-adapter-claude-fs/src/sessions.rs`, extend the imports: `forge_domain::{..., find_snippet, title_matches, SearchHit, Snippet}` (add to the existing `use forge_domain::{...}` list) and `use crate::transcript_cwd::Transcript;` (extend the existing `transcript_cwd` import).

Add near the other helpers:

```rust
// The text the user and the assistant wrote in one transcript line; tool input, tool results and
// thinking are not part of it.
fn message_texts(line: &Value) -> Vec<String> {
    if !matches!(line.get("type").and_then(Value::as_str), Some("user" | "assistant")) {
        return Vec::new();
    }
    match line.pointer("/message/content") {
        Some(Value::String(text)) => vec![text.clone()],
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

// The quote around the first match in the transcript's text. A line that cannot hold the query is
// skipped without being parsed; that shortcut is only taken for a plain ASCII query, because JSON
// writes quotes and backslashes escaped, so a query with them is not found in the raw line.
async fn first_snippet(path: &Path, query: &str) -> Option<Snippet> {
    let file = tokio::fs::File::open(path).await.ok()?;
    let mut lines = BufReader::new(file).split(b'\n');
    let shortcut = query.is_ascii() && !query.contains(['"', '\\']);
    let lowered = query.trim().to_lowercase();
    loop {
        let Ok(Some(line)) = lines.next_segment().await else {
            return None;
        };
        // An invalid-UTF-8 line and a half-written last line are skipped.
        let Ok(text) = std::str::from_utf8(&line) else {
            continue;
        };
        if shortcut && !text.to_lowercase().contains(&lowered) {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(text) else {
            continue;
        };
        if let Some(snippet) = message_texts(&value).iter().find_map(|body| find_snippet(body, query)) {
            return Some(snippet);
        }
    }
}

async fn search_transcript(transcript: &Transcript, query: &str) -> Option<SearchHit> {
    let id = transcript.path.file_stem()?.to_str()?.to_string();
    let title = read_title(&transcript.path).await;
    let snippet = first_snippet(&transcript.path, query).await;
    let title_matched = title.as_deref().is_some_and(|title| title_matches(title, query));
    if snippet.is_none() && !title_matched {
        return None;
    }
    let updated_at = time::OffsetDateTime::from(transcript.modified).format(&Rfc3339).ok()?;
    let created_at = created_at_of(&transcript.path, &updated_at).await;
    Some(SearchHit {
        project: transcript.cwd.clone(),
        session: SessionSummary { id: SessionId(id), title, updated_at, created_at },
        snippet,
    })
}
```

Add inside `impl SessionRepositoryPort for ClaudeSessionAdapter`:

```rust
    async fn search_sessions(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, SessionRepositoryError> {
        let query = query.trim();
        if query.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let root = self.claude_home.join("projects");
        let mut dirs = match tokio::fs::read_dir(&root).await {
            Ok(dirs) => dirs,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(SessionRepositoryError::SearchFailed(format!("{}: {}", root.display(), e))),
        };
        let mut transcripts: Vec<Transcript> = Vec::new();
        while let Some(entry) = dirs
            .next_entry()
            .await
            .map_err(|e| SessionRepositoryError::SearchFailed(format!("{}: {}", root.display(), e)))?
        {
            let dir = entry.path();
            if !tokio::fs::metadata(&dir).await.map(|m| m.is_dir()).unwrap_or(false) {
                continue;
            }
            transcripts.extend(assign_cwds(transcripts_newest_first(&dir).await).await);
        }
        // Newest first across every project, so the scan can stop as soon as the limit is met.
        transcripts.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.path.cmp(&b.path)));

        let mut hits = Vec::new();
        for transcript in &transcripts {
            if let Some(hit) = search_transcript(transcript, query).await {
                hits.push(hit);
                if hits.len() == limit {
                    break;
                }
            }
        }
        Ok(hits)
    }
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p forge-adapter-claude-fs`
Expected: all pass, including the 10 new tests. If `a_bad_line_does_not_fail_the_search` finds 1 instead of 2, the half-written line is aborting the read of `s1`'s later lines or `s2` — fix `first_snippet` to `continue`, never `return`, on a bad line (the code above does).

- [ ] **Step 6: Commit**

```bash
git add -A crates
git commit -m "feat: search the titles and message text of every transcript" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Palette rows, palette state and ⌘K

**Files:**
- Create: `crates/forge-features/src/search.rs`
- Modify: `crates/forge-features/src/lib.rs`, `crates/forge-features/Cargo.toml`
- Modify: `crates/forge-features/src/workspace.rs`
- Modify: `crates/forge-features/src/shortcut.rs`

**Interfaces:**
- Consumes: `SearchHit`, `Snippet`, `title_matches` (Task 1); `instant`, `newest_first` (Task 3); `SessionRepositoryPort::search_sessions` (Task 6).
- Produces: `SearchRow { key: EntryKey, project: ProjectSummary, title: String, snippet: Option<Snippet>, updated_at: Option<String> }`; `recent_rows(&[SidebarGroup], usize)`, `merge_search(&[SidebarGroup], &str, &[SearchHit]) -> Vec<SearchRow>`; `Shortcut::OpenSearch` (⌘K); on `Workspace`: `search_open()`, `open_search()`, `close_search()`, `search_query()`, `set_search_query(String)`, `search_rows() -> Vec<SearchRow>`, `open_search_row(&SearchRow)`.

- [ ] **Step 1: Write the failing tests for the rows**

Create `crates/forge-features/src/search.rs` with only tests:

```rust
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use forge_application::LiveSessionId;
    use forge_domain::{ProjectSummary, SearchHit, SessionId, SessionSummary, Snippet};
    use forge_entities::SessionMarker;

    use super::*;
    use crate::sidebar::{EntryKey, SidebarEntry, SidebarGroup};

    fn entry(id: &str, title: &str, updated: Option<&str>) -> SidebarEntry {
        SidebarEntry {
            key: EntryKey::Past { project: PathBuf::from("/a"), session: SessionId(id.to_string()) },
            session_id: Some(SessionId(id.to_string())),
            title: title.to_string(),
            updated_at: updated.map(str::to_string),
            created_at: None,
            marker: SessionMarker::Idle,
        }
    }

    fn groups(entries: Vec<SidebarEntry>) -> Vec<SidebarGroup> {
        vec![SidebarGroup {
            project: ProjectSummary {
                path: PathBuf::from("/a"),
                last_active: String::new(),
                session_count: entries.len(),
                exists: true,
            },
            entries,
        }]
    }

    fn hit(id: &str, title: &str, updated: &str, snippet: Option<&str>) -> SearchHit {
        SearchHit {
            project: PathBuf::from("/elsewhere"),
            session: SessionSummary {
                id: SessionId(id.to_string()),
                title: Some(title.to_string()),
                updated_at: updated.to_string(),
                created_at: updated.to_string(),
            },
            snippet: snippet.map(|matched| Snippet {
                before: String::new(),
                matched: matched.to_string(),
                after: String::new(),
            }),
        }
    }

    fn titles(rows: &[SearchRow]) -> Vec<&str> {
        rows.iter().map(|row| row.title.as_str()).collect()
    }

    #[test]
    fn a_title_match_is_a_row_without_a_quote() {
        let sidebar = groups(vec![
            entry("s1", "Debug and identify error cause", Some("2026-10-01T00:00:00Z")),
            entry("s2", "Report writing", Some("2026-10-02T00:00:00Z")),
        ]);

        let rows = merge_search(&sidebar, "ERROR", &[]);

        assert_eq!(titles(&rows), ["Debug and identify error cause"]);
        assert_eq!(rows[0].snippet, None);
        assert_eq!(rows[0].project.path, PathBuf::from("/a"));
    }

    #[test]
    fn a_body_hit_on_a_listed_session_adds_its_quote_to_that_row_once() {
        let sidebar = groups(vec![entry("s1", "Error cause", Some("2026-10-01T00:00:00Z"))]);
        let hits = vec![hit("s1", "Error cause", "2026-10-01T00:00:00Z", Some("error"))];

        let rows = merge_search(&sidebar, "error", &hits);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].snippet.as_ref().unwrap().matched, "error");
    }

    #[test]
    fn a_body_hit_whose_title_does_not_match_still_gets_a_row_from_the_sidebar_entry() {
        let sidebar = groups(vec![entry("s1", "Untitled", Some("2026-10-01T00:00:00Z"))]);
        let hits = vec![hit("s1", "Untitled", "2026-10-01T00:00:00Z", Some("borrow"))];

        let rows = merge_search(&sidebar, "borrow", &hits);

        assert_eq!(titles(&rows), ["Untitled"]);
        assert_eq!(rows[0].key, EntryKey::Past { project: PathBuf::from("/a"), session: SessionId("s1".to_string()) });
    }

    #[test]
    fn a_hit_for_an_open_session_opens_the_open_session() {
        let mut open = entry("s1", "Chat", Some("2026-10-01T00:00:00Z"));
        open.key = EntryKey::Live(LiveSessionId(4));
        let sidebar = groups(vec![open]);
        let hits = vec![hit("s1", "Chat", "2026-10-01T00:00:00Z", Some("needle"))];

        let rows = merge_search(&sidebar, "needle", &hits);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, EntryKey::Live(LiveSessionId(4)));
    }

    #[test]
    fn a_hit_the_sidebar_has_not_listed_yet_becomes_a_row_in_the_hits_own_project() {
        let sidebar = groups(vec![]);
        let hits = vec![hit("fresh", "Fresh", "2026-10-03T00:00:00Z", Some("needle"))];

        let rows = merge_search(&sidebar, "needle", &hits);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].project.path, PathBuf::from("/elsewhere"));
        assert_eq!(
            rows[0].key,
            EntryKey::Past { project: PathBuf::from("/elsewhere"), session: SessionId("fresh".to_string()) }
        );
    }

    #[test]
    fn rows_come_newest_first_and_an_unsaved_session_first_of_all() {
        let mut unsaved = entry("u", "needle unsaved", None);
        unsaved.session_id = None;
        unsaved.key = EntryKey::Live(LiveSessionId(9));
        let sidebar = groups(vec![
            entry("old", "needle old", Some("2026-10-01T00:00:00Z")),
            entry("new", "needle new", Some("2026-10-03T00:00:00Z")),
            unsaved,
        ]);

        let rows = merge_search(&sidebar, "needle", &[]);

        assert_eq!(titles(&rows), ["needle unsaved", "needle new", "needle old"]);
    }

    #[test]
    fn korean_titles_match() {
        let sidebar = groups(vec![entry("s1", "최적의 로컬 LLM 모델", Some("2026-10-01T00:00:00Z"))]);

        assert_eq!(merge_search(&sidebar, "로컬", &[]).len(), 1);
    }

    #[test]
    fn an_empty_query_lists_the_most_recent_sessions() {
        let sidebar = groups(vec![
            entry("a", "one", Some("2026-10-01T00:00:00Z")),
            entry("b", "two", Some("2026-10-03T00:00:00Z")),
            entry("c", "three", Some("2026-10-02T00:00:00Z")),
        ]);

        let rows = recent_rows(&sidebar, 2);

        assert_eq!(titles(&rows), ["two", "three"]);
    }
}
```

- [ ] **Step 2: Run to see them fail**

Add `mod search;` to `lib.rs`. Run: `cargo test -p forge-features search`
Expected: compile errors.

- [ ] **Step 3: Implement the rows**

Above the tests in `crates/forge-features/src/search.rs`:

```rust
use forge_domain::{title_matches, ProjectSummary, SearchHit, Snippet};

use crate::arrange::{instant, newest_first};
use crate::sidebar::{EntryKey, SidebarEntry, SidebarGroup};

/// How many sessions the palette lists before anything is typed.
pub const RECENT_LIMIT: usize = 12;

/// One line of the search palette.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchRow {
    /// What opens it: the open session if it is open, else the past session.
    pub key: EntryKey,
    pub project: ProjectSummary,
    pub title: String,
    pub snippet: Option<Snippet>,
    pub updated_at: Option<String>,
}

fn row_of(project: &ProjectSummary, entry: &SidebarEntry, snippet: Option<Snippet>) -> SearchRow {
    SearchRow {
        key: entry.key.clone(),
        project: project.clone(),
        title: entry.title.clone(),
        snippet,
        updated_at: entry.updated_at.clone(),
    }
}

fn sort_rows(rows: &mut [SearchRow]) {
    rows.sort_by_key(|row| newest_first(instant(row.updated_at.as_deref())));
}

/// The newest sessions, for the palette before a query is typed.
pub fn recent_rows(groups: &[SidebarGroup], limit: usize) -> Vec<SearchRow> {
    let mut rows: Vec<SearchRow> = groups
        .iter()
        .flat_map(|group| group.entries.iter().map(|entry| row_of(&group.project, entry, None)))
        .collect();
    sort_rows(&mut rows);
    rows.truncate(limit);
    rows
}

/// The palette's rows for `query`: every listed session whose title matches, plus the sessions the
/// transcript search found (`hits`), each session once, newest first. A hit of a session the
/// sidebar lists adds its quote to that session's row; a hit the sidebar has not listed yet (a
/// session that appeared since the last reload) gets a row of its own in the folder it names.
pub fn merge_search(groups: &[SidebarGroup], query: &str, hits: &[SearchHit]) -> Vec<SearchRow> {
    let mut rows: Vec<SearchRow> = groups
        .iter()
        .flat_map(|group| {
            group
                .entries
                .iter()
                .filter(|entry| title_matches(&entry.title, query))
                .map(|entry| row_of(&group.project, entry, None))
        })
        .collect();

    for hit in hits {
        let listed = groups.iter().find_map(|group| {
            group
                .entries
                .iter()
                .find(|entry| entry.session_id.as_ref() == Some(&hit.session.id))
                .map(|entry| (&group.project, entry))
        });
        let row = match listed {
            Some((project, entry)) => row_of(project, entry, hit.snippet.clone()),
            None => SearchRow {
                key: EntryKey::Past { project: hit.project.clone(), session: hit.session.id.clone() },
                project: ProjectSummary {
                    path: hit.project.clone(),
                    last_active: hit.session.updated_at.clone(),
                    session_count: 0,
                    exists: true,
                },
                title: hit.session.title.clone().unwrap_or_else(|| "Untitled".to_string()),
                snippet: hit.snippet.clone(),
                updated_at: Some(hit.session.updated_at.clone()),
            },
        };
        match rows.iter_mut().find(|known| known.key.dom_key() == row.key.dom_key()) {
            Some(known) => {
                if known.snippet.is_none() {
                    known.snippet = row.snippet;
                }
            }
            None => rows.push(row),
        }
    }
    sort_rows(&mut rows);
    rows
}
```

Run: `cargo test -p forge-features search` → 8 pass.

- [ ] **Step 4: Write the failing test for ⌘K**

In `crates/forge-features/src/shortcut.rs` tests add:

```rust
    #[test]
    fn cmd_k_opens_the_search_on_a_mac_and_ctrl_k_elsewhere() {
        assert_eq!(shortcut_for("k", false, true, false, true), Some(Shortcut::OpenSearch));
        assert_eq!(shortcut_for("K", false, true, false, true), Some(Shortcut::OpenSearch), "caps lock");
        assert_eq!(shortcut_for("k", true, false, false, false), Some(Shortcut::OpenSearch));
        assert_eq!(shortcut_for("k", true, false, false, true), None, "Ctrl+K on a Mac");
        assert_eq!(shortcut_for("k", false, true, true, true), None, "Cmd+Shift+K");
    }
```

and extend `while_the_overlay_is_open_only_opening_settings_is_allowed`'s array with `Shortcut::OpenSearch`.

- [ ] **Step 5: Implement ⌘K**

In `shortcut.rs` add `OpenSearch,` to `enum Shortcut`, `"k" => Some(Shortcut::OpenSearch),` to the match in `shortcut_for`, and update the doc comment to mention Cmd+K. In `Workspace::on_key` (`workspace.rs`) add the arm `Shortcut::OpenSearch => self.open_search(),` next to `NewSession`.

- [ ] **Step 6: Add the palette state to `Workspace`**

In `crates/forge-features/Cargo.toml` add `tokio = { version = "1", features = ["time"] }` under `[dependencies]`.

In `workspace.rs`: add imports `use forge_domain::SearchHit;` (extend the existing list), `use std::time::Duration;`, `use crate::search::{merge_search, recent_rows, SearchRow, RECENT_LIMIT};`. Add constants and fields:

```rust
// Typing waits this long before the transcripts are scanned.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(200);
const SEARCH_LIMIT: usize = 50;
```

Fields in `struct Workspace`: `search_open: Signal<bool>, search_query: Signal<String>, search_hits: Signal<Vec<SearchHit>>, search_task: Signal<Option<dioxus::core::Task>>,` and in the provider `search_open: Signal::new(false), search_query: Signal::new(String::new()), search_hits: Signal::new(Vec::new()), search_task: Signal::new(None),`.

Methods in `impl Workspace`:

```rust
    pub fn search_open(&self) -> bool {
        *self.search_open.read()
    }

    /// Opens the palette empty, listing the newest sessions.
    pub fn open_search(&self) {
        let mut open = self.search_open;
        let mut query = self.search_query;
        let mut hits = self.search_hits;
        query.set(String::new());
        hits.set(Vec::new());
        open.set(true);
    }

    pub fn close_search(&self) {
        self.cancel_search();
        let mut open = self.search_open;
        open.set(false);
    }

    pub fn search_query(&self) -> String {
        self.search_query.read().clone()
    }

    /// Sets the query. The transcripts are scanned after a short pause, and typing again cancels a
    /// scan that is still going, so only the answer to the latest query is ever shown.
    pub fn set_search_query(&self, text: String) {
        self.cancel_search();
        let mut query = self.search_query;
        let mut hits = self.search_hits;
        query.set(text.clone());
        // Quotes of an older query would be wrong for this one.
        hits.set(Vec::new());
        if text.trim().is_empty() {
            return;
        }
        let this = *self;
        let services = (*self.services.peek()).clone();
        let task = spawn(async move {
            tokio::time::sleep(SEARCH_DEBOUNCE).await;
            match services.sessions.search_sessions(&text, SEARCH_LIMIT).await {
                Ok(found) => hits.set(found),
                Err(error) => this.report(error.to_string()),
            }
        });
        let mut slot = self.search_task;
        slot.set(Some(task));
    }

    fn cancel_search(&self) {
        let mut slot = self.search_task;
        if let Some(task) = slot.write().take() {
            task.cancel();
        }
    }

    /// The palette's rows: the newest sessions for an empty query, else the matches.
    pub fn search_rows(&self) -> Vec<SearchRow> {
        let query = self.search_query();
        if query.trim().is_empty() {
            recent_rows(&self.sidebar(), RECENT_LIMIT)
        } else {
            merge_search(&self.sidebar(), &query, &self.search_hits.read())
        }
    }

    /// Opens the session a row stands for and closes the palette.
    pub fn open_search_row(&self, row: &SearchRow) {
        match &row.key {
            EntryKey::Live(id) => self.select(*id),
            EntryKey::Past { session, .. } => self.open_past(row.project.clone(), session.clone()),
        }
        self.close_search();
    }
```

Import `EntryKey` too: `use crate::sidebar::{cycle_target, merge_sidebar, replace_project_sessions, EntryKey, LiveSummary, SidebarGroup};`.

In `lib.rs` add:

```rust
pub use search::{merge_search, recent_rows, SearchRow, RECENT_LIMIT};
```

- [ ] **Step 7: Build and test**

Run: `cargo build -p forge-features && cargo test -p forge-features`
Expected: builds, all tests pass. If `dioxus::core::Task` is not at that path, find where `spawn` returns its type (`cargo doc` or the compiler's message) and use that path; the field's behaviour stays.

- [ ] **Step 8: Commit**

```bash
git add -A crates
git commit -m "feat: the palette's rows and state, and ⌘K to open it" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The sidebar draws the arranged groups, the filter menu, collapse, hide and resize

**Files:**
- Modify: `crates/forge-widgets/src/sidebar.rs`
- Create: `crates/forge-widgets/src/filter_menu.rs`
- Modify: `crates/forge-widgets/src/session_row.rs`, `crates/forge-widgets/src/lib.rs`
- Modify: `crates/forge-pages/src/workspace_page.rs`
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`

**Interfaces:**
- Consumes: `Workspace::{prefs, update_prefs, change_prefs_unsaved, save_prefs, arranged_sidebar, open_search}` (Tasks 4, 7); `ArrangedGroup`, `Heading`, `FilterMenu` and friends (Tasks 3, 5).
- Produces: `SidebarFilterMenu` component; `SessionRow` gets an optional prop `project_name: Option<String>`; `Sidebar` draws the arranged groups; `WorkspacePage` draws the resize handle and the hidden-sidebar rail.

This task has no unit tests of its own (the widgets are drawn code); the pure parts are tested in Tasks 3 and 5, and Task 10 checks the result in the running app. The theme test below guards the class names.

- [ ] **Step 1: Add the CSS classes to the theme test, watch it fail**

In `crates/forge-shared/src/theme.rs`, in the list of classes in `the_stylesheet_defines_the_layout_classes_the_components_use`, add:

```rust
            ".sidebar-top",
            ".sidebar-search",
            ".icon-button",
            ".filter-wrap",
            ".filter-menu",
            ".filter-submenu",
            ".menu-backdrop",
            ".group-toggle",
            ".sidebar-resize",
            ".sidebar-rail",
```

Run: `cargo test -p forge-shared`
Expected: fails, `theme.css is missing .sidebar-top`.

- [ ] **Step 2: Add the CSS**

Append to `crates/forge-shared/src/theme.css`:

```css
.sidebar-top { display: flex; align-items: center; gap: 4px; margin: 0 4px 8px; }
.sidebar-search { flex: 1; min-width: 0; display: flex; align-items: center; gap: 8px; padding: 6px 10px; border: 1px solid var(--border); border-radius: 8px; background: var(--bg-card); color: var(--fg-muted); font: inherit; text-align: left; cursor: pointer; }
.sidebar-search:hover { border-color: var(--fg-muted); }
.sidebar-search .keys { margin-left: auto; font-size: 12px; }
.icon-button { flex: none; width: 28px; height: 28px; border: none; border-radius: 6px; background: transparent; color: var(--fg-muted); font: inherit; font-size: 15px; line-height: 1; cursor: pointer; }
.icon-button:hover, .icon-button.open { background: var(--bg-hover); color: var(--fg); }
.sidebar-new { display: flex; align-items: center; gap: 4px; }
.sidebar-new .button { flex: 1; width: auto; }
.filter-wrap { position: relative; flex: none; }
.filter-menu { left: auto; right: 0; min-width: 190px; z-index: 6; }
.filter-submenu { position: absolute; left: 100%; top: 0; margin-left: 4px; min-width: 130px; padding: 4px; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-card); box-shadow: var(--shadow); }
.row-menu-item.highlight { background: var(--bg-hover); }
.row-menu-item .value { margin-left: auto; color: var(--fg-muted); font-size: 12px; }
.menu-backdrop { position: fixed; inset: 0; z-index: 5; }
.group-toggle { flex: 1; min-width: 0; display: flex; align-items: center; gap: 6px; padding: 0; border: none; background: transparent; color: inherit; font: inherit; text-align: left; cursor: pointer; }
.group-toggle .chevron { flex: none; width: 10px; font-size: 10px; color: var(--fg-muted); }
.sidebar-resize { flex: none; width: 5px; margin-left: -3px; cursor: col-resize; background: transparent; z-index: 2; }
.sidebar-resize:hover, .sidebar-resize.dragging { background: var(--accent-soft); }
.sidebar-rail { flex: none; width: 36px; padding: 12px 4px 0; box-sizing: border-box; background: var(--bg-sidebar); border-right: 1px solid var(--border); }
.workspace.resizing { cursor: col-resize; user-select: none; }
```

Run: `cargo test -p forge-shared` → passes.

- [ ] **Step 3: Give `SessionRow` an optional project name**

In `crates/forge-widgets/src/session_row.rs`, change the component signature to:

```rust
pub fn SessionRow(
    project: PathBuf,
    entry: SidebarEntry,
    active: bool,
    onopen: EventHandler<()>,
    /// Set in date and no-grouping lists, where the heading does not say which project a row is in.
    #[props(default)]
    project_name: Option<String>,
) -> Element {
```

and replace the `subtitle:` expression in the final `SidebarItem` with:

```rust
                        subtitle: row_subtitle(
                            project_name.as_deref(),
                            entry.updated_at.as_deref().map(|updated_at| relative_time(updated_at, now)),
                        ),
```

Add the function and its tests to the same file:

```rust
/// The grey text at the right of a row: how long ago, and in a date list also the project.
pub fn row_subtitle(project_name: Option<&str>, ago: Option<String>) -> String {
    match (project_name, ago.filter(|ago| !ago.is_empty())) {
        (Some(name), Some(ago)) => format!("{name} · {ago}"),
        (Some(name), None) => name.to_string(),
        (None, Some(ago)) => ago,
        (None, None) => String::new(),
    }
}
```

```rust
    #[test]
    fn a_row_subtitle_joins_the_project_and_the_time() {
        assert_eq!(row_subtitle(None, Some("3d ago".to_string())), "3d ago");
        assert_eq!(row_subtitle(Some("forge"), Some("3d ago".to_string())), "forge · 3d ago");
        assert_eq!(row_subtitle(Some("forge"), None), "forge");
        assert_eq!(row_subtitle(None, None), "");
    }
```

Run: `cargo test -p forge-widgets row_subtitle` → passes.

- [ ] **Step 4: The filter menu widget**

Create `crates/forge-widgets/src/filter_menu.rs`:

```rust
use dioxus::prelude::*;
use forge_features::{
    current_index, filter_key, group_label, refocus_workspace, sort_label, use_workspace, FilterChoice, FilterMenu,
    FilterOutcome, FilterRow, FILTER_ROWS, GROUP_ITEMS, SORT_ITEMS,
};

/// The icon beside "+ New session" and the menu it opens: sort by, group by, show empty groups.
/// The menu takes focus when it opens; arrows move, Right opens a submenu, Left or Escape closes
/// it, Enter chooses (the keys are the physical ones, so a Korean input method does not matter).
#[component]
pub fn SidebarFilterMenu() -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let mut menu = use_signal(FilterMenu::default);
    let prefs = workspace.prefs();
    let state = menu();

    let sub_items: Vec<(String, bool, FilterChoice)> = match FILTER_ROWS[state.top] {
        FilterRow::Sort => SORT_ITEMS
            .iter()
            .map(|item| (sort_label(*item).to_string(), prefs.sort_by == *item, FilterChoice::Sort(*item)))
            .collect(),
        FilterRow::Group => GROUP_ITEMS
            .iter()
            .map(|item| (group_label(*item).to_string(), prefs.group_by == *item, FilterChoice::Group(*item)))
            .collect(),
        FilterRow::ShowEmpty => Vec::new(),
    };

    rsx! {
        div { class: "filter-wrap",
            button {
                class: if open() { "icon-button open" } else { "icon-button" },
                title: "Sort, group and filter sessions",
                onclick: move |_| {
                    menu.set(FilterMenu::default());
                    open.set(!open());
                },
                "⇅"
            }
            if open() {
                div {
                    class: "menu-backdrop",
                    onclick: move |_| {
                        open.set(false);
                        refocus_workspace();
                    },
                }
                div {
                    class: "row-menu filter-menu",
                    tabindex: "0",
                    onclick: move |event| event.stop_propagation(),
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        let modified = event
                            .modifiers()
                            .intersects(Modifiers::CONTROL | Modifiers::META | Modifiers::ALT);
                        let Some(key) = filter_key(&event.code().to_string()).filter(|_| !modified) else {
                            return;
                        };
                        event.stop_propagation();
                        event.prevent_default();
                        let mut next = menu();
                        let outcome = next.key(key, &workspace.prefs());
                        menu.set(next);
                        match outcome {
                            FilterOutcome::Stay => {}
                            FilterOutcome::Choose(choice) => {
                                workspace.update_prefs(|prefs| choice.apply(prefs));
                                open.set(false);
                                refocus_workspace();
                            }
                            FilterOutcome::Close => {
                                open.set(false);
                                refocus_workspace();
                            }
                        }
                    },
                    for (index, row) in FILTER_ROWS.iter().enumerate() {
                        button {
                            key: "{index}",
                            class: if state.top == index { "row-menu-item highlight" } else { "row-menu-item" },
                            onclick: {
                                let row = *row;
                                move |_| {
                                    if row == FilterRow::ShowEmpty {
                                        workspace.update_prefs(|prefs| FilterChoice::ToggleShowEmpty.apply(prefs));
                                        open.set(false);
                                        refocus_workspace();
                                    } else {
                                        menu.set(FilterMenu { top: index, sub: Some(current_index(row, &workspace.prefs())) });
                                    }
                                }
                            },
                            match row {
                                FilterRow::Sort => rsx! {
                                    span { "Sort by" }
                                    span { class: "value", "{sort_label(prefs.sort_by)} ›" }
                                },
                                FilterRow::Group => rsx! {
                                    span { "Group by" }
                                    span { class: "value", "{group_label(prefs.group_by)} ›" }
                                },
                                FilterRow::ShowEmpty => rsx! {
                                    span { "Show empty groups" }
                                    span { class: "value", if prefs.show_empty { "✓" } }
                                },
                            }
                        }
                    }
                    if let Some(highlighted) = state.sub {
                        div { class: "filter-submenu",
                            for (index, (label, current, choice)) in sub_items.into_iter().enumerate() {
                                button {
                                    key: "{index}",
                                    class: if highlighted == index { "row-menu-item highlight" } else { "row-menu-item" },
                                    onclick: move |_| {
                                        workspace.update_prefs(|prefs| choice.apply(prefs));
                                        open.set(false);
                                        refocus_workspace();
                                    },
                                    span { "{label}" }
                                    span { class: "value", if current { "✓" } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

In `crates/forge-widgets/src/lib.rs` add `mod filter_menu;` and `pub use filter_menu::SidebarFilterMenu;`.

- [ ] **Step 5: Rewrite the `Sidebar` component**

In `crates/forge-widgets/src/sidebar.rs` change the imports to:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use forge_features::{use_settings, use_workspace, Active, EntryKey, Heading};
use forge_shared::{Button, ButtonKind};
use time::OffsetDateTime;

use crate::filter_menu::SidebarFilterMenu;
use crate::session_row::SessionRow;
```

Keep `project_label`, `ancestor_name` and `project_labels` and all their tests as they are. Replace the `Sidebar` component (everything from its doc comment down to the end of the component, before `#[cfg(test)]`) with:

```rust
/// Search and filter on top, then the sessions grouped as the preferences say, then Settings.
#[component]
pub fn Sidebar() -> Element {
    let workspace = use_workspace();
    let settings = use_settings();
    let prefs = workspace.prefs();
    let groups = workspace.arranged_sidebar(OffsetDateTime::now_utc());
    // Headings of folder groups are told apart when two folders share a name; rows in a date
    // list show their folder's plain name.
    let paths: Vec<PathBuf> = groups
        .iter()
        .filter_map(|group| match &group.heading {
            Heading::Project(project) => Some(project.path.clone()),
            _ => None,
        })
        .collect();
    let labels: HashMap<PathBuf, String> = paths.iter().cloned().zip(project_labels(&paths)).collect();
    let active = workspace.active();
    let notice = workspace.notice();

    rsx! {
        div { class: "sidebar", style: "width: {prefs.clamped_width()}px",
            div { class: "sidebar-top",
                button {
                    class: "sidebar-search",
                    title: "Search sessions (⌘K)",
                    onclick: move |_| workspace.open_search(),
                    span { "⌕" }
                    span { "Search" }
                    span { class: "keys", "⌘K" }
                }
                button {
                    class: "icon-button",
                    title: "Hide the sidebar",
                    onclick: move |_| workspace.update_prefs(|prefs| prefs.hidden = true),
                    "‹"
                }
            }
            div { class: "sidebar-new",
                Button {
                    label: "+ New session",
                    kind: ButtonKind::Primary,
                    onclick: move |_| workspace.show_new_session(),
                }
                SidebarFilterMenu {}
            }
            if let Some(message) = notice {
                div { class: "banner error",
                    "{message} "
                    Button { label: "Dismiss", onclick: move |_| workspace.dismiss_notice() }
                }
            }
            for group in groups {
                div { key: "{group.id}",
                    match &group.heading {
                        Heading::None => rsx! {},
                        Heading::Bucket(bucket) => rsx! {
                            div { class: "sidebar-group-header",
                                {group_toggle(workspace, &group.id, group.collapsed, bucket.label().to_string(), String::new(), false)}
                            }
                        },
                        Heading::Project(project) => rsx! {
                            div { class: "sidebar-group-header",
                                {group_toggle(
                                    workspace,
                                    &group.id,
                                    group.collapsed,
                                    labels.get(&project.path).cloned().unwrap_or_default(),
                                    project.path.display().to_string(),
                                    !project.exists,
                                )}
                                if project.exists {
                                    button {
                                        class: "group-add",
                                        title: "New session in this folder",
                                        onclick: {
                                            let path = project.path.clone();
                                            move |_| workspace.show_new_session_in(path.clone())
                                        },
                                        "+"
                                    }
                                }
                            }
                        },
                    }
                    for item in group.entries.iter() {
                        SessionRow {
                            key: "{item.entry.key.dom_key()}",
                            project: item.project.path.clone(),
                            entry: item.entry.clone(),
                            project_name: match group.heading {
                                Heading::Project(_) => None,
                                _ => Some(project_label(&item.project.path)),
                            },
                            active: match &item.entry.key {
                                EntryKey::Live(id) => active == Active::Live(*id),
                                EntryKey::Past { .. } => false,
                            },
                            onopen: {
                                let project = item.project.clone();
                                let key = item.entry.key.clone();
                                move |_| match &key {
                                    EntryKey::Live(id) => workspace.select(*id),
                                    EntryKey::Past { session, .. } => {
                                        workspace.open_past(project.clone(), session.clone())
                                    }
                                }
                            },
                        }
                    }
                }
            }
            div { class: "sidebar-footer",
                button { class: "sidebar-footer-button", onclick: move |_| settings.open(),
                    span { "⚙" }
                    span { "Settings" }
                    span { class: "keys", "⌘," }
                }
            }
        }
    }
}

// A heading that collapses and expands its group when clicked.
fn group_toggle(
    workspace: forge_features::Workspace,
    id: &str,
    collapsed: bool,
    title: String,
    tooltip: String,
    missing: bool,
) -> Element {
    let id = id.to_string();
    rsx! {
        button {
            class: "group-toggle",
            title: if collapsed { "Expand" } else { "Collapse" },
            onclick: move |_| workspace.update_prefs(|prefs| prefs.toggle_collapsed(&id)),
            span { class: "chevron", if collapsed { "▸" } else { "▾" } }
            span {
                class: if missing { "sidebar-group-title missing" } else { "sidebar-group-title" },
                title: "{tooltip}",
                "{title}"
            }
        }
    }
}
```

(`Path` is still used by `project_label`; keep its import.)

- [ ] **Step 6: Resize handle and hidden rail in the page**

Replace the body of `WorkspacePage` in `crates/forge-pages/src/workspace_page.rs` with this version (keep the doc comment and the existing effects):

```rust
use dioxus::prelude::*;
use forge_features::{refocus_workspace, use_settings_provider, use_workspace_provider, Active};
use forge_widgets::{ChatWindow, NewSessionPanel, SearchPalette, SettingsOverlay, Sidebar};

/// The whole screen: the sidebar on the left and, on the right, either the new-session panel or
/// the active session. The root also catches the session shortcuts.
#[component]
pub fn WorkspacePage() -> Element {
    let settings = use_settings_provider();
    let workspace = use_workspace_provider();
    let mut dragging = use_signal(|| false);
    // Key events only reach the root while something inside it has focus. Whenever the overlay
    // closes or the main pane switches view, the focused input or button may have been removed
    // and focus falls to <body>, which silences every shortcut until the next click. Give focus
    // back to the root — but only if nothing has it, so a composer the person is typing in keeps it.
    use_effect(move || {
        let _ = (settings.is_open(), workspace.search_open(), workspace.active());
        refocus_workspace();
    });

    // The tabs list the definitions of the project the person is looking at.
    use_effect(move || settings.set_project(workspace.context_project()));

    let hidden = workspace.prefs().hidden;

    rsx! {
        div {
            class: if dragging() { "workspace resizing" } else { "workspace" },
            tabindex: "0",
            // Key events only reach the root while something inside it has focus.
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onkeydown: move |event| {
                // The palette is an overlay too: the session shortcuts wait while it is open.
                if !workspace.on_key(&event, settings.is_open() || workspace.search_open()) {
                    settings.on_key(&event);
                }
            },
            // The sidebar's width follows the pointer while the handle is held; it is saved once, on release.
            onmousemove: move |event| {
                if dragging() {
                    let x = event.client_coordinates().x.max(0.0) as u32;
                    workspace.change_prefs_unsaved(|prefs| prefs.width = x.clamp(forge_domain::MIN_SIDEBAR_WIDTH, forge_domain::MAX_SIDEBAR_WIDTH));
                }
            },
            onmouseup: move |_| {
                if dragging() {
                    dragging.set(false);
                    workspace.save_prefs();
                }
            },
            if hidden {
                div { class: "sidebar-rail",
                    button {
                        class: "icon-button",
                        title: "Show the sidebar",
                        onclick: move |_| workspace.update_prefs(|prefs| prefs.hidden = false),
                        "›"
                    }
                }
            } else {
                Sidebar {}
                div {
                    class: if dragging() { "sidebar-resize dragging" } else { "sidebar-resize" },
                    onmousedown: move |event| {
                        event.prevent_default();
                        dragging.set(true);
                    },
                }
            }
            match workspace.active() {
                Active::NewSession => rsx! { NewSessionPanel {} },
                Active::Live(id) => rsx! { ChatWindow { key: "{id:?}", id } },
            }
            SettingsOverlay {}
            SearchPalette {}
        }
    }
}
```

`forge-pages` needs `forge-domain` for the two constants: add `forge-domain = { path = "../forge-domain" }` to `crates/forge-pages/Cargo.toml`.

`SearchPalette` is written in Task 9, so this task cannot build alone: do Task 9 Steps 1 to 3 next, then come back for Steps 7 and 8.

- [ ] **Step 7: (after Task 9, Step 3) Build the workspace**

Run: `cargo build --workspace`
Expected: builds. Likely fixes: an unused import, `event.client_coordinates()` (if the method is named differently in this Dioxus, use the mouse event's `page_coordinates`), `Workspace` not exported under `forge_features::Workspace` (it is: `pub use workspace::{..., Workspace, ...}`).

- [ ] **Step 8: Commit**

```bash
git add -A crates
git commit -m "feat: the sidebar draws sorted, grouped and collapsible groups with a filter menu, and can be hidden and resized" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The search palette

**Files:**
- Create: `crates/forge-widgets/src/search_palette.rs`
- Modify: `crates/forge-widgets/src/lib.rs`
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`

**Interfaces:**
- Consumes: `Workspace::{search_open, search_query, set_search_query, search_rows, open_search_row, close_search}` and `SearchRow` (Task 7); `split_at_match` (`forge_domain`); `relative_time` (`forge_entities`); `project_label` (`crate::sidebar`).
- Produces: `SearchPalette` component (draws nothing while closed).

- [ ] **Step 1: Add the class names to the theme test, watch it fail**

In `crates/forge-shared/src/theme.rs` add to the class list: `".palette-backdrop"`, `".palette"`, `".palette-input"`, `".palette-row"`, `".palette-snippet"`, `".palette-empty"`. Run `cargo test -p forge-shared` → fails.

- [ ] **Step 2: Add the CSS**

Append to `crates/forge-shared/src/theme.css`:

```css
.palette-backdrop { position: fixed; inset: 0; z-index: 20; display: flex; justify-content: center; align-items: flex-start; padding-top: 12vh; background: var(--bg-overlay); }
.palette { width: min(640px, calc(100vw - 48px)); max-height: 70vh; display: flex; flex-direction: column; background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius-lg); box-shadow: var(--shadow); overflow: hidden; }
.palette-input-row { display: flex; align-items: center; gap: 8px; padding: 12px 14px; border-bottom: 1px solid var(--border); }
.palette-input { flex: 1; min-width: 0; border: none; outline: none; background: transparent; color: var(--fg); font: inherit; font-size: 16px; }
.palette-close { flex: none; width: 26px; height: 26px; border: none; border-radius: 6px; background: transparent; color: var(--fg-muted); font: inherit; font-size: 18px; line-height: 1; cursor: pointer; }
.palette-close:hover { background: var(--bg-hover); color: var(--fg); }
.palette-results { overflow-y: auto; padding: 6px; }
.palette-row { display: grid; grid-template-columns: 1fr auto; gap: 2px 12px; padding: 8px 10px; border-radius: 8px; cursor: pointer; }
.palette-row.selected { background: var(--bg-hover); }
.palette-title { min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.palette-title b, .palette-snippet b { font-weight: 700; color: var(--fg); }
.palette-project { margin-left: 8px; font-size: 12px; color: var(--fg-muted); }
.palette-date { grid-row: 1; grid-column: 2; font-size: 11px; color: var(--fg-muted); white-space: nowrap; }
.palette-snippet { grid-column: 1 / -1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-size: 12px; color: var(--fg-muted); }
.palette-empty { padding: 16px; text-align: center; color: var(--fg-muted); }
```

Run: `cargo test -p forge-shared` → passes.

- [ ] **Step 3: Write the palette**

Create `crates/forge-widgets/src/search_palette.rs`:

```rust
use dioxus::prelude::*;
use forge_domain::split_at_match;
use forge_entities::relative_time;
use forge_features::use_workspace;
use time::OffsetDateTime;

use crate::sidebar::project_label;

/// The search palette: a box over the dimmed window that lists the sessions whose title or message
/// text contains what is typed. Up and Down move, Enter opens, Escape or a click outside closes.
/// It draws nothing while closed; the body is a separate component so its highlight starts at the
/// first row every time it opens.
#[component]
pub fn SearchPalette() -> Element {
    let workspace = use_workspace();
    if !workspace.search_open() {
        return rsx! {};
    }
    rsx! { PaletteBody {} }
}

#[component]
fn PaletteBody() -> Element {
    let workspace = use_workspace();
    let mut highlight = use_signal(|| 0usize);
    let query = workspace.search_query();
    let rows = workspace.search_rows();
    let count = rows.len();
    let selected = highlight().min(count.saturating_sub(1));
    let now = OffsetDateTime::now_utc();

    rsx! {
        div { class: "palette-backdrop", onclick: move |_| workspace.close_search(),
            div { class: "palette", onclick: move |event| event.stop_propagation(),
                div { class: "palette-input-row",
                    input {
                        class: "palette-input",
                        r#type: "text",
                        value: "{query}",
                        placeholder: "Search sessions",
                        spellcheck: "false",
                        onmounted: move |mounted| async move {
                            let _ = mounted.set_focus(true).await;
                        },
                        oninput: move |event| {
                            highlight.set(0);
                            workspace.set_search_query(event.value());
                        },
                        onkeydown: {
                            let rows = rows.clone();
                            move |event: KeyboardEvent| match event.key() {
                                Key::Escape => {
                                    event.stop_propagation();
                                    workspace.close_search();
                                }
                                Key::ArrowDown => {
                                    event.prevent_default();
                                    event.stop_propagation();
                                    highlight.set((selected + 1).min(count.saturating_sub(1)));
                                }
                                Key::ArrowUp => {
                                    event.prevent_default();
                                    event.stop_propagation();
                                    highlight.set(selected.saturating_sub(1));
                                }
                                // Enter that ends an IME composition confirms the text, not the row.
                                Key::Enter if !event.is_composing() => {
                                    event.prevent_default();
                                    event.stop_propagation();
                                    if let Some(row) = rows.get(selected) {
                                        workspace.open_search_row(row);
                                    }
                                }
                                _ => {}
                            }
                        },
                    }
                    button {
                        class: "palette-close",
                        title: "Close",
                        onclick: move |_| workspace.close_search(),
                        "×"
                    }
                }
                div { class: "palette-results",
                    for (index, row) in rows.iter().enumerate() {
                        div {
                            key: "{row.key.dom_key()}",
                            class: if index == selected { "palette-row selected" } else { "palette-row" },
                            onmouseenter: move |_| highlight.set(index),
                            onclick: {
                                let row = row.clone();
                                move |_| workspace.open_search_row(&row)
                            },
                            div { class: "palette-title",
                                match split_at_match(&row.title, &query) {
                                    Some((before, matched, after)) => rsx! {
                                        "{before}"
                                        b { "{matched}" }
                                        "{after}"
                                    },
                                    None => rsx! { "{row.title}" },
                                }
                                span { class: "palette-project", "{project_label(&row.project.path)}" }
                            }
                            span { class: "palette-date",
                                {row.updated_at.as_deref().map(|at| relative_time(at, now)).unwrap_or_default()}
                            }
                            if let Some(snippet) = &row.snippet {
                                div { class: "palette-snippet",
                                    "{snippet.before}"
                                    b { "{snippet.matched}" }
                                    "{snippet.after}"
                                }
                            }
                        }
                    }
                    if rows.is_empty() {
                        div { class: "palette-empty", "No sessions match" }
                    }
                }
            }
        }
    }
}
```

In `crates/forge-widgets/src/lib.rs` add `mod search_palette;` and `pub use search_palette::SearchPalette;`. `forge-widgets` needs `forge-domain`: add `forge-domain = { path = "../forge-domain" }` to its `Cargo.toml`.

- [ ] **Step 4: Build, then finish Task 8's Step 7**

Run: `cargo build --workspace`
Expected: builds (this also completes Task 8). Fix compile errors the same way as Task 8 Step 7.

- [ ] **Step 5: Run every test**

Run: `cargo test --workspace`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add -A crates
git commit -m "feat: the search palette" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Check it in the running app (harness)

**Files:**
- Read: `scripts/harness/README.md`, `scripts/harness/build_fixture.py`
- Modify (if the fixture has no message text or timestamps to search): `scripts/harness/build_fixture.py`, `scripts/harness/test_build_fixture.py`

The harness runs the real window on a fake Claude home; nothing here touches the real `~/.claude` or the real Forge config. No message is sent. Typing uses `osascript` `key code` for single keys and `keystroke` only for plain ASCII with an English input method.

- [ ] **Step 1: Check the fixture can show the features**

Read `build_fixture.py`. The fixture needs, for the checks below: sessions in at least two projects; at least one session whose first user message contains the word `refactor` and whose title does not; two sessions with the same title; sessions at different ages (set file times with `os.utime`: today, 3 days, 20 days, 60 days ago). If any is missing, add it to the builder and a test for it to `test_build_fixture.py`, run `python3 -m unittest discover -s scripts/harness`, and commit as `test: fixture sessions for sidebar search and grouping`.

- [ ] **Step 2: Launch and look**

Run: `scripts/harness/launch.sh` then `python3 scripts/harness/capture.py` and open `/private/tmp/forge-harness-shot.png`. Check by looking, one capture after each action:

1. Default: the sidebar has the search box, `‹`, "+ New session" with the `⇅` icon, project groups with `▾`.
2. Click a group heading: it collapses (`▸`), rows hide; click again: it expands.
3. Click `⇅`: the menu shows "Sort by Activity ›", "Group by Folder ›", "Show empty groups". Press Right (`key code 124`) then Down (`125`) then Enter (`36`): the submenu opens and a choice applies; the sidebar reorders. Do this for Group by Date (headings Today / Previous 7 days …, rows show `project · age`) and None (no headings).
4. Click the search box (or press ⌘K): the palette opens listing recent sessions. Type `refactor` (English input): the row whose body has it appears with the word in bold in the quote; ArrowDown/ArrowUp move the highlight; Enter opens it and the palette closes; Escape closes it without opening.
5. Click `‹`: the sidebar hides and the `›` rail shows; click `›`: it returns. Drag the right edge of the sidebar: the width follows and stays after a restart.
6. Quit with `scripts/harness/stop.sh`, launch again: sort, grouping, collapsed groups, width and hidden state are as left (they are in the fixture's `FORGE_CONFIG_PATH` file).

Expected: every item behaves as written. Anything that does not is a bug: fix it with a failing test first where the logic is pure, else in the widget, and re-check.

- [ ] **Step 3: Check what could not be a unit test**

- The 200 ms debounce and the cancel: type `refactor` quickly, then delete it and type `zzz`; the list must end showing "No sessions match", never the `refactor` rows.
- Korean: switch the input method to Korean, type a Korean word that is in a fixture title (add one to the fixture if there is none); the row appears; Enter during composition does not open the row (press Enter once to finish the composition, then again to open).
- `⌘K` with the settings overlay open does nothing; `⌘K` while the sidebar is hidden opens the palette.

Record the result of each check (worked / what was seen) for the as-built notes.

- [ ] **Step 4: Commit any fixture or fix changes**

```bash
git add -A scripts crates
git commit -m "fix: what the harness check of the sidebar found" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

(Skip the commit if nothing changed.)

---

### Task 11: Backlog, as-built notes and memory

**Files:**
- Modify: `docs/superpowers/roadmap/backlog.md`
- Modify: `docs/superpowers/roadmap/README.md` (the tentative plan order: Plan 17 row)
- Modify: `docs/superpowers/specs/2026-10-03-sidebar-search-groups-design.md` (an "As built" section at the end)
- Create: `/Users/james/.claude/projects/-Users-james-Developer-forge/memory/plan-17-sidebar-search.md`
- Modify: `/Users/james/.claude/projects/-Users-james-Developer-forge/memory/MEMORY.md`

- [ ] **Step 1: Update the backlog**

In `backlog.md`: set the `Seen` of SES-07 and SES-08 to `seen`, and their `Plan` to `Plan 17 (built: ...)` naming what was built and what was not (SES-07: sessions only, title and text; SES-08: sort, group by folder / date / none, empty groups, collapse; not status, environment, PR, custom groups; SES-11: collapse and resize built, back and forward not). Add a row `SES-12 | Back and forward history buttons in the title bar | seen | none | M | - | Left out of Plan 17 (SES-11); needs a navigation history model.` Mark OBS-07 as done in part: filter menu and search seen; "사이드바 편집…" and the search's Enter, `...`, "작성" mode, shortcut, empty view still unseen. Add a row for what the search result `...` offers if it was never seen (`OBS-08`).

- [ ] **Step 2: Write the as-built section of the spec**

Append `## As built (<date>)` to the spec: what was built; the five differences listed at the top of this plan; every item of "Not verified in the real desktop app" with what was chosen; the harness results from Task 10 (worked, or what was seen and fixed); what was not checked (dark mode, IME beyond the one check, a very large `~/.claude` — say how long a search took on the real data only if it was measured).

- [ ] **Step 3: Write the memory**

Create `plan-17-sidebar-search.md` in the memory directory with the frontmatter (`name: plan-17-sidebar-search`, a one-line `description`, `metadata: type: project`), what was built, what was left out, and the open points. Link `[[plan-14-roadmap]]` and `[[driving-the-forge-app-visually]]`. Add one line to `MEMORY.md`: `- [Plan 17: sidebar search and groups](plan-17-sidebar-search.md) — built SES-07/08/11 parts; what is left`.

- [ ] **Step 4: Final verification**

Run: `cargo test --workspace && cargo build --workspace`
Expected: all pass. Report the real counts and any failure; do not claim success without this output.

- [ ] **Step 5: Commit**

```bash
git add -A docs
git commit -m "docs: record Plan 17 as built, and what was not verified in the real app" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

(The memory files live outside the repository and are not committed.)
