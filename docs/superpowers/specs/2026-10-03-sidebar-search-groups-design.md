# Plan 17: sidebar search, filter menu and collapse (SES-07, SES-08, SES-11)

Plan 14 listed the sidebar of Claude Code desktop. Plan 17 builds the parts that fit Forge. Evidence is in `docs/superpowers/roadmap/observations.md` (round 1, and round 3 for the filter menu and the search palette).

## Scope, decided with the user on 2026-10-03

The user chose the roadmap order (sidebar search and groups) and chose to include body search.

| Id | Decision | Why |
| --- | --- | --- |
| SES-07 search palette | build: sessions only, title and body | Seen: a palette in the middle of the window; results show the title, a quote of the matching text, a relative date. Forge has only Code sessions, so the tabs Artifacts, 프로젝트, 예약됨 and 코드 (same as 세션) are not built. The "작성" mode was never seen. |
| SES-08 filter menu | build: sort (이름 / 생성 날짜 / 최근 활동), group by (폴더 / 날짜 / 없음), show empty groups | Seen with its values (round 3). |
| SES-08 status filter (활성 / 보관됨 / 전체) | not built | Needs SES-02 (archive). |
| SES-08 environment filter | not built | Forge has only local sessions. |
| SES-08 "PR 상태 표시" | not built | Forge has no PR data. |
| SES-08 group by 상태, 사용자 지정 그룹 | not built | No status to group by (see status filter); nothing creates custom groups. |
| SES-08 collapse chevron on a group | build | Seen on hover. |
| SES-11 collapse the sidebar, resize it | build | Seen: toggle button in the title bar. Resizing was not tried in the app; Forge adds a drag handle (an assumption, stated in the as-built notes). |
| SES-11 back / forward history buttons | not built | Left in the backlog; it needs a navigation history model. |
| SES-10 "사이드바 편집…" | not built | Contents never seen. |

## What exists today

`workspace.sidebar()` returns `Vec<SidebarGroup>`, one group per project in the order the project port gives, each with `SidebarEntry { key, session_id, title, updated_at, marker }` (`forge-features/src/sidebar.rs`). `merge_sidebar` mixes the sessions on disk with the open ones. `Sidebar` (`forge-widgets/src/sidebar.rs`) draws a "+ New session" button, a banner, the groups (heading and `+`) and a Settings button. `SessionSummary` has `id`, `title`, `updated_at`; there is no creation time. `AppConfig` (`forge-domain/src/config.rs`) holds the saved settings and is read and written through `ConfigRepositoryPort`.

## Sidebar view: sort, group, empty groups, collapse

```rust
pub struct SidebarView {
    pub sort_by: SortBy,        // Name | Created | Activity   (default Activity)
    pub group_by: GroupBy,      // Folder | Date | None        (default Folder)
    pub show_empty: bool,       // default false
}
```

A pure function `arrange(groups, &view, now) -> Vec<SidebarGroup>` in `forge-features` applies it to what `merge_sidebar` returns. No I/O, no clock of its own (`now` is passed in), so every case is a plain test.

- **Folder** (today's grouping): one group per project, projects in their given order. Entries are sorted by `sort_by` inside each group.
- **Date**: all entries of all projects in one list, sorted by `sort_by`, then split into buckets by the date that matches the sort (activity for "최근 활동" and "이름", creation for "생성 날짜"): 오늘, 어제, 지난 7일, 지난 30일, 이전. The heading is the bucket name. An entry keeps its project, so opening, renaming and deleting still work; its row shows the project name as the subtitle (a row in a Folder group has none). Round 1 saw chats "grouped by date" in chat mode, but the bucket names and limits were never seen; these five are a choice made here, and this is stated in the as-built notes.
- **None**: one group with no heading, entries sorted. Same row subtitle as Date.
- **Empty groups**: a project with no sessions is hidden unless `show_empty` is on. Only Folder grouping has empty groups. A project that is the target of a new session in the middle of creation is not special-cased.
- **Sort**: 이름 is case-insensitive title order (a session without a title sorts by its first message text as shown today); 생성 날짜 newest first; 최근 활동 newest first. An entry with no time (a new session with no transcript yet) sorts first.

`SessionSummary` gets `created_at: String`. The Claude fs adapter takes it from the timestamp of the first line of the transcript, falling back to the file's creation time, then to `updated_at`. `SidebarEntry` gets `created_at: Option<String>`.

**Collapse.** Each group heading gets a chevron; a click hides the group's rows. State is a set of project paths (Folder grouping) or bucket names (Date) in `SidebarView.collapsed`. The group that holds the active session cannot hide its active row: it stays collapsed but the active row is shown (so the user never loses sight of where they are).

**Saved.** `AppConfig` gets `sidebar: SidebarPrefs { sort_by, group_by, show_empty, collapsed, width, hidden }`, all `#[serde(default)]`, so an old config file loads unchanged. Saved on each change through the existing port.

## Filter menu

The icon at the right of the first group's heading in the real app opens one menu for all groups (round 3). Forge puts the icon in the sidebar, next to the search box, because with Date or None grouping there may be no group heading to hold it. This is a deliberate difference and is written in the as-built notes.

Rows: "정렬 기준" (current value shown at the right, submenu of three radio items), "그룹화 기준" (three), "빈 그룹 표시" (checkbox). The menu reuses the keyboard and focus handling of the session row menu from Plan 15 (arrow keys, Right opens a submenu, Left or Escape closes it, number keys by physical key so a Korean input method does not break them).

Forge's UI text is English today, so the labels are "Sort by", "Group by", "Show empty groups", and the items "Name / Created / Activity" and "Folder / Date / None". The Korean wording above is the evidence, not a copy target.

## Search palette

Opened by the search box at the top of the sidebar and by `⌘K` (the real app's shortcut was not seen; `⌘K` is an assumption, stated in the as-built notes; if it conflicts with something Forge already binds, the plan drops it).

- A modal over a dimmed window, input with a close `x`, Escape closes.
- Results: one row per matching session, with the title (match in bold), a quote of the matching text with the match in bold (nothing when only the title matched), the project name, and a relative date at the right ("지난 1년" in the real app; Forge: "3 days ago", "last month", the same scale).
- Up and Down move the highlight, Enter opens the highlighted session (a past session opens view-only or live as the sidebar row does today, through the same `open` path), a click opens it too. The palette closes on open.
- No query: shows the most recent sessions (the real app's empty state was not seen; this is the cheapest thing that is useful, and is stated in the as-built notes). No match: a line "No sessions match".
- Matching is case-insensitive substring over the title and the message text (user and assistant text blocks; not tool input, tool results or thinking). Not fuzzy. The match is on the characters typed, so Korean input works; while an input method is composing, the query is not run (the composition event, as the composer already does).

### Port

`SessionRepositoryPort` gets:

```rust
async fn search_sessions(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, SessionRepositoryError>;

pub struct SearchHit { pub project: PathBuf, pub session: SessionSummary, pub snippet: Option<Snippet> }
pub struct Snippet { pub before: String, pub matched: String, pub after: String }  // about 40 chars each side
```

The Claude fs adapter scans every project's transcripts for the query. It reads each `.jsonl` line by line and stops at the first matching text block of a session (one snippet per session), and keeps the title match separate. It skips lines that cannot contain a user or assistant text block without parsing them (a cheap `contains` check on the raw line first, then a parse only for lines that pass). Sessions are ordered by `updated_at`, newest first, and the scan stops after `limit` hits (50).

Cost control, since transcripts can be large and numerous: the query is run after 200 ms of no typing; a newer query cancels the older one (the page drops the older task's result by a counter; the scan itself runs on a blocking thread and checks a cancel flag between files). Nothing is indexed or cached on disk.

Sessions that are open in Forge and not yet written to disk are searched in memory by the feature layer (title only plus the message text in memory), and merged with the port's hits, so a session never disappears from search because it is live.

## Sidebar collapse and width (SES-11)

A toggle button at the left of the top bar hides the sidebar. No keyboard shortcut is added, because the real app's was not seen. The saved `hidden` flag restores it on start. When hidden, the chat takes the full width. A drag handle on the right edge resizes it between 200 and 480 px, saved as `width`. Default is today's width.

## Layout

```
+----------------------+
| [search box      ⌘K] |   opens the palette
| [filter]  + New      |   filter menu icon, new session
|  ▾ forge          +  |   group heading: chevron, name, add
|    ● session title   |
|  ▸ other             |   collapsed
| ⚙ Settings           |
+----------------------+
```

## Testing

- `arrange`: each sort, each group, empty groups on and off, the date buckets at the edges (midnight, exactly 7 days), entries with no time, Korean titles, a collapsed group that holds the active session.
- `AppConfig`: an old file with none of the new fields loads with the defaults; a round trip keeps them.
- The adapter: `search_sessions` against a temp directory of transcripts (a match in the title only, in the body only, in tool input only (no hit), in thinking only (no hit), two matches in one session (one hit), case-insensitive match, a Korean query, a transcript with a bad line (skipped, not a failure), the limit).
- The palette: pure functions for the snippet cut (match at the start, at the end, longer than the window, multibyte characters not split) and for the relative date.
- A harness scenario (Plan 13 harness) that opens the palette, types, and checks the results; the menu keys checked with the harness as Plan 15 did.
- Real-app comparison is by screenshot against `observations.md`, not by pixel diff.

## Not verified in the real desktop app (to state in the as-built notes)

Enter on a result, the `...` on a result, the "작성" mode, the palette's shortcut, the empty and no-match views, the exact Code-mode date buckets, whether the sidebar resizes. Each is a choice made here, not a copy.
