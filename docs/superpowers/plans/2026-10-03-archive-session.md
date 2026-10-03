# Archive a Session Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A session can be archived from its sidebar menu (item or key A), hidden from the default list, found again under a Status filter, and restored from a bar in its chat window.

**Architecture:** The archived set is a list of `(project, session id)` in `AppConfig.archived`, saved by a new `SettingsUseCase::save_archived`. `Workspace` keeps it in a signal, marks `SidebarEntry.archived` when it builds the sidebar, and `arrange` filters by `SidebarPrefs.status`. No port or adapter changes: Claude's transcripts are never touched.

**Tech Stack:** Rust workspace (`crates/*`), Dioxus (desktop UI), serde, tokio, mockall.

**Spec:** `docs/superpowers/specs/2026-10-03-archive-design.md`

## Deviations from the spec (found while reading the code)

The spec said an archived session opens through an existing "view-only path". There is none: `Workspace::open_past` opens an ordinary live session, and `can_send` is true when its folder exists (`registry.rs:232`). So:

1. The Unarchive bar is drawn by `ChatWindow` in place of the message box when the open session is archived. No change to `forge-application`'s live session.
2. `RowActions.archive` is `Unarchive` for an **open** archived session too (that is how a session opened from the Archived view can be restored from the sidebar). An open session that is not archived cannot be archived (`Unavailable`), as the spec says.
3. The filter is applied inside `arrange` from `SidebarEntry.archived` and `SidebarPrefs.status`; `arrange` does not take the archived list. `Workspace::sidebar()` sets `archived` on each entry.
4. Archive only acts on past (not open) rows, so the selected session is never the one that disappears. The spec's "clear the selection" clause is not needed.
5. `GroupBy` has `Flat`, not `None` (the label is "None"). The code below uses `Flat`.

## Global Constraints

- UI text is English (Forge's current text): "Archive", "Unarchive", "Status", "Active", "Archived", "All".
- The menu key matches the physical key: `"KeyA"`, no modifier (so a Korean input method works).
- An old config file (no `archived`, no `sidebar.status`) loads unchanged: both fields are `#[serde(default)]`.
- Claude transcripts and `forge-adapter-claude-fs` are not changed.
- An archived id with no matching session is ignored, never an error.
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

1. Config written by this version and read by an older one: unknown fields are ignored by serde, so the older app just shows every session. Pin that an old file loads (Task 1).
2. An archived id whose transcript was deleted outside Forge: no row, no crash, `archived` list untouched (Task 3).
3. The same session id in two projects: archiving one must not hide the other (Task 3, match on project and id).
4. Archive pressed with a Korean input method on: `KeyA` works though the key types "ㅁ" (Task 3).
5. Two quick saves (archive, then change a filter): both must survive, since each save reads the file and writes it back (Task 2 uses the same write lock as `save_sidebar`).

---

## File structure

| File | Change |
| --- | --- |
| `crates/forge-domain/src/config.rs` | `ArchivedRef`, `StatusFilter`, `SidebarPrefs.status`, `AppConfig.archived` |
| `crates/forge-domain/src/lib.rs` | re-export the new types |
| `crates/forge-application/src/use_cases/settings.rs` | `save_archived` |
| `crates/forge-features/src/sidebar.rs` | `SidebarEntry.archived`, `mark_archived`, `ArchiveAction`, `RowActions.archive`, `MenuKeyAction::ToggleArchive` |
| `crates/forge-features/src/arrange.rs` | status filter |
| `crates/forge-features/src/filter_menu.rs` | Status row |
| `crates/forge-features/src/workspace.rs` | archived signal, archive and unarchive, marking |
| `crates/forge-features/src/lib.rs` | re-exports |
| `crates/forge-widgets/src/filter_menu.rs` | Status row and submenu |
| `crates/forge-widgets/src/session_row.rs` | menu item, A key, archive icon |
| `crates/forge-widgets/src/chat_window.rs` | Unarchive bar |
| `crates/forge-shared/src/theme.css` | styles for the icon and the bar |

---

### Task 1: Domain types and config fields

**Files:**
- Modify: `crates/forge-domain/src/config.rs`
- Modify: `crates/forge-domain/src/lib.rs`
- Test: same file (`mod tests`)

**Interfaces:**
- Produces: `ArchivedRef { project: PathBuf, session_id: String }` (`Debug, Clone, PartialEq, Eq, Serialize, Deserialize`); `StatusFilter { Active (default), Archived, All }`; `SidebarPrefs.status: StatusFilter`; `AppConfig.archived: Vec<ArchivedRef>`.

- [ ] **Step 1: Write the failing tests** (add to `mod tests` in `config.rs`)

```rust
    #[test]
    fn an_old_config_loads_with_nothing_archived_and_the_active_filter() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();

        assert!(config.archived.is_empty());
        assert_eq!(config.sidebar.status, StatusFilter::Active);
    }

    #[test]
    fn a_sidebar_saved_before_the_status_filter_existed_still_loads() {
        let config: AppConfig =
            serde_json::from_str(r#"{"sidebar":{"sort_by":"name","group_by":"date"}}"#).unwrap();

        assert_eq!(config.sidebar.sort_by, SortBy::Name);
        assert_eq!(config.sidebar.status, StatusFilter::Active);
    }

    #[test]
    fn archived_sessions_and_the_status_filter_survive_a_round_trip() {
        let config = AppConfig {
            archived: vec![ArchivedRef { project: "/work/a".into(), session_id: "s1".to_string() }],
            sidebar: SidebarPrefs { status: StatusFilter::Archived, ..SidebarPrefs::default() },
            ..AppConfig::default()
        };

        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();

        assert_eq!(back, config);
    }

    #[test]
    fn an_unknown_status_value_reads_as_active() {
        let config: AppConfig = serde_json::from_str(r#"{"sidebar":{"status":"deleted"}}"#).unwrap();

        assert_eq!(config.sidebar.status, StatusFilter::Active);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p forge-domain config::tests`
Expected: FAIL to compile, "cannot find type `StatusFilter`".

- [ ] **Step 3: Implement**

In `config.rs`, after `GroupBy`:

```rust
/// Which sessions the sidebar lists. An unknown stored value reads as `Active`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusFilter {
    Archived,
    All,
    #[default]
    #[serde(other)]
    Active,
}

/// A session the person archived. The Claude transcript is not changed; only Forge's config
/// remembers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchivedRef {
    pub project: std::path::PathBuf,
    pub session_id: String,
}
```

Add to `SidebarPrefs` (after `group_by`) `pub status: StatusFilter,`; in `Default` add `status: StatusFilter::default(),`. Add to `AppConfig` (after `sidebar`):

```rust
    #[serde(default)]
    pub archived: Vec<ArchivedRef>,
```

In the existing `sidebar_preferences_survive_a_round_trip` test, add `status: StatusFilter::All,` to the `SidebarPrefs` literal. Re-export `ArchivedRef` and `StatusFilter` from `crates/forge-domain/src/lib.rs` next to `GroupBy` and `SidebarPrefs`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p forge-domain`
Expected: PASS. Then `cargo build --workspace` to find other `AppConfig { .. }` literals; fix any that fail with `..Default::default()`.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain
git commit -m "feat: config remembers archived sessions and the sidebar status filter

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Save the archived list

**Files:**
- Modify: `crates/forge-application/src/use_cases/settings.rs`
- Test: same file (`mod tests`, uses `MockConfigRepositoryPort` and `with_lists()`)

**Interfaces:**
- Consumes: `ArchivedRef`, `AppConfig.archived` (Task 1).
- Produces: `SettingsUseCase::save_archived(&self, archived: &[ArchivedRef]) -> Result<AppConfig, ConfigError>`.

- [ ] **Step 1: Write the failing tests**

```rust
    #[tokio::test]
    async fn save_archived_replaces_only_the_archived_list() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save()
            .withf(|c: &AppConfig| {
                c.archived.len() == 1
                    && c.archived[0].session_id == "s1"
                    && c.base_url.as_deref() == Some("https://old.test")
                    && c.enabled_skills == vec!["brainstorming".to_string()]
            })
            .times(1)
            .returning(|_| Ok(()));
        let usecase = SettingsUseCase::new(Arc::new(mock));
        let archived = vec![forge_domain::ArchivedRef { project: "/p".into(), session_id: "s1".to_string() }];

        let saved = usecase.save_archived(&archived).await.unwrap();

        assert_eq!(saved.archived, archived);
    }

    #[tokio::test]
    async fn save_archived_does_not_replace_a_config_it_cannot_read() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load()
            .times(1)
            .returning(|| Err(ConfigError::ParseFailed("bad".to_string())));
        mock.expect_save().times(0);

        let result = SettingsUseCase::new(Arc::new(mock)).save_archived(&[]).await;

        assert!(result.is_err());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p forge-application save_archived`
Expected: FAIL to compile, "no method named `save_archived`".

- [ ] **Step 3: Implement** (next to `save_sidebar`; import `ArchivedRef` in the `use forge_domain::{...}` line)

```rust
    /// Saves which sessions are archived. Like `save_sidebar`, a config that cannot be read is never
    /// replaced, and it shares the write lock so a quick archive and filter change both survive.
    pub async fn save_archived(&self, archived: &[ArchivedRef]) -> Result<AppConfig, ConfigError> {
        let _saving = self.write_lock.lock().await;
        let mut config = self.config.load().await?;
        config.archived = archived.to_vec();
        self.config.save(&config).await?;
        Ok(config)
    }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p forge-application`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-application
git commit -m "feat: save the archived session list

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Sidebar entries know they are archived; the menu offers Archive and key A

**Files:**
- Modify: `crates/forge-features/src/sidebar.rs`
- Modify: `crates/forge-features/src/lib.rs` (re-export `ArchiveAction`, `mark_archived`)
- Modify tests in: `sidebar.rs`, `search.rs`, `arrange.rs` (the `entry(...)` helpers)

**Interfaces:**
- Consumes: `ArchivedRef` (Task 1).
- Produces:
  - `SidebarEntry.archived: bool`
  - `pub fn mark_archived(groups: &mut [SidebarGroup], archived: &[ArchivedRef])`
  - `pub enum ArchiveAction { Archive, Unarchive, Unavailable }` (`Debug, Clone, Copy, PartialEq, Eq`)
  - `RowActions.archive: ArchiveAction`
  - `MenuKeyAction::ToggleArchive`

- [ ] **Step 1: Write the failing tests** (add to `mod tests` in `sidebar.rs`; the helper `entry(key, session)` exists)

```rust
    fn past_entry(project: &str, id: &str) -> SidebarEntry {
        entry(EntryKey::Past { project: PathBuf::from(project), session: SessionId(id.to_string()) }, Some(id))
    }

    fn one_group(project: &str, entries: Vec<SidebarEntry>) -> SidebarGroup {
        SidebarGroup {
            project: ProjectSummary {
                path: PathBuf::from(project),
                last_active: String::new(),
                session_count: entries.len(),
                exists: true,
            },
            entries,
        }
    }

    fn archived(project: &str, id: &str) -> ArchivedRef {
        ArchivedRef { project: PathBuf::from(project), session_id: id.to_string() }
    }

    #[test]
    fn mark_archived_flags_the_entries_the_list_names() {
        let mut groups = vec![one_group("/a", vec![past_entry("/a", "s1"), past_entry("/a", "s2")])];

        mark_archived(&mut groups, &[archived("/a", "s2")]);

        assert!(!groups[0].entries[0].archived);
        assert!(groups[0].entries[1].archived);
    }

    #[test]
    fn the_same_session_id_in_another_project_is_not_archived() {
        let mut groups = vec![
            one_group("/a", vec![past_entry("/a", "s1")]),
            one_group("/b", vec![past_entry("/b", "s1")]),
        ];

        mark_archived(&mut groups, &[archived("/a", "s1")]);

        assert!(groups[0].entries[0].archived);
        assert!(!groups[1].entries[0].archived, "only /a's s1 was archived");
    }

    #[test]
    fn an_archived_id_with_no_session_is_ignored() {
        let mut groups = vec![one_group("/a", vec![past_entry("/a", "s1")])];

        mark_archived(&mut groups, &[archived("/a", "gone"), archived("/zzz", "s1")]);

        assert!(!groups[0].entries[0].archived);
    }

    #[test]
    fn a_past_session_can_be_archived_and_an_archived_one_unarchived() {
        let mut past = entry(EntryKey::Past { project: PathBuf::from("/a"), session: SessionId("s".to_string()) }, Some("s"));
        assert_eq!(past.actions().archive, ArchiveAction::Archive);

        past.archived = true;
        assert_eq!(past.actions().archive, ArchiveAction::Unarchive);
    }

    #[test]
    fn an_open_session_cannot_be_archived_but_an_open_archived_one_can_be_restored() {
        let mut open = entry(EntryKey::Live(LiveSessionId(1)), Some("s"));
        assert_eq!(open.actions().archive, ArchiveAction::Unavailable);

        open.archived = true;
        assert_eq!(open.actions().archive, ArchiveAction::Unarchive);
    }

    #[test]
    fn a_session_with_no_claude_id_cannot_be_archived() {
        let fresh = entry(EntryKey::Live(LiveSessionId(1)), None);
        assert_eq!(fresh.actions().archive, ArchiveAction::Unavailable);
    }

    #[test]
    fn the_a_key_toggles_archive_by_the_physical_key() {
        let mut all = actions(true, DeleteAction::Allowed);
        all.archive = ArchiveAction::Archive;
        assert_eq!(menu_key_action("KeyA", false, all), Some(MenuKeyAction::ToggleArchive));
        all.archive = ArchiveAction::Unarchive;
        assert_eq!(menu_key_action("KeyA", false, all), Some(MenuKeyAction::ToggleArchive));
        all.archive = ArchiveAction::Unavailable;
        assert_eq!(menu_key_action("KeyA", false, all), None);
        all.archive = ArchiveAction::Archive;
        assert_eq!(menu_key_action("KeyA", true, all), None, "⌘A is select all, not archive");
    }
```

The test module may need `use forge_domain::ArchivedRef;` and `use std::path::PathBuf;` (check what its `use` lines already import). `LiveSessionId(1)` stands for however the existing tests build one; copy their construction.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p forge-features sidebar::tests`
Expected: FAIL to compile, "cannot find type `ArchiveAction`".

- [ ] **Step 3: Implement** in `sidebar.rs`

Add `use forge_domain::ArchivedRef;` to the `forge_domain` import. Add the field after `marker` in `SidebarEntry`:

```rust
    /// The person archived it (Forge's config remembers; `mark_archived` sets this).
    pub archived: bool,
```

Add `archived: false,` to the three `SidebarEntry { .. }` literals in `merge_sidebar` (lines ~208, ~220, ~243) and to the `entry(...)` test helpers in `sidebar.rs`, `search.rs` and `arrange.rs`.

Add the enum and field:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveAction {
    Archive,
    Unarchive,
    /// An open session that is not archived (close it first), or one with no transcript yet.
    Unavailable,
}
```

`RowActions` gets `pub archive: ArchiveAction,`. Update `actions()`:

```rust
    pub fn actions(&self) -> RowActions {
        let archive = match (&self.key, &self.session_id, self.archived) {
            (_, None, _) => ArchiveAction::Unavailable,
            (_, Some(_), true) => ArchiveAction::Unarchive,
            (EntryKey::Past { .. }, Some(_), false) => ArchiveAction::Archive,
            (EntryKey::Live(_), Some(_), false) => ArchiveAction::Unavailable,
        };
        match (&self.key, &self.session_id) {
            (_, None) => RowActions { can_rename: false, can_fork: false, delete: DeleteAction::Unavailable, archive },
            (EntryKey::Live(_), Some(_)) => RowActions { can_rename: true, can_fork: true, delete: DeleteAction::WhileOpen, archive },
            (EntryKey::Past { .. }, Some(_)) => RowActions { can_rename: true, can_fork: true, delete: DeleteAction::Allowed, archive },
        }
    }
```

`MenuKeyAction` gets `ToggleArchive`; update its doc line to mention A. In `menu_key_action` add:

```rust
        "KeyA" if actions.archive != ArchiveAction::Unavailable => Some(MenuKeyAction::ToggleArchive),
```

Add:

```rust
/// Sets `archived` on every entry that `archived` names (same project and same session id).
/// Names with no session are ignored.
pub fn mark_archived(groups: &mut [SidebarGroup], archived: &[ArchivedRef]) {
    for group in groups {
        for entry in &mut group.entries {
            entry.archived = entry.session_id.as_ref().is_some_and(|id| {
                archived
                    .iter()
                    .any(|item| item.project == group.project.path && item.session_id == id.0)
            });
        }
    }
}
```

Fix the existing `RowActions { .. }` literals in the tests (`actions()` helper and the `assert_eq!`s at ~426, 432, 438) by adding `archive: ArchiveAction::Archive` / `Unavailable` / `Unavailable` as the new rules say (past: Archive; open: Unavailable; fresh: Unavailable). In `lib.rs` add `ArchiveAction` and `mark_archived` to the `sidebar::{...}` re-export.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p forge-features`
Expected: PASS (all of `forge-features`, since three helpers changed).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: sidebar entries know when they are archived, and the row menu has A

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The status filter in `arrange`

**Files:**
- Modify: `crates/forge-features/src/arrange.rs`
- Test: same file (helpers `entry`, `group`, `prefs`, `now` exist)

**Interfaces:**
- Consumes: `SidebarEntry.archived` (Task 3), `SidebarPrefs.status` (Task 1).
- Produces: `arrange` drops entries by `prefs.status` before grouping.

- [ ] **Step 1: Write the failing tests**

```rust
    fn archived_entry(title: &str) -> SidebarEntry {
        SidebarEntry { archived: true, ..entry(title, None, Some("2026-09-29T00:00:00Z")) }
    }

    fn titles(arranged: &[ArrangedGroup]) -> Vec<String> {
        arranged.iter().flat_map(|g| g.entries.iter().map(|i| i.entry.title.clone())).collect()
    }

    fn with_status(status: StatusFilter, group_by: GroupBy) -> SidebarPrefs {
        SidebarPrefs { status, ..prefs(SortBy::Name, group_by) }
    }

    #[test]
    fn active_hides_archived_entries_and_archived_shows_only_them() {
        let groups = vec![group("/a", vec![entry("kept", None, None), archived_entry("shelved")])];

        let active = arrange(&groups, &with_status(StatusFilter::Active, GroupBy::Folder), now(), None);
        let archived = arrange(&groups, &with_status(StatusFilter::Archived, GroupBy::Folder), now(), None);
        let all = arrange(&groups, &with_status(StatusFilter::All, GroupBy::Folder), now(), None);

        assert_eq!(titles(&active), vec!["kept"]);
        assert_eq!(titles(&archived), vec!["shelved"]);
        assert_eq!(titles(&all), vec!["kept", "shelved"]);
    }

    #[test]
    fn the_status_filter_applies_in_date_and_flat_grouping_too() {
        let groups = vec![group("/a", vec![entry("kept", None, Some("2026-09-29T00:00:00Z")), archived_entry("shelved")])];

        for by in [GroupBy::Date, GroupBy::Flat] {
            let active = arrange(&groups, &with_status(StatusFilter::Active, by), now(), None);
            assert_eq!(titles(&active), vec!["kept"], "{by:?}");
        }
    }

    #[test]
    fn a_project_whose_sessions_are_all_archived_is_an_empty_group_in_the_active_view() {
        let groups = vec![group("/a", vec![archived_entry("shelved")])];

        let hidden = arrange(&groups, &with_status(StatusFilter::Active, GroupBy::Folder), now(), None);
        let shown = arrange(
            &groups,
            &SidebarPrefs { show_empty: true, ..with_status(StatusFilter::Active, GroupBy::Folder) },
            now(),
            None,
        );

        assert!(hidden.is_empty());
        assert_eq!(shown.len(), 1);
        assert!(shown[0].entries.is_empty());
    }
```

Add `StatusFilter` to the test module's `use forge_domain::{...}`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p forge-features arrange::tests`
Expected: FAIL (the tests compile; the first assertion fails because `arrange` shows archived entries).

- [ ] **Step 3: Implement.** In `arrange.rs`, rename the existing `arrange` to `arrange_filtered` (private, same body) and add:

```rust
fn shown(entry: &SidebarEntry, status: StatusFilter) -> bool {
    match status {
        StatusFilter::Active => !entry.archived,
        StatusFilter::Archived => entry.archived,
        StatusFilter::All => true,
    }
}

/// The sidebar as it is drawn: `groups` (from `merge_sidebar`) limited to the sessions `prefs.status`
/// asks for, then sorted, grouped and collapsed as `prefs` says. `now` is passed in so the date
/// buckets are testable; `active` is the open session on screen, whose row stays visible in a
/// collapsed group.
pub fn arrange(
    groups: &[SidebarGroup],
    prefs: &SidebarPrefs,
    now: OffsetDateTime,
    active: Option<LiveSessionId>,
) -> Vec<ArrangedGroup> {
    let limited: Vec<SidebarGroup> = groups
        .iter()
        .map(|group| SidebarGroup {
            project: group.project.clone(),
            entries: group.entries.iter().filter(|entry| shown(entry, prefs.status)).cloned().collect(),
        })
        .collect();
    arrange_filtered(&limited, prefs, now, active)
}
```

Move the existing doc comment of `arrange` onto nothing (delete it; the new one replaces it). Add `StatusFilter` to the `forge_domain` import at the top.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p forge-features`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features/src/arrange.rs
git commit -m "feat: the sidebar lists active, archived or all sessions

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Status row in the filter menu

**Files:**
- Modify: `crates/forge-features/src/filter_menu.rs`
- Modify: `crates/forge-features/src/lib.rs` (re-export `status_label`, `STATUS_ITEMS`)
- Modify: `crates/forge-widgets/src/filter_menu.rs`

**Interfaces:**
- Consumes: `StatusFilter`, `SidebarPrefs.status`.
- Produces: `STATUS_ITEMS: [StatusFilter; 3]` (Active, Archived, All); `status_label(StatusFilter) -> &'static str`; `FilterRow::Status` (first row, so `FILTER_ROWS` has four); `FilterChoice::Status(StatusFilter)`.

- [ ] **Step 1: Write the failing tests and fix the three that assume the row order.** In `filter_menu.rs` `mod tests`:

Replace `up_and_down_move_over_the_three_rows_and_stop_at_the_ends` with:

```rust
    #[test]
    fn up_and_down_move_over_the_four_rows_and_stop_at_the_ends() {
        let prefs = SidebarPrefs::default();
        let mut menu = FilterMenu::default();

        press(&mut menu, &[FilterKey::Up], &prefs);
        assert_eq!(menu.top, 0);
        press(&mut menu, &[FilterKey::Down, FilterKey::Down, FilterKey::Down, FilterKey::Down, FilterKey::Down], &prefs);
        assert_eq!(menu.top, 3);
    }
```

In `right_opens_the_submenu_on_the_current_value` change the key list to `&[FilterKey::Down, FilterKey::Right]` (Sort is now the second row). In `enter_in_a_submenu_chooses...` change the keys to `&[FilterKey::Down, FilterKey::Down, FilterKey::Right, FilterKey::Down, FilterKey::Enter]` (Group is the third row). In `enter_on_show_empty_toggles...` change `&[FilterKey::Down, FilterKey::Down]` to three `Down`s. Add:

```rust
    #[test]
    fn the_first_row_is_status_and_it_opens_on_the_current_value() {
        let prefs = SidebarPrefs { status: StatusFilter::Archived, ..SidebarPrefs::default() };
        let mut menu = FilterMenu::default();

        press(&mut menu, &[FilterKey::Right], &prefs);

        assert_eq!(menu.sub, Some(1));
        let outcome = press(&mut menu, &[FilterKey::Down, FilterKey::Enter], &prefs);
        assert_eq!(outcome, FilterOutcome::Choose(FilterChoice::Status(StatusFilter::All)));
    }

    #[test]
    fn choosing_a_status_changes_the_preferences() {
        let mut prefs = SidebarPrefs::default();

        FilterChoice::Status(StatusFilter::Archived).apply(&mut prefs);

        assert_eq!(prefs.status, StatusFilter::Archived);
        assert_eq!(status_label(StatusFilter::Active), "Active");
        assert_eq!(status_label(StatusFilter::Archived), "Archived");
        assert_eq!(status_label(StatusFilter::All), "All");
    }
```

Add `StatusFilter` to the test module's `forge_domain` import.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p forge-features filter_menu`
Expected: FAIL to compile, "cannot find ... `StatusFilter`" / `status_label`.

- [ ] **Step 3: Implement** in `filter_menu.rs`

```rust
pub const STATUS_ITEMS: [StatusFilter; 3] = [StatusFilter::Active, StatusFilter::Archived, StatusFilter::All];

pub fn status_label(status: StatusFilter) -> &'static str {
    match status {
        StatusFilter::Active => "Active",
        StatusFilter::Archived => "Archived",
        StatusFilter::All => "All",
    }
}
```

Change the import to `use forge_domain::{GroupBy, SidebarPrefs, SortBy, StatusFilter};`. Add `Status` as the first variant of `FilterRow` and make `FILTER_ROWS: [FilterRow; 4] = [FilterRow::Status, FilterRow::Sort, FilterRow::Group, FilterRow::ShowEmpty]`. Add `Status(StatusFilter)` to `FilterChoice` with `FilterChoice::Status(status) => prefs.status = status,` in `apply`. In `current_index` add `FilterRow::Status => STATUS_ITEMS.iter().position(|item| *item == prefs.status).unwrap_or(0),`. In `key`, the submenu `Enter` match gets `FilterRow::Status => FilterChoice::Status(STATUS_ITEMS[index]),`. Re-export `status_label` and `STATUS_ITEMS` from `lib.rs` in the `filter_menu::{...}` list.

In `crates/forge-widgets/src/filter_menu.rs`: import `status_label` and `STATUS_ITEMS`; add to the `sub_items` match

```rust
        FilterRow::Status => STATUS_ITEMS
            .iter()
            .map(|item| (status_label(*item).to_string(), prefs.status == *item, FilterChoice::Status(*item)))
            .collect(),
```

and to the row-label `match row`

```rust
                                FilterRow::Status => rsx! {
                                    span { "Status" }
                                    span { class: "value", "{status_label(prefs.status)} ›" }
                                },
```

Update the doc comment to "status, sort by, group by, show empty groups". Change the button title to "Filter, sort and group sessions".

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p forge-features && cargo build -p forge-widgets`
Expected: PASS and a clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features crates/forge-widgets/src/filter_menu.rs
git commit -m "feat: a Status row in the sidebar filter menu

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Workspace keeps the archived list

**Files:**
- Modify: `crates/forge-features/src/workspace.rs`

**Interfaces:**
- Consumes: `mark_archived` (Task 3), `SettingsUseCase::save_archived` (Task 2), `ArchivedRef`.
- Produces on `Workspace`:
  - `pub fn is_archived(&self, project: &Path, session: &SessionId) -> bool`
  - `pub fn archive_session(&self, project: PathBuf, session: SessionId)`
  - `pub fn unarchive_session(&self, project: PathBuf, session: SessionId)`
  - `sidebar()` returns entries with `archived` set.

The workspace is Dioxus signals and is not unit-tested in this crate. The pure part that decides the new list lives in a free function that is.

- [ ] **Step 1: Write the failing tests** (a new `mod tests` at the bottom of `workspace.rs`, or append to the existing one if present)

```rust
    #[test]
    fn archiving_adds_once_and_unarchiving_removes() {
        let one = ArchivedRef { project: "/a".into(), session_id: "s1".to_string() };
        let mut list = vec![];

        toggle_archived(&mut list, Path::new("/a"), "s1", true);
        toggle_archived(&mut list, Path::new("/a"), "s1", true);
        assert_eq!(list, vec![one]);

        toggle_archived(&mut list, Path::new("/b"), "s1", false);
        assert_eq!(list.len(), 1, "another project's s1 is a different session");
        toggle_archived(&mut list, Path::new("/a"), "s1", false);
        assert!(list.is_empty());
    }
```

(`use std::path::Path;` and `use forge_domain::ArchivedRef;` in the test module if not already imported.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p forge-features toggle_archived`
Expected: FAIL to compile, "cannot find function `toggle_archived`".

- [ ] **Step 3: Implement.** Imports: add `ArchivedRef` to the `forge_domain` line and `mark_archived` to the `crate::sidebar` line; `use std::path::{Path, PathBuf};`.

Free function (near `load_past`):

```rust
/// Adds or removes one session in the archived list; adding twice keeps one entry.
fn toggle_archived(list: &mut Vec<ArchivedRef>, project: &Path, session: &str, archive: bool) {
    let same = |item: &ArchivedRef| item.project == project && item.session_id == session;
    if archive {
        if !list.iter().any(same) {
            list.push(ArchivedRef { project: project.to_path_buf(), session_id: session.to_string() });
        }
    } else {
        list.retain(|item| !same(item));
    }
}
```

Add the field `archived: Signal<Vec<ArchivedRef>>,` to `Workspace`, `archived: Signal::new(Vec::new()),` in the provider, and load it with the prefs: in `reload_prefs` take `let mut archived = self.archived;` and inside the spawn after `prefs.set(config.sidebar);` add `archived.set(config.archived);` (the first line becomes `prefs.set(config.sidebar.clone());`, or move both from `config` after cloning `config.sidebar`).

Change `sidebar()`:

```rust
        let mut groups = merge_sidebar(&self.past.read(), &open);
        mark_archived(&mut groups, &self.archived.read());
        groups
```

Add the methods (after `delete_session`):

```rust
    /// Whether the person archived this session.
    pub fn is_archived(&self, project: &Path, session: &SessionId) -> bool {
        self.archived
            .read()
            .iter()
            .any(|item| item.project == project && item.session_id == session.0)
    }

    /// Hides a session from the default list. Nothing on disk changes.
    pub fn archive_session(&self, project: PathBuf, session: SessionId) {
        self.set_archived(project, session, true);
    }

    /// Puts an archived session back in the default list.
    pub fn unarchive_session(&self, project: PathBuf, session: SessionId) {
        self.set_archived(project, session, false);
    }

    fn set_archived(&self, project: PathBuf, session: SessionId, archive: bool) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        let mut archived = self.archived;
        toggle_archived(&mut archived.write(), &project, &session.0, archive);
        let list = self.archived.peek().clone();
        // Root scope: the save must finish even if the row that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            if let Err(error) = services.settings.save_archived(&list).await {
                this.report(format!("failed to save the archived sessions: {error}"));
            }
        });
    }
```

Also delete: in `delete_session`, after `Ok(()) =>` add `this.set_archived(project.clone(), session.clone(), false);` before `this.reload_project(project)` (a deleted session leaves the list). `session` is moved into `registry.delete(&project, &session)` by reference, so the clone is fine; if the borrow checker complains, clone before the `match`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p forge-features && cargo build --workspace`
Expected: PASS and a clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features/src/workspace.rs
git commit -m "feat: the workspace archives and unarchives sessions and saves the list

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The row menu, the icon and the Unarchive bar

**Files:**
- Modify: `crates/forge-widgets/src/session_row.rs`
- Modify: `crates/forge-widgets/src/chat_window.rs`
- Modify: `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `RowActions.archive`, `ArchiveAction`, `MenuKeyAction::ToggleArchive`, `SidebarEntry.archived`, `Workspace::archive_session`, `unarchive_session`, `is_archived`.

These are Dioxus views; they are checked by running the app (Task 8). Keep the logic in the tested functions above.

- [ ] **Step 1: Row menu item and key.** In `session_row.rs` import `ArchiveAction` from `forge_features`. Change `has_menu` to also be true when `actions.archive != ArchiveAction::Unavailable`, and the button title to "Rename, fork, archive or delete".

In the `onkeydown` match add (before `None => {}`):

```rust
                                        Some(MenuKeyAction::ToggleArchive) => {
                                            event.stop_propagation();
                                            event.prevent_default();
                                            if let Some(id) = session_id.clone() {
                                                match actions.archive {
                                                    ArchiveAction::Archive => workspace.archive_session(project.clone(), id),
                                                    _ => workspace.unarchive_session(project.clone(), id),
                                                }
                                            }
                                            mode.set(RowMode::Normal);
                                            refocus_workspace();
                                        }
```

Between the Fork item and the Delete item add:

```rust
                            if actions.archive != ArchiveAction::Unavailable {
                                button {
                                    class: "row-menu-item",
                                    onclick: {
                                        let project = project.clone();
                                        let session_id = session_id.clone();
                                        move |_| {
                                            if let Some(id) = session_id.clone() {
                                                match actions.archive {
                                                    ArchiveAction::Archive => workspace.archive_session(project.clone(), id),
                                                    _ => workspace.unarchive_session(project.clone(), id),
                                                }
                                            }
                                            mode.set(RowMode::Normal);
                                            refocus_workspace();
                                        }
                                    },
                                    span { if actions.archive == ArchiveAction::Archive { "Archive" } else { "Unarchive" } }
                                    span { class: "menu-key", "A" }
                                }
                            }
```

- [ ] **Step 2: The icon.** `SidebarItem` (in `forge-entities`) takes the title; show an archived row with a prefix instead of changing that crate: in `session_row.rs` pass

```rust
                        title: if entry.archived { format!("🗄 {}", entry.title) } else { entry.title.clone() },
```

(In the Renaming and Confirming modes the plain `entry.title` is still used, which is right.)

- [ ] **Step 3: The bar.** In `chat_window.rs`, before the `rsx!`, compute:

```rust
    let archived = view
        .claude_session_id
        .as_ref()
        .is_some_and(|session| workspace.is_archived(&view.project_path, session));
```

and replace `if view.can_send { ... } else { ... }` by:

```rust
            if archived {
                div { class: "banner info archived-bar",
                    span { "This session is archived." }
                    button {
                        class: "archived-bar-button",
                        onclick: {
                            let project = view.project_path.clone();
                            let session = view.claude_session_id.clone();
                            move |_| {
                                if let Some(session) = session.clone() {
                                    workspace.unarchive_session(project.clone(), session);
                                }
                            }
                        },
                        "Unarchive"
                    }
                }
            } else if view.can_send {
                (existing SendMessageBox block, unchanged)
            } else {
                (existing "folder no longer exists" banner, unchanged)
            }
```

- [ ] **Step 4: CSS.** Append to `theme.css` next to `.banner.info`:

```css
.archived-bar { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.archived-bar-button { border: 1px solid var(--border); background: transparent; color: var(--fg); border-radius: var(--radius); padding: 4px 10px; cursor: pointer; }
.archived-bar-button:hover { background: var(--hover); }
```

If `--hover` is not a token in the file, use whichever hover token `.row-menu-item:hover` uses.

- [ ] **Step 5: Build and commit**

Run: `cargo build --workspace && cargo test --workspace`
Expected: clean build, all tests PASS.

```bash
git add crates/forge-widgets crates/forge-shared
git commit -m "feat: Archive in the row menu and key A, and an Unarchive bar in the chat

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Check it in the running app, then update the docs

**Files:**
- Modify: `docs/superpowers/specs/2026-10-03-archive-design.md` (as-built notes, deviations above)
- Modify: `docs/superpowers/roadmap/backlog.md` (SES-02, SES-06, SES-08 rows)
- Modify: `docs/superpowers/roadmap/README.md` if it lists plans

- [ ] **Step 1: Drive the app.** Follow the memory note "Driving the Forge app visually" (screencapture, Quartz clicks, a `FORGE_CLAUDE_HOME` fixture with two projects and three sessions, one project having a session id that the other also has). Check and screenshot, in order:
  1. Row menu shows Archive with A; press A: the row leaves the list.
  2. Quit and start again: it is still gone (the config kept it).
  3. Filter menu, Status, Archived: the session shows with the 🗄 prefix; All shows both kinds.
  4. Open the archived session: the Unarchive bar replaces the message box.
  5. Click Unarchive: the message box returns and Status Active lists it.
  6. With a Korean input method on, open a row menu and press the A key: it archives.
  7. The same id in the other project stays listed.

- [ ] **Step 2: Run the whole suite**

Run: `cargo test --workspace`
Expected: PASS. Report any failure with its output; do not claim success without it.

- [ ] **Step 3: Docs.** In `backlog.md`: SES-02 Forge `has`, Plan `Plan 20 (built)`, with a note of what was built and that the menu wording and the bar's wording are assumptions; SES-06 Forge `has` (R, F, A, D); SES-08 notes: the status filter is built. Fill the spec's "As-built notes" with the five deviations from the top of this plan and the results of Step 1. Commit:

```bash
git add docs
git commit -m "docs: Plan 20 as built, backlog updated

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
