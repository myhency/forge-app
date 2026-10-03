# Backlog

Columns and values are explained in [README.md](README.md). Ids are never reused. Sizes: S (about one task), M (a plan), L (more than one plan).

Forge status was read from `crates/` on 2026-10-03.

## SES: sessions and the sidebar

| Id | Feature | Seen | Forge | Size | Plan | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| SES-01 | Fork a session (menu item "포크", key F) | seen | none | M | Plan 19 (built) | Built: menu item "Fork" and key F; the adapter copies the transcript with new uuids (not the SDK call). Seen in round 4: at once, copies the whole transcript into a new session "<title> (fork)" in the same project, opened and selected; folder, model, effort and mode are kept; the original stays. SDK `forkSession` does the copy. |
| SES-02 | Archive a session (menu item "보관", key A), and a way to see and restore archived ones | seen | has | M | Plan 20 (built) | Round 4: archived sessions show under the status filter "보관됨" with an archive icon; opened, they are read-only with a bar "보관 해제". Built in Plan 20: menu item Archive / Unarchive (key A), a Status filter (Active, Archived, All) and a bar "This session is archived. Unarchive" in place of the message box. Forge remembers it in its own config (`archived`), not in the transcript. The menu item "보관" itself, and the wording of the bar, were not seen in the real app: both are assumptions. Compare Forge's delete (spec §19). |
| SES-03 | View transcript (submenu): 일반 (normal), 사고 과정 (thinking), 자세히 (detailed) | seen | has | S | Plan 15 (built) | Radio choice, normal is the default. Forge: a Normal / Thinking / Detailed control in the chat header. What the desktop app shows for "자세히" was not seen (needs OBS-01); Forge opens each tool call with input and result. |
| SES-04 | Output style (submenu): only "새 스타일..." | seen | none | S | won't? | No built-in styles listed. What "새 스타일..." does was not tried. |
| SES-05 | Open in (submenu): 새 창 (new window), 클라우드 (run in the cloud) | seen | none | S | won't? | Not editors or Finder. Cloud runs are out of scope; a new window needs multi-window. The user decides. |
| SES-06 | Single-key shortcuts in the session menu (R rename, F fork, A archive, D delete) | seen | has | S | Plan 15 (built: R, D) | F built in Plan 19; A built in Plan 20. Forge matches the physical key, so R and D work with a Korean input method on. |
| SES-07 | Sidebar search | seen | partial | M | Plan 17 (built: sessions only) | A palette in the middle of the window over a dimmed app (not a filter of the list): tabs 전체, Artifacts, 세션, 코드, 프로젝트, 예약됨; rows with a kind icon, the title and a quote of the matching text (match in bold) and a relative date. Forge: title and message text of every session, quote, project, date, ⌘K, Up/Down/Enter/Escape; no tabs. Not seen: what Enter and `...` on a row do, the "작성" mode, the shortcut, the empty and no-match views. |
| SES-08 | Project group header tools: `+` new session in that project, filter/sort icon, collapse chevron | seen | partial | M | Plan 17 (built: sort, group, empty groups, collapse) | Filter menu seen (observations round 3): 상태, 환경, 그룹화 기준 (날짜, 폴더, 상태, 사용자 지정 그룹, 없음), 정렬 기준 (이름, 생성 날짜, 최근 활동), 빈 그룹 표시, PR 상태 표시. Forge built sort (Name / Created / Activity), group by (Folder / Date / None), show empty groups, and the collapse chevron. Status built in Plan 20 (Active, Archived, All, in the filter menu's first row). Not built: environment, PR state, custom groups, group by status. Forge puts the filter icon beside "+ New session", not in a group heading. |
| SES-09 | Recovery screen for a session whose transcript is missing ("CLI 세션 가져오기", archive, delete) and a bar for a missing folder ("폴더 선택", archive) | seen | partial | M | won't (Plan 15) | Forge opens such a session view-only (spec §23). The import action was not tried. Plan 15: the missing-transcript screen cannot occur in Forge, which lists sessions from the transcripts on disk; "폴더 선택" is unchecked. |
| SES-10 | Edit the sidebar ("사이드바 편집…", under "더보기") | partly | none | S | - | Only the menu item seen. Probably chooses which items show. |
| SES-11 | Sidebar width, collapse button, back/forward history buttons in the title bar | seen | partial | S | Plan 17 (built: collapse, resize) | Collapse and resize built (a drag handle; the real app was not checked for resizing). Back and forward are SES-12. |
| SES-12 | Back and forward history buttons in the title bar | seen | none | M | - | Left out of Plan 17 (SES-11); needs a navigation history model. |

## CMP: composer (the input box)

| Id | Feature | Seen | Forge | Size | Plan | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| CMP-01 | `+` menu: attach file or photo (⌘U), add folder, import a GitHub issue, slash commands, connectors (submenu), plugins (submenu) | seen | partial | L | Plan 18 (built: file or photo, folder, slash commands, connectors, plugins; not built: GitHub issue) | Forge has slash commands (spec §21, §22). The GitHub item is greyed with "이 폴더에는 GitHub 원격 저장소가 없습니다" when the folder has no remote. Attachments need a protocol change. |
| CMP-02 | Voice input (microphone button with a chevron) | partly | none | L | - | Chevron menu not opened. Needs OS audio and speech; decide if in scope. |
| CMP-03 | Permission mode menu with five entries: 자동 "Claude가 권한 결정을 처리합니다", 수동 "변경하기 전에 항상 확인", 편집 자동 수락, 계획, 권한 무시 (separate "활성화" button); number keys 1 to 4 | seen | partial | M | Plan 16 (built) | Forge has four modes (default, accept edits, plan, bypass; `permission_mode.rs`) and no "auto" mode. Check the SDK for an auto mode before planning. Bypass needs an extra step to enable. Plan 16: the SDK has `auto`; Forge now has five modes, changed in the composer, bypass asks first. |
| CMP-04 | Model picker in the composer: Opus 5.5, Sonnet 5.5 (badge "기본값"), Fable 5.1 (badge "사용 크레딧 필요"), Haiku 4.5, "더 많은 모델" submenu; keys 1 to 4 | seen | partial | M | Plan 16 (built) | Forge sets the default model in Settings only, and a choice applies to the next new session (spec §16). Changing the model of a running session was not tried in the app. Plan 16: Forge changes it in the running session with the SDK's `setModel`; "더 많은 모델" and the badges are not built. Seen with the real SDK: Sonnet 5.5 (`claude-sonnet-5-5`) is refused by the bundled Claude Code 2.1.283 and Fable 5.1 needs usage credits; Settings uses the same ids (OTH-07). |
| CMP-05 | Effort level next to the model ("중간") | seen | partial | M | Plan 19 (built; no "추천", five stops from the SDK) | Round 4: a popover with a slider of six stops from "더 빠르게" to "더 스마트하게", the recommended stop marked "추천". The SDK has `applyFlagSettings({effortLevel})`, a query option `effort` and `supportedEffortLevels` per model (five levels); the sixth stop's name is not known. |
| CMP-06 | Environment chips above the composer on a new session: "로컬" (where it runs), the folder name, an add-folder button | seen | partial | S | Plan 18 (built: "Local" and folder-name chips; the "로컬" menu was not seen); Plan 24 (built: folder menu with "Recent" and "Open folder…", add-folder button, mode and model pickers, send icon; not built: "Local" menu, "No folder") | Forge has a folder choice on the new-session screen (spec §22). The "로컬" chip's menu (other environments) was not opened. |
| CMP-07 | Circle at the right end of the composer's bottom row | seen | partial | S | won't (Plan 19) | Round 4: a popover with context use (56.4k / 1M), plan limits (session and weekly) and cloud credit. Forge's `UsageRing` already shows context, tokens and cost; plan limits come from the account and cannot be read from transcripts. |

## PNL: panels and tools in a session

| Id | Feature | Seen | Forge | Size | Plan | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| PNL-01 | Terminal panel docked on the right: tabs with `+`, expand to full width, close; shows "Working directory ... does not exist" when the folder is gone | seen | partial | L | Plan 21A (built) | Needs a pty on the Rust side. Shell content and keys were not tried (the folder was missing). Built in Plan 21A: a `>_` toggle in the chat header opens a column at the right (480 px, 280 to 900, drag the left edge), tabs with `+` and `×`, expand and collapse, close; a login shell in the session folder (pty in Rust, screen by xterm.js). A missing folder shows the red message and a Restart bar; a shell that exits shows "Process exited (n)" and Restart. Not seen in the real app and not built: whether the terminal stacks with Changes and Preview; leaving a session ends its shells (the dock layout is kept). |
| PNL-02 | Changes panel (second toolbar icon) | seen | partial | L | Plan 21B (built) | Round 5: a column at the right with a list icon (toggles a file list next to the diff), `⋮` with "새로고침", expand, close; empty text "표시할 변경 사항이 없습니다". Not seen with real changes (diff, file rows, actions). Forge has a diff viewer for Edit/Write calls (spec §18). Built in Plan 21B, read only: a `±` toggle, a file list and the diff from `git` (status letter, +/- counts), Refresh, and a reload when a tool call finishes. Left: stage, revert, commit. |
| PNL-03 | Preview browser panel (globe icon) | seen | partial | L | Plan 21B (built) | Round 5: tabs ("새 탭", `+`), back/forward/reload, a URL field, an element-picker icon, a layout icon, `⋮`, expand, close; a first-run modal offers importing browser cookies. Not seen with a loaded page. Needs an embedded web view; decide scope (cookie import: leave out?). Built in Plan 21B: tabs, per-tab history, reload, address bar; the page opens in the system browser because the window's web view opens every `http(s)` address outside and an `<iframe>` stays blank (spike). Left: a page inside the panel (a `forge://` proxy into an `<iframe>`, or a child web view), cookie import, element picker, layout icon. |
| PNL-04 | Files panel (`⋮` menu, ⇧⌘F) | seen | partial | L | Plan 21B (built) | Round 5: a file tree plus an editor with a filter field (`?` searches inside files) and `⋮` options (tree ⇧⌘Y, preview tab, auto save, Tab indent, word wrap, indent guides, whitespace, scroll past end, tab size 4). Not seen with files. Writing files is new for Forge. Built in Plan 21B, read only: Files in the header `⋮` menu, lazy tree without `.git`/`node_modules`, name filter, `?` content search, highlighted viewer with line numbers. Left: the editor, auto save and the other options, ⇧⌘F and ⇧⌘Y. |
| PNL-05 | iOS simulator (`⋮` menu) | partly | none | L | won't? | macOS and Xcode only. Propose leaving out; the user decides. |
| PNL-06 | "Prevent sleep" toggle, "이 세션에서만" (this session only) | seen | has | S | Plan 21A (built) | Toggle seen in `⋮`; not switched on. Needs a macOS sleep assertion. Built in Plan 21A: the chat header `⋮` menu has "Prevent sleep" ("This session only"); it runs `caffeinate -i -w <forge pid>` per session, off by default, not saved, released when the session closes. |
| PNL-07 | Toolbar with title, a rename/fork menu under the title chevron, the folder name as a tag | seen | partial | S | - | Forge has a header; compare the details when planning SES. |

## PG: pages outside a session

| Id | Feature | Seen | Forge | Size | Plan | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| PG-01 | Routines: list of scheduled tasks with title, prompt, state ("일시중지됨") and schedule; tabs "내 항목" and "템플릿"; search, filter, sort; "새 루틴" menu; notice that local routines run only while the computer is on and online | seen | none | L | - | Creating one was not tried. Needs a scheduler; the notice means no cloud run is required. |
| PG-02 | Customize page: tabs skills, connectors, plugins; "내 항목" and "탐색" (explore); "추가" menu; search, filter, sort; featured banner and cards with "체험" (try) and `+` | seen | partial | L | - | Forge has Skills, Plugins and MCP tabs in Settings (spec §17) that toggle enabled state. Explore, try and add need a catalogue; the source was not checked. |
| PG-03 | Artifacts page | partly | none | L | - | Not opened. Decide if Forge needs it. |
| PG-04 | Dispatch (beta) | partly | none | L | won't? | Switches the app to chat mode. Unclear value for Forge. |
| PG-05 | Home dashboard: sessions, messages, total tokens, active days, peak hour, favourite model, a day heatmap; tabs overview and models; ranges all, 30 days, 7 days | seen | partial | M | Plan 24 (built) | Forge has a usage ring per session (spec §20). The dashboard sums over all transcripts. The "Little Prince" comparison line is a joke; skip unless asked. Built in Plan 24: card with Overview and Models tabs, All / 30d / 7d, six tiles, heatmap, per-model chart; messages and tokens are counted per transcript line, as the app does (round 6; see the spec "As built"). Left: the book caption, hover tooltips. |
| PG-06 | Greeting and examples on the new-session screen ("다음 일정은 무엇인가요?") | seen | partial | S | - | Compare with Forge's new-session panel. |
| PG-07 | "새로운 기능" (what's new) link, top right of the home screen | seen | none | S | - | Where it leads was not seen. |
| PG-08 | Account menu at the bottom of the sidebar (name, plan, chevron) and an icon beside it | partly | none | S | - | Not opened. |
| PG-09 | Chat mode (the left icon of the title-bar toggle; a different product surface with Projects and pinned chats) | seen | none | L | won't | Forge is the Code side only (spec §2). |

## OBS: observation tasks (do before planning the entries they unlock)

| Id | What to look at | Unlocks |
| --- | --- | --- |
| OBS-01 | A session with real content: streaming, tool-call blocks, permission prompt, diff, sub-agent, stop button. Needs a message in a throwaway folder; ask the user first | chat view parity, PNL-02 |
| OBS-02 | Submenus of the session menu (open in, transcript, output style) | SES-03..05 |
| OBS-03 | Model "더 많은 모델" submenu, effort options, the composer circle on hover, the "로컬" chip menu | CMP-04..07 |
| OBS-04 | Changes, files and preview panels, with a real folder | PNL-02..04 |
| OBS-05 | Settings window and the keyboard shortcut list | shortcuts, settings parity |
| OBS-06 | Split panes or several sessions at once (the docs describe them; not seen in this window) | spec §7 "multiple sessions" |
| OBS-07 | (partly done in Plan 17: filter menu and search results seen) Sidebar filter menu, "사이드바 편집…" contents, sidebar search results | SES-07, SES-08, SES-10 |
| OBS-08 | The search palette: what Enter and `...` do on a row, the "작성" mode, the shortcut that opens it, the empty and no-match views, and "사이드바 편집…" | SES-07, SES-10 |

## OTH: carried over from earlier plans

| Id | Feature | Source | Plan |
| --- | --- | --- | --- |
| OTH-01 | Package the app | Plan 13 candidates | - |
| OTH-02 | Chat view extras | Plan 13 candidates | - |
| OTH-03 | Trash restore for deleted sessions | Plan 13 candidates | - |
| OTH-04 | Split panes | Plan 13 candidates | - |
| OTH-05 | Plan 12's deferred minors | spec §23.8 | - |
| OTH-06 | Test harness levels B (scenario runner) and C (SSR render tests) | Plan 13, deferred by the user | - |
| OTH-07 | Check the model ids in `KNOWN_MODELS` against the bundled Claude Code: `claude-sonnet-5-5` was refused by `setModel` in a running session; Settings and new sessions use the same ids | Plan 16 | Plan 19 (built: Sonnet is `claude-sonnet-5`, "Sonnet 5") |
