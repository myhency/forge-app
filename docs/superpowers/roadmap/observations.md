# Observations of Claude Code desktop

Round 1: 2026-10-03, the installed `/Applications/Claude.app` (Korean UI, Pro plan, Sonnet 5.5 selected), 1200 x 800 pt window, Code mode. Text only; no screenshots are committed.

Labels are quoted in Korean so they can be matched against the app. Backlog ids are in brackets.

## What was done to the app

Only navigation and opening menus. No message was sent and no data was changed. Two slips, both undone: the "발송" item switched the app to chat mode (switched back with the title-bar toggle), and a click aimed at a toolbar icon expanded the terminal panel (closed again). The app was left on the session "Debug and identify error cause".

## Window layout (Code mode)

- Title bar: window buttons, sidebar toggle, back and forward arrows, and a two-button toggle between chat mode (speech bubbles) and Code mode (`</>`). [SES-11, PG-09]
- Sidebar, top to bottom: search box "검색"; "새로 생성"; "Artifacts"; "루틴"; "발송" with a "베타" badge; "사용자 지정"; "더보기" (opens one item, "사이드바 편집…"). Then one group per project (`claude-code-study`, `mywork`), each with its sessions as plain rows with a small circle. The first group has `+` and a filter/sort icon; the second only `+`. Bottom: account name with plan and chevron, and an icon at the right. [SES-07, SES-08, SES-10, PG-08]
- The sidebar changes in chat mode: it shows "프로젝트", "예약됨" in place of "루틴", pinned chats and chats grouped by date. That is a different product surface. [PG-09]

## New-session screen (home)

- Greeting "Hyunseung님, 다음 일정은 무엇인가요?" and a usage card: tabs "개요" and "모델"; ranges "전체", "30일", "7d"; tiles for sessions, messages, total tokens, active days, peak hour, favourite model; a day heatmap; a joke line comparing tokens to a book. [PG-05, PG-06]
- "새로운 기능" link at the top right. [PG-07]
- Above the composer: chips "로컬", the folder name (`mywork`) and an add-folder button. [CMP-06]
- Composer: placeholder "작업을 설명하거나 질문하세요". Bottom row, left to right: `+`, microphone with chevron, permission mode ("자동"); at the right the model ("Sonnet 5.5"), effort ("중간") and a circle. [CMP-01..07]

## Composer menus

- `+` menu: "파일 또는 사진 추가" (⌘U), "폴더 추가", "GitHub 이슈 가져오기" (greyed: "이 폴더에는 GitHub 원격 저장소가 없습니다"), "슬래시 명령어", "커넥터" (submenu), "플러그인" (submenu). [CMP-01]
- Permission mode menu, title "모드": "자동 / Claude가 권한 결정을 처리합니다" (checked, key 1); "수동 / 변경하기 전에 항상 확인" (2); "편집 자동 수락 / 모든 파일 편집을 자동으로 수락" (3); "계획 / 변경하기 전에 계획 세우기" (4); "권한 무시" with a "활성화" button. [CMP-03]
- Model menu: "Opus 5.5" (1); "Sonnet 5.5" with badge "기본값" (checked); "Fable 5.1" with badge "사용 크레딧 필요" (3); "Haiku 4.5" (4); "더 많은 모델" (submenu). [CMP-04]

## Session screen

- Header: a laptop icon (runs locally), the title with a chevron, the folder as a tag; at the right four icons: terminal, a box with a plus (changes, inferred), a globe (preview, inferred), `⋮`. [PNL-01..07]
- Title chevron menu: "다음에서 열기" (submenu), "이름 변경" R, "포크" F, "트랜스크립트 보기" (submenu), "출력 스타일" (submenu), "보관" A, "삭제" D (red). [SES-01..06]
- `⋮` menu: "파일" (⇧⌘F), "iOS 시뮬레이터", and a toggle "컴퓨터 절전 모드 방지 / 이 세션에서만" (off). [PNL-04..06]
- A session whose transcript is missing shows "디스크에서 세션을 찾을 수 없음 / 이 디렉터리에서 새로 시작하려면 메시지를 보내세요." with buttons "CLI 세션 가져오기", "보관", "삭제"; if the folder is gone, a bar "작업 폴더가 더 이상 존재하지 않습니다. 계속하려면 다른 폴더를 선택하세요." with "폴더 선택" and "보관". [SES-09]
- Terminal panel: opens at the right (about a third of the width) with a title "터미널", a `+` for tabs, an expand button and close. Expanded, it fills the area right of the sidebar. With the folder missing it shows `Working directory "/Users/james/Developer/claude-code-study" does not exist`. When it is open the toolbar icons move left. [PNL-01]

## Pages

- "루틴": heading, tabs "내 항목" and "템플릿", search, filter, sort and a dark "새 루틴" button with a dropdown. A notice "로컬 루틴은 컴퓨터가 켜져 있고 온라인 상태일 때만 실행됩니다." One card: title with a warning icon, the prompt, "일시중지됨 · 1분마다, 1시간마다, 매일". [PG-01]
- "사용자 지정": tabs "스킬", "커넥터", "플러그인"; a switch "내 항목" / "탐색"; search, filter, sort and a dark "추가" button with a dropdown. In explore: a banner "From Anthropic / Data" with "추가", a "새 스킬" list of cards, each with "체험" and `+`, and the source ("Anthropic Directory" or "제작: Anthropic"). [PG-02]
- Chat-mode "프로젝트": a grid of project cards with a date, a pin icon on one, a sample project, and "새 프로젝트". Out of scope. [PG-09]

## Round 2: session menu submenus (2026-10-03, Plan 15)

Same app and window. Only menus were opened (title chevron, then each submenu with the arrow keys; a click on a submenu row only highlights it, the right arrow opens it). Nothing was chosen. Escape closes a submenu first, then the menu. The session was the one with the missing transcript and folder, so the menu may differ for a normal session (OBS-01 still open).

- "다음에서 열기" (open in): "새 창" (new window, key 1); a heading "계속하기" (continue) with "클라우드 / 컴퓨터 없이 실행" (run in the cloud without the computer, key 2). It is not a list of editors or Finder. [SES-05]
- "트랜스크립트 보기" (view transcript): three radio choices "일반" (normal, checked), "사고 과정" (thinking process), "자세히" (detailed). [SES-03]
- "출력 스타일" (output style): one item, "새 스타일..." (new style). No built-in styles were listed. [SES-04]
- Shortcut letters shown at the right of the menu rows: 이름 변경 R, 포크 F, 보관 A, 삭제 D. Not tried whether the keys work while the menu is open. [SES-06]

## Round 3: sidebar filter and search (2026-10-03, Plan 17)

Same app and window. Menus were opened with the mouse and the arrow keys, nothing was chosen. One search word was typed ("general"); it changes no data. "사이드바 편집…" under "더보기" was not opened.

- Group header: the project name, `+` and a filter icon at the right. The name shows a small chevron while the pointer is over the header (collapse). [SES-08]
- Filter menu (icon at the right of the header): rows with the current value at the right and a chevron: "상태" 활성, "환경" 전체, "그룹화 기준" 폴더, "정렬 기준" 최근 활동. Then "빈 그룹 표시" (show empty groups, unchecked) and "PR 상태 표시" (show PR state, checked). [SES-08]
- "상태" submenu (radio): 활성 (checked), 보관됨, 전체. [SES-08, SES-02]
- "환경" submenu (checkboxes): 모든 환경 (checked), then 로컬, 클라우드, 원격 제어, SSH, Slack. [SES-08]
- "그룹화 기준" submenu (radio): 날짜, 폴더 (checked), 상태, 사용자 지정 그룹, then a line, 없음. [SES-08]
- "정렬 기준" submenu (radio): 이름, 생성 날짜, 최근 활동 (checked). [SES-08]
- The filter is one menu shared by all groups: its value is shown in the menu, not per project (not checked whether the second group's own header has it; it showed only `+`). [SES-08]
- Search box "검색" at the top of the sidebar: a click opens a palette in the middle of the window over a dimmed app, not a filter of the sidebar list. Above the palette a small switch "검색" / "작성" with a `Tab` hint. The input has a close `x`. Tabs under it: 전체, Artifacts, 세션, 코드, 프로젝트, 예약됨. [SES-07]
- Results (query "general", tab 전체): rows with an icon for the kind (`</>` for a code session, a list icon, a speech bubble for a chat), the title with the matched word bold, a quote of the matching text from the body with the word bold, and a relative date at the right ("지난 1년", "지난 한 달"). The first row is highlighted and shows `...` and an Enter icon. So it searches titles and message text, over every product surface, not only Code. [SES-07]
- Not seen: what Enter does on a row (assumed: opens it), what `...` offers, the "작성" mode, keyboard shortcut to open the palette, the empty-result view.

## Round 4: archive, effort, context circle, fork (2026-10-03, Plan 19)

Same app and window. The app had logged the user out in the middle of the round (a "기기를 확인하려면 다시 로그인하세요" dialog; "나중에" led to a welcome screen); the user logged in again and the round went on. The one data change: **포크 was chosen once** on "Spreadsheet access" (mywork), with the user's permission, and made one copy that the user decides what to do with. Nothing else was chosen, archived, deleted or sent. The status filter was set to 보관됨 to look and set back to 활성.

- Archived view: the status filter "보관됨" lists only archived sessions, grouped by project (the other groups disappear). A row has an archive-box icon in place of the circle and greyed text. When the filter is not the default, the filter menu gets a last item "필터 지우기". [SES-02, SES-08]
- An archived session opens read-only with its whole transcript. The bar at the bottom, in place of the composer, says "이 세션은 보관되었습니다. 이 세션에서 계속 작업하려면 보관을 취소하세요." with a button "보관 해제". The toolbar keeps changes and share/`⋮` icons but not the terminal and globe. Not seen: the title menu of an archived session. [SES-02]
- Effort: the label "중간" at the right of the model name opens a popover titled "노력 중간" with a `?` (tooltip: "더 높은 노력은 더 철저한 응답을 의미하지만, 시간이 더 오래 걸리고 한도를 더 빨리 사용합니다."), the end labels "더 빠르게" and "더 스마트하게", and a slider with six stops, a thumb on the second stop and the word "추천" under it (the model's recommended level; "중간" for Sonnet 5.5). The names of the other stops were not seen (moving the slider changes the setting). [CMP-05]
- Context circle (the circle at the right end of the bottom row): a popover with "컨텍스트 윈도우 56.4k / 1M (6%)", a segmented bar (one colour per kind of content) and a `>`; "플랜 사용량 한도 · Pro" with "세션 한도" (resets in 4 h 52 min, 8%) and "주간 · 모든 모델" (resets Wed 6:00, 35%); "클라우드 세션 크레딧" ("US$100 중 US$100 남음", expiry date); a button "자세한 내역 보기". The plan limits come from the account, not the transcript. [CMP-07]
- Opening a session can show a toast "이 세션을 Sonnet 5 모델의 최신 버전인 Sonnet 5.5 모델로 전환했습니다." with "실행 취소" and a close `x`. The model label read "Sonnet 5.5" afterwards. [CMP-04, OTH-07]
- Fork (title menu, "포크", key F; enabled on a normal session): at once (under half a second, no dialog, no wait) a new session "Spreadsheet access (fork)" appears at the top of the same project group, above the original, and is opened and selected. The title is the old title plus " (fork)". The whole transcript is copied (the last message is the same), and the folder tag, the model (Sonnet 5.5), effort (중간) and mode (편집 자동 수락) are the same. The composer is empty with the cursor in it. The original stays in the list, unchanged. [SES-01]
- Not seen: what a fork of a fork is called, a fork of a running session, a fork of an archived one, and the names of the effort stops.

## Not seen

See the `OBS` table in [backlog.md](backlog.md). In short: any session with real content (tool blocks, permission prompt, diff), the submenus, the changes/files/preview panels, the settings window, shortcuts, split panes, the filter and sidebar-edit menus, and the Artifacts page.
