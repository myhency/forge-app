# Backlog

Columns and values are explained in [README.md](README.md). Ids are never reused. Sizes: S (about one task), M (a plan), L (more than one plan).

Forge status was read from `crates/` on 2026-10-03.

## SES: sessions and the sidebar

| Id | Feature | Seen | Forge | Size | Plan | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| SES-01 | Fork a session (menu item "포크", key F) | partly | none | M | - | Menu item seen, its effect not. Does it copy the transcript, or start from a point in it? Observe. |
| SES-02 | Archive a session (menu item "보관", key A), and a way to see and restore archived ones | partly | none | M | - | Where archived sessions go was not seen. Archive is not delete. Compare Forge's delete (spec §19). |
| SES-03 | View transcript (submenu): 일반 (normal), 사고 과정 (thinking), 자세히 (detailed) | seen | none | S | - | Radio choice, normal is the default. Effect on the chat view not seen (needs OBS-01). |
| SES-04 | Output style (submenu): only "새 스타일..." | seen | none | S | - | No built-in styles listed. What "새 스타일..." does was not tried. |
| SES-05 | Open in (submenu): 새 창 (new window), 클라우드 (run in the cloud) | seen | none | S | won't? | Not editors or Finder. Cloud runs are out of scope; a new window needs multi-window. The user decides. |
| SES-06 | Single-key shortcuts in the session menu (R rename, F fork, A archive, D delete) | seen | none | S | - | Forge has rename and delete in the menu, no keys shown. |
| SES-07 | Sidebar search | partly | none | M | - | Search box seen at the top of the sidebar; what it searches (titles, text) and how results look were not seen. |
| SES-08 | Project group header tools: `+` new session in that project, filter/sort icon, collapse chevron | partly | partial | M | - | Header, `+` and filter icon seen; the filter's contents were not. Forge has a new-session panel (spec §22). |
| SES-09 | Recovery screen for a session whose transcript is missing ("CLI 세션 가져오기", archive, delete) and a bar for a missing folder ("폴더 선택", archive) | seen | partial | M | - | Forge opens such a session view-only (spec §23). The import action was not tried. |
| SES-10 | Edit the sidebar ("사이드바 편집…", under "더보기") | partly | none | S | - | Only the menu item seen. Probably chooses which items show. |
| SES-11 | Sidebar width, collapse button, back/forward history buttons in the title bar | seen | none | S | - | Collapse and history buttons seen; whether the sidebar resizes was not tried. |

## CMP: composer (the input box)

| Id | Feature | Seen | Forge | Size | Plan | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| CMP-01 | `+` menu: attach file or photo (⌘U), add folder, import a GitHub issue, slash commands, connectors (submenu), plugins (submenu) | seen | partial | L | - | Forge has slash commands (spec §21, §22). The GitHub item is greyed with "이 폴더에는 GitHub 원격 저장소가 없습니다" when the folder has no remote. Attachments need a protocol change. |
| CMP-02 | Voice input (microphone button with a chevron) | partly | none | L | - | Chevron menu not opened. Needs OS audio and speech; decide if in scope. |
| CMP-03 | Permission mode menu with five entries: 자동 "Claude가 권한 결정을 처리합니다", 수동 "변경하기 전에 항상 확인", 편집 자동 수락, 계획, 권한 무시 (separate "활성화" button); number keys 1 to 4 | seen | partial | M | - | Forge has four modes (default, accept edits, plan, bypass; `permission_mode.rs`) and no "auto" mode. Check the SDK for an auto mode before planning. Bypass needs an extra step to enable. |
| CMP-04 | Model picker in the composer: Opus 5.5, Sonnet 5.5 (badge "기본값"), Fable 5.1 (badge "사용 크레딧 필요"), Haiku 4.5, "더 많은 모델" submenu; keys 1 to 4 | seen | partial | M | - | Forge sets the default model in Settings only, and a choice applies to the next new session (spec §16). Changing the model of a running session was not tried in the app. |
| CMP-05 | Effort level next to the model ("중간") | partly | none | M | - | Options not opened. Check the SDK for an effort option. |
| CMP-06 | Environment chips above the composer on a new session: "로컬" (where it runs), the folder name, an add-folder button | seen | partial | S | - | Forge has a folder choice on the new-session screen (spec §22). The "로컬" chip's menu (other environments) was not opened. |
| CMP-07 | Circle at the right end of the composer's bottom row | inferred | partial | S | - | Probably context use. Forge has a usage ring (spec §20). Hover it to learn what it shows. |

## PNL: panels and tools in a session

| Id | Feature | Seen | Forge | Size | Plan | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| PNL-01 | Terminal panel docked on the right: tabs with `+`, expand to full width, close; shows "Working directory ... does not exist" when the folder is gone | seen | none | L | - | Needs a pty on the Rust side. Shell content and keys were not tried (the folder was missing). |
| PNL-02 | Changes panel (second toolbar icon) | inferred | none | L | - | Not opened. Forge has a diff viewer for Edit/Write calls (spec §18). |
| PNL-03 | Preview browser panel (globe icon) | inferred | none | L | - | Not opened. |
| PNL-04 | Files panel (`⋮` menu, ⇧⌘F) | partly | none | L | - | Menu item seen, panel not opened. |
| PNL-05 | iOS simulator (`⋮` menu) | partly | none | L | won't? | macOS and Xcode only. Propose leaving out; the user decides. |
| PNL-06 | "Prevent sleep" toggle, "이 세션에서만" (this session only) | seen | none | S | - | Toggle seen in `⋮`; not switched on. Needs a macOS sleep assertion. |
| PNL-07 | Toolbar with title, a rename/fork menu under the title chevron, the folder name as a tag | seen | partial | S | - | Forge has a header; compare the details when planning SES. |

## PG: pages outside a session

| Id | Feature | Seen | Forge | Size | Plan | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| PG-01 | Routines: list of scheduled tasks with title, prompt, state ("일시중지됨") and schedule; tabs "내 항목" and "템플릿"; search, filter, sort; "새 루틴" menu; notice that local routines run only while the computer is on and online | seen | none | L | - | Creating one was not tried. Needs a scheduler; the notice means no cloud run is required. |
| PG-02 | Customize page: tabs skills, connectors, plugins; "내 항목" and "탐색" (explore); "추가" menu; search, filter, sort; featured banner and cards with "체험" (try) and `+` | seen | partial | L | - | Forge has Skills, Plugins and MCP tabs in Settings (spec §17) that toggle enabled state. Explore, try and add need a catalogue; the source was not checked. |
| PG-03 | Artifacts page | partly | none | L | - | Not opened. Decide if Forge needs it. |
| PG-04 | Dispatch (beta) | partly | none | L | won't? | Switches the app to chat mode. Unclear value for Forge. |
| PG-05 | Home dashboard: sessions, messages, total tokens, active days, peak hour, favourite model, a day heatmap; tabs overview and models; ranges all, 30 days, 7 days | seen | partial | M | - | Forge has a usage ring per session (spec §20). The dashboard sums over all transcripts. The "Little Prince" comparison line is a joke; skip unless asked. |
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
| OBS-07 | Sidebar filter and "사이드바 편집…" contents, sidebar search results | SES-07, SES-08, SES-10 |

## OTH: carried over from earlier plans

| Id | Feature | Source | Plan |
| --- | --- | --- | --- |
| OTH-01 | Package the app | Plan 13 candidates | - |
| OTH-02 | Chat view extras | Plan 13 candidates | - |
| OTH-03 | Trash restore for deleted sessions | Plan 13 candidates | - |
| OTH-04 | Split panes | Plan 13 candidates | - |
| OTH-05 | Plan 12's deferred minors | spec §23.8 | - |
| OTH-06 | Test harness levels B (scenario runner) and C (SSR render tests) | Plan 13, deferred by the user | - |
