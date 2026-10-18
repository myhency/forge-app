# Changes Panel 2A: Menus and State Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Changes panel gets `File list position ›`, `Separate test, build and generated files`, a `Commits N ›` list in the branch menu, no `Uncommitted changes` row on a clean tree, and a chosen view and base that survive a restart.

**Architecture:** Two new view options in `ChangesViewPrefs`; a pure `file_class` function in `forge-entities` sorts a path into Normal, Test or Generated; `forge-features/change_rows.rs` builds the tree and the stack order with those groups; `GitPort` gets one read-only call, `is_dirty`; the widgets draw the sections, the position (pure CSS with a container query for `Auto`) and the commit list; the saved view and base go in `AppConfig.changes_modes`, keyed like `archived` by project path and Claude session id.

**Tech Stack:** Rust 2021, Dioxus 0.7 desktop, the `git` CLI, `tempfile` for adapter tests, CSS container queries (WKWebView on macOS, WebView2 on Windows).

**Spec:** `docs/superpowers/specs/2026-10-17-changes-panel-2-design.md` (stage 50A). Observations: `docs/superpowers/roadmap/observations.md`, rounds 19c, 19d, 20 and 20b. Plan 48 (`docs/superpowers/plans/2026-10-17-changes-panel.md`) built what this plan extends.

## Global Constraints

- `GitPort` never changes a repository: no `add`, `commit`, `checkout`, `reset`, `stash`. (Plan 48)
- Every git command goes through `GitCli::run` (10 s timeout, `GIT_LITERAL_PATHSPECS=1`, `LC_ALL=C`, `--no-optional-locks`). (Plan 48)
- Every user-visible string goes through `tr!` and has its Korean text in `crates/forge-shared/src/ko.rs`, kept sorted by English text in byte order; `cargo test -p forge-shared` checks both ways (`the_table_is_sorted_and_has_no_duplicates`, `every_sentence_in_the_sources_has_a_korean_text`, `every_korean_entry_is_used_in_the_sources`).
- The panel is a viewer: no stage, unstage, revert or commit button (Plan 49 adds the commit card).
- Old configs must still read: every new config field has a serde default. (spec "Settings")
- The split rules are exactly those of observation round 20b: Test = folder `tests`/`test`/`spec`/`specs`/`__tests__`, name `*.test.*`, `*.spec.*`, `*_test.<ext>`, `test_*.<ext>`; Generated = folder `dist`/`.next`, name `*.generated.*`, `*.min.js`, `*.min.css`, `*.js.map`, `*.css.map`, `*.lock`, `package-lock.json`, `pnpm-lock.yaml`; `build/`, `out/`, `target/` and `*.d.ts` stay Normal. Case-sensitive. (spec "Splitting special files")
- A section is closed when it first appears; the stacked diffs are NOT hidden by a closed section; the search popover lists all files. (observation round 20b)
- `Auto` file list position is the right column while the panel body is 400 px or wider and under the diffs below that (a guess chosen so the default panel width stays on the right, to be recorded as such). (spec)
- Already done in Plan 48, not part of this plan: the `Copy ›` submenu (`Absolute path`, `Relative path`, `File name`) and `Open in` disabled for a deleted file (`crates/forge-features/src/file_menu.rs`, `MenuTarget::Change { deleted: true }` offers only Copy).
- Rust style: match the surrounding code (comments explain why, short; no new crates).

## Review Focus

- A repository with no commit, a detached HEAD or a folder that is not a repository: `is_dirty` and the panel must not crash. Test: `is_dirty_reports_a_missing_repository_as_an_error` (Task 4); the panel treats an error as dirty.
- Only ignored files changed (a clean tree for git): the `Uncommitted changes` row is hidden. Test: `an_ignored_file_does_not_make_the_tree_dirty` (Task 4).
- A path that looks like a test but is not: a file named `tests` (not a folder), `contest_x.py`, `latest_test_data` with no extension, an upper-case `Tests/` folder. Test: `look_alikes_stay_normal` (Task 2).
- A changed file list where every file is Normal, or where a section would be empty: no empty `Test files 0` row. Test: `an_empty_section_has_no_row` (Task 3).
- A commit whose subject holds Korean letters or the search text in a different case: the commit filter finds it. Test: `the_commit_filter_matches_subject_sha_and_author_without_case` (Task 6).
- A saved mode whose session no longer exists, a saved commit that is gone, or two sessions of one project: each keeps its own entry; a deleted session drops its entry. Tests: `a_mode_is_kept_per_session`, `the_default_mode_leaves_no_entry` and `forgetting_a_session_drops_its_entry` (Task 7).
- An old config file with no `changes_modes` and an old `changes_view` with no new fields: reads with the defaults. Tests: `an_old_config_has_no_saved_modes`, `a_partial_changes_view_keeps_the_defaults_for_the_rest` (Tasks 1 and 7).

---

### Task 0: Branch and the documents

**Files:**
- Modify: `docs/superpowers/specs/2026-10-17-changes-panel-2-design.md`
- Modify: `docs/superpowers/roadmap/backlog.md` (PNL-02 row)

- [ ] **Step 1: Create the branch**

```bash
cd /Users/james/orca/workspaces/forge/features
git status --short   # expect clean
git switch -c plan-50
```

- [ ] **Step 2: Correct the spec for what Plan 48 already built**

In the spec, section "50A: menus and state", sub-section "File row menu", replace its whole body with:

```markdown
### File row menu

Nothing to build: `Copy ›` already offers `Absolute path`, `Relative path` and `File name`, and a deleted file's menu already offers only `Copy ›` (`file_menu(MenuTarget::Change { deleted: true })`). Plan 48 listed both as left out before the desktop app's submenu was seen; round 20 showed the same three items.
```

In the "Success" list of the spec delete the bullet `` `Copy ›` offers Absolute path, Relative path, File name. `` and in "Not built after this plan" delete `` `Open in` for a deleted file beyond disabling it, ``. Add under "Decisions made with the user": `- 2026-10-17, while planning 50A: \`Copy ›\` and the deleted file's \`Open in\` were already built in Plan 48; they leave the scope.`

- [ ] **Step 3: Backlog row**

In `docs/superpowers/roadmap/backlog.md`, row `PNL-02`, change the `Plan` cell `Plan 21B (built); Plan 48 (built)` to `Plan 21B (built); Plan 48 (built); Plan 50 (planned: 50A menus and state, 50B diff comments, 50C look and minors)`.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/specs/2026-10-17-changes-panel-2-design.md docs/superpowers/roadmap/backlog.md docs/superpowers/plans/2026-10-17-changes-panel-2a.md
git commit -m "docs: Plan 50A plan; Copy and deleted Open in were already built

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 1: The two view options in the config

**Files:**
- Modify: `crates/forge-domain/src/config.rs` (struct near line 148, `choice_enum` macro stays; tests near line 686)
- Modify: `crates/forge-domain/src/lib.rs` (the `pub use config::{...}` list at line 35)

**Interfaces:**
- Produces: `forge_domain::FileListPosition { Auto, Left, Right }` (`Copy`, `Default = Auto`, serde `snake_case`); `ChangesViewPrefs.file_list_position: FileListPosition`; `ChangesViewPrefs.separate_special: bool` (default `false`).

- [ ] **Step 1: Write the failing tests**

In the `tests` module of `config.rs`, replace `changes_view_defaults_and_round_trips` and add one test:

```rust
    #[test]
    fn changes_view_defaults_and_round_trips() {
        let config: AppConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(
            config.changes_view,
            ChangesViewPrefs {
                group_by_folder: true,
                side_by_side: false,
                wrap: true,
                highlight_words: true,
                hide_whitespace: false,
                file_list_position: FileListPosition::Auto,
                separate_special: false,
            }
        );
        let prefs = ChangesViewPrefs {
            side_by_side: true,
            hide_whitespace: true,
            file_list_position: FileListPosition::Left,
            separate_special: true,
            ..ChangesViewPrefs::default()
        };
        let config = AppConfig { changes_view: prefs.clone(), ..AppConfig::default() };
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.changes_view, prefs);
    }

    #[test]
    fn the_file_list_position_is_written_in_snake_case() {
        let config: AppConfig = serde_json::from_str(r#"{"changes_view":{"file_list_position":"right"}}"#).unwrap();
        assert_eq!(config.changes_view.file_list_position, FileListPosition::Right);
        assert!(!config.changes_view.separate_special);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-domain changes_view 2>&1 | tail -15`
Expected: FAIL to compile, `cannot find type FileListPosition`.

- [ ] **Step 3: Implement**

In `config.rs`, above `ChangesViewPrefs` add the enum, and add the two fields and defaults:

```rust
/// Where the Changes panel puts its file list. `Auto` is the right column, under the diffs when
/// the panel is narrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileListPosition {
    #[default]
    Auto,
    Left,
    Right,
}

/// How the Changes panel draws a diff (its `⋮` menu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChangesViewPrefs {
    /// Whether the right column shows folders (else a flat list).
    pub group_by_folder: bool,
    pub side_by_side: bool,
    pub wrap: bool,
    /// Whether the changed words of a changed line are marked.
    pub highlight_words: bool,
    /// Whether git leaves out whitespace-only changes (`-w`).
    pub hide_whitespace: bool,
    pub file_list_position: FileListPosition,
    /// Whether test files and generated files are listed apart from the others.
    pub separate_special: bool,
}

impl Default for ChangesViewPrefs {
    fn default() -> Self {
        Self {
            group_by_folder: true,
            side_by_side: false,
            wrap: true,
            highlight_words: true,
            hide_whitespace: false,
            file_list_position: FileListPosition::Auto,
            separate_special: false,
        }
    }
}
```

(Remove the old struct and `Default` impl those replace.) In `crates/forge-domain/src/lib.rs` add `FileListPosition` to the `pub use config::{ ... }` list, keeping the list's order style.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-domain 2>&1 | tail -8`
Expected: PASS. Then `cargo build --workspace 2>&1 | tail -5`: other crates build (they only use the struct with `..Default`; if a literal of `ChangesViewPrefs` elsewhere fails, add `..ChangesViewPrefs::default()`).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain/src/config.rs crates/forge-domain/src/lib.rs
git commit -m "feat: Changes view options for the file list position and splitting special files

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Sorting a path into Normal, Test or Generated

**Files:**
- Create: `crates/forge-entities/src/file_class.rs`
- Modify: `crates/forge-entities/src/lib.rs` (add `mod file_class;` among the mods, alphabetical near `extension_rows`, and `pub use file_class::{file_class, FileClass};`)

**Interfaces:**
- Produces: `forge_entities::FileClass { Normal, Test, Generated }` (`Debug, Clone, Copy, PartialEq, Eq, Hash`); `forge_entities::file_class(path: &str) -> FileClass` (path uses `/`).

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-entities/src/file_class.rs` with only the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_paths_seen_in_the_desktop_app_are_test_files() {
        for path in ["__tests__/b.js", "spec/a.rb", "src/x.test.ts", "tests/test_x.py", "bar.spec.ts", "foo_test.go", "test_root.py"] {
            assert_eq!(file_class(path), FileClass::Test, "{path}");
        }
    }

    #[test]
    fn the_paths_seen_in_the_desktop_app_are_generated_files() {
        for path in [".next/n.js", "dist/bundle.js", "api.generated.ts", "Cargo.lock", "package-lock.json", "pnpm-lock.yaml", "x.js.map", "x.min.js", "yarn.lock"] {
            assert_eq!(file_class(path), FileClass::Generated, "{path}");
        }
    }

    #[test]
    fn the_paths_seen_staying_in_the_normal_list_are_normal() {
        for path in ["build/out.txt", "build/app.js", "out/o.js", "target/t.rs", "bundle.d.ts", "main.py", "a.txt", "src/c.txt"] {
            assert_eq!(file_class(path), FileClass::Normal, "{path}");
        }
    }

    #[test]
    fn the_follow_the_pattern_cases_are_sorted_too() {
        assert_eq!(file_class("test/Main.java"), FileClass::Test);
        assert_eq!(file_class("specs/a.rb"), FileClass::Test);
        assert_eq!(file_class("styles/app.min.css"), FileClass::Generated);
        assert_eq!(file_class("styles/app.css.map"), FileClass::Generated);
    }

    #[test]
    fn a_folder_anywhere_in_the_path_counts() {
        assert_eq!(file_class("packages/web/tests/unit/a.rs"), FileClass::Test);
        assert_eq!(file_class("apps/site/dist/assets/a.png"), FileClass::Generated);
    }

    #[test]
    fn look_alikes_stay_normal() {
        // A file called `tests` is not a folder; a longer word is not the keyword.
        for path in ["tests", "src/tests", "contest_x.py", "latest_test_data", "testing/a.py", "mydist/a.js", "Tests/a.rs", "notes.test", "x.lockfile"] {
            assert_eq!(file_class(path), FileClass::Normal, "{path}");
        }
    }

    #[test]
    fn a_test_file_inside_a_generated_folder_is_a_test_file() {
        assert_eq!(file_class("dist/a.test.js"), FileClass::Test);
    }

    #[test]
    fn korean_names_are_sorted_by_the_same_rules() {
        assert_eq!(file_class("테스트/한글.txt"), FileClass::Normal);
        assert_eq!(file_class("tests/한글.txt"), FileClass::Test);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Add `mod file_class;` and the `pub use` to `lib.rs` first, then run: `cargo test -p forge-entities file_class 2>&1 | tail -15`
Expected: FAIL to compile (`file_class` not defined).

- [ ] **Step 3: Implement**

Put above the tests in `file_class.rs`:

```rust
//! Which of a changed file's three groups it belongs to, for the Changes panel's `Separate test,
//! build and generated files`. The rules are the ones seen in the desktop app (observation round
//! 20b): `build/`, `out/`, `target/` and `*.d.ts` were seen staying in the normal list.

/// The group a changed file is listed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileClass {
    Normal,
    Test,
    Generated,
}

const TEST_FOLDERS: [&str; 5] = ["tests", "test", "spec", "specs", "__tests__"];
const GENERATED_FOLDERS: [&str; 2] = ["dist", ".next"];
const GENERATED_ENDINGS: [&str; 5] = [".min.js", ".min.css", ".js.map", ".css.map", ".lock"];

fn is_test_name(name: &str) -> bool {
    if name.contains(".test.") || name.contains(".spec.") {
        return true;
    }
    // `foo_test.go` and `test_x.py` need an extension; a bare `latest_test_data` is not a test.
    match name.rsplit_once('.') {
        Some((stem, _)) => stem.ends_with("_test") || name.starts_with("test_"),
        None => false,
    }
}

fn is_generated_name(name: &str) -> bool {
    name.contains(".generated.")
        || name == "package-lock.json"
        || name == "pnpm-lock.yaml"
        || GENERATED_ENDINGS.iter().any(|ending| name.ends_with(ending))
}

/// The group of `path` (a repository path with `/`). A test file inside a generated folder is a
/// test file.
pub fn file_class(path: &str) -> FileClass {
    let mut parts: Vec<&str> = path.split('/').collect();
    let name = parts.pop().unwrap_or("");
    if parts.iter().any(|folder| TEST_FOLDERS.contains(folder)) || is_test_name(name) {
        return FileClass::Test;
    }
    if parts.iter().any(|folder| GENERATED_FOLDERS.contains(folder)) || is_generated_name(name) {
        return FileClass::Generated;
    }
    FileClass::Normal
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-entities file_class 2>&1 | tail -15`
Expected: PASS, 8 tests. If `look_alikes_stay_normal` fails on one path, fix the rule (not the test): each of those paths was chosen to stay Normal.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities/src/file_class.rs crates/forge-entities/src/lib.rs
git commit -m "feat: file_class sorts a changed path into normal, test or generated

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The tree and the stack with the split

**Files:**
- Modify: `crates/forge-features/src/change_rows.rs`
- Modify: `crates/forge-features/src/lib.rs:121` (re-exports)
- Modify (callers, so the workspace keeps building): `crates/forge-widgets/src/changes_panel.rs`, `crates/forge-widgets/src/changes_tree.rs`

**Interfaces:**
- Consumes: `forge_entities::{file_class, FileClass}` (Task 2).
- Produces (all re-exported from `forge_features`):
  - `ChangesSession.open_sections: HashSet<FileClass>`
  - `TreeRow::Section { kind: FileClass, count: usize, open: bool }` (exported as `ChangeTreeRow`)
  - `tree_rows(files: &[ChangedFile], group: bool, closed: &HashSet<String>, separate: bool, open_sections: &HashSet<FileClass>) -> Vec<TreeRow>`
  - `stack_order(files: &[ChangedFile], group: bool, separate: bool) -> Vec<usize>`
  - `toggle_section(session: &mut ChangesSession, kind: FileClass)`

- [ ] **Step 1: Write the failing tests**

Append inside the `tests` module of `change_rows.rs` (the `paths` helper already there gets a `Section` arm in Step 3; add these tests now):

```rust
    fn open(kinds: &[FileClass]) -> HashSet<FileClass> {
        kinds.iter().copied().collect()
    }

    #[test]
    fn with_the_split_off_the_rows_are_as_before() {
        let files = vec![file("b.txt"), file("tests/a.rs"), file("dist/x.js")];
        let off = tree_rows(&files, true, &HashSet::new(), false, &HashSet::new());
        assert_eq!(paths(&off), vec!["dist/", " dist/x.js", "tests/", " tests/a.rs", "b.txt"]);
    }

    #[test]
    fn the_split_lists_normal_files_then_a_closed_section_per_group() {
        let files = vec![file("b.txt"), file("tests/a.rs"), file("dist/x.js"), file("src/c.txt")];
        let rows = tree_rows(&files, true, &HashSet::new(), true, &HashSet::new());
        assert_eq!(paths(&rows), vec!["src/", " src/c.txt", "b.txt", "section:Test:1:closed", "section:Generated:1:closed"]);
    }

    #[test]
    fn an_open_section_lists_its_own_folder_tree_one_level_in() {
        let files = vec![file("b.txt"), file("tests/a.rs"), file("src/x.test.ts"), file("dist/x.js")];
        let rows = tree_rows(&files, true, &HashSet::new(), true, &open(&[FileClass::Test]));
        assert_eq!(
            paths(&rows),
            vec!["b.txt", "section:Test:2:open", " src/", "  src/x.test.ts", " tests/", "  tests/a.rs", "section:Generated:1:closed"]
        );
    }

    #[test]
    fn an_empty_section_has_no_row() {
        let files = vec![file("a.txt"), file("b.txt")];
        let rows = tree_rows(&files, true, &HashSet::new(), true, &open(&[FileClass::Test, FileClass::Generated]));
        assert_eq!(paths(&rows), vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn a_flat_list_splits_too() {
        let files = vec![file("z.txt"), file("a_test.go"), file("a.txt")];
        let rows = tree_rows(&files, false, &HashSet::new(), true, &open(&[FileClass::Test]));
        assert_eq!(paths(&rows), vec!["a.txt", "z.txt", "section:Test:1:open", " a_test.go"]);
    }

    #[test]
    fn the_stack_shows_every_file_normal_then_test_then_generated() {
        let files = vec![file("dist/x.js"), file("tests/a.rs"), file("b.txt"), file("src/c.txt")];
        // Indexes: 0 dist, 1 tests, 2 b, 3 src/c. Grouped tree order inside each group.
        assert_eq!(stack_order(&files, true, true), vec![3, 2, 1, 0]);
        assert_eq!(stack_order(&files, true, false), vec![0, 3, 1, 2]);
    }

    #[test]
    fn a_click_on_a_section_row_opens_and_closes_it() {
        let mut session = ChangesSession::default();
        toggle_section(&mut session, FileClass::Test);
        assert!(session.open_sections.contains(&FileClass::Test));
        toggle_section(&mut session, FileClass::Test);
        assert!(session.open_sections.is_empty());
    }
```

Also update every existing call in this module's tests: `tree_rows(&files, G, &C)` → `tree_rows(&files, G, &C, false, &HashSet::new())` and `stack_order(&files, G)` → `stack_order(&files, G, false)` (there are calls in `a_grouped_tree_lists_folders_first_then_files`, `a_flat_list_is_by_path`, `a_closed_folder_hides_what_is_inside_it`, `the_stack_follows_the_tree_order_even_when_a_folder_is_closed`, `a_file_row_knows_its_folder_and_name`).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-features change_rows 2>&1 | tail -15`
Expected: FAIL to compile (`toggle_section`, `FileClass`, wrong argument count).

- [ ] **Step 3: Implement**

In `change_rows.rs`:

1. Imports: add `use forge_entities::{file_class, FileClass};` under the existing `use forge_ports...`.
2. `ChangesSession`: add the field and its doc:

```rust
    /// The `Test files` and `Generated files` sections opened by hand (they start closed).
    pub open_sections: HashSet<FileClass>,
```

3. `TreeRow`: add the variant

```rust
    /// The head of the `Test files` or `Generated files` section.
    Section { kind: FileClass, count: usize, open: bool },
```

4. Replace `walk`, `build`, `tree_rows` and `stack_order` with versions that take the ids of the files to list and a starting depth:

```rust
fn walk(node: &Node, prefix: &str, depth: usize, files: &[ChangedFile], closed: &HashSet<String>, respect_closed: bool, rows: &mut Vec<TreeRow>) {
    for (name, child) in &node.folders {
        let path = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        let open = !closed.contains(&path);
        rows.push(TreeRow::Folder { path: path.clone(), name: name.clone(), depth, open });
        if open || !respect_closed {
            walk(child, &path, depth + 1, files, closed, respect_closed, rows);
        }
    }
    let mut ids = node.files.clone();
    ids.sort_by(|a, b| files[*a].path.cmp(&files[*b].path));
    rows.extend(ids.into_iter().map(|index| file_row(files, index, depth)));
}

/// The rows for `ids` (indexes into `files`), starting `depth0` levels in.
fn build(files: &[ChangedFile], ids: &[usize], group: bool, closed: &HashSet<String>, respect_closed: bool, depth0: usize) -> Vec<TreeRow> {
    if !group {
        let mut order = ids.to_vec();
        order.sort_by(|a, b| files[*a].path.cmp(&files[*b].path));
        return order.into_iter().map(|index| file_row(files, index, depth0)).collect();
    }
    let mut root = Node::default();
    for &index in ids {
        let mut node = &mut root;
        let mut parts: Vec<&str> = files[index].path.split('/').collect();
        parts.pop();
        for part in parts {
            node = node.folders.entry(part.to_string()).or_default();
        }
        node.files.push(index);
    }
    let mut rows = Vec::new();
    walk(&root, "", depth0, files, closed, respect_closed, &mut rows);
    rows
}

fn ids_of(files: &[ChangedFile], class: FileClass) -> Vec<usize> {
    (0..files.len()).filter(|index| file_class(&files[*index].path) == class).collect()
}

/// The rows of the right column: folders (folders first, then files, each by name) or a flat list.
/// With `separate`, the normal files come first; the test files and the generated files follow,
/// each group under a `Section` row, and are listed only while their section is in `open_sections`.
/// A section with no file has no row.
pub fn tree_rows(files: &[ChangedFile], group: bool, closed: &HashSet<String>, separate: bool, open_sections: &HashSet<FileClass>) -> Vec<TreeRow> {
    if !separate {
        let all: Vec<usize> = (0..files.len()).collect();
        return build(files, &all, group, closed, true, 0);
    }
    let mut rows = build(files, &ids_of(files, FileClass::Normal), group, closed, true, 0);
    for kind in [FileClass::Test, FileClass::Generated] {
        let ids = ids_of(files, kind);
        if ids.is_empty() {
            continue;
        }
        let open = open_sections.contains(&kind);
        rows.push(TreeRow::Section { kind, count: ids.len(), open });
        if open {
            rows.extend(build(files, &ids, group, closed, true, 1));
        }
    }
    rows
}

/// The order of the stacked diffs: the tree's order with every folder open and every section
/// open (a closed section does not hide its diffs, as in the desktop app).
pub fn stack_order(files: &[ChangedFile], group: bool, separate: bool) -> Vec<usize> {
    let groups: Vec<Vec<usize>> = if separate {
        [FileClass::Normal, FileClass::Test, FileClass::Generated].into_iter().map(|class| ids_of(files, class)).collect()
    } else {
        vec![(0..files.len()).collect()]
    };
    groups
        .iter()
        .flat_map(|ids| build(files, ids, group, &HashSet::new(), false, 0))
        .filter_map(|row| match row {
            TreeRow::File { index, .. } => Some(index),
            TreeRow::Folder { .. } | TreeRow::Section { .. } => None,
        })
        .collect()
}

/// A click on a section row: opens a closed section, closes an open one.
pub fn toggle_section(session: &mut ChangesSession, kind: FileClass) {
    if !session.open_sections.remove(&kind) {
        session.open_sections.insert(kind);
    }
}
```

(`file_row` keeps its signature. The doc comment on `split_folder` that was misplaced stays as is: do not touch.)

5. In the `tests` module's `paths` helper add the arm:

```rust
                TreeRow::Section { kind, count, open } => format!("section:{kind:?}:{count}:{}", if *open { "open" } else { "closed" }),
```

6. `forge-features/src/lib.rs:121`: add `toggle_section` to the `pub use change_rows::{...}` list. Also add `pub use forge_entities::FileClass;` if `forge_features` does not already re-export it (the widgets already depend on `forge_entities`, so they can import it directly; skip the re-export).

7. Keep the workspace building: in `changes_panel.rs` change the call `stack_order(&listed, prefs.group_by_folder)` to `stack_order(&listed, prefs.group_by_folder, prefs.separate_special)`; in `changes_tree.rs` change `tree_rows(&files, prefs.group_by_folder, &closed)` to `tree_rows(&files, prefs.group_by_folder, &closed, prefs.separate_special, &open_sections)`, add a prop `open_sections: HashSet<forge_entities::FileClass>` to `ChangesTree`, and add a match arm that draws nothing for now:

```rust
                        ChangeTreeRow::Section { .. } => rsx! {},
```

Pass `open_sections: session.open_sections.clone()` where `ChangesTree {` is built in `changes_panel.rs` (two places: the `with_commits` closure and the main body).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-features change_rows 2>&1 | tail -15` then `cargo build --workspace 2>&1 | tail -5`
Expected: PASS and a clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features/src/change_rows.rs crates/forge-features/src/lib.rs crates/forge-widgets/src/changes_panel.rs crates/forge-widgets/src/changes_tree.rs
git commit -m "feat: the changed file tree and the diff stack can split test and generated files

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Asking git whether the tree is dirty

**Files:**
- Modify: `crates/forge-ports/src/git_port.rs` (the `GitPort` trait, near line 108)
- Modify: `crates/forge-adapter-git/src/lib.rs` (`impl GitPort for GitCli`, near line 279; tests near line 440)

**Interfaces:**
- Produces: `GitPort::is_dirty(&self, cwd: &Path) -> Result<bool, GitError>`: true when `git status` reports any change or untracked file (ignored files do not count).

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module of `crates/forge-adapter-git/src/lib.rs` (it has the `repo()` helper and `git()` runner):

```rust
    #[test]
    fn a_clean_repository_is_not_dirty() {
        assert!(!GitCli::new().is_dirty(repo().path()).unwrap());
    }

    #[test]
    fn a_modified_file_makes_the_tree_dirty() {
        let dir = repo();
        fs::write(dir.path().join("a.txt"), "changed\n").unwrap();
        assert!(GitCli::new().is_dirty(dir.path()).unwrap());
    }

    #[test]
    fn an_untracked_file_makes_the_tree_dirty() {
        let dir = repo();
        fs::write(dir.path().join("new.txt"), "x\n").unwrap();
        assert!(GitCli::new().is_dirty(dir.path()).unwrap());
    }

    #[test]
    fn an_ignored_file_does_not_make_the_tree_dirty() {
        let dir = repo();
        fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();
        git(dir.path(), &["add", ".gitignore"]);
        git(dir.path(), &["commit", "-q", "-m", "ignore logs"]);
        fs::write(dir.path().join("x.log"), "noise\n").unwrap();
        assert!(!GitCli::new().is_dirty(dir.path()).unwrap());
    }

    #[test]
    fn is_dirty_works_from_a_subfolder_and_a_repository_with_no_commit() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        assert!(!GitCli::new().is_dirty(dir.path()).unwrap());
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/a.txt"), "x\n").unwrap();
        assert!(GitCli::new().is_dirty(&dir.path().join("sub")).unwrap());
    }

    #[test]
    fn is_dirty_reports_a_missing_repository_as_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(GitCli::new().is_dirty(dir.path()), Err(GitError::NotARepository));
    }
```

If `GitError::NotARepository` does not derive `PartialEq` the last assertion fails to compile: then use `assert!(matches!(..., Err(GitError::NotARepository)))`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-adapter-git is_dirty 2>&1 | tail -15`
Expected: FAIL to compile (`no method is_dirty`).

- [ ] **Step 3: Implement**

In `git_port.rs`, inside `trait GitPort` after `file_patch`:

```rust
    /// Whether the working tree has a change or an untracked file (an ignored file is neither). The
    /// panel hides its `Uncommitted changes` row when this is false.
    fn is_dirty(&self, cwd: &Path) -> Result<bool, GitError>;
```

In `impl GitPort for GitCli`:

```rust
    fn is_dirty(&self, cwd: &Path) -> Result<bool, GitError> {
        let root = self.root(cwd)?;
        // `normal` lists an untracked folder as one entry, which is all a yes or no needs.
        let output = self.checked(self.run(&root, &["status", "--porcelain=v1", "-z", "--untracked-files=normal"])?)?;
        Ok(!output.stdout.is_empty())
    }
```

`mockall`'s `automock` on the trait picks the new method up; if any hand-written `impl GitPort for` exists elsewhere, `cargo build --workspace --all-targets` will point at it: add the method returning `Ok(false)`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-adapter-git 2>&1 | tail -8` and `cargo build --workspace --all-targets 2>&1 | tail -5`
Expected: PASS; clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-ports/src/git_port.rs crates/forge-adapter-git/src/lib.rs
git commit -m "feat: GitPort::is_dirty tells a clean tree from a changed one

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The `⋮` menu rows, the sections and the position

**Files:**
- Modify: `crates/forge-widgets/src/changes_header.rs` (`ViewMenu`)
- Modify: `crates/forge-widgets/src/changes_tree.rs` (the `Section` arm)
- Modify: `crates/forge-widgets/src/changes_panel.rs` (the layout class)
- Modify: `crates/forge-shared/src/theme.css` (near line 587 `.changes-body`)
- Modify: `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: `FileListPosition`, `ChangesViewPrefs.separate_special` (Task 1); `toggle_section`, `ChangeTreeRow::Section` (Task 3).
- Produces: `fn position_label(position: FileListPosition) -> &'static str` and `fn position_class(position: FileListPosition) -> &'static str` in `changes_header.rs` (`pub(crate)`), unit tested.

- [ ] **Step 1: Write the failing tests**

At the end of the `tests` module of `changes_header.rs`:

```rust
    #[test]
    fn each_position_has_its_label_and_its_css_class() {
        assert_eq!(position_label(FileListPosition::Auto), tr!("Auto"));
        assert_eq!(position_label(FileListPosition::Left), tr!("Left"));
        assert_eq!(position_label(FileListPosition::Right), tr!("Right"));
        assert_eq!(position_class(FileListPosition::Auto), "pos-auto");
        assert_eq!(position_class(FileListPosition::Left), "pos-left");
        assert_eq!(position_class(FileListPosition::Right), "pos-right");
    }
```

and add `use forge_domain::{ChangesViewPrefs, FileListPosition};` at the top of the file (replacing the `ChangesViewPrefs`-only import).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-widgets each_position 2>&1 | tail -10`
Expected: FAIL to compile (`position_label` not found).

- [ ] **Step 3: Implement the menu**

In `changes_header.rs` add above `check_item`:

```rust
pub(crate) fn position_label(position: FileListPosition) -> &'static str {
    match position {
        FileListPosition::Auto => tr!("Auto"),
        FileListPosition::Left => tr!("Left"),
        FileListPosition::Right => tr!("Right"),
    }
}

/// The class on `.changes-body` that places the file list.
pub(crate) fn position_class(position: FileListPosition) -> &'static str {
    match position {
        FileListPosition::Auto => "pos-auto",
        FileListPosition::Left => "pos-left",
        FileListPosition::Right => "pos-right",
    }
}

/// `File list position  Auto ›`: a flyout opens at the left of the row (the menu sits at the
/// panel's right edge) while the pointer is on the row or the flyout.
#[component]
fn PositionRow(current: FileListPosition) -> Element {
    let workspace = use_workspace();
    let mut flyout = use_signal(|| false);
    rsx! {
        div {
            class: "context-sub",
            onmouseenter: move |_| flyout.set(true),
            onmouseleave: move |_| flyout.set(false),
            button { class: "picker-item", r#type: "button", role: "menuitem", "aria-haspopup": "menu",
                span { class: "picker-text", {tr!("File list position")} }
                span { class: "picker-hint", "{position_label(current)} ›" }
            }
            if flyout() {
                div { class: "picker-menu context-submenu left", role: "menu",
                    for option in [FileListPosition::Auto, FileListPosition::Left, FileListPosition::Right] {
                        button {
                            key: "{position_class(option)}",
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitemradio",
                            "aria-checked": "{current == option}",
                            onclick: move |_| {
                                workspace.update_changes_view_prefs(|p| p.file_list_position = option);
                                flyout.set(false);
                            },
                            span { class: "picker-text", "{position_label(option)}" }
                            if current == option {
                                span { class: "picker-hint", "✓" }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

In `ViewMenu`, change the menu `div`'s class to `"picker-menu cd-menu flyout"` and, between the `Show files` and `Group files by folder` rows and after the latter, insert the two rows so the block reads:

```rust
                    {check_item(tr!("Show files"), list_open, move |_| on_toggle_list.call(()))}
                    PositionRow { current: prefs.file_list_position }
                    {check_item(tr!("Group files by folder"), prefs.group_by_folder, move |_| workspace.update_changes_view_prefs(|p| p.group_by_folder = !p.group_by_folder))}
                    {check_item(tr!("Separate test, build and generated files"), prefs.separate_special, move |_| workspace.update_changes_view_prefs(|p| p.separate_special = !p.separate_special))}
                    div { class: "picker-divider" }
```

(`check_item` takes a `&'static str` label; `tr!` returns one.)

- [ ] **Step 4: Implement the section rows**

In `changes_tree.rs`, replace the empty `Section` arm from Task 3 with:

```rust
                        ChangeTreeRow::Section { kind, count, open } => rsx! {
                            li { key: "section:{count}:{kind:?}", class: "cd-section",
                                button {
                                    class: "cd-row",
                                    r#type: "button",
                                    onclick: move |_| workspace.update_changes_session(id, |session| toggle_section(session, kind)),
                                    span { class: "cd-chevron", if open { "▾" } else { "▸" } }
                                    span { class: "cd-name", "{section_title(kind)}" }
                                    span { class: "cd-counts", "{count}" }
                                }
                            }
                        },
```

add `toggle_section` to the `forge_features` import, `use forge_entities::{icon_of, FileClass};`, and below `commit_meta`:

```rust
fn section_title(kind: FileClass) -> &'static str {
    match kind {
        FileClass::Test => tr!("Test files"),
        FileClass::Generated => tr!("Generated files"),
        FileClass::Normal => "",
    }
}
```

- [ ] **Step 5: Implement the layout**

In `changes_panel.rs`, add `use crate::changes_header::position_class;` and wrap the two places that draw `div { class: "changes-body", ... }` as

```rust
div { class: "changes-wrap",
    div { class: "changes-body {position_class(prefs.file_list_position)}", ... }
}
```

(keep each body's children unchanged). In `crates/forge-shared/src/theme.css` replace the `.changes-body` rule at line 587 and add the position rules after the `.cd-side` rule (line 589):

```css
.changes-wrap { flex: 1; min-height: 0; min-width: 0; display: flex; container-type: inline-size; }
.changes-body { flex: 1; min-height: 0; min-width: 0; display: flex; }
.cd-menu.flyout { max-height: none; overflow: visible; }
.changes-body.pos-left { flex-direction: row-reverse; }
.changes-body.pos-left .cd-side { border-left: none; border-right: 1px solid var(--border); }
/* A narrow panel puts the list under the diffs; a container query on the wrapper, because a
   container cannot restyle itself. */
@container (max-width: 559px) {
  .changes-body.pos-auto { flex-direction: column; }
  .changes-body.pos-auto .cd-side { width: auto; min-width: 0; max-height: 40%; border-left: none; border-top: 1px solid var(--border); }
}
```

- [ ] **Step 6: Korean text**

Insert into `crates/forge-shared/src/ko.rs` in byte order (find each neighbour with the editor; the table is sorted by the English key):

```rust
    ("File list position", "파일 목록 위치"),
    ("Generated files", "생성된 파일"),
    ("Left", "왼쪽"),
    ("Right", "오른쪽"),
    ("Separate test, build and generated files", "테스트, 빌드, 생성된 파일 분리"),
    ("Test files", "테스트 파일"),
```

(`Auto` already exists.)

- [ ] **Step 7: Run to verify it passes**

Run: `cargo test -p forge-widgets -p forge-shared 2>&1 | tail -12` and `cargo build -p forge-app 2>&1 | tail -3`
Expected: PASS and a clean build.

- [ ] **Step 8: Commit**

```bash
git add crates/forge-widgets crates/forge-shared
git commit -m "feat: Changes panel menu rows for the list position and the test and generated split

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: `Commits N ›` and the clean tree

**Files:**
- Modify: `crates/forge-features/src/change_rows.rs`, `crates/forge-features/src/lib.rs` (re-export)
- Modify: `crates/forge-widgets/src/changes_header.rs` (`BranchButton`)
- Modify: `crates/forge-widgets/src/changes_tree.rs` (`CommitsSection`)
- Modify: `crates/forge-widgets/src/changes_panel.rs` (load `is_dirty`, pass it down)
- Modify: `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Produces: `filter_commits(commits: &[CommitInfo], query: &str) -> Vec<usize>` and `show_uncommitted(dirty: bool, view: &ChangeView) -> bool` in `change_rows.rs`, re-exported from `forge_features`.
- `BranchButton` gets a prop `dirty: bool`; `CommitsSection` gets a prop `dirty: bool`.

- [ ] **Step 1: Write the failing tests**

In the `tests` module of `change_rows.rs` (it already imports `CommitInfo` through `use super::*`):

```rust
    fn commit(sha: &str, subject: &str, author: &str) -> CommitInfo {
        CommitInfo { sha: sha.to_string(), subject: subject.to_string(), author: author.to_string(), time: 0 }
    }

    #[test]
    fn the_commit_filter_matches_subject_sha_and_author_without_case() {
        let list = vec![
            commit("d5bfbad1234", "Update a.txt", "myhency"),
            commit("0aa11bb2233", "한글 제목 수정", "Kim"),
            commit("77cc88dd991", "Fix Build", "Lee"),
        ];
        assert_eq!(filter_commits(&list, ""), vec![0, 1, 2]);
        assert_eq!(filter_commits(&list, "UPDATE"), vec![0]);
        assert_eq!(filter_commits(&list, "d5bf"), vec![0]);
        assert_eq!(filter_commits(&list, "kim"), vec![1]);
        assert_eq!(filter_commits(&list, "한글"), vec![1]);
        assert_eq!(filter_commits(&list, "build"), vec![2]);
        assert!(filter_commits(&list, "zzz").is_empty());
    }

    #[test]
    fn a_clean_tree_hides_the_uncommitted_row_unless_it_is_the_chosen_view() {
        assert!(show_uncommitted(true, &ChangeView::All));
        assert!(!show_uncommitted(false, &ChangeView::All));
        assert!(!show_uncommitted(false, &ChangeView::Commit("abc".to_string())));
        assert!(show_uncommitted(false, &ChangeView::Uncommitted));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-features change_rows 2>&1 | tail -10`
Expected: FAIL to compile (`filter_commits`, `show_uncommitted` not found).

- [ ] **Step 3: Implement the two functions**

In `change_rows.rs` after `filter_files`:

```rust
/// The commits whose subject, short or full sha or author holds `query` (no case); every commit
/// for an empty query.
pub fn filter_commits(commits: &[CommitInfo], query: &str) -> Vec<usize> {
    let query = query.to_lowercase();
    (0..commits.len())
        .filter(|index| {
            let commit = &commits[*index];
            query.is_empty()
                || commit.subject.to_lowercase().contains(&query)
                || commit.sha.to_lowercase().contains(&query)
                || commit.author.to_lowercase().contains(&query)
        })
        .collect()
}

/// Whether the `Uncommitted changes` row is offered: not on a clean tree (the desktop app hides it
/// there), except while it is the chosen view, so the person can still see which view this is.
pub fn show_uncommitted(dirty: bool, view: &ChangeView) -> bool {
    dirty || *view == ChangeView::Uncommitted
}
```

Add both to the `pub use change_rows::{...}` list in `forge-features/src/lib.rs`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-features change_rows 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 5: The panel loads `is_dirty`**

In `changes_panel.rs` add, next to the `commits` signal: `let mut dirty = use_signal(|| true);` and an effect like the `info` one:

```rust
    let mut dirty_ticket = use_signal(|| 0u64);
    {
        let cwd = cwd.clone();
        use_effect(move || {
            let _ = refresh();
            let _ = finished();
            let ticket = dirty_ticket.peek().wrapping_add(1);
            dirty_ticket.set(ticket);
            let (git, cwd) = (workspace.git(), cwd.clone());
            spawn(async move {
                let Some(read) = run_blocking(move || git.is_dirty(&cwd)).await else { return };
                if *dirty_ticket.peek() == ticket {
                    // An error (not a repository, git missing) keeps the row: showing more is safer.
                    dirty.set(read.unwrap_or(true));
                }
            });
        });
    }
```

Pass `dirty: dirty()` to `BranchButton { ... }` and `CommitsSection { id, commits: list, view: session.view.clone(), dirty: dirty() }`.

- [ ] **Step 6: The `Commits N ›` list in the branch menu**

In `changes_header.rs`: add the prop `dirty: bool` to `BranchButton`; add `let mut choosing_commit = use_signal(|| false);`; reset it where `choosing_base.set(false)` is called (in the button's `onclick`); compute

```rust
    let shown_commits: Vec<CommitInfo> = forge_features::filter_commits(&commits, &query()).into_iter().map(|index| commits[index].clone()).collect();
```

Replace the menu's `if choosing_base() { ... } else { ... }` with a three-way chain: `if choosing_base() {` (unchanged) `} else if choosing_commit() {` the commit list `} else {` the main rows `}`. The commit list:

```rust
                        input {
                            class: "cd-menu-search",
                            r#type: "text",
                            placeholder: tr!("Search commits"),
                            value: "{query}",
                            oninput: move |event| query.set(event.value()),
                            onmounted: move |mounted| async move {
                                let _ = mounted.set_focus(true).await;
                            },
                        }
                        for commit in shown_commits {
                            button {
                                key: "{commit.sha}",
                                class: "picker-item",
                                r#type: "button",
                                role: "menuitem",
                                title: "{commit.subject}",
                                onclick: {
                                    let sha = commit.sha.clone();
                                    move |_| {
                                        workspace.update_changes_session(id, |session| session.view = ChangeView::Commit(sha.clone()));
                                        open.set(false);
                                    }
                                },
                                span { class: "picker-text", "{commit.subject}" }
                                span { class: "picker-hint", "{&commit.sha[..commit.sha.len().min(7)]}" }
                            }
                        }
```

In the main rows: wrap the `Uncommitted changes` button in `if forge_features::show_uncommitted(dirty, &view) { ... }` and, after the `Compare to` button, add:

```rust
                        if !commits.is_empty() {
                            button {
                                class: "picker-item",
                                r#type: "button",
                                role: "menuitem",
                                onclick: move |_| {
                                    query.set(String::new());
                                    choosing_commit.set(true);
                                },
                                span { class: "picker-text", {tr!("Commits")} }
                                span { class: "picker-hint", "{commits.len()} ›" }
                            }
                        }
```

(The base list and the commit list share the `query` signal, which is cleared when either opens, as the base list already does.) The `.cd-menu` of the branch button keeps its scroll (no flyout class), so a long commit list scrolls.

- [ ] **Step 7: The `Commits` section on a clean tree**

In `changes_tree.rs`, `CommitsSection` takes `dirty: bool` and wraps the `Uncommitted changes` button:

```rust
                if forge_features::show_uncommitted(dirty, &view) {
                    button { /* the existing Uncommitted changes button, unchanged */ }
                }
```

- [ ] **Step 8: Korean text and build**

Insert `("Search commits", "커밋 검색"),` into `ko.rs` in byte order (after `"Search changed files"`/`"Search branches"` neighbours: compare as bytes). Run `cargo test -p forge-features -p forge-widgets -p forge-shared 2>&1 | tail -12` and `cargo build -p forge-app 2>&1 | tail -3`.
Expected: PASS; clean build.

- [ ] **Step 9: Commit**

```bash
git add crates/forge-features crates/forge-widgets crates/forge-shared
git commit -m "feat: Commits list in the branch menu; a clean tree has no Uncommitted row

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The chosen view and base survive a restart

**Files:**
- Modify: `crates/forge-domain/src/config.rs`, `crates/forge-domain/src/lib.rs`
- Modify: `crates/forge-features/src/change_rows.rs`, `crates/forge-features/src/workspace.rs`, `crates/forge-features/src/lib.rs`
- Modify: `crates/forge-widgets/src/changes_panel.rs`

**Interfaces:**
- Produces:
  - `forge_domain::SavedChangeView { All, Uncommitted, Commit(String) }` and `forge_domain::ChangeModeRef { project: PathBuf, session_id: String, view: SavedChangeView, base: Option<String> }`; `AppConfig.changes_modes: Vec<ChangeModeRef>` (serde default).
  - In `change_rows.rs`: `upsert_mode(list: &mut Vec<ChangeModeRef>, project: &Path, session: &str, view: &ChangeView, base: Option<&str>) -> bool` (true when the list changed) and `forget_mode(list: &mut Vec<ChangeModeRef>, project: &Path, session: &str) -> bool`, `mode_of(list: &[ChangeModeRef], project: &Path, session: &str) -> Option<(ChangeView, Option<String>)>`.
  - `Workspace::restore_change_mode(&self, id: LiveSessionId)` and `Workspace::remember_change_mode(&self, id: LiveSessionId)`.

- [ ] **Step 1: Write the failing config tests**

In `config.rs` tests:

```rust
    #[test]
    fn an_old_config_has_no_saved_modes() {
        let config: AppConfig = serde_json::from_str("{}").unwrap();
        assert!(config.changes_modes.is_empty());
    }

    #[test]
    fn saved_modes_survive_a_round_trip() {
        let config = AppConfig {
            changes_modes: vec![
                ChangeModeRef { project: "/work/a".into(), session_id: "s1".to_string(), view: SavedChangeView::Commit("abc".to_string()), base: Some("dev".to_string()) },
                ChangeModeRef { project: "/work/a".into(), session_id: "s2".to_string(), view: SavedChangeView::Uncommitted, base: None },
            ],
            ..AppConfig::default()
        };
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.changes_modes, config.changes_modes);
    }
```

- [ ] **Step 2: Run to verify it fails, then implement**

Run: `cargo test -p forge-domain saved_modes 2>&1 | tail -8` (FAIL to compile). Then in `config.rs`, beside `ArchivedRef`:

```rust
/// What a session's Changes panel compared, kept so it is the same after a restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedChangeView {
    All,
    Uncommitted,
    Commit(String),
}

/// The Changes panel's view and base of one session, found by project folder and Claude session
/// id like `ArchivedRef`. A session on the default (all changes, default base) has no entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeModeRef {
    pub project: std::path::PathBuf,
    pub session_id: String,
    pub view: SavedChangeView,
    pub base: Option<String>,
}
```

Add to `AppConfig` next to `archived`: `#[serde(default)] pub changes_modes: Vec<ChangeModeRef>,` and `changes_modes: Vec::new(),` in its `Default` impl (next to `archived: Vec::new(),`). Export `ChangeModeRef` and `SavedChangeView` in `forge-domain/src/lib.rs`. Run `cargo test -p forge-domain 2>&1 | tail -8`: PASS; `cargo build --workspace 2>&1 | tail -5` (a literal `AppConfig { .. }` without a spread would fail: add `changes_modes: Vec::new()` there).

- [ ] **Step 3: Write the failing list tests**

In `change_rows.rs` tests (add `use forge_domain::{ChangeModeRef, SavedChangeView};` to the imports at the top of the file, and `use std::path::Path;`):

```rust
    #[test]
    fn a_mode_is_kept_per_session() {
        let mut list = Vec::new();
        assert!(upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::Uncommitted, None));
        assert!(upsert_mode(&mut list, Path::new("/p"), "s2", &ChangeView::Commit("abc".to_string()), Some("dev")));
        assert_eq!(mode_of(&list, Path::new("/p"), "s1"), Some((ChangeView::Uncommitted, None)));
        assert_eq!(mode_of(&list, Path::new("/p"), "s2"), Some((ChangeView::Commit("abc".to_string()), Some("dev".to_string()))));
        assert_eq!(mode_of(&list, Path::new("/p"), "s3"), None);
        assert_eq!(mode_of(&list, Path::new("/other"), "s1"), None);
    }

    #[test]
    fn saving_the_same_mode_again_changes_nothing() {
        let mut list = Vec::new();
        upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::Uncommitted, None);
        assert!(!upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::Uncommitted, None));
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn the_default_mode_leaves_no_entry() {
        let mut list = Vec::new();
        assert!(!upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::All, None));
        assert!(list.is_empty());
        upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::Uncommitted, None);
        assert!(upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::All, None));
        assert!(list.is_empty());
    }

    #[test]
    fn a_picked_base_with_all_changes_is_kept() {
        let mut list = Vec::new();
        assert!(upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::All, Some("dev")));
        assert_eq!(mode_of(&list, Path::new("/p"), "s1"), Some((ChangeView::All, Some("dev".to_string()))));
    }

    #[test]
    fn forgetting_a_session_drops_its_entry() {
        let mut list = Vec::new();
        upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::Uncommitted, None);
        upsert_mode(&mut list, Path::new("/p"), "s2", &ChangeView::Uncommitted, None);
        assert!(forget_mode(&mut list, Path::new("/p"), "s1"));
        assert!(!forget_mode(&mut list, Path::new("/p"), "s1"));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].session_id, "s2");
    }
```

- [ ] **Step 4: Run to verify it fails, then implement**

Run: `cargo test -p forge-features change_rows 2>&1 | tail -10` (FAIL to compile). Implement in `change_rows.rs`:

```rust
fn to_saved(view: &ChangeView) -> SavedChangeView {
    match view {
        ChangeView::All => SavedChangeView::All,
        ChangeView::Uncommitted => SavedChangeView::Uncommitted,
        ChangeView::Commit(sha) => SavedChangeView::Commit(sha.clone()),
    }
}

fn from_saved(view: &SavedChangeView) -> ChangeView {
    match view {
        SavedChangeView::All => ChangeView::All,
        SavedChangeView::Uncommitted => ChangeView::Uncommitted,
        SavedChangeView::Commit(sha) => ChangeView::Commit(sha.clone()),
    }
}

fn same_session(entry: &ChangeModeRef, project: &Path, session: &str) -> bool {
    entry.project == project && entry.session_id == session
}

/// The saved view and base of one session.
pub fn mode_of(list: &[ChangeModeRef], project: &Path, session: &str) -> Option<(ChangeView, Option<String>)> {
    list.iter().find(|entry| same_session(entry, project, session)).map(|entry| (from_saved(&entry.view), entry.base.clone()))
}

/// Saves a session's view and base. The default (all changes, no picked base) has no entry, so an
/// untouched session adds nothing. True when the list changed.
pub fn upsert_mode(list: &mut Vec<ChangeModeRef>, project: &Path, session: &str, view: &ChangeView, base: Option<&str>) -> bool {
    let is_default = *view == ChangeView::All && base.is_none();
    if is_default {
        return forget_mode(list, project, session);
    }
    let next = ChangeModeRef { project: project.to_path_buf(), session_id: session.to_string(), view: to_saved(view), base: base.map(str::to_string) };
    match list.iter_mut().find(|entry| same_session(entry, project, session)) {
        Some(entry) if *entry == next => false,
        Some(entry) => {
            *entry = next;
            true
        }
        None => {
            list.push(next);
            true
        }
    }
}

/// Drops a session's entry (its session was deleted). True when there was one.
pub fn forget_mode(list: &mut Vec<ChangeModeRef>, project: &Path, session: &str) -> bool {
    let before = list.len();
    list.retain(|entry| !same_session(entry, project, session));
    list.len() != before
}
```

with `use std::path::Path;` and `use forge_domain::{ChangeModeRef, SavedChangeView};` at the top (non-test). Re-export `forget_mode, mode_of, upsert_mode` in `forge-features/src/lib.rs`. Run `cargo test -p forge-features change_rows 2>&1 | tail -10`: PASS.

- [ ] **Step 5: The workspace plumbing**

In `workspace.rs`:

1. Add to the `forge_domain` import `ChangeModeRef`; add the field `changes_modes: Signal<Vec<ChangeModeRef>>,` after `changes_sessions` (line ~122), initialise `changes_modes: Signal::new(Vec::new()),` next to `changes_sessions`'s initialiser, and in `reload_prefs` add `let mut changes_modes = self.changes_modes;` and `changes_modes.set(config.changes_modes);` inside the `if let Ok(config)` block.
2. Below `update_changes_session` add:

```rust
    /// The project folder and Claude session id of an open session; `None` before its first message.
    fn session_ref(&self, id: LiveSessionId) -> Option<(PathBuf, String)> {
        self.live.read().iter().find(|entry| entry.handle.id() == id).and_then(|entry| {
            let view = entry.view.read();
            view.claude_session_id.as_ref().map(|session| (view.project_path.clone(), session.0.clone()))
        })
    }

    /// Puts back the view and base this session had when Forge last saved them, unless the person
    /// has already changed them in this run.
    pub fn restore_change_mode(&self, id: LiveSessionId) {
        let Some((project, session)) = self.session_ref(id) else { return };
        let Some((view, base)) = crate::change_rows::mode_of(&self.changes_modes.read(), &project, &session) else { return };
        self.update_changes_session(id, |state| {
            if state.view == ChangeView::All && state.base.is_none() {
                state.view = view;
                state.base = base;
            }
        });
    }

    /// Saves this session's view and base (a no-op when they are what is already saved).
    pub fn remember_change_mode(&self, id: LiveSessionId) {
        let Some((project, session)) = self.session_ref(id) else { return };
        let state = self.changes_session(id);
        let mut modes = self.changes_modes;
        let changed = crate::change_rows::upsert_mode(&mut modes.write(), &project, &session, &state.view, state.base.as_deref());
        if changed {
            self.save_change_modes();
        }
    }

    fn save_change_modes(&self) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        let list = self.changes_modes.peek().clone();
        // Root scope: the save must finish even if the panel that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            if let Err(error) = services.settings.save_code_settings(move |config| config.changes_modes = list).await {
                this.report(tr!("failed to save the changes view: {error}", error = error));
            }
        });
    }
```

(`ChangeView` is already imported in workspace.rs through `crate::change_rows::ChangesSession`; add `ChangeView` to that `use crate::change_rows::{...}` line.)

3. In `delete_session`, inside `Ok(()) => {` after `this.set_archived(...)`:

```rust
                    let mut modes = this.changes_modes;
                    if crate::change_rows::forget_mode(&mut modes.write(), &project, &session.0) {
                        this.save_change_modes();
                    }
```

4. Add the Korean text `("failed to save the changes view: {error}", "변경 사항 보기를 저장하지 못했습니다: {error}"),` to `ko.rs` in byte order.

- [ ] **Step 6: Wire the panel**

In `changes_panel.rs`, near the top of `ChangesPanel` after `let workspace = use_workspace();`:

```rust
    // Back to what this session compared last time; before the loaders' effects run.
    use_hook(move || workspace.restore_change_mode(id));
    let mode_memo = use_memo(move || {
        let state = workspace.changes_session(id);
        (state.view, state.base)
    });
    use_effect(move || {
        let _ = mode_memo();
        workspace.remember_change_mode(id);
    });
```

(`Workspace` is `Copy`, as the other closures in this file rely on.)

- [ ] **Step 7: Run to verify it passes**

Run: `cargo test -p forge-domain -p forge-features -p forge-widgets -p forge-shared 2>&1 | tail -12` and `cargo build -p forge-app 2>&1 | tail -3`
Expected: PASS; clean build.

- [ ] **Step 8: Commit**

```bash
git add crates
git commit -m "feat: a session's Changes view and base survive a restart

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Check in the real window and record the stage

**Files:**
- Modify: `docs/superpowers/specs/2026-10-17-changes-panel-2-design.md` ("As built")
- Modify: `docs/superpowers/roadmap/backlog.md` (PNL-02 row)
- Create: `/Users/james/.claude/projects/-Users-james-Developer-forge/memory/plan-50a-changes-menus.md` and its line in `MEMORY.md`

- [ ] **Step 1: Whole test run**

Run: `cargo test --workspace 2>&1 | tail -20`
Expected: all pass (the plugin round-trip test is known flaky: re-run it alone if it is the only failure).

- [ ] **Step 2: Real-window check**

Read `scripts/harness/README.md` and launch with `scripts/harness/launch.sh`. Make the harness session folder a git repository on a branch `claude/x` with a `main` base and two commits, then add (uncommitted) these files: `main.py`, `a.txt` (modified), `tests/test_x.py`, `src/x.test.ts`, `spec/a.rb`, `foo_test.go`, `dist/bundle.js`, `package-lock.json`, `Cargo.lock`, `x.min.js`, `build/out.txt`, `target/t.rs`, `bundle.d.ts`, plus a Korean-named `한글.txt`. Open the Changes panel and check each, capturing with `scripts/harness/capture.py` and clicking with `scripts/harness/click.py`:

1. `⋮` menu: the rows in the app's order (`Show files`, `File list position ›`, `Group files by folder`, `Separate test, build and generated files`, rule, fold rows, view rows, `Refresh`); hover on `File list position ›` opens a flyout at the left with `Auto ✓`, `Left`, `Right`; the whole menu is not clipped.
2. `Left` moves the list to the left of the diffs, `Right` back, `Auto` at the panel's width. Make the window narrow (`scripts/harness/resize.py`, below the 400 px panel body) and confirm `Auto` puts the list under the diffs while `Right` keeps it at the right.
3. The split on: normal files first (`build/`, `target/`, `bundle.d.ts` among them), then `Test files 4` and `Generated files 4` closed, opening to their own trees; the diffs at the left follow the same order and are all shown with the sections closed; switching it off returns the single tree; the choice and the position are still set after closing and reopening the panel.
4. Branch menu: `Commits 2 ›` opens a list with a search field; typing a Korean word and an upper-case word filters; a click shows that commit and the button names it. Commit everything so the tree is clean: the `Uncommitted changes` row is gone from the menu and from the `Commits` section; choose `Uncommitted changes` on a dirty tree, clean it, and the row stays while it is the chosen view.
5. Set the view to a commit, close the session, reopen it from the sidebar: the Changes panel opens on the same commit; pick `All changes` and reopen: the entry is gone from `FORGE_CONFIG_PATH`'s JSON (`changes_modes`).
6. Dark mode and the 700 pt window (Plan 26 layout): the flyout and the sections look right.

Record anything that differs from the desktop app (the commit list is inline in the menu, not a flyout beside it).

- [ ] **Step 3: Record the stage**

In the spec's "As built" section write a `### Stage 50A (built on branch plan-50)` block: what was built, what differs (the commit list replaces the menu content like the base list instead of a flyout; the `Auto` narrow rule is a guess, with the real value checked above; closed folders are shared between the normal tree and the sections), what was checked in the window and what was not. In `backlog.md` set the PNL-02 `Plan` cell to `... Plan 50A (built); 50B and 50C planned` and move the finished items out of the "Still not built" sentence of that row (`File list position ›`, `Separate test, build and generated files`, `Commits N ›`, the full `Copy ›` list, keeping the chosen mode across a restart). Write the memory note `plan-50a-changes-menus.md` (frontmatter as the other plan notes: name, description, metadata.type project) with the merge state, the Copy/Open-in finding, the `Auto` guess and the `file_class` rules' source, and add its line to `MEMORY.md`.

- [ ] **Step 4: Commit**

```bash
git add docs
git commit -m "docs: Plan 50A as built, checked in the real window

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
