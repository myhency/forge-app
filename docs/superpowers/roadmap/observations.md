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

## Round 5: right-hand panels (2026-10-03, Plan 21)

Same app and window, on a normal session whose folder is empty or missing (so the panels showed their empty states; a folder with real changes and files was not seen). Only toolbar icons and menus were opened. No message was sent. The cookie import in the preview was not chosen. All panels were closed again at the end.

- Panels are columns right of the chat. Opening one shrinks the chat. Changes and Preview opened together stack in one column (Changes above, Preview below, each with its own header). Files opened as a third column at the far right. The chat column narrows each time (it was about 2/3 of the window with one column, about 1/3 with three). Each panel's toolbar icon in the title bar is highlighted while it is open and toggles the panel (a second click closes it). Icons shift left as columns open. [PNL-01..04]
- Panel header actions, same in all: `⋮` menu, an expand button (fills the area right of the sidebar) and a close `x`. [PNL-01..04]
- Title bar: with a panel open a folder icon appears beside the folder tag/title (it is the same action as `파일` in `⋮`, not clicked). Title text truncates ("Spreadsheet...") as the chat narrows. [PNL-04, PNL-07]
- Changes panel: a list icon at the left of the header toggles a file list (a column about 40% wide at the left) next to the diff area. Empty: the text "표시할 변경 사항이 없습니다" in the middle. The panel `⋮` has one item, "새로고침" (refresh). Not seen: a diff, the file list with rows, staging or revert actions. [PNL-02]
- Preview panel: header with a tab "새 탭" and `+` (tabs), at the right a layout icon, `⋮`, expand, close. Under it a browser bar: back, forward, reload, a URL field "URL 입력", and at the right an element-picker icon (cursor). The first time it opens, a modal "사이트에 로그인 상태 유지": "브라우저에서 쿠키를 가져와서 여기와 모든 세션에서 사이트에 로그인된 상태로 열리도록 하세요." with three ticks (only cookies and site data, never passwords; the user chooses the sites; nothing leaves the computer) and buttons "다시 표시 안 함", "나중에", "쿠키 가져오기" (not chosen). The modal covers the panel, including its close `x`; the toolbar globe icon still closes it. Not seen: a loaded page, the layout icon, the panel `⋮`, the element picker. [PNL-03]
- Files panel (`⋮` in the title bar, "파일", ⇧⌘F): header with a list icon (file tree toggle), the title "파일", a search icon, `⋮`, expand, close. A filter field "필터링하거나 ?를 입력해 파일 내 검색" (filter by name, or start with `?` to search inside files), focused on open. Empty folder: "폴더가 비어 있음". Its `⋮` menu: "파일 트리 표시" (checked, ⇧⌘Y); then checks "미리보기 탭" (on), "자동 저장" (on), "Tab 키로 들여쓰기" (on); then "자동 줄 바꿈" (on), "들여쓰기 가이드" (on), "공백 표시" (off), "끝 이후까지 스크롤" (off), and "탭 크기" with the value 4 and a submenu. So the panel is a file tree plus an editor (it saves files), with a preview tab. Not seen: the tree with files, an open file, the editor, the preview tab. [PNL-04]
- The terminal icon was not clicked this round (Round 1 saw that panel). Not seen: whether the terminal stacks in the same column as Changes and Preview.
- Not seen: with real changes and files; the iOS simulator; the "컴퓨터 절전 모드 방지" toggle (seen off in Round 1, not switched).

## Forge compared with the real app, Plan 21A (2026-10-03)

Looked at in Forge's own window through the harness, not in the real app. Differences found:

- Forge's terminal ends its shells when the person leaves the session (the chat is unmounted); the real app was not seen on a session switch.
- Forge's `⋮` menu stays open after the switch is clicked; the real app's menu was not seen to be used.
- The header icons are the glyph `>_` and `⋮`; the real app draws its own icons.
- With the dock open and the chat narrow, the composer's row (mode, model) wraps; not seen in the real app.

## Not seen

See the `OBS` table in [backlog.md](backlog.md). In short: any session with real content (tool blocks, permission prompt, diff), the submenus, the changes/files/preview panels, the settings window, shortcuts, split panes, the filter and sidebar-edit menus, and the Artifacts page.

## Round 6: Home dashboard (2026-10-03, Plan 24)

Same app and window. Only the "새로 생성" item was clicked, then the card's tabs and range buttons. Nothing was sent or changed. Numbers are the user's; they are kept here only to show how the ranges differ.

- The card sits under the greeting on the new-session screen, about 800 pt wide at the left of the content column. Top row: tabs "개요" (overview) and "모델" (models) at the left; ranges "전체" (all), "30일", "7d" at the right. Defaults: 개요 and 전체. [PG-05]
- 개요 has six tiles in two rows of three: "세션" (sessions), "메시지" (messages), "총 토큰" (total tokens, shortened: 2.4B), "활성 일수" (active days), "최다 사용 시간" (peak hour, "오후 9시"), "즐겨 사용한 모델" (favourite model, "Sonnet 5"). Below them a heatmap and one caption line.
- The range changes the tiles: 전체 158 sessions / 14,710 messages / 12 active days; 30일 153 / 14,625 / 8; 7d 147 / 14,382 / 7. Total tokens, peak hour and favourite model did not change on this data. So sessions are counted by activity inside the range, not by creation date.
- Heatmap: 7 rows (weekdays, Sunday first: the last column's bottom cell is Saturday 10월 3일, today) by 27 columns (weeks, newest at the right), square cells, grey when empty, blue in several shades by activity. It did **not** change with the range: same cells for 전체, 30일 and 7d. The week ends at today.
- Caption under the heatmap: "<book>보다 약 N배 더 많은 토큰을 사용했습니다." The book changes with the count (전체 2.4B: "War and Peace" 3270x; 30일 and 7d: "The Great Gatsby" 38462x, 38230x). A joke; skip (as noted in PG-05).
- 모델 tab: a stacked bar chart of tokens per day, y axis 0 to 800M in steps of 200M, x labels every few bars; below it a legend with one row per model: colour square, name, "<in> 입력 · <out> 출력" (for example "90.8k 입력 · 5.3M 출력") and a percentage of the total (55.0%). Models seen: Sonnet 5, Sonnet 5.5, Opus 5.5, Haiku 4.5, ordered by share, shades of one blue. 전체 shows bars for days with data only, spaced by date (labels 4월 25일 ... 10월 2일, gaps for unused days); 7d shows exactly seven bars with a label under each ("9월 27일" ... "10월 3일"), including a day with no bar. The percentages differ slightly by range.
- Not seen: hover tooltips on cells and bars, the 30일 bar chart, a dark theme, an empty state for a user with no sessions, how the app counts "총 토큰" (whether cache tokens are in; the legend shows only input and output).

### Round 6, part 2: the new-session composer (2026-10-03)

Same session of looking. Menus were opened and closed with Escape; one letter was typed into the box and deleted; nothing was sent. Contents of the "+", mode and model menus are in Round 1 and are not repeated.

- Above the box, left to right: chips "로컬" (laptop icon), the folder name with a folder icon, and a square folder-with-plus button (not clicked: it probably opens a file dialog). A small pixel creature stands on the box's top edge at the right (decoration; it moved between captures). [CMP-06]
- "로컬" chip menu (opens upward): "로컬" (checked), "클라우드", "원격 제어", "SSH", the last three with a right chevron (submenus, not opened). [CMP-06]
- Folder chip menu (opens upward): "폴더 없음" (no folder); a divider; a small grey heading "최근" (recent) over a list of folder names, the current one checked, in recency order (8 shown); a divider; "폴더 열기…". [CMP-06]
- Box: one row, placeholder "작업을 설명하거나 질문하세요", a return-arrow icon at the right (grey when empty, black once a letter is typed). Focus darkens the border. The row under the box has `+`, a microphone, a chevron, then the mode name "자동"; at the right "Sonnet 5.5", "중간" and a circle. [CMP-01..07]
- Microphone chevron menu (opens upward): heading "마이크" over the input devices of the Mac (the default marked "Default - ...", checked, then each device by name), a divider, and a switch "길게 눌러 녹음" (press and hold to record, on). [CMP-02]
- The caption under the dashboard's heatmap names a different book on each visit (seen: War and Peace, The Great Gatsby, Harry Potter and the Philosopher's Stone), so it is chosen at random or by the count.

### Round 6, part 3: the right side of the composer's bottom row (2026-10-03)

Popovers were opened with the mouse and closed with Escape; nothing was chosen.

- Model menu (opens upward, right-aligned to the model name): "Opus 5.5" (1), "Sonnet 5.5" with a grey badge "기본값" (checked), "Fable 5.1" with a badge "ⓘ 사용 크레딧 필요" (3), "Haiku 4.5" (4); a divider; "더 많은 모델" with a right chevron. The number keys sit at the right of the rows (2 is hidden by the check). [CMP-04]
- "더 많은 모델" (opened with Right arrow after pointing at it): a second menu, opening to the **left** of the first, with no numbers: "Sonnet 5", "Opus 5", "Fable 5" (badge "사용 크레딧 필요"), "Opus 4.8", "Opus 4.7", "Opus 4.6", "Sonnet 4.6". The first row is highlighted on opening. The button label for a model from this list was not seen (a model that is in the main list shows its name, "Sonnet 5.5"). [CMP-04]
- Effort popover ("중간"): header "노력" in grey then the current level; a `?` icon at the right; labels "더 빠르게" and "더 스마트하게" over a six-stop slider with a white thumb and a dot per stop; "추천" under the second stop (the thumb is there, so "중간" is the second stop). The filled part is darker. [CMP-05]
- Circle popover on a fresh new-session screen: "컨텍스트 윈도우" with the value "0" and an empty bar; a divider; "플랜 사용량 한도 · Pro" with an arrow at the right (a link, not clicked); "세션 한도" with "2시간 1분 후 재설정" and a percentage, a blue bar; "주간 · 모든 모델" with "재설정: (수) 오후 6:00" and a percentage, a blue bar; a divider; "클라우드 세션 크레딧" with a `?` icon and "US$100 중 US$100 남음", an empty bar, and "만료: 11월 5일 오후 4:59 GMT+9". The circle itself is an empty ring on this screen. [CMP-07]

## Round 7: the chat view (2026-10-04, Plan 23 replaced by a chat-view plan)

The user allowed real messages in a throwaway folder (`/private/tmp/forge-observe`) and complex tasks. Looked at the saved session "Spreadsheet access" and two new sessions (mode 자동, then 수동). Pixel sizes below are points (the capture is 2x, the viewer showed it at 1/1.2). `scripts/observe/claude_app.py` gained `type`, `key` (letters go by physical key code, Korean IME safe), `scroll`; mouse is moved before a click.

### Layout of one turn
- Column: text starts at x≈300 pt inside a 1200 pt window (sidebar 233 pt), ends at ≈935 pt; the composer spans the same width (≈640 pt). No avatars, no names, no "Claude" label. Page background off-white.
- User message: a grey rounded bubble, right-aligned, hugs its text, max width ≈ the column; URLs in it are blue and underlined. Small gap (~40 pt) before the next block.
- Assistant text: plain text on the page, no bubble, black. Markdown: paragraphs, bullet and numbered lists, inline code as a pale-red chip with mono text, file names as blue underlined links (`fizz.py`, `README.md`).
- Under the last assistant message of a turn: a row of small grey icons (copy, share/branch, pin, speaker) and a relative time ("3개월 전"). Seen on a saved session; not seen while a turn runs.
- The top of the scroll area fades out under the header (text is cut with a gradient, no hard line). A round white "↓" button floats centred above the composer when the view is not at the bottom.

### While Claude works
- Right after send: the user bubble, then an orange three-dot spinner with the elapsed time ("2초") in grey.
- Then a live tool label replaces the time: spinner, a verb, the object, the elapsed time, a chevron: "생성 중 hello.txt 4초 ›", "읽는 중 hello.txt 6초 ›", "Running fizz.py 1분 21초 ›" (Bash uses its `description` as written, English). The label is grey; the object is darker. The elapsed time keeps counting while a permission prompt waits.
- The send arrow becomes a round stop button (a ring with a dot) at the right of the composer. The placeholder is "/ 입력 시 명령어" (typing a slash gives commands) while busy and when a saved session is open; a new session says "작업을 설명하거나 질문하세요".
- Sidebar: a new project group appears (folder name) with the session; the dot is a filled grey circle while running, an orange circle while waiting for input, an empty ring when idle. The session title is generated from the first message ("hello.txt 파일 생성 및 읽기").
- The context ring at the right of the composer's bottom row is empty at start and fills as the session grows.

### Permission prompt (mode 수동)
- In the transcript a pill "입력이 필요합니다 ›" (orange text on pale orange) with an orange ringed dot at its left; when the tool is a file write the pill is followed by green/red chips "+9 -0".
- The composer is replaced by a bordered card of the same width, rising above the bottom: title "Claude가 쓰기 fizz.py하도록 허용하시겠습니까?" (verb + file name spliced into one sentence: "쓰기", "편집"; for Bash "run Run fizz.py" so the template is `Claude가 <tool verb> <title>하도록 ...`), then the file name, a mono box with the full path, the path again, and a diff block with line numbers, `+`/`-` markers, green and red line backgrounds, syntax colours; an edit shows context lines and a word-level highlight inside the changed line ("Fizz" to "Foo"). The card is scrollable inside; the diff is cut at the card height.
- Bash card: title, the description ("Run fizz.py"), "This command requires approval", a mono box with the command (`python3 fizz.py`).
- Buttons at the bottom right: "항상 허용" with keys `2` and ⇧⌘↩ (white), "한 번만 허용" with `3` and ⌘↩ (black) and a chevron that splits the button. ⌘↩ allowed once (tested three times). A blue tooltip "자동 모드 사용해 보기" with a close × pointed at the mode label while in manual mode. The composer text box is hidden while the card shows; the stop button stays.

### After the turn
- A grey one-line summary replaces the tool labels: "생성됨 및 읽기 hello.txt +1 -0 ›" for one step; for many steps counts by kind: "생성된 파일 2개, 실행된 명령 2개, 편집된 파일 1개 +12 -1 ›". Past-tense Korean verbs for file tools ("생성됨", "수정됨"); Bash keeps its English description in the past ("Ran fizz.py", "Re-ran fizz.py").
- Opening the summary shows a bordered box, one row per step separated by hairlines, in order: file rows ("생성됨 fizz.py +9 -0 ›"), Bash rows, and grey rows with the assistant's text between tools ("1~2단계 완료, ..."). The chevron turns down. A Bash row opened earlier showed the red result and `key: value` input (round 7 part 1 below).
- A file row opens the file in a **right-hand panel** (the main column narrows; title bar gets a folder icon button): tab with the file name and ×, header icons (outline, search, ⋮, expand, close), the file name, then code with line numbers, fold chevrons and indent guides. Not an inline diff.
- At the end of the turn a card: "hello.txt  실행 취소  +1 -0 ›" for one file; for several a header "파일 2 개 편집됨  실행 취소  +12 -1" and one row each (file-type icon, name, +N -N, chevron). "실행 취소" (undo) is grey text.
- The composer's placeholder becomes a suggested next prompt ("hello.txt 삭제해줘", grey); not seen whether Tab accepts it.

### Earlier saved-session details (part 1)
- A question answered through the ask-user tool: a bordered card (≈ 265 pt wide, not full width) with each question in grey and the answer under it in black.
- A web link card: bold host, the host and "브라우저에서 열림" below, a small "열기" button and ⋮ at the right.
- Tool group with an error: "사용함 도구 3개 (1개 실패) ›" collapsed (grey); opened, a bordered box with the narration first, then rows "Opened a page 실패 ›" ("실패" in dark red), each row opens to show the error text in dark red, then the input as `tabId: main` / `url: ...` (mono key, normal value).
- Bottom row of the composer in a saved session: `+`, mic, chevron, mode ("편집 자동 수락"), at the right model "Sonnet 5.5", effort "중간", context ring.

### Round 7, part 2: questions, markdown, background tasks (2026-10-04)

- Markdown rendering (all checked in one reply): h2/h3 are bold, same family, slightly larger; bullets are filled dot, then hollow circle, then filled square by depth; ordered lists nest as 1. then a. then i.; nested ordered lists keep indentation of about 29 pt per level. Block quote: a 3 pt light-grey bar at the left, text in dark grey, several paragraphs allowed. Task list: grey rounded checkboxes (checked ones are grey with a tick and the text is struck through, unchecked are empty). Table: full column width, a grey header row, hairline rows, columns spread over the full width (not shrunk to content), centred cells follow `:---:`. Code block: white card with a hairline border, syntax colours, **hugs its content width** (not full width), a copy icon appears at the top right on hover. Horizontal rule: a light hairline across the column. Bare URL and `[text](url)` are blue and underlined; an unresolved reference link `[a][guide]` stays plain text. Inline code is a pale-red chip in mono.
- AskUserQuestion: while waiting, the transcript shows a grey line "질문 중 <header> ⌄" and the questions as plain grey lines. The composer is replaced by a card of the same width: a round orange chip "1/2" and the question in bold; at the right `⌄` (collapse) and `×` (dismiss). Options are grey rows with a bold label, a grey description and a control at the right: a checkbox for multi-select, a small box with the number key for single select (1 to n; the last row "기타" has the next number and a text field "여기에 답변을 입력하세요"). Footer: "뒤로" (second question on) at the left, "건너뛰기" and a primary button "다음 ⌘↩" (grey until something is picked, black after; the last question says "제출"). A single-select pick moves to the next question at once or on the button (not distinguished). After submitting, the transcript shows a bordered answer card (narrow, about 265 pt wide) with each question in grey and the answer in black, then the reply.
- Reply footer: icons copy, branch, pin, speaker, then "지금" / "2분 전", only under the last message of a turn; they appear after the turn ends, not while running.
- Running state of a turn without tools: orange three dots, then "6초 · 202 토큰" in grey, updating about every second.
- Background task: a Bash call with `run_in_background` shows "<description> 실시간 출력 보기 ›" (the link is a button); the reply "백그라운드에서 실행 중입니다..." appears at once and a blue pill "실행 중인 작업 1개" stays at the bottom of the transcript. Clicking the link or the pill opens a right-hand panel "백그라운드 작업": section "실행 중", a card with the description, "Bash  29초" (live time), a mono box `$ sleep 60`, "아직 출력 없음", and a square stop button at the card's right. The panel has expand and close at the top. The pill turns light blue with a darker fill while the panel is open.
- The composer's placeholder is a suggested next prompt only when the last turn ended (for example "그 답변을 color_season.md 파일로 저장해줘"); it is a real next step, generated per reply.

## Round 8: the permission card and a denial (2026-10-04, Plan 23B)

The user allowed one more message in the throwaway folder. Mode 수동, asked Claude to create `/private/tmp/forge-observe/obs.txt` with "x"; looked at the card, opened its menu, chose "거부" (deny). `obs.txt` was never written. The session's mode was set back to 자동.

- The permission card does **not replace the composer** (Round 7 guessed it did, because a tooltip hid the left button). The card sits above the composer; the composer stays under it, empty, with the placeholder "/ 입력 시 명령어" and the round stop button at the right.
- Card, top to bottom: title "Claude가 쓰기 obs.txt하도록 허용하시겠습니까?" (bold; verb + file name inside one sentence), the file name, a grey mono box with the full path, the path again as plain text, the diff block (`+ 1  x` on a green line), then the buttons row.
- Buttons, left to right: "거부" with the key chips `1` and `Esc` (white, small); at the right "항상 허용" with `2` and `⇧⌘↩` (white), and "한 번만 허용" with `3` and `⌘↩` (black) joined to a `⌄` button. The `⌄` opens a one-item menu above it: "허용하고 Auto 모드로 전환" with the grey line "이 세션의 나머지 작업을 자동으로 검토합니다." Pressing `⌄` again closes it; `Esc` is deny, so it was not used to close.
- While the card shows: the sidebar dot is orange, the transcript has the pill "입력이 필요합니다 ›" with the orange ringed dot at its left; the tooltip "자동 모드 사용해 보기" appeared again over the mode label until dismissed.
- After "거부": the turn ends at once. The transcript shows one line "생성됨 obs.txt 실패 ›" ("생성됨" grey, the name dark, "실패" dark red) and no reply from Claude. A short row appears under the user's message: "1분 전" and three icons (copy, a rewind arrow ↺, branch). The composer shows the next suggested prompt again ("fizz.py 다시 실행해줘").
- The earlier background task finished by itself: the pill "실행 중인 작업 1개" was gone and a new line "끝" had been added after the "백그라운드에서 실행 중입니다" text; the tool line read "Slept for 60 seconds ›" (past tense of the live "Sleeping for 60 seconds").

### Round 8, part 2: the opened denied row (2026-10-04, the user opened it in the real app)

- The line "생성됨 obs.txt 실패 ›" opened in place (the chevron turned down) into a bordered box across the column width. At the top, in dark red: "The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). STOP what you are doing and wait for the user to tell you how to proceed." This is the text the tool result carried, so the model saw it and stopped (no reply followed).
- Under it, the tool input as `key: value` lines: the key in grey mono, the value in normal text; a path value is blue and underlined (`file_path: /private/tmp/forge-observe/obs.txt`, `content: x`). No JSON braces.
- Under the box the row of small icons changed to branch, pin and "2분 전" (no copy and no rewind this time; the icons depend on the turn).
- Forge today: the sidecar answers a denial with "Denied by user", so the model gets a much weaker instruction than the real app's; and the opened tool row (23A) shows pretty JSON, not `key: value` lines.

### Round 8, part 3: a written markdown file and its preview (2026-10-04, the user's session "최적의 로컬 LLM 모델" under mywork, preview opened by the user)

Looked at only; the only things pressed were view toggles and menus.

- **Where the file appears:** the tool line "생성됨 local-llm-guide.md +82 −0 ›" (the `+82` green, `−0` red chips), and the reply text names the file as a blue underlined link ("local-llm-guide.md에 저장했습니다"). Clicking that link in the text opened the preview panel at the right; the title bar got a folder icon button next to the session title (a file-panel toggle, the "mywork" folder).
- **The tool line opened in place** (chevron down): a bordered box (about the column width) with the full path as a grey line ("/Users/james/Developer/myw…local-llm-guide.md", middle cut with an ellipsis), then the diff of the write: line numbers, a green `+` in the gutter and a light green background on every line, syntax colours for markdown (headings blue). Long lines wrap. The box is cut at a fixed height and scrolls inside; the page behind keeps its own scroll.
- **A round white "↓" button** floated above the composer, centred, while the page was not at the bottom, and went away after clicking it (it scrolls to the end).
- **Preview panel (right, about 45% of the window):** top row from the left: an outline/list icon (opens a file tree), one tab with the file name in italic and `×` (italic = a preview tab that a later file replaces), then at the right a search icon, `⋮`, expand arrows and close `×`. Under it the file name as a plain line and, at the far right, a small `</>` button. Content is the rendered markdown: h1 about 30 pt, h2, h3 bold, bullets, a full-width table with grey header and grey cell backgrounds (cells separated by white gaps, not borders), links blue.
- **`</>`** switches to the source: line numbers, fold chevrons next to headings and lists, markdown syntax coloured (headings blue bold), soft-wrapped lines; the button becomes an eye icon to go back to the rendering.
- **`⋮` menu:** "파일 트리 표시" with `⇧⌘Y`; then check items "미리보기 탭" ✓, "자동 저장" ✓, "Tab 키로 들여쓰기" ✓; a divider; "자동 줄 바꿈" ✓, "들여쓰기 가이드" ✓, "공백 표시", "끝 이후까지 스크롤"; "탭 크기  4 ›". So the panel is an editor too (auto save, tab indent), not a viewer only. Not tried: typing into it.
- **Outline icon:** opened a left column inside the panel with a filter box ("필터링하거나 ?를 입력해 파일 내…") and the file list (one file), and squeezed the rendering to the right (the table then scrolls sideways with its own scrollbar).
- **End of the turn in this session:** the reply ended with a bullet list, then only the icon row: copy, branch, pin, speaker and "3분 전". **No file card and no "실행 취소"** under this turn, although a file was written. In the `hello.txt` session of Round 7 the card was there. Not resolved; possible causes: this session is on the older plan with a different folder (not a git repo?), or the card is only for turns whose file stays unchanged. Needs one more look before building the card's undo.
- Composer in this layout: the panel narrows the chat column; the composer shrinks with it and keeps the suggested prompt ("LM Studio 설치 방법 알려줘").

## Round 9: the Files panel as an editor and line comments (2026-10-04)

Same real app, session `hello.txt 파일 생성 및 읽기` under forge-observe (folder `/private/tmp/forge-observe`), Files panel on `fizz.py`. The user showed two screenshots first (a blue "자동 저장 켜짐" tip under the panel `⋮`, "이제 파일을 변경하면 자동으로 저장됩니다. 이 기능을 끄려면 이 메뉴를 열고 자동 저장의 선택을 해제하세요.", button "알겠습니다"; and a "2번 줄 / 댓글 추가" box). No message was sent. `fizz.py` was edited once and restored from a copy; the pending cards and the comment were removed again.

- **Click a line number in the gutter:** the line gets a light blue band across the code column, a round blue `+` button appears left of the number, and a small floating bar with two buttons sits under the line, right-aligned: `컨텍스트로 첨부` | `댓글 추가`. Clicking another line number moves the selection and the bar. [PNL-04]
- **`댓글 추가`:** an inline box opens right under that line, pushing the following lines down. Blue border, grey title "3번 줄", a text field "댓글 추가" (focused) and a round `⊕` button at the right. Typing then `⊕` stores the comment: the box turns grey-bordered and shows the text with a pencil (edit) and a red trash (delete) icon at the right. Clicking the stored comment gives it the blue border again. [PNL-04]
- **A comment is a chip in the composer at once.** Under `⊕`, a card appears above the message box text field: about 200 px wide and 200 px high, the text at the top ("observe test comment"), a small grey badge at the bottom-left, `댓글`. Nothing is sent; the user sends it with their next message. Removing the card with the `×` that appears at its top-right corner (on hover) also removes the comment from the panel. [PNL-04]
- **`컨텍스트로 첨부`:** the same kind of card, with the line's text (`print("Foo")`) and the badge `발췌` (excerpt). The line keeps its blue band. Clicking the card opens a modal: the title is `<absolute path>:L5`, a grey line "0.02KB • 1줄 • 원본과 형식이 일치하지 않을 수 있습니다" (may not match the original formatting), the line in a bordered code box, and `×` at the top-right. The comment card was clicked too: it focuses the comment in the panel, no modal. [PNL-04]
- **Several cards sit side by side** in one row above the text field, the message box grows taller for them.
- **Editing:** a click puts the caret in the code (line 10 had 8 blank spaces of indent that were kept). Typing changes the text with no dirty mark on the tab and no "saved" text; the file on disk had the change about 2 seconds later (checked with diff). The empty line had a faint grey italic look for the typed `# edit` (a comment colour), nothing special otherwise.
- **Disk conflict:** after the file changed on disk behind the editor, nothing happened at once (the editor kept showing the old text, 3 s later). After the next edit in the editor (an undo with ⌘Z) a grey banner appeared at the top of the editor: "편집을 시작한 이후 디스크에서 이 파일이 변경되었습니다." with two buttons at the right, `취소` and `재정의`. The disk was not written meanwhile (autosave paused). `취소` closed the banner and showed the disk text again (the editor's edits dropped). `재정의` was not pressed (it presumably writes the editor's text over the disk). [PNL-04]
- **Not seen:** a drag across several lines, shift-click for a range, comment on a range, how a sent message shows the comment or the excerpt in the chat, whether unsent cards survive switching sessions, the `자동 저장` tip's trigger (first edit?), `재정의`, the search icon of the panel, binary or large files, creating or renaming files.

### Round 9, part 2: sending a message with a comment and an excerpt (2026-10-04, allowed by the user)

In the same session a comment on line 3 ("이 줄 설명해줘") and an excerpt of line 5 were attached, then the message "붙인 내용 보고 ok라고만 답해줘" was sent (Return). The reply was "ok".

- **In the chat:** the two cards sit in one row right-aligned, ABOVE the grey user bubble (excerpt on the left, comment on the right, same size as in the composer, about 200 x 200 px, each with its badge `발췌` / `댓글`), and the bubble with the typed text under them. So the attachments are not inside the bubble.
- **After sending:** the composer is empty again; the comment box on line 3 is gone from the panel (the comment was consumed by the message); the blue band on line 5 stays; the sent cards do nothing when clicked (no modal, unlike the unsent excerpt card) .
- **Under the user bubble**, after the reply, a row appears: the time ("지금", grey) and three icons: copy, undo-style arrow (rewind, not pressed), and branch (fork). Same row style as Round 7.
- **While it worked:** the usual dots and seconds ("2초"), the composer's send button turned into a round stop icon.
- **Not seen:** a range selection (shift-click or drag; the observe script cannot shift-click yet), a comment on several lines, how a later resume of the session shows the cards, what the model actually received as text (the real app's wire format). `fizz.py` was not changed by this.
- **Told by the user (they tried it in the real app, 2026-10-04):** closing the session and opening it again shows the sent cards the same way (they stay in the chat). Dragging over several lines behaves like picking one line: the same blue band over the lines, the same `컨텍스트로 첨부 | 댓글 추가` bar, and the same kind of card, so a range works like a single line. (The card text for a range and its `경로:L5-L8`-style label were not looked at by me.)

### Round 9, part 3: the Files panel `⋮` menu, item by item (2026-10-04)

The user asked whether the menu was fully observed; Round 5 and 8 only listed the items. This round opened each one that shows on this short file and put every setting back to what it was.

- **The menu is a popover under the `⋮`**, right-aligned to it, about 290 px wide at 1.2 scale. Items from the top: `파일 트리 표시` ✓ with `⇧⌘Y` at the right; a divider; `미리보기 탭` ✓, `자동 저장` ✓, `Tab 키로 들여쓰기` ✓; a divider; `자동 줄 바꿈` ✓, `들여쓰기 가이드` ✓, `공백 표시`, `끝 이후까지 스크롤`, `탭 크기` with the value `4` and a `>` at the right. The ticks are blue and right-aligned.
- **A click on a check item does not close the menu.** The tick appears or goes away at once and the editor behind changes at once. Escape closes it (the `⋮` then keeps a blue focus ring). A click on the `⋮` while the menu is open closes it.
- **`공백 표시`:** every space in the code gets a faint grey middle dot (also in the indent: 4 dots per level, and on the empty line 10 that holds 8 spaces); no mark for line ends.
- **`들여쓰기 가이드`:** off removes the thin vertical lines in the indent.
- **`탭 크기`:** a submenu opens to the LEFT of the menu (the menu is at the screen edge), a small popover with three items: `2`, `4` ✓, `8`. Not chosen (the file holds spaces, so a tab size shows nothing here).
- **`끝 이후까지 스크롤`:** switched on and off; nothing to see in a file of 11 lines (the usual meaning: you may scroll until the last line is at the top). Not verified on a long file.
- **`자동 줄 바꿈`, `미리보기 탭`, `Tab 키로 들여쓰기`, `파일 트리 표시`:** not switched (no long line in the file; the others change what a click on a file does or hide the tree, so I left them). Round 8 saw that the preview tab shows its name in italic and a later file replaces it.
- **The blue tip under the `⋮`** the user showed first ("자동 저장 켜짐": changes are saved by themselves from now on; open this menu and untick 자동 저장 to turn it off; button `알겠습니다`) is a one-time hint, probably on the first edit.
- **A range in the editor** (the user had dragged lines 4 to 9): a light blue band over all the lines, to the end of the longest line's width, and the `컨텍스트로 첨부 | 댓글 추가` bar floats ABOVE the first selected line (right under line 2 here, over line 3), not under the last one.

### Round 9, part 4: tabs, markdown source, wrap and scroll past end (2026-10-04)

Opened `README.md` (2 long lines, markdown) in the same Files panel to see the settings that `fizz.py` could not show. Everything was put back; the README tab was closed and both files are unchanged.

- **Tabs:** clicking a file in the tree adds a tab to the right of the open one. `README.md` came as an ITALIC tab with a `×` (the preview tab). The older `fizz.py` tab, which had been edited, is upright (a normal, kept tab). The tree row of the open file has a grey band. Clicking `×` on the italic tab closed it and `fizz.py` came back with its range selection still in place (selection is kept per tab).
- **Markdown file:** opens RENDERED (inline code with a light grey chip, body text) with a `</>` button at the right of the file-name row; the source view swaps it for an eye icon (as in Round 8).
- **Source view of markdown:** line numbers, a fold chevron next to line 1, a grey band on the line with the caret, long lines soft-wrapped at the panel width; a wrapped line has its number only on the first row. Line 3 is empty.
- **`자동 줄 바꿈` off:** each file line stays on one row and runs past the right edge; nothing is wrapped (the panel clips it, no scrollbar was visible at that moment). On again it wraps as before.
- **`끝 이후까지 스크롤` on:** the wheel keeps scrolling after the last line is in view, until the last line (line 3) sits at the top of the editor, with empty space under it. Off: the content does not scroll past the end (a short file does not move).
- **The `⋮` keeps a blue focus ring** after the menu is closed with Escape, until something else is clicked.
- **Not seen:** a tab with unsaved or conflicting changes (any mark), drag to reorder tabs, middle-click, the tab's own context menu, opening a binary or very large file, the panel search icon at the top right (the filter field was not used with a `?` search this round).

### Round 9, part 5: the rest of the Files panel (2026-10-04)

Done with test files in `/private/tmp/forge-observe` (a binary file, a small PNG, text files of 4 MB, 10 MB, 15 MB, 20 MB and 32 MB); all removed again, `fizz.py` and `README.md` are as before. The script `scripts/observe/claude_app.py` gained `click --button right|middle`, `click --shift` and `drag`, and the `delete` key.

- **Right click on a file row in the tree** (menu, top to bottom): `컨텍스트로 첨부`; `복사 >` (submenu `절대 경로`, `상대 경로`, `파일 이름`); divider; `다음에서 열기 >` (submenu `VS Code`, a divider, `기본 앱`); `Finder에서 보기`; divider; `파일 트리에서 표시`; divider; `파일에서 찾기` with `⌘F`; `파일 내용 복사`; `파일 다시 로드`; divider; `파일 닫기`. The first item put a blue chip with the file name (`fizz.py`) into the message box, not a card (a card is for lines; a whole file is a chip inside the text, with the caret after it). Submenus open to the right of the menu. Nothing else was pressed.
- **Right click on a tab:** `컨텍스트로 첨부`; `복사 >`; divider; `파일 트리에서 표시`; divider; `열어 두기` (keep open); `파일 닫기`; `다른 파일 닫기`; `모든 파일 닫기`. `열어 두기` turned an italic preview tab into a normal upright one (a later file no longer replaces it).
- **Dragging a tab** (pressing, moving 360 px right, releasing) reordered it: the dragged tab went to the end; the selected tab stayed selected. Tabs are about 180 px wide and each has its own `×`; the selected one has a light grey pill.
- **File kinds:** (a) a binary file (random bytes): the editor area is empty with the text "이 파일은 바이너리 데이터로 보이며 텍스트로 미리 볼 수 없습니다." and two small buttons, `Finder에서 보기` and `그래도 텍스트로 보기` (the second shows the bytes as text with replacement characters and a row per line, soft wrapped). (b) a PNG: the picture shown centred at its real size (64 px at 1.2 scale about 107 px wide) on the plain page, nothing else; the tree icon of an image is a small picture glyph. (c) a text file of 4.3 MB, 60 000 lines: opened at once, no warning, the gutter got wider for 5-digit line numbers. (d) a text file of 10 MiB opened too. (e) 15 MiB and 20 MiB and 32 MB did not: the area shows "이 파일은 미리 보기를 사용할 수 없습니다" and a grey line "여기서 미리 보기를 하기에는 파일이 너무 큽니다." and the full path under it. So the limit sits between 10.5 and 15.7 MB (not narrowed further). A markdown file shows its rendered view first; a Python file has a `</>` glyph in the tree, plain text files a page glyph, markdown a pencil-page glyph.
- **The tree** did not drop the removed files at once; a few minutes later (after other clicks) it listed only the files that were left, so it refreshes by itself, not instantly.
- **Filter field, by name:** typing `md` kept the files whose name contains it (`panel1.md`, `README.md`) and printed the matching letters in bold; a `×` appears at the right end of the field to clear it. [PNL-04]
- **`?` search (inside files):** typing `?Buzz` showed three small toggles at the right end of the field: `Aa` (match case), `ab` underlined (whole word), `.*` (regular expression), and `×`. Results are grouped by file: the file row with its glyph and, under it, one row per hit: a grey line number, then the line text with the match on a blue background, long lines cut with `…` on both sides around the match. Several files in one list. A click on a hit opened the file (an existing tab was reused, a new file gets an italic tab), put the caret on that line (a light grey band) and opened the in-file find bar at the top of the editor: a field with the term (`Buzz`), `Aa`, `ab`, `.*`, the count "2개 중 2개", an up and a down arrow and `×`; every match in the file is shaded, the current one has a green box. [PNL-04]
- **The search icon at the top right of the panel** (the magnifier next to `⋮`) only moved the cursor into the filter field at the top left and selected its text; it opened nothing new.
- **Unsaved state:** typing in a tab shows nothing on the tab (no dot, no star) during the 2 s before auto save writes the file. The conflict banner (round 9 part 1) came back by itself when the editor and the disk differed and the next edit was made.
- **Right click in the editor text** (on line 5): the caret moves to the clicked line (light grey band) and a menu opens at the pointer (flipped to the left near the edge): `컨텍스트로 첨부`; `댓글 추가`; divider; `실행 취소 ⌘Z`, `다시 실행 ⇧⌘Z` (both grey when nothing to undo); divider; `잘라내기 ⌘X`, `복사 ⌘C`, `붙여넣기 ⌘V`; divider; `전체 선택 ⌘A`; divider; `줄로 이동 ⌃G`. So `컨텍스트로 첨부` and `댓글 추가` are also reachable from the text, not only from the gutter bar. The menu was closed with Escape.

### Round 10: the window at different widths (2026-10-07)

Resized the real Claude window (height 800 pt, widths 1500 down to 600) with `osascript` (`set size of window 1`), on the Code tab, once on the new-session screen and once on a session with the Changes panel open. Nothing was sent, and the window and the panel were put back at the end (1200 x 800, panel closed). Widths below are points.

- **Smallest window:** 600 pt wide (asking for 500 gave 600). Height was not tried.
- **Sidebar:** fixed at 280 pt from 1500 down to 710 pt; it does not shrink. At 700 pt and below it is gone, and the whole window is the main pane (the title bar keeps traffic lights, the sidebar toggle and back/forward; the chat/code switch and the sidebar search are hidden with the sidebar). The switch sits between 700 and 710.
- **New-session screen:** the content is one centred column; with the sidebar shown it is the window minus the sidebar minus a side margin of 40 pt (25 pt from about 850 pt down). The column stops growing at about 770 pt (seen at 1500). Greeting and the usage card sit at the left edge of that column; the card stops at 480 pt and shrinks below that (420 pt at 750), its heatmap cells shrink with it (the same count of cells, smaller). The composer fills the column. The chips row above it (`로컬`, folder, add folder) never wraps or hides. The composer's bottom row (`+`, mic, mode at the left; model and effort at the right) kept all items at 600 pt.
- **A session, chat column:** the text column is centred and about 735 pt wide at 1100 pt; the user's bubbles align to its right edge; the composer has the same width as the text.
- **Opening a panel (Changes) at 1100:** the sidebar stays 280 pt, the chat column shrinks to about 300 pt (text rewraps, the composer shrinks with it) and the panel takes the rest (about 470 pt) as a rounded card with a small margin all round. The title bar loses the terminal icon and the folder tag turns into a folder icon; the session title is cut with an ellipsis.
- **Panel at 1050, 1000 and 950:** the panel is about 350 to 400 pt wide and now covers the right part of the chat; the chat does not shrink any more (its text runs under the panel and is clipped). So the panel pushes the chat until the chat is about 300 pt wide, then floats over it.
- **Panel at 900 and 700:** the same floating card, about 350 pt (900) and 400 pt (700, with the sidebar already gone). The title bar keeps only the folder icon and `⋮` at 900; at 700 the title is cut to "h." with a chevron and the folder icon is gone. The panel's own header keeps `⋮`, expand and close.
- **Sidebar toggle at a narrow width (650 pt):** the sidebar comes back as an overlay, 280 pt wide, on top of the main pane (the pane is not pushed and not dimmed; its text shows to the right of the overlay). Clicking the toggle again hides it.
- **Not seen:** below 600 pt (the window refuses), short heights, several panels at once, the Files panel's own narrow behaviour.

### Round 11: permission card and question card with a lot of content (2026-10-07)

Real Claude window, a throwaway session in `/private/tmp/forge-observe`, mode switched to 수동. Sent two messages (allowed by the user): one asking for a ~400 line `big.py` in one Write, one asking for four AskUserQuestion questions with four long options each. `big.py` was denied and never written. One option was clicked by mistake (a stale coordinate after a resize); it answered question 1 and moved to 2/4. The window was resized to 400 pt high and restored to 800.

- **Big Write card:** same layout as round 8 (title, file name, path box, path text, diff, buttons). The diff block has a **fixed height cap** of about 330 pt (the window was 800 pt) and scrolls **inside** the card; title, path rows and the button row stay put. The bottom edge of the diff fades out; after scrolling a little a fade also shows at the top. A copy icon appears at the right end of the path row on hover (probably appears on the diff block). The card never grew to push the buttons off screen. Scroll by wheel moved about one line per notch.
- **Transcript while waiting:** a grey line of the form `입력이 필요합니다` as an orange chip with a round marker and a chevron; the sidebar dot for the session turns orange.
- **After deny:** the transcript shows `생성됨 big.py 중지됨 >` (grey, the file name darker); the turn ends there.
- **Card height vs window:** the card is anchored to the bottom above the composer and grows upward. With a 4-question card at 800 pt the card is about 590 pt high and the chat above is down to a 130 pt strip. At 480 pt (card top at 150 pt) and 400 pt (card top at 110 pt) the card keeps header (chip, title, collapse, ×) and footer (뒤로 / 건너뛰기 / 다음) fixed and **only the option list scrolls**, with a fade at the bottom edge. The minimum height of the window is 400 pt (asking for 330 gave 400).
- **Option rows:** a row is about 94 pt with a two-line description; descriptions wrap, nothing is cut. The `기타` row with its text field is the last row of the list.
- **Collapse `⌄` in the header** was not pressed on purpose; hovering it gives a rounded grey background. The `×` skipped **all** questions at once; Claude then said it was skipped and stopped.
- **Single select:** a click on a row selected it and moved to the next question at once (2/4, `뒤로` appears). The transcript line `질문 중 <header> ⌄` lists all four question texts as grey lines.
- **Not seen:** the multiSelect question (3/4), the answer card after submitting, the `⌄` collapse result, a Bash card with a very long command, a permission card for a diff with very long lines (horizontal scroll).

### Round 11, part 2: the rest of the card behaviour (2026-10-07)

Same session and folder. Two more questions (multiSelect + single), a Bash heredoc, a Write with very long lines. Everything permission-related was denied; nothing was written or run. The window was resized to 400, 480, 560 and 1000 pt high and put back to 800.

- **Card height cap = 60 % of the window height**, measured for the permission card (title + file + path + diff + buttons): 338 pt at 560, 480 pt at 800, 600 pt at 1000 (exactly 0.6 each). The diff or command box takes what is left and scrolls inside. The question card is not capped the same way at normal heights (its size follows its content: 4 options plus `기타` is about 425 to 440 pt at 800), but at 480 and 400 pt it also keeps header and footer and only the list scrolls.
- **Bash card:** the title is `Claude가 run <description>하도록 허용하시겠습니까?` (the verb "run" and the English description spliced in), then the description again as plain text, then a grey mono box with the whole command. Long lines **wrap** (no horizontal scroll); the box scrolls inside with a fade at the bottom. Same buttons as Write (거부 1/Esc, 항상 허용 2, 한 번만 허용 3 + `⌄`). After a deny the transcript line is the description in grey plus `중지됨 ›`.
- **Long lines in a diff:** an unbroken 500-character token wraps at the box width (mid-token, by character); a normal sentence wraps at spaces. The line number sits on the first row of a wrapped line only; the `+` marker too. The green background spans all the rows. No horizontal scroll.
- **multiSelect question:** the title ends with `(여러 개 선택 가능)`; the row's control is an empty rounded checkbox (no number key chip); a checked row turns white with a thin border and the checkbox becomes blue with a tick; `기타` has a checkbox too. `다음 ⌘↩` (grey) turns black once one row is checked. Nothing moves on its own; the user presses `다음`.
- **Collapse `⌄`:** the card shrinks to one header row (chip `1/2`, the question in bold, `>` instead of `⌄`, `×`), pinned just above the composer, so the chat gets almost the full height again. The `>` opens it back with the same state.
- **Last question, single select:** a click on a row **submits at once** (no `제출` press) when it is the last question; for earlier questions it moves on at once. The `제출` button exists for a skipped or text answer.
- **Answer card in the transcript** (after submit): a bordered card about 265 pt wide on a tinted page (width follows the longest line, here 530 px of 2000), each question in grey, the answer in black (a multi select answer is comma-joined: `Vercel, Cloudflare Pages/Workers`). Claude then answers in plain text. A small survey line `이번 세션에서 Claude는 어떤가요? 나쁨 보통 양호 ×` shows at the end of the turn (not touched).
- **Composer after the turn:** the composer shows a grey suggestion as placeholder (`big.py 다시 만들어줘`) instead of `/ 입력 시 명령어`.

### Round 11, part 3: the cards with the right panel open (2026-10-07)

Same session, window 800 pt high and 1200 wide, the Changes panel open (top bar icon `±`; the session folder is not a Git repository, so the panel shows "변경 사항을 보려면 Git 저장소가 필요합니다"). The chat column shrank to about 330 pt (sidebar 280 pt, panel about 540 pt). Write `big2.py` and a two-question AskUserQuestion; the Write was denied and the questions skipped (`×`). The panel was closed again afterwards.

- **Permission card in the narrow column:** the card keeps the same top and bottom as at full width (top at the same height, so the 60 % cap holds in a narrow column too). The title and the file name rows stay; the diff wraps much earlier (about 28 characters per row) and the line number and `+` stay on the first row of a wrapped line. The three buttons **stack vertically**, each as a full-width row in this order: `거부 1 Esc`, `항상 허용 2 ⇧⌘↩`, `한 번만 허용 3 ⌘↩ ⌄` (the last is black). The stacked buttons take about 150 pt, so the diff area becomes the smaller part of the card (about 360 px of 800 in the screenshot) and still scrolls inside with the bottom fade.
- **Question card in the narrow column:** the same cap (card top equal to the permission card's). The title wraps to two lines (the chip stays on the first line; `⌄` and `×` sit at the right, vertically centred), the option list scrolls inside (about four option rows visible; the `기타` row is cut by the fade at the bottom), and the footer (`건너뛰기`, `다음 ⌘↩`) stays on one row, right aligned. Option descriptions wrap to three lines.
- **So the cap is 60 % of the window height for both cards** (my part 2 note said the question card was not capped the same way; it is: a card that fits is smaller, a card that does not fit stops at 60 %).
- **After the skip:** Claude answered "두 질문 모두 건너뛰셨으니..." and listed what was not written; the transcript lines are `요청됨 배포 환경 >` and `생성됨 big2.py 중지됨 >`.
- **Not seen:** the Files panel and the terminal open instead of Changes; the panel in its expanded (full window) mode, which hides the chat.

### Round 11, part 4: Forge's own cards with a lot of content and the right panel open (2026-10-07, Plan 27 groundwork)

Forge in the harness (window 1200 x 832 pt), a live session in `my-app` on the real Claude, mode `Ask before edits`. A big Write and a two-question AskUserQuestion; the Write was denied and the questions skipped. Nothing was written.

- **Permission card, full width, 832 pt high:** the diff is capped at 40 vh and scrolls inside, the buttons are visible. The card is about 540 pt high (65 % of the window) and covers the top of the user's message bubble above it.
- **Permission card, window 432 pt high:** the title, file name, path box, path text and the diff (cut to 40 vh) fill the window; **the button row and the message box are below the bottom edge**, so Deny / Allow cannot be seen (keys 1, 2, 3, Esc would work). Only the diff has a height cap; the title rows, the command box (`perm-box`) and the card itself have none.
- **Permission card with the Changes panel open (chat column about 420 pt):** the button row does not fit: `Always allow` and `Allow once` wrap to two lines, and the black `Allow once` button with its `⌄` is **cut off at the right edge of the column**. The message box's control row wraps and the stop button drops to its own line below. The diff header wraps (path on two lines, `new content` on two lines). The scroll-to-end arrow sits on top of the `Normal` tab.
- **Question card (two questions, long English descriptions, panel open):** no height cap at all: the card is the whole chat column from the top bar (card top at 160 px of the 1387 px screenshot) down to where the message box begins, the message box is pushed to the bottom edge and its control row and send/stop button are below the window. The `1/2` chip, title, `×` are at the top, `Other` and `Skip` / `Next` at the bottom of a card that is as tall as its content; on a short window the footer would leave the window as well. There is no collapse button.
- **Differences to the Claude app (see part 2 and 3):** Claude caps both cards at 60 % of the window height and scrolls only the diff/command/option list; stacks the three buttons vertically in a narrow column; has a collapse `⌄` on the question card.

## Round 12: which files the right panel can show (2026-10-08)

Same real app, session `hello.txt 파일 생성 및 읽기` under forge-observe. Eight sample files were made in `/private/tmp/forge-observe/samples/` (csv, json, pdf, ipynb, html, png, svg, aiff) and clicked once each in the Files panel tree. No message was sent. The panel was closed again; the sample files were left.

Docs (code.claude.com/docs/en/desktop): "Click a file path in the chat or diff viewer to open it in the file pane. HTML, PDF, image, and video paths open in the Browser pane instead." The docs do not list other types.

What the Files panel showed in the real app (all as an italic preview tab, file name row `samples / name`):

| File | Tree icon | Panel body | `</>` toggle |
|---|---|---|---|
| `.csv` | table icon | rendered table: grey header row, grey cells with white gaps (same look as a markdown table) | yes, switches to source |
| `.html` | globe icon | the page rendered inside the panel (red h1, a real button) | yes |
| `.pdf` | plain file icon | the PDF drawn inline on a dark grey background, page at the top | no |
| `.png` | image icon | the picture at its own size, centred | no |
| `.svg` | image icon | drawn on a dark green background, zoom in, zoom out and fit buttons at the bottom right | yes |
| `.json` | `</>` icon | source with syntax colours, NOT pretty printed (one line stays one line) | no |
| `.ipynb` | plain file icon | raw JSON text, not a notebook view | no |
| `.aiff` (audio) | plain file icon | "이 파일은 바이너리 데이터로 보이며 텍스트로 미리 볼 수 없습니다." with `Finder에서 보기` and `그래도 텍스트로 보기` | no |

- Already in Forge (Plan 25): text editor, markdown rendered with `</>`, picture (png, jpg, gif, webp, svg as a plain image), the binary note with the same two buttons, too large note.
- Not in Forge: csv table, html rendered, pdf, svg zoom bar and source toggle, a dark backdrop for pdf and svg.
- Not seen: video (no sample made), a large csv, a pdf with many pages (scroll, page count, zoom), whether the html runs scripts or opens links, whether an image has zoom, what a click on a pdf path in the chat does (the docs say it opens the Browser pane, not the Files panel).

### Round 12, part 2: PDF pages, big csv, html scripts and links (2026-10-08)

Same session and folder. More samples were added to `samples/`: `multi.pdf` (12 pages), `big.csv` (5000 rows, 6 columns), `wide.csv` (40 columns), `script.html` (an inline script, a button, three links, a relative image and a relative stylesheet), `other.html`, `s.css`. No message was sent. Both link dialogs were cancelled.

- **PDF with many pages:** all pages are stacked in one column, each page as wide as the panel, separated by a thick dark line; the mouse wheel scrolls through them. No page counter, no zoom control and no toolbar showed (not even on hover or while scrolling). Not tried: pinch or ⌘ zoom, text selection, a search in the PDF.
- **Big csv:** the table shows only the first 500 rows, with a grey note above it: "처음 500개 행만 표시합니다. 더 보려면 소스로 전환하세요." (only the first 500 rows are shown; switch to the source to see more). Long cell text wraps, so a row can grow tall; a column that does not fit is cut at the right edge and the table scrolls sideways.
- **Wide csv (40 columns):** the table keeps every column at its natural width and scrolls sideways; no wrapping of the header. The `</>` toggle stays at the right of the file-name row.
- **`.css` file:** gets the `</>` icon in the tree (a code file), opens as text.
- **HTML scripts:** run. `script.html` showed "script RAN" (an inline `<script>` changed the text) and the button's `onclick` changed its label to "clicked". So the page is live, not a static render.
- **HTML relative resources:** a relative `<img src="pic.png">` showed a broken image icon and the `<link rel="stylesheet">` was not applied (the h1 stayed black). So files next to the html are NOT loaded; only what is inside the file works.
- **HTML links:** a click on the relative link `other.html` opened a dialog "외부 링크 열기" ("You are leaving Claude to visit an external link") with the address `https://www.claudeusercontent.com/other.html`, buttons `취소` and `링크 열기` (so the page is shown from the `claudeusercontent.com` origin, and a relative link resolves against it). The external link opened the same dialog with `https://example.com/`. The `#anchor` link did nothing visible (the page was too short to scroll). No link navigated inside the panel.
- **Not seen:** the chat path click (needs a message that names an html or pdf path; the docs say it opens the Browser pane, not the Files panel), video files, whether a pdf or an image can zoom.

### Round 12, part 3: clicking a path in the chat (2026-10-08, one message sent with the user's OK)

In the same throwaway session one message was sent: "도구는 쓰지 말고, 아래 세 경로를 백틱 없이 그대로 한 줄씩만 답해줘. <page.html> <doc.pdf> <pic.png>" (no tool). The reply listed the three absolute paths, each as a blue underlined link. A small grey "이번 세션에서 Claude는 어떤가요?" rating row showed under the reply (not touched). The cookie modal was closed with `나중에`; nothing else was chosen. Panels were closed again at the end.

- **Click on the `.html` path:** opened the BROWSER pane (the globe icon in the title bar turned blue), not the Files panel. The Browser pane stacked UNDER the open Files panel in the same right-hand column (Files above, Browser below, each with its own header). The first time it showed the "사이트에 로그인 상태 유지" cookie modal (the one from Round 5). After `나중에` the pane showed a tab `page.html` (not italic) with `×` and `+`, a bar with back, forward, reload, a URL field holding the plain path `/private/tmp/forge-observe/samples/page.html`, and the element-picker icon at the right; the page was drawn under it (red "Hello HTML" and the button). Header icons: a layout icon, `⋮`, expand, close.
- **Click on the `.pdf` path:** a new tab `doc.pdf` in the same Browser pane. It shows a full PDF viewer (the browser engine's own): a dark toolbar with a sidebar toggle, page box "1 / 1", zoom out and in, fit to page, rotate, draw/annotate, undo, redo, download, print and `⋮`; a thumbnail strip at the left (page 1 selected, its number under it) and the page at the right. So the same PDF has two looks: bare stacked pages in the Files panel, a full viewer in the Browser pane.
- **Click on the `.png` path:** neither panel. A dark overlay (lightbox) covered the window with the picture in its middle (the red square), the absolute path as a white caption under it, and a white `×` at the top right of the picture. `×` closed it. So an image path in the chat opens a lightbox, which the docs do not say (they say the Browser pane).
- **The Files tree and the chat path are different doors:** clicking the file in the Files tree (Round 12 part 1) gives the Files panel look (pdf bare, html in the panel); clicking its path in the chat gives the Browser pane look (pdf viewer, html with a URL bar).
- **Not seen:** the `.mp4` path (no sample), `⋮` and the layout icon of the Browser pane, what the Browser pane does with a `.csv`, `.md` or `.svg` path in the chat, whether an image lightbox has zoom or arrows.

### Round 12, part 4: csv, md, svg and png paths in the chat, and the image lightbox (2026-10-08, a second message sent with the user's OK)

Same session. One more message with no tool: "아래 네 경로를 … <samples/data.csv> <README.md> <samples/pic.svg> <samples/pic.png>"; the reply gave four blue links.

- **`.csv` path:** opens in the FILES panel, as an italic preview tab `data.csv` rendered as the table; the tree opens its folder and marks the file's row.
- **`.md` path:** opens in the FILES panel, rendered (italic tab `README.md`), the tree row marked.
- **`.svg` path:** opens in the BROWSER pane as a new tab `pic.svg` (white page, the picture at its own size at the top left, URL field with the path). Not the Files panel, unlike a click on the same file in the tree (that shows the dark-green zoom viewer).
- **So the rule seen so far:** html, pdf, svg open in the Browser pane; csv and md open in the Files panel; png opens a lightbox. The Browser pane keeps its tabs while it is closed and opened again (`page.html`, `doc.pdf`, `pic.svg` were all still there).
- **Image lightbox (png):** the mouse wheel does nothing. A click on the picture closes it. No zoom control showed (only the `×`). Keyboard zoom was not tried (the one attempt was a mistake, see below). Not seen: a larger picture, arrows between pictures.
- **A mistake in this round, put right:** `claude_app.py key equal cmd` typed e, q, u, a, l with ⌘ (the script only knows a few key names and sent the rest as letters). In the real app that archived the session `hello.txt 파일 생성 및 읽기` (⌘A is archive, A in the session menu) and marked it unread (U); the app was not quit. Both were undone through the session's right click menu (`보관 해제`, `읽음으로 표시`) and the Status filter, which was switched to `보관됨` to look, was set back to `활성`. The script now exits with an error for an unknown multi letter key name.

## Round 13: the Customize page (2026-10-08, Plan 30, PG-02)

Same real app, sidebar item "사용자 지정". Only pages, tabs, menus and dialogs were opened. Nothing was added, removed, switched off, connected or uploaded; every dialog was cancelled. No message was sent.

- **Page:** heading "사용자 지정"; tabs `Skills`, `커넥터`, `플러그인`; a switch `내 항목` / `탐색`; at the right a search icon, a filter icon, a sort icon (the sort icon is missing on the connector tab) and a dark `+ 추가` button with a menu. The menu differs by tab.
- **`추가` menu, Skills:** Skill 업로드, Skill 만들기, Claude로 만들기, 화면 녹화. **Plugins:** 마켓플레이스 추가, 마켓플레이스 관리, 플러그인 업로드, Claude로 만들기. Connector tab: not opened.
- **Skills, 내 항목:** two groups with a count: `내가 만듦` (6) and `Anthropic 및 파트너 제공` (23). A row: icon, name, a tag (the plugin it came from, e.g. `M365 connect`), the source ("내가 만든" / "Anthropic 작성"), the description cut to one line, and the date at the right (a relative time or a date). A row of the second group also had a `⋮` at the right (menu not opened).
- **Skill detail** (click a row): a back link "내 Skills", icon, name, a line "내가 만든 · M365 connect에서 · 3월 25일 업데이트됨"; tabs `개요` and `구성 파일 · N`. `개요`: the description at the left; a side card at the right with 생성자 (사용자), 플러그인 (M365 connect), 커넥터 및 도구 (a warning "다른 서비스로 데이터를 전송할 수 있음" with an info icon, and the connector name `m365`). `구성 파일`: a version menu "M365 connect v1 · 현재 버전" with a chevron, a download icon, a file tree (folder `commands`, file `m365-calendar.md`) and the file at the right with its path and an eye / `</>` switch (rendered or source). The rendered file showed the front matter (`description`, `allowed-tools`, `argument-hint`) as a heading, then the body.
- **Skills, 탐색:** a banner card "From Anthropic" / "Data" with a one-line text and a black `추가` button (the banner colour differs by tab: green on Skills, blue on Plugins); a section "새 Skills" with `모두 보기 →`; cards in two columns: icon, name, two lines of text, the source line ("Anthropic Directory" or "제작: Anthropic"), and a `+` button at the top right. Below: a section "가장 많이 설치된 Skill" (more sections below, not scrolled).
- **Explore card detail** (click the card, not `+`): same layout as the detail above, with a black `추가` button at the top right, "Anthropic 작성 · 9월 15일 업데이트됨", the full description, a side card with only 생성자 (Anthropic); `구성 파일 · 2`.
- **Connectors, 내 항목:** a table with columns 커넥터, 유형, 상태, and no groups. Rows: Claude in Chrome (데스크톱, badge `포함됨`, check), Filesystem (데스크톱, check), GitHub 통합 (웹, check), Google Drive (웹, check), puppeteer (데스크톱, badge `로컬 개발`, check), context7 (웹, badge `플러그인`, second line "Context7 플러그인에서 제공됨", state "세션에서 연결됨"), and Fellow.ai, Gmail, Google Calendar, Vercel (웹, a `연결` button each). At the bottom a grey card "Claude 디렉터리용 개발": "커넥터 또는 플러그인을 등록하여 Claude를 사용하는 모든 사람에게 제공하세요." with `개발자 문서 ↗` and a black `디렉토리에 제출`. So desktop connectors are the local MCP servers; web ones are remote with an account sign-in.
- **Plugins, 내 항목:** three groups. `이 프로젝트에서` (0) with a project picker button at the right (the folder name, here `forge-observe`) and the empty text "이 프로젝트에 설치된 플러그인이 아직 없습니다."; `내가 만듦` (1; name, tag `엔지니어링`, `+4` more tags, date, `⋮`); `Anthropic 및 파트너 제공` (8). A row: icon, name, a blue badge `새로 생성` on recent ones, "<author> 작성 · description", a relative date, `⋮`. A plugin with no skills reads "Skills 0개".
- **Plugin row `⋮` menu:** `끄기` and, in red with a trash icon, `제거`. Not chosen.
- **마켓플레이스 관리 (dialog):** "이 컴퓨터에서 Claude Code로 추가한 마켓플레이스입니다. 마켓플레이스를 제거하면 해당 플러그인도 함께 제거됩니다." Cards: name (`claude-hud`), the GitHub repo (`jarrodwatts/claude-hud`), "마지막 업데이트: 2026. 2. 21. 오전 1:21", `⋮` each; a black `완료`. The two cards are the marketplaces the user added with the Claude Code CLI, so the app reads the same plugin state as the CLI. The `⋮` menu was not opened.
- **마켓플레이스 추가 (dialog):** two choices: "Anthropic 소스 탐색" ("Anthropic과 파트너가 선별한 플러그인 마켓플레이스") and "저장소에서 추가" ("GitHub 저장소 또는 Git URL에서 플러그인 마켓플레이스 동기화"); `취소`. Neither was chosen, so the form for a repo address was not seen.
- **Plugins, 탐색:** the same screen as Skills 탐색 with the plugin catalogue ("새 플러그인", the same cards; the banner is blue).
- **Not seen:** the connector 탐색 tab and `추가` menu; the filter and sort menus; the search field; `모두 보기`; what `+`, `체험` and the banner `추가` do (they install); the connector detail; the "Anthropic 소스 탐색" screen; the repo form; Skill upload and create; the row `⋮` of skills; whether an organisation (Team / Enterprise) can restrict or push a catalogue (the docs may say; not read).

## Round 14: Routines (2026-10-09, Plan 31, PG-01)

Same app and window. Only pages and menus were opened. The form of a new local routine was opened and left with `취소`; nothing was saved, run, paused or deleted.

- **List ("루틴" in the sidebar):** title, tabs `내 항목` and `템플릿`, and at the right a search icon, a filter icon, a sort icon and a black `새 루틴 ⌄`. A notice under them: "로컬 루틴은 컴퓨터가 켜져 있고 온라인 상태일 때만 실행됩니다." A card: title (a warning icon beside it), the description, then "일시중지됨 · 1분마다, 1시간마다, 매일" (state and schedule).
- **`새 루틴` menu:** two items, `로컬` and `클라우드`. Cloud is out of scope for Forge.
- **New local routine** (breadcrumb "루틴 / 새 로컬 루틴"): the same notice at the top; `이름 *` (placeholder `daily-code-review`), `설명 *`, `지침` (a large text box with a placeholder about reviewing the commits of the last 24 hours). Under the instructions, in the same box: a permission-mode chip (`수동`) at the left and a model menu (`기본값`) at the right; a grey bar with `폴더 선택` at the left and a `워크트리` checkbox (greyed until a folder is chosen) at the right. `스케줄`: a segmented control `수동 · 매시간 · 매일 · 평일마다 · 매주 · 사용자 지정`; `매일` is the default with a field `시각` (`오전 09:49`, a clock icon). `사용자 지정` shows a field `Cron 표현식` (placeholder `0 9 * * *`). A line: "루틴은 서버 성능을 위해 몇 분간 무작위 지연을 사용합니다." Buttons `취소` and `만들기` (greyed until the required fields are filled). Not seen: the menus of `수동`, `기본값` and the other schedule choices (`매시간`, `매주` fields).
- **Leaving with a changed form:** `취소` asks "저장하지 않은 변경 사항을 삭제하시겠습니까?" ("변경 사항이 저장되지 않아 손실됩니다.") with `계속 편집` and a red `취소`. Switching the schedule tab alone counted as a change.
- **Detail** (click a card; breadcrumb "루틴 / <name>"): top right a pencil (edit), a trash (delete) and a black `▷ 지금 실행` (run now). Left column: `설명`; `상태` with a switch and a tag (`일시중지됨`); `폴더` (the path in a chip); `반복` ("1분마다, 1시간마다, 매일"); `항상 허용됨` with the grey text "실행 중에 부여한 승인이 여기에 표시됩니다." Right column: `지침` (cut after three lines with a fade and `모두 보기 ⌄`); `기록` (history): rows with a status icon and a time ("오늘 오전 10:19" with the tag `건너뜀` and an info icon; "3월 10일 오후 10:41에" and "3월 10일 오후 10:40에" with a check). So a run missed while the computer was off or the routine was paused is shown as skipped, with a reason on the info icon (not read).
- **Not seen:** the `템플릿` tab; the search, filter and sort menus; what a history row opens (assumed: the session the run made); the info text of `건너뜀`; the edit form of an existing routine; what `지금 실행` does (assumed: starts a session at once); the `클라우드` form.

## Round 15: the Settings window (2026-10-09, OBS-05)

Same app and window. Opened with the account menu at the bottom of the sidebar (name, plan, chevron) and its item `설정` (shortcut ⌘,). Only pages were opened and scrolled. No toggle, menu value or button was changed or pressed; the window was closed with Escape. The window is a modal over the app (a dimmed background, a close `×` at the top right), a search field at the top of the left list, and three groups in the list.

- **The account menu:** the email, then `설정` (⌘,), `사용량`, `언어`, `도움 받기`; a divider; `플랜 업그레이드`, `앱 및 확장 프로그램 다운로드`, `게스트 패스 공유`, `변경 로그 보기`, `더 알아보기` (submenu); a divider; `API 키 받기` (opens outside); a divider; `로그아웃`. Not opened: the submenu.
- **Left list:** group `설정`: 일반, 계정, 개인정보보호, 결제, 사용량, 기능, 메모리, 디자인 시스템, Claude Code, Chrome용 Claude. Group `이 컴퓨터`: 시스템, 확장 프로그램, 개발자. Group `사용자 지정`: Skills, 커넥터, and more below (not scrolled). The window opened on `Claude Code` the first time, on the last page seen later.
- **일반 (General):** `화면 모드`: `테마` (three icons: system, light, dark), `채팅 글꼴` (a menu, `Anthropic Serif`), `대화 내용 텍스트 크기` (작게 / 보통 / 크게), `대화 기록 너비` (좁게 / 보통 / 넓게; narrow was chosen), `모션` (시스템 / 줄임; "스트리밍 응답 및 기타 인터페이스 요소의 애니메이션을 줄입니다"). `작업`: `신뢰할 수 있는 폴더` with `관리` ("이 기기의 폴더를 신뢰하면 Claude가 해당 폴더와 그 하위 폴더를 사용하도록 허용하게 됩니다…"), `기본 브라우저` (a menu, `내장 브라우저`). `음성`: `언어` (한국어), `스타일`, `속도` (보통). `알림`, all switches on: 응답 완료, 예약된 작업, Code 알림, Code 권한 요청, Claude Code 클라우드 세션 이메일, 메시지 전송, 멘션 및 답글, 내 아티팩트에 달린 댓글, 나와 공유된 아티팩트.
- **기능 (Features):** `일반`: `도구 액세스 모드` (a menu, `필요할 때 도구 로드`), `커넥터 검색` (on), `메시지가 플래그 지정되면 모델 전환` (on). `비주얼`: Artifacts (on, greyed), `AI 기반 아티팩트` (off), `인라인 시각화` (on). `코드 실행 및 파일 생성`: `클라우드 코드 실행 및 파일 생성` (on), `외부 네트워크 접속 허용` (on) with a card: `도메인 허용 목록` (a menu, `패키지 매니저만`), an info line, `추가 허용 도메인` (a field and `추가`). `Skills`: "Skills가 맞춤 설정으로 이동했습니다" with a link. This page is for the chat side; little of it applies to a coding session.
- **메모리 (Memory):** switches `채팅 검색 및 참조` (on), `채팅에서 메모리 생성` (on), `민감한 주제를 메모리에 포함` (off); `다른 AI 제공업체에서 메모리 가져오기` with `가져오기 시작`; a list `나` (Profile) and `주제` (Interests, Real Estate Finance, with a one-line text and "7월 19일 업데이트됨"); a message box at the bottom "변경하거나 삭제할 내용을 Claude에게 알려주세요". This is chat memory, separate from Claude Code's CLAUDE.md and auto memory.
- **Claude Code** (the page the user asked for), top to bottom:
  - `일반`: `세션 상태 분류` (on; "Claude가 세션을 차단됨, 검토 준비됨 또는 완료됨으로 자동 분류하도록 허용합니다. … 플랜 사용량에 포함됩니다. 새 세션에 적용됩니다"), `메시지가 플래그 지정될 때 모델 전환` (on; "이 옵션을 끄면 세션이 대신 일시 중지됩니다"; the same switch appears again in `세션` with "이 컴퓨터의 로컬 세션에 적용됩니다").
  - `코드 모양`: two menus `Claude Light` and `Claude Dark` (code themes), each with a preview of a diff (3 lines of `function greet`, a removed and an added line); `코드 글꼴` (a text field, placeholder `예: JetBrains Mono`; "코드 및 터미널에 사용할 사용자 지정 고정폭 글꼴").
  - `화면 모드`: `인터페이스 글꼴` (Anthropic Sans / 시스템 / OpenDyslexic, a link `글꼴 라이선스`), `기본 대화 기록 보기` (일반 / 사고 과정 / 자세히; "세션이 기본으로 열리는 보기입니다. 세션의 트랜스크립트 보기 메뉴에서 보기를 선택하면 해당 세션에만 적용됩니다").
  - `원격 제어`: `새 세션을 Remote Control에 연결` (off).
  - `세션`: `권한 우회 모드 허용` (off, with a risk text and a link), `동적 워크플로우` (off; "복잡한 작업을 위해 여러 에이전트를 병렬로 실행"), `알림에 주의 끌기` (off; bounce the Dock icon), `Claude가 작업하는 동안 컴퓨터 절전 모드 방지` (on) with a sub switch `배터리 전원 사용 시 절전 모드 방지` (on), `프롬프트 제안` (on), `비활성 세션 보관` (a menu, `없음`), `워크트리 위치` (a menu, `프로젝트 내부`), `출력 스타일` (a menu, `기본값`; "새 세션에 적용되며, 세션 메뉴에서 개별 세션에 대해 재정의할 수 있습니다"), `로컬 샌드박스` (off) with a sub switch `엄격한 샌드박스 모드` (off, greyed).
  - `Browser`: `브라우저 도구` (on), `쿠키 유지` (a menu, `종료 시까지`; "세션별을 선택하면 각 Code 세션이 고유한 복사본을 가집니다"), `허용된 사이트` with `관리`.
  - `모바일 시뮬레이터`: `iOS 시뮬레이터` (on).
  - `풀 리퀘스트`: `브랜치 접두사` (a text field, `claude`), `풀 리퀘스트 자동 생성` (off, cloud sessions only), `풀 리퀘스트 자동 수정` (off), `PR 병합 또는 닫기 후 자동 보관` (off).
  - `인증 토큰`: a table of the apps and scopes of every login (Claude Code with `user:file_upload`, `user:inference`, `user:mcp_servers`, `user:plugins`, `user:profile`, `user:sessions:claude_code`), "2시간 전 연결됨", 73 rows in pages of 5 ("15페이지 중 1페이지").
  - `Claude Code (CLI, Desktop, IDE)`: `Anthropic에 저장된 세션 삭제` with a `삭제…` button, `공유 설정` with `관리` (claude.ai/code sharing).
- **시스템 (System):** `데스크톱 앱 버전` (2.31226.0), `시작 시 실행` (off), `빠른 액세스 바로가기` (a menu `사용자 설정…` and a key chip `⇧⌘ Space` with a clear button), `음성 바로가기` (`단축키 없음`), `음성 입력 단축키` (`Command+D`), `메뉴 바` (on; show Claude in the menu bar), `컴퓨터 절전 모드 방지` (on; for scheduled tasks), `저장 공간` (`관리`), `저장 폴더` (two buttons, the first one cut off by the window edge, and `변경`). Lower: a Chrome section ("브라우저 1개 연결됨", `다시 확인`, a row `Browser 1 · macOS · 기기 ID`), `기본 제공 브라우저에서 링크 열기` (on), `허용된 사이트` (`관리`), `컴퓨터 사용` (beta; "이 컴퓨터에서 실행되지 않는 클라우드 Code 세션에서는 사용할 수 없습니다"): `컴퓨터 사용 활성화` (on), `Claude가 완료되면 앱 숨기기 해제` (on), `Claude가 허용한 앱을 사용하는 방식` (백그라운드 / 전체 제어), and a list of allowed apps below (not scrolled).
- **개발자 (Developer):** `로컬 MCP 서버` ("작업 중인 MCP 서버를 추가하고 관리하세요"), a button `구성 편집`, a list at the left (Filesystem, puppeteer) and a detail at the right: name and a blue `실행 중` badge, "이 서버는 확장 프로그램에 의해 관리됩니다", `명령어`, `인수`, and a `로그 보기` button; a card "API 키가 필요하신가요?" with `Claude Platform으로 이동 ↗`.
- **Not seen:** 계정, 개인정보보호, 결제, 사용량, 디자인 시스템, Chrome용 Claude, 확장 프로그램, Skills and 커넥터 as settings pages, the end of 시스템 and the end of the left list; every menu's choices (themes, `비활성 세션 보관`, `워크트리 위치`, `출력 스타일`, `쿠키 유지`, `도구 액세스 모드`); what `관리` opens on `허용된 사이트` and `신뢰할 수 있는 폴더`; the search field; the account menu submenu.
- **A slip in this round, put right:** a click made while the Settings window was not open landed on the sidebar. It opened the Routines list and collapsed the group `forge-observe`; the group was opened again with one click. Nothing was changed otherwise. Lesson: capture after opening the account menu before the next click; `claude_app.py key` has no name for the comma, so ⌘, cannot be sent, use the account menu.

### Round 15, part 2: the rest of the Settings window (2026-10-09)

Same rules as part 1: pages scrolled and drop-down lists opened and closed (by clicking the same button again); no value was chosen. Money, card and account details are not written down here, only what each page has.

- **계정:** `프로필`: `프로필 사진` (PNG, JPEG, WebP, up to 10 MB), `성명`, `Claude가 어떻게 불러드릴까요?`, `어떤 일을 하시나요?` (a menu), `Claude 지침` (a text box, "Claude는 Anthropic의 가이드라인 내에서 이 항목과 관련된 모든 계정의 채팅 전반에서 이를 기억합니다"). `계정`: `모든 기기에서 로그아웃` (button) and `계정 삭제` (greyed: cancel the subscription first). Account data; out of scope for Forge.
- **개인정보보호:** two links (`데이터를 보호하는 방법`, `데이터 사용 방법`), `환경설정`: `위치 메타데이터` (on), `Help improve our AI models` (off; "Allow the use of your chats and coding sessions to train and improve Anthropic AI models"); `내 데이터`: `데이터 내보내기`, `공유된 채팅` / `공유된 아티팩트` / `업로드한 파일` (each with `관리`).
- **결제:** the plan card (name, monthly, renewal date) with `플랜 조정`; `결제` with `결제 수단 추가`, the card on file with a badge `기본값` and `업데이트`; `청구서`: a table (날짜, 총액 with an info icon, 상태, 작업 `PDF 다운로드`).
- **사용량:** a headline that warns when the weekly limit will run out before the reset ("이 속도라면 수요일 재설정 전에 사용량이 소진됩니다. 소진 예상 시점: 일요일 저녁"), buttons `플랜 업그레이드` and `사용량 추가 구매`; two bars: `현재 세션` ("오후 1:40에 재설정", a percentage, red when nearly full) and `이번 주` ("재설정: (수요일) 오후 6:00", blue); `제한 초기화` ("현재 초기화 없음"); `클라우드 세션 크레딧` with a `?` icon and a bar for the included credit with its expiry. The same numbers as the circle popover (CMP-07, round 4), now with a forecast line.
- **디자인 시스템:** "디자인 시스템을 사용하면 Claude가 디자인, 덱, 프로토타입을 만들 때 회원님의 색상, 타이포그래피, 컴포넌트를 활용합니다."; `새 디자인 시스템 ⌄`; a name search; a table (이름, 사용 가능 대상 with an info icon, 게시일); empty text "아직 디자인 시스템이 없습니다. 직접 만들거나 팀원에게 공유를 요청하면 여기에 표시됩니다." Chat side (design and slides); out of scope.
- **Chrome용 Claude:** an illustration, `Claude in Chrome 사용 설정` (on; "이 설정은 확장 프로그램에만 적용됩니다"), `사이트 권한` ("Claude in Chrome과 Claude 데스크톱 앱의 내장 브라우저에 적용됩니다") with `모든 사이트 기본값` (a menu, `기본 정책 선택`).
- **확장 프로그램:** "Claude가 컴퓨터의 앱, 데이터 및 도구와 직접 상호작용할 수 있도록 허용합니다." Button `확장 프로그램 찾아보기`; `컴퓨터에 설치됨`: a row (icon, name `Filesystem`, `구성`, `⋯`); `고급 설정`; a hint ".MCPB 또는 .DXT 파일을 여기에 드래그해서 설치하세요". The row `⋯` and `구성` were not opened.
- **Skills, 커넥터, 플러그인 (group `사용자 지정`):** the same screens as the Customize page (round 13), with the same title row (tabs `내 항목` / `탐색`, a search field, filter, sort, `+ 추가`), shown inside the window. The connector table is the same as in round 13 with Gmail, Google Calendar, Microsoft 365 and Vercel as `연결` rows. Group `플랫폼` at the bottom of the list has `API 키` (an arrow icon: opens outside). The left list scrolls; its end is `API 키`.
- **Drop-down lists of the Claude Code page:**
  - `Claude Light` / `Claude Dark` (code themes): a search field and a list with a check at the current one. Light: Claude Light, GitHub Light, Pierre Light, One Light, Catppuccin Latte, Solarized Light, Vitesse Light, Min Light, Rosé Pine Dawn, and more below (not scrolled).
  - `비활성 세션 보관`: 없음, 1일, 2일, 7일, 14일, 30일 (with a search field).
  - `워크트리 위치`: `프로젝트 내부` with the path `.claude/worktrees`, and `사용자 설정…` (a folder choice, not opened).
  - `출력 스타일`: 기본값, Concise, Explanatory, Learning, Proactive. These are Claude Code's own output styles, so the SDK or the CLI lists them.
  - `쿠키 유지`: 종료 시까지, 공유됨, 세션별.
- **시스템, end of the page:** `컴퓨터 사용 (beta)` continues with `거부된 앱` ("Claude가 이러한 앱에 액세스하기 위해 요청하는 모든 요청은 자동으로 거부됩니다", a menu `앱 추가`, empty text), `접근성` with a state `승인됨` and `화면 녹화` with `승인됨` (macOS permissions). The page ends there.
- **Not seen:** the submenu `더 알아보기` of the account menu; what `관리` shows on `신뢰할 수 있는 폴더`, `허용된 사이트`, `공유된 채팅`; `사용자 설정…` of the worktree menu; the dark theme list and the rest of the light list; the `음성` menus (`스타일`, `속도`); `채팅 글꼴`, `기본 브라우저`, `도구 액세스 모드`, `도메인 허용 목록` choices; the search field of the window.
- **A slip in this part, put right:** a shell loop that passed the click point as one string failed and sent three Escape keys; the first closed the Settings window. Nothing else changed. Lesson: close a drop-down by clicking its button again, and close the window with its `×`; do not send Escape while a window is open unless closing is the goal.

### Round 15, part 3: the shortcut list and the last menus (2026-10-09)

Same rules. The list opens from the account menu: `더 알아보기` (click the row; hovering did not open it the first time) then `단축키` (⌘/). A modal titled `단축키` with a `×`, groups and rows (name at the left, keys at the right, `또는` between alternatives). Nothing was pressed except opening and closing.

- **The account menu's `더 알아보기` submenu:** `Anthropic에 대하여` (external), `Claude Academy`, `이용 정책` (external), `개인정보 처리방침` (external), `이용약관` (external), `개인정보 보호 선택`, a divider, `단축키 ⌘/`.
- **일반 (General):** 세션 검색 또는 시작 ⌘K; 검색 ⇧⌘K; 최근 항목 간 전환 ⌃Q; 사이드바 표시/숨기기 ⌘B; 단축키 ⌘/; 설정 ⌘,; 새 세션 ⌘N; 현재 설정으로 새 세션 시작 ⇧⌘N; 탭, 패널 또는 세션 닫기 ⌘W; 닫힌 세션 다시 열기 ⇧⌘T; 다음 세션 ⇧⌘] or ⌃Tab or ⌥↓; 이전 세션 ⇧⌘[ or ⌃⇧Tab or ⌥↑; 세션으로 이동 ⌘1…9; 다음 / 이전 사이드바 탭 ⌥⌘→ / ⌥⌘←; 세션을 분할 보기로 열기 ⌥+Click; 오른쪽에 새 세션 ⌃⌘\; 아래 새 세션 ⌃⌘-; 다음 / 이전 분할 보기로 포커스 이동 ⌃] / ⌃[; 분할 보기 닫기 ⌃⌘W; 세션 고정 / 고정 해제 ⌥⌘P; 세션 이름 변경 ⌥⌘R; 세션 보관 ⌥⌘A; 세션을 읽음/읽지 않음으로 표시 ⌥⌘U; 세션 링크 복사 ⌘L or ⌥⌘L; 세션 PR 열기 ⌥⌘G; 세션 포크 ⌥⌘O; 트랜스크립트 보기 ⌃O; Claude의 응답 중지 Esc; 세션을 이전 메시지로 되돌리기 Esc, then Esc.
- **패널:** 변경 사항 전환 ⇧⌘D; 변경 사항 또는 파일에서 파일 목록 전환 ⇧⌘Y; 변경 사항 중 다음 / 이전 변경 ⌥F5 or F7 / ⌥⇧F5 or ⇧F7; 변경 사항 또는 파일에서 파일로 이동 ⌘P; 브라우저 전환 ⇧⌘B; 브라우저에서 요소 선택 ⇧⌘S; 브라우저에서 페이지에 주석 달기 ⇧⌘X; 새 브라우저 탭 ⌘T; 닫힌 브라우저 탭 다시 열기 ⇧⌘T; 다음 / 이전 브라우저 탭 ⌃Tab / ⌃⇧Tab; 브라우저 탭으로 이동 ⌘1…9; 브라우저에서 주소 표시줄에 포커스 ⌘L; 브라우저에서 페이지 새로고침 ⌘R; 파일 토글 ⇧⌘F; 선택 항목을 컨텍스트로 첨부 ⇧⌘L; 터미널 전환 ⌘J or ⌃`; 창 닫기 ⌘\; 패널 펼치기/접기 ⇧⌘\; 사이드 채팅 전환 ⌘;.
- **작성기 (composer):** 모드 메뉴 열기 ⇧⌘M; 모델 메뉴 열기 ⇧⌘I; 노력 선택기 열기 ⇧⌘E; 빠른 모드 전환 ⌥⌘F; 메뉴 항목 선택 1…9; 파일 또는 사진 추가 ⌘U; 음성 입력 전환 ⌘D; 나중에 보내도록 대기열에 추가 (Claude가 작업 중일 때) ⌘Enter; 보내고 새 세션 페이지에 머무르기 ⌘Enter (the same keys, the list shows both); 포크된 세션에서 전송 (로컬 세션) ⌥⌘Enter.
- **iOS 시뮬레이터:** 화면 키보드 ⌘K or ⌥⌘K; 홈 ⇧⌘H; 잠금 ⌘L; 볼륨 ⌘↑ / ⌘↓; 회전 ⌘← / ⌘→; 흔들기 ⌃⌘Z; 라이트 또는 다크 모드 전환 ⇧⌘A; 스크린샷 저장 ⌘S; 녹화 시작 또는 중지 ⌘R; 시뮬레이터 연결 해제 ⌘Q. The list ends there.
- **Settings `일반`, the menus that were open:** `신뢰할 수 있는 폴더` `관리` opens a dialog with the same text, one row per folder (a folder icon, the path, a `×` to remove it), `+ 폴더 추가` and a black `완료`; `기본 브라우저` has two choices: `내장 브라우저` and `Chrome (Claude in Chrome)`; the three `음성` rows (`언어` 한국어, `스타일` empty, `속도` 보통) are for voice input in chat; they were not opened.
- **Notes for Forge:**
  - The app's own shortcuts overlap Forge's: ⌘K search, ⌘N new session, ⌘B sidebar, ⌘W, ⌘, settings; to check against `crates/forge-features/src/shortcut.rs` before planning.
  - Single keys of the session menu (R, F, A, D; SES-06) are the menu's own; the global ones use ⌥⌘ (rename ⌥⌘R, archive ⌥⌘A, fork ⌥⌘O, pin ⌥⌘P, unread ⌥⌘U).
  - Split panes and a `사이드 채팅` exist (OBS-06): `⌥+Click`, `⌃⌘\`, `⌃⌘-` open or split; a side chat toggles with ⌘;.
  - The composer keys (⇧⌘M mode, ⇧⌘I model, ⇧⌘E effort) match Forge's pickers (CMP-03..05); queueing a message while Claude works (⌘Enter) is not in Forge's backlog.
- **Not seen:** the dark theme list; what `사이드 채팅` looks like; what `빠른 모드` is (⌥⌘F); the window search; the three voice menus.

## Round: dynamic workflows (SET-12), 2026-10-09

Opened Settings > Claude Code > `세션` in the installed app and turned `동적 워크플로우` on and off once (left off). Text: "Claude가 복잡한 작업을 위해 여러 에이전트를 병렬로 실행하도록 합니다. 워크플로우는 사용량 한도를 빠르게 많이 소모할 수 있습니다." Off by default, right under `권한 우회 모드 허용`, no sub-option, no dialog. On wrote `"enableWorkflows": true` to `~/.claude/settings.json`; off removed the key (not `false`). A running Claude Code session got the `Workflow` tool at once and lost it again. With the real SDK, `settings: { enableWorkflows: true }` alone puts `Workflow` in the init tool list; `false` or nothing leaves it out.

## Round: a bigger workflow in the desktop app, 2026-10-09

Switched `동적 워크플로우` on, started a session in `forge-observe` (title came by itself: "Parallel agent file analysis workflow") and asked for three parallel agents over three folders and a fourth that merges them. About 2 minutes. Switched the setting off again afterwards. A test session "Parallel agent file analysis workflow" stays in the real app's `forge-observe` group; it can be archived by hand.

What the window showed, in order:

- Right after sending: a line `실행 중 명령어 5초 ›` with three moving dots; the title bar then shows the session title, the folder badge `forge-observe` and, at the right, five icons (terminal, changes, browser, folder, `⋮`).
- Before the workflow starts: a card in the chat, `sample-risk-review`, `워크플로우 시작 중`, and the reply card of a tool group `실행된 명령 1개, 사용함 도구 1개 ›` (inside: `Skill 실행됨 /workflow-authoring` and one `ls` command: Claude read the authoring skill first).
- The permission card for the workflow itself is its own kind: title `Claude가 워크플로우 실행 sample-risk-review하도록 허용하시겠습니까?`, the workflow's description line ("Three parallel agents review ..."), a grey note `동적 워크플로우는 여러 하위 에이전트를 병렬로 실행하며 사용량 한도를 많이 소모할 수 있습니다. 작업 패널에서 언제든지 중단할 수 있습니다.` (with `작업 패널` as a link) and two buttons only: `거부` (1, Esc) and `한 번만 허용` (2, ⌘↵). There is no `항상 허용` here (the ordinary tool card has it). The sidebar row of the session got an orange dot.
- While it runs: the card shows `워크플로우  3개 에이전트  20초` with three small progress dots, then `1분 27초` (a clock that counts up), with a `›` at the right edge. Under the reply text: `실행 중인 작업 1개`.
- Permission requests of the agents inside the workflow come to the same place as any other: a card `Claude가 run하도록 허용하시겠습니까?` (the tool name is shown as `run`), a one-line command preview, the reason it could not be auto-allowed (`find with '-exec' executes commands ... cannot be auto-allowed by a Bash(find:*) prefix rule`; `A variable in this command can't be checked before it runs`), the full command in a code box, and `거부` / `한 번만 허용` (no `항상 허용`). Several came one after the other (one per agent command).
- A usage banner sat above the composer the whole time: `현재 속도라면 주간 한도에 예상보다 일찍 도달합니다  67% 사용됨 · 재설정: 10월 14일 (수) 오후 6:00` with an `×`.
- When it finished: the card shrinks to one grey line `워크플로 실행됨 sample-risk-review` (plain text; clicking it does nothing). The follow-up turn starts in a new paragraph (`The workflow finished: all four agents completed with no errors. Here is the merged summary.`), then headings, bullet lists and a numbered `Ranked risks` list, and a last line `Each agent's raw report is in the workflow's journal.jsonl.` with `journal.jsonl` as a link. The sidebar dot goes back to the plain circle.

Typing note for the harness: `osascript` ⌘V and `keystroke` do not reach this window reliably; posting key events with a Unicode string through Quartz (`CGEventKeyboardSetUnicodeString`) typed a whole sentence without a lost letter.

### The background tasks panel (seen on the user's own running workflow, 2026-10-09)

The user ran a second workflow ("sample-folder-risk-report") and left the panel open. Only looked and opened folds; no permission was answered.

- The panel opens at the right of the chat, headed `백그라운드 작업`, with three icons: a trash can (clear finished tasks), expand (the panel takes the whole chat area, with a collapse icon in the same place) and `×`. The chat shows a blue-highlighted chip `실행 중인 작업 1개` under the reply while the panel is for that task.
- Section `실행 중`, then one card per task: the workflow name, a stop button (a small square, top right), `워크플로우  <elapsed>` (the clock counts up), `3개 에이전트  157.9k 토큰` (the token count), the workflow's description in grey, then `단계` (steps).
- Each step is a fold with its name, a count `0/3` and a row of dots (one per agent; filled blue while running). Open: a table `모델 / 시간` (narrow panel) or `에이전트 / 모델 / 토큰 / 시간` (expanded panel; agent names `domain`, `features`, `widgets`), one row per agent, every row counting the same elapsed time. A step that has not started shows `아직 시작된 에이전트 없음`. Clicking an agent row does nothing.
- A workflow whose agent waits for permission keeps counting time and stays at `0/3`; the permission card is in the chat, not in the panel. Seen for about 2 minutes. The panel's token count did not change while it waited.

### After the permission was answered (same workflow, 2026-10-09)

I allowed the three read-only agent commands (`find ... wc -l`, `head`/`grep` over the sample files) one by one; the user had asked for the part after approval to be watched.

- The dots show state by colour: blue = running, amber = waiting for permission, grey = done. The step name turns amber while an agent in it waits. The first agent to be allowed went blue and finished in about 13 s (a check mark appears before its row and its time stops); the others stayed amber until their cards were answered. The step count went `0/3`, `1/3`, `2/3`; the token count grew only while agents ran (157.9k, 162.2k, 169.9k, 175.5k).
- The chat card (`워크플로우  3개 에이전트  <time>` with the dots) mirrors the panel and is shortened to the one grey line `워크플로 실행됨 <name>` when the workflow ends.
- The reply's status line below the text changed too: while the task ran `··· 30초 · 17.7k 토큰 · 실행 중인 작업 1개` (the chip is blue); at the end `··· 42초 · 75.1k 토큰 · 작업 1개 완료`. These are the time and tokens of the main turn, not of the workflow.
- The Merge step started and ended between two captures (under 5 s): the whole run took 3 min 43 s, most of it waiting for permission.
- When it ended, the panel lost its `실행 중` section. It shows a fold `완료 1 ›`; the trash icon turns dark (there is now something to clear). Opened, the card reads: name and total time at the right (`3분 43초`), `워크플로우  완료됨`, `4개 에이전트  234k 토큰`, the description in grey, `단계` with `Analyze 3/3 ›` and `Merge 1/1 ›` (folded, grey dots), and no stop button.
- The usage banner above the composer changed during the run: `세션 사용량 한도 초과 임박  오후 11:50에 재설정  [업그레이드]  ×` in place of the weekly one.
- The final summary came without a second approval; the follow-up text is a long reply with headings, a numbered `Ranked risks` list and file names as links.

## Round 17: Forge's Preview web view checked in the real window (2026-10-16, Plan 44)

Forge itself (harness fixture, a `python3 -m http.server` on `localhost` with an index and a second page), not the desktop app. Compared against rounds 5, 11 and 12 of this file.

- `localhost:8130` loads inside the Preview panel; a link inside the page changes the address bar and the tab name (the page's title); back works and enables forward; typing and scrolling work in the page.
- A page that Forge reports as `http://host/` while the person typed `http://host` is one entry in the history (fixed during the check; before it, back was enabled on the first page).
- The panel expanded, the window resized to 1000 x 640, and the floating dock (Preview beside Files) all keep the page exactly on the panel body.
- The search palette hides the page and is fully visible over the Preview column; Escape brings the page back. After a click inside the page, ⌘K opens the palette and the next typing goes to the palette's field (checked with `key code 40 using {command down}`).
- A server that is off (`localhost:9`): the panel shows "이 주소에 연결할 수 없습니다.", the address and `다시 시도`; typing a good address afterwards replaces it. `javascript:alert(1)` is refused with the "Enter a web address" notice and nothing loads.
- A `target=_blank` link opens a new Preview tab.
- Switching to another session hides the page and the other session shows no panel; coming back shows the same tabs and pages. After quitting, no `forge-app` process is left.
- Paths clicked in the chat: `page.html`, `doc.pdf` and `pic.svg` open as new Preview tabs named by the file (`file://` address in the bar); `data.csv` opens in the Files panel; `pic.png` opens the lightbox.
- Differences from the desktop app (rounds 11 and 12): the desktop app's PDF view has a dark toolbar (page box, zoom, rotate, print) and a thumbnail strip; Forge shows WebKit's own PDF view (no toolbar). The desktop app's bar shows the plain path for a local file; Forge shows the `file://` address. The first-run cookie modal, the layout icon, the element picker and the panel `⋮` are not built.
- Harness notes: `osascript keystroke "k" using command down` reaches the page with Option set (alt true), so Forge's shortcuts ignore it; use `key code 40 using {command down}`. `keystroke` of a long address can drop letters (the field is controlled); type one letter at a time with a short delay. When reading a capture that was resized to a different aspect, convert x and y with their own factors before `click.py`.
- Not checked: Windows (WebView2), dragging the panel's left edge by hand (the window resize and expand were), closing a tab and a session with several web views in the real window (unit tests cover the calls).

## Round 18: workflow tokens, the finished fold and a stopped run (2026-10-17, OTH-10)

The desktop app, session `obs-a dynamic workflow` in `/private/tmp/forge-observe`, mode 수동. Two small workflows were run with the user's go-ahead (obs-a: Analyze 2 agents, Merge 1 agent; obs-b: Analyze 2 agents). Compared with the agents' own `agent-*.jsonl` files under `subagents/workflows/wf_*`.

**Tokens**
- The running card shows `2개 에이전트  99.1k 토큰` and the number did not move while both agents waited for permission (40 s and 54 s). The finished card showed `3개 에이전트  157.4k 토큰`.
- The number matches the sum, over the agents, of the token total of each agent's last assistant message (`input + cache_creation + cache_read + output`, the size of its last call): obs-a 157,477 at the end; 99,244 when each agent had made one call. It is not the sum of all calls (417,957 for obs-a). Per agent that is about 49.6k each in the first minute.
- The chat card never shows tokens (`워크플로우  2개 에이전트  26초` and the dots only).

**Dots and permission**
- An agent waited for permission (a `find … -exec` card showed in the chat) while the dots, in both the chat card and the panel, stayed blue and the step name stayed plain. Seen 40 s and 54 s in obs-a and 54 s in obs-b. Amber was not seen in this round (round 16 saw it with three agents, after the first card was answered). Not explained.
- The dots of a step that has not started are one pale dot; the `Merge 0/1` row and `Analyze` rows read `모델 / 시간` with one row per agent, the same elapsed time on every row.

**The finished fold**
- When a workflow ends the panel shows only `완료 1 ›` (the `실행 중` section is gone). Opened (`완료 1 ⌄`), the card reads: name and total time at the right (`1분 17초`), `워크플로우  완료됨`, `3개 에이전트  157.4k 토큰`, the description in grey, `단계` and the steps `Analyze 2/2 ›`, `Merge 1/1 ›` with grey dots, all grey text, no stop button. Steps are folded.
- With a run going and one finished, the panel shows `실행 중` on top and `완료 1 ⌄` under it, open as it was left. The newest finished card comes first (`완료 2`: obs-b above obs-a).
- The finished cards stay on screen with their steps folded each; there is no fold per card.

**A stopped run**
- The stop button (a small square, top right of the running card) stops at once. The chat line becomes `워크플로 실행됨 obs-b 중지됨` (obs-a's was `워크플로 실행됨 obs-a`). The card moves to `완료 2`: `4초` at the right (not the 80 s it ran), `워크플로우  중지됨`, `2개 에이전트  99.1k 토큰` (the last number kept), the description, `Analyze 0/2` with two hollow dots. The permission card that was open disappeared.

**The trash icon**
- It clears every finished card at once: the panel shows a diamond icon and `백그라운드 작업이 여기에 표시됩니다`, and the trash icon turns grey (nothing to clear). The chat lines `워크플로 실행됨 …` stay.

Not seen: an agent that fails (and the card text for it), whether the finished list survives a switch to another session or a restart, the expanded panel's per-agent token column for a finished run, and whether Bash tasks join the fold.

### Round 18, part 2: a denied agent, a failed run, the finished list across sessions, a Bash task (2026-10-17)

Same session. obs-c: Analyze with three agents (a, b, c) and a script that throws `new Error('obs-c boom')` after them.

- **Amber again not seen.** Three agents, 31 s and 45 s into the run with agent requests open: all three dots blue (chat card and panel). So the two rounds that saw nothing amber (obs-a, obs-b, obs-c) had 2 or 3 agents; round 16's amber is not reproduced.
- **A denied agent turns red.** After `거부` on agent b's card its dot went red in the chat card and the panel, and the step's name (`Analyze`) turned dark red. Its time stopped (38 s while the others went on). It is not counted: the step read `0/3`, then `1/3` when agent a finished (a ✓ before its row, its dot grey). Tokens grew (148.9k, 153.1k, 157.5k).
- **A failed run.** After the agents ended the script threw. Chat line: `워크플로 실행됨 obs-c 실패` with `실패` in dark red. Panel card (in the finished fold): name and total time (`1분 11초`) at the right, `워크플로우  실패` (`실패` red), `3개 에이전트  157.5k 토큰`, the description in grey, then a pink box headed `오류` (red) with the error text (`Error: obs-c boom` and two `at …` lines), then `단계` with `Analyze 2/3 ›` and grey, red, grey dots. No stop button.
- **The finished list stays with its session.** Switching to another session and back kept the panel open, and obs-a's session showed its `완료 1 ⌄` as it was left (open). The other session, which had run a workflow long before, showed `완료 1 ›` (folded): the list is rebuilt from the session's own record, so it does not depend on the app having been open when the workflow ran.
- **A Bash background task joins the fold.** `sleep 12; echo done` (run in the background) ended and the fold read `완료 2`: a small card first (the newest on top): its description (`Sleep 12 seconds then print done`), then `Bash  완료됨  12초`. No tokens, no steps, no stop button, no name at the right.
- The chat showed no `워크플로 실행됨` line for the Bash task; only workflows have it.

Still not seen: the finished fold's expanded steps of a failed run (the `Analyze 2/3 ›` fold was not opened), and the per-agent token column of the expanded panel.

## Round 17 (2026-10-17): worktree, from the new-session screen and Settings

Desktop app, nothing sent. Looked for the way to start a session in a worktree (SET-08 needs it).

- **New-session screen, folder = a git repo (`forge`):** the row under the composer is `이 컴퓨터` (environment chip), the folder chip, and the permission mode (`자동`). **No branch chip and no worktree toggle** was visible. The environment menu: 로컬 (`이 컴퓨터` ⌥⌘1, recent folders ⌥⌘2…, `더 보기`), 클라우드, Remote Control, `새 환경` ⌥⌘0. The folder menu: search, `폴더 없음`, `최근` list, `폴더 열기…`. The `+` menu: 파일 또는 사진 추가 ⌘U, 폴더 추가, GitHub 이슈 가져오기 (greyed: "이 폴더에는 GitHub 원격 저장소가 없습니다"), 슬래시 명령어, 커넥터 >, 플러그인 >. The permission menu: 자동, 수동, 편집 자동 수락, 계획, 권한 무시 (greyed "Claude Code 설정에서 활성화").
- **Docs say** (code.claude.com/docs/en/desktop and /worktrees): select the **worktree** option next to the branch name when starting a session; default place `<project>/.claude/worktrees/<name>` on branch `worktree-<name>`; `Branch prefix` is prepended to the branch name; a folder inside the project is ignored (default used); new worktrees branch from the default branch (`origin/HEAD`); `.worktreeinclude` copies gitignored files; archive removes the worktree; project settings, hooks, skills and `.mcp.json` are read from the main checkout.
- **Settings > Claude Code** (the settings search for "워크트리" jumps to the row):
  - `비활성 세션 보관` — "일정 기간 활동이 없으면 로컬 세션을 자동으로 보관합니다. 실행 중이거나 백그라운드 작업이 있는 세션은 보관되지 않으며, 커밋되지 않은 변경사항이 있는 워크트리는 디스크에 유지됩니다." Menu (with a search field on top): 없음 (default), 1일, 2일, 7일, 14일, 30일.
  - `관련 없는 작업을 요청할 때` — menu, default `질문하기`.
  - `워크트리 위치` — "이 컴퓨터에서 격리된 코딩 세션에 사용할 Git 워크트리를 저장할 위치입니다. SSH 세션은 각 SSH 연결의 워크트리 폴더 설정을 사용합니다." Menu: `프로젝트 내부` with the path `.claude/worktrees` (checked) and `사용자 설정…`, which opens the system folder dialog ("열기", starts in Downloads); cancelled, the value stays.
- **Not seen:** where the worktree option sits in a running session; what a worktree session looks like in the sidebar and header; what `사용자 설정…` shows after a folder is picked.

### Round 17, part 2 (2026-10-17): the worktree switch and a worktree session

The user allowed one short message in a throwaway git repo (`/private/tmp/forge-observe-git`, made for this; one commit, `a.txt`). Added through the folder menu's `폴더 열기…` (system dialog, `⇧⌘G` types a path). The app first asked "이 워크스페이스를 신뢰하시겠습니까?" ("Claude Code가 이 폴더의 파일을 읽거나, 쓰거나, 실행할 수 있습니다. 이 작업 공간을 신뢰하는 경우에만 계속하세요." with the path, `취소` / `작업 공간 신뢰`); trusted.

- **Where the option is:** the **folder chip's menu** on the new-session screen. For a git folder it ends with two rows under the recent list and `폴더 열기…`: `브랜치  master >` (a submenu: search field `브랜치 검색…`, the local branches, the current one checked) and `워크트리` (a switch). For a folder that is not a git repo both rows are missing. The switch was **on** for both git folders tried (`forge`, the new repo), so a worktree is the default for a git folder.
- **Sent** "Reply with just the word ok. Do not use any tools." in mode `자동`, branch `master`, switch on. The session started at once and replied "ok"; the title became "Acknowledge instruction" (auto).
- **Session screen:** a grey line above the first message: `시작 위치: 이 컴퓨터 · forge-observe-git · 환경으로 저장`. The right panel has a `환경` card: `이 컴퓨터` (with `저장`), the folder (`forge-observe-git · /private/tmp`), a branch row `claude/acknowl…ion-75511b` with a tag `워크트리`, and a `Remote Control` switch; then `컨텍스트 72.2k` and `커넥터` cards. The sidebar row is plain (no worktree mark) under the project group of the main repo (`forge-observe-git`).
- **On disk:**
  - the worktree is `<repo>/.claude/worktrees/acknowledge-instruction-75511b` on a new branch `claude/acknowledge-instruction-75511b` made from `master` (`git worktree list` shows it); the name is a slug of the title plus six hex digits. `.gitignore` was not touched.
  - the Claude transcript is under `~/.claude/projects/-private-tmp-forge-observe-git--claude-worktrees-acknowledge-instruction-75511b/`, and every line's `cwd` is the worktree path (the main repo's project folder has none).
  - the desktop app keeps `~/Library/Application Support/Claude/git-worktrees.json` (`worktrees.<name>`: `name`, `path`, `leasedBy` = the app's session id, `baseRepo`, `branch`, `sourceBranch`, `createdAt`, `originTopLevel`, `anchors` with `trustedAt`, `autoTrusted`) and its session file (`claude-code-sessions/.../local_<id>.json`) has `cwd` = the worktree, `originCwd` = the main repo, `worktreePath`, `worktreeName`, `branch`, `sourceBranch`, `spawnSeed.worktreeHookBased = false`.
- **Not done on purpose:** archive (the observe script forbids it) and turning the switch off. Not seen: what archive does to the worktree, what the row looks like while the repo has uncommitted changes, the switch off.

## Round 19: Changes panel with real changes (2026-10-17)

Same app and window, on the session in the worktree of `/private/tmp/forge-observe-git` (`a.txt` modified, `b.txt` new). Nothing was sent and nothing was staged or committed: there is no such control. Menus were opened and closed with Escape; the panel's mode was set back to `모든 변경사항`.

- **No stage, unstage or revert control anywhere**, and no commit control inside the Changes panel: not on a file row, not in the diff headers, not in the panel `⋮`, not in the file row menu, not in the `커밋되지 않은 변경사항` mode. The panel is a viewer. (Corrected in round 19b below: commit lives in the `환경` side panel.)
- **Header**: a branch button `master → claude/<branch> ⌄` (base, then the session's branch); at the right a search icon, the file list toggle, `⋮`, expand, close.
- **Branch button menu**: `모든 변경사항` (check, `master 대비`), `커밋되지 않은 변경사항`, a rule, `비교 대상 <base> ›` (submenu, not opened). Choosing the second mode replaces the button text with `커밋되지 않은 변경사항 ⌄`.
- **Body**: one stacked diff for every file, each with a header (chevron, file icon, name, `+N -M`) that folds it; line numbers, `-` rows in red and `+` rows in green with the changed words darker; a fold row `더 많은 줄 보기` for the unchanged lines after the hunk. The file list is a column at the RIGHT (about 25% wide), name and `+N -M` per row.
- **Panel `⋮`**: `파일 표시` (check, ⇧⌘Y), `파일 목록 위치` (`자동 ›`), `폴더별 파일 그룹화` (check), `테스트, 빌드, 생성된 파일 분리`; `모든 파일 접기`, `모든 파일 펼치기`; `나란히`, `자동 줄 바꿈` (check), `변경된 단어 강조` (check), `공백 변경 숨기기`; `새로고침`.
- **File row right click**: `컨텍스트로 첨부`, `복사 ›`, `파일 열기` (⌘↵), `다음에서 열기 ›`, `Finder에서 보기`.
- **Not seen**: the search icon, the `비교 대상` and `복사` submenus, a click on a file row, the file list position submenu, a commit or PR button with a clean tree or with commits ahead.

### Round 19b: the `변경 사항` card of the side panel (2026-10-17)

The user allowed stage and commit in the throwaway repo. The first toolbar icon opens the side panel (cards `환경`, `컨텍스트`, `커넥터`); with a dirty tree it also shows a card `변경 사항 ⌄`: a Changes icon, `+5 -1` (counts of all uncommitted changes) and a button `변경 사항 커밋`. The chevron folds the card (folded: only the title and a grip icon). With a clean tree the card is gone.

- **The button sends a normal chat message**: `Commit the working tree changes with a sensible message.` (English in a Korean UI). Claude then ran 3 commands (`실행된 명령 3개`) and answered `I committed everything as d5bfbad on claude/...`. Result on disk: one commit with every change, tracked and untracked, message written by Claude. The button turns grey while the turn runs; the composer shows the stop button.
- So the desktop app has NO git code for commit: stage and commit are a prompt. Nothing was seen for stage, unstage or revert; no revert exists.
- This cost one real message of the user's account in the throwaway repo (allowed as a commit; the message itself was not asked about separately).
- Not seen: the `+5 -1` or the icon as a click target, a PR button (clean tree with commits ahead), the `환경` card's rows as menus, the card with a long file list.

### Round 19c: the Changes panel with a commit on the branch (2026-10-17)

After the commit of round 19b, with two more changed files (`a.txt`, `b.txt` modified and `src/d.txt`, `src/e.txt` new).

- **Branch button menu has a third row**: `커밋  1 ›` (the number of commits of the branch beyond the base; submenu not opened). The first row is `모든 변경사항 ✓ master 대비`, the second `커밋되지 않은 변경사항`.
- **`비교 대상 <base> ›` submenu**: a field `브랜치 검색` and a list of branches, the chosen one ticked, the default with the grey badge `기본값`.
- **File list is a tree** (`폴더별 그룹화` on): a folder row `src` with a chevron and its files indented; loose files after it. A diff header shows the folder as grey text after the name (`e.txt src`) when the file is in a folder.
- **A `커밋 N ⌄` section sits at the bottom of the right column** (separated by a rule): rows `모든 변경사항`, `커밋되지 않은 변경사항` (the same two modes as the menu) and one row per commit: the subject on one line (cut with `…`), under it `d5bfbad · <author> · 4분 전` in grey. The chosen row has a grey fill.
- **Choosing a commit row** shows that commit's own diff and file list, and the header becomes `d5bfbad Update a.txt and add b.txt and src/c.txt ⌄` (the menu button now names the commit; the base arrow is gone).
- **Search icon**: a popover under the header with a field `변경된 파일 검색` and the changed files (icon, name, grey folder, `+N -M`); click on a row is the likely jump (not clicked).
- Not seen: `복사 ›` and `파일 목록 위치 ›` submenus, a click on a file row or a diff header, the `커밋 1 ›` submenu of the menu, the card with a clean tree and commits ahead.

### Round 19d: the desktop app beside Forge's built Changes panel (2026-10-17, Plan 48)

The desktop app's Changes panel was opened (and closed again) on the worktree session of `/private/tmp/forge-observe-git` (clean tree, one commit `d5bfbad` on the branch: `a.txt` changed, `b.txt` and `src/c.txt` new); capture `/private/tmp/p48-desktop-02.png`. Nothing was sent. Forge was looked at in the harness on a fixture repository with the same shape (`main`, `claude/x` with two commits, edits, untracked, binary and large files). What differs:

- **Header**: the desktop app has no title; the branch button is the first thing in the header. Forge keeps the panel title `변경 사항` and puts the branch button beside it.
- **Line numbers**: the desktop app draws ONE number column (a removed line shows its old number, an added line its new number, a context line its new number). Forge draws two columns (old and new) in the unified view.
- **Changed lines** have a coloured bar at their left edge in the desktop app (green for added, red for removed); Forge has the row fill only.
- **Fold chevron**: the desktop app uses `⌄` / `›` outline chevrons; Forge uses `▾` / `▸`.
- **The `Commits` section with a clean tree**: the desktop app lists `모든 변경사항` and the commits, with no `커밋되지 않은 변경사항` row (it had one in round 19c, when the tree was dirty). Forge always shows the `커밋되지 않은 변경사항` row.
- **Search popover rows**: round 19c saw `+N -M` on each row of the desktop app's search list; Forge's rows show the name and the grey folder only.
- **Branch menu**: the desktop app has a third row `커밋 N ›` when the branch has commits; Forge leaves it out (spec: the right column's list has the same content).
- Same in both: stacked sections with sticky headers, the grey folder after the name, `+N -M` coloured counts, `더 많은 줄 보기` after the last hunk, the tree at the right with `커밋 N` at its foot and rows `subject` / `sha · author · age`.

### Round 20: the menus and the diff comment box that Plan 48 left out (2026-10-17, Plan 50)

Same session and worktree as round 19 (clean tree, one commit `d5bfbad`). Nothing was sent, staged or changed; the one view setting that was switched on (`테스트, 빌드, 생성된 파일 분리`) was switched off again. Captures are not committed.

- **Panel `⋮` > `파일 목록 위치 ›`**: the row shows the current value `자동` at the right; its submenu is `자동` (check), `왼쪽`, `오른쪽`.
- **Panel `⋮` > `테스트, 빌드, 생성된 파일 분리`**: a switch row; the menu stays open and the row gets a check. The repository had no test, build or generated file, so what the switch does to the list was NOT seen.
- **Branch button menu on a clean tree** (one commit): `모든 변경사항 ✓ master 대비`, `비교 대상 master ›`, `커밋 1 ›`. There is no `커밋되지 않은 변경사항` row on a clean tree.
- **`커밋 N ›` submenu** (opens beside the row): a field `커밋 검색` with the caret in it, then one row per commit: the subject cut with `…` at the left and the short sha in grey at the right (`d5bfbad`). Not clicked.
- **File row right click > `복사 ›`**: `절대 경로`, `상대 경로`, `파일 이름`.
- **A diff line shows a round blue `+` in the left margin while the pointer is over it.** Clicking it opens a comment box under that line (the rest of the diff moves down): a grey title `2번 줄`, the placeholder `Claude에게 변경할 내용을 알려주세요`, a round icon at the bottom right; blue border while editing. Escape closed it with nothing saved. Not seen: sending a comment, where it goes, a range, a comment on a removed line.

### Round 20b: `테스트, 빌드, 생성된 파일 분리` (2026-10-17, Plan 50)

Throwaway files (uncommitted, removed again with `git clean`) were put in the worktree of `/private/tmp/forge-observe-git`; the switch was on, then off again. Nothing was sent or committed.

- **With the switch on**, the right column shows the normal files first (folders grouped as usual), then two sections, each a row with a chevron, the name and a count at the right, a thin rule between: `테스트 파일 N` and `생성된 파일 N`. A section is COLLAPSED when it first appears. Opened, it holds its own folder tree (`src`, `tests`, … with files indented). The stacked diffs at the left follow the same order: normal files, then test files, then generated files. With the switch off the list is one tree, all mixed.
- **Test files (7 of 7 probes)**: a folder `tests/`, `spec/` or `__tests__/` (any file in it), a name `*.test.*` or `*.spec.*`, `*_test.go`, `test_*.py` (also at the repository root: `test_root.py`).
- **Generated files (9 of 9 probes)**: a folder `dist/` or `.next/`, a name `*.generated.*`, `*.min.js`, `*.js.map`, `*.lock` (`Cargo.lock`, `yarn.lock`), `package-lock.json`, `pnpm-lock.yaml`.
- **NOT separated** (stay in the normal list): `build/*`, `out/*`, `target/*` (a Rust file in `target/`), `bundle.d.ts`, `main.py`, `a.txt`.
- A folder row inside a section is named by its own folder (`src` holds `x.test.ts` under the test section while `src/c.txt` stays in the normal tree).
- **Not seen**: whether the collapsed state is remembered, whether a section's counts change with the search, a section with no match (presumably hidden).

## Round 20: split panes and the side chat (2026-10-19, OBS-06, OTH-04)

Same desktop app, window 1200 x 800 pt, a Code-side session (`Acknowledge instruction`) open with its Changes panel. Only layout keys, menus and the × buttons were used. No message was sent, nothing was deleted or archived. The window was left as found (one pane, Changes panel open, no side chat).

**Opening a pane**
- `⌥+Click` on a sidebar row opens that session in a second pane at the right of the first. Both rows stay highlighted in the sidebar. The two panes share the width in half (about 460 pt each at 1200 pt with the sidebar shown).
- `⌃⌘\` opens a **new-session pane** beside the focused pane (to its right) and gives it focus: a blue border around the pane and the cursor in its box. `⌃⌘-` does the same below the focused pane. So the layout is a tree of side-by-side and stacked panes: with two sessions stacked on the left and a new-session pane at the right, `⌃⌘-` on the right pane gave four panes (a column of two sessions, a column of two new-session panes).
- `⌘\` (listed as `창 닫기`) did nothing visible with the focus in a composer. The × in a pane header closes it.

**What a pane looks like**
- A pane is a full session view: its own header, transcript, composer, mode, model and effort, and **its own panels** (Changes in the left pane, `백그라운드 작업` in the right one at the same time). A pane's panels sit inside that pane, as a rounded card over its right part.
- Header of a narrow pane (side by side): the title (cut with `...`, with its chevron), `⋮`, and `×`. The folder tag and the toolbar icons are gone. A wide pane (stacked, full width) keeps the toolbar: a first icon (session rail, `세션 레일 표시`, only in some panes), terminal, changes, browser, files, `⋮`, `×`; the title is complete.
- A new-session pane has the title `새 세션`, a link `새로운 기능` (blue, at the right: this is where PG-07 lives) and `×`. Under it: a list of recent folders (`forge-observe`, `mywork`), `환경 선택 ⌄`, a row `Claude Code CLI 세션 N개 가져오기`, then the composer with the chips `이 컴퓨터` and the folder, the mode and the model. No greeting and no usage card here. The pixel creature stands on the box.
- Closing a pane gives its space to the neighbour; no pane keeps the focus border afterwards. Closing the new-session pane brought back the earlier layout (two sessions stacked).

**The `⋮` menu of a pane**
- Side by side: `세션 레일 표시`, `터미널 ⌘J`, `변경 사항 ⇧⌘D`, `Browser ⇧⌘B`, `파일 ⇧⌘F`, `백그라운드 작업` (checked when open), `iOS 시뮬레이터`; a divider; the sleep switch (`컴퓨터 절전 모드 방지 / 이 세션에서만`); a divider; `분할 화면을 왼쪽으로 이동`, `분할 보기 아래로 이동`, `분할 보기 닫기`.
- Stacked: only the items that do not fit as icons (`백그라운드 작업`, `iOS 시뮬레이터`), the sleep switch, then `분할 보기 위로 이동`, `분할 보기를 오른쪽으로 이동`, `분할 보기 닫기`. So the menu moves the pane (swap with the neighbour, or turn the split from side by side to stacked) and the labels follow the orientation. Choosing `분할 보기 아래로 이동` on the right pane stacked the panes: the first session on top, the second below.

**Resizing**
- The divider between two columns drags (a thin grab bar at the middle of the line). Dragging it left by about 140 pt widened the right pane; the left pane shrank to about 320 pt, its title was cut to one letter and its panels kept their width and covered the chat. Not tried: the horizontal divider, a minimum width, double click to reset.

**Side chat (`⌘;`)**
- `⌘;` opens a panel at the right (in the panel column, below the Changes panel, which shrank): a tab `사이드 채팅 ×`, a `+` for more tabs, expand and close at the right. Empty: `아직 메시지가 없습니다` in the middle and a box `이 세션에서 답장하기...` with a return icon. `⌘;` again closes it. Not seen: what the side chat sends (the box says it replies in this session), what `+` does, a side chat with messages.

**Trash (OTH-03)**
- The `더보기` item of the sidebar opens a one-row menu: `사이드바 편집...`. No trash or restore item was seen anywhere; deleting was not tried (it is destructive). Nothing suggests the desktop app has a trash.

**A plain click on a sidebar row while split (checked a second time, 2026-10-19)**
- Two panes side by side (`Acknowledge instruction` | `obs-a dynamic workflow`, the focus in the right one), then a plain click on a third row (`Parallel agent file analysis workflow`): the session opened as a **third pane stacked under the left pane**; the other two panes stayed. So a plain click does not replace a pane's session while the screen is split; it adds a pane (here under the first one). Closing it with its × gave back the two panes. The row was highlighted in the sidebar like the other open ones.

**Not seen**: `세션 레일 표시`, the contents of `사이드바 편집...`, the drag of a session row into a pane, keys inside a pane (focus moves between panes), how many panes the app allows, whether the layout is restored on restart.

**Pane limit (checked 2026-10-19, OTH-04)**
- `⌃⌘\` pressed three more times with the New-session pane already focused did nothing: no second New-session pane (same rule as Forge: a target is not shown twice).
- ⌥+click on rows built a **2×2 grid** (Acknowledge top left, New session top right, two sessions below): four panes. Each pane has its own header with `⋮` and `×`.
- With four panes, further ⌥+clicks on other rows (three tried) did not make a fifth pane. Each replaced the session in the **bottom-right** pane (the last one opened): `Report writing…` → `Add scheduled…` → `obs-a…` → `Parallel agent…`. So the limit is **four panes**; at the limit ⌥+click replaces the last-opened pane. Not settled: whether it replaces the focused pane (a click into the top-left pane before the ⌥+click did not change which pane was replaced; that click may have been swallowed).
- The layout was a grid (a row of two over a row of two), not a column of tall panes. All four panes were closed with their × and the window ended as found (one pane, Changes panel open). No message was sent.

## Round 21: environments (2026-10-19, OTH-11)

Desktop app 2.42304.2, nothing sent, nothing saved. Looked at the new-session environment chip, the `새 환경` dialog (all four tabs), the edit dialog of a saved environment, the `환경` card of a session, and Settings. The observe script's `bring_to_front` hung on `osascript ... activate` once; it now uses `open -a` with a 10 s timeout.

**Environment chip menu** (new-session screen, the chip left of the folder chip; label is the environment name, `이 컴퓨터` by default):
- A search field `환경 검색…`, then a group `로컬`: `이 컴퓨터` (⌥⌘1, checked when current), the saved local environments (`forge-observe`, ⌥⌘2, then ⌥⌘3…), and `더 보기 N ›` (a submenu with the rest, here `mywork`). Below: `클라우드 ›`, `Remote Control ›`, then `+ 새 환경` (⌥⌘0).
- A saved environment row shows on hover a gear (edit) and a star (pin: the pinned ones get the ⌥⌘ numbers and stay out of `더 보기`). Clicking the row picks it; Return on a focused row also picks it.
- `클라우드 ›` submenu: `기본값`, `클라우드 환경 추가…`. `Remote Control ›` submenu is a text card: `Remote Control 설정`, "여기서 코딩하려면 내 컴퓨터에서 `claude rc`를 실행하세요.", and `여기에서 Remote Control 켜기`, "이 컴퓨터에서 Remote Control 기능이 꺼져 있으므로 휴대폰이나 claude.ai로 이 컴퓨터에서 세션을 시작할 수 없습니다."
- **Picking a saved environment** replaced the chip text with its name, removed the folder chip (the environment holds the folders), switched the permission mode from `자동` to `수동` (the saved one's mode, or auto not allowed there), and added a row `자동 모드 켜기` under the recent-folders card. The recent-folders card marks the matching folder with a check.

**`새 환경` dialog** (title, `×`, tabs `로컬 | 클라우드 | SSH | Remote Control`, `취소` + a black action button):
- `로컬`: `이름` (prefilled with the current folder name, selected), `폴더 (선택 사항)` with `+ 추가`; each folder is a row: folder icon, name, branch picker (`master ⌄`, mono) and a `워크트리` checkbox (checked) for a git folder, or the grey text `Git 저장소가 아님` for a non-git folder. A card `환경 변수` / `이 환경에는 없음` (or the count) with `›`. Buttons `취소`, `환경 만들기`.
- `환경 변수` page (back arrow, `×`): "<name> 내에서 시작된 세션에 대해 설정됨.", label `환경 변수`, a mono text area with a placeholder example (`NODE_ENV=production`, `GIT_AUTHOR_NAME=Your Name`, `# Multiline values - wrap in quotes`, `CONFIG="key1=val1` / `key2=val2"`), the note "`.env 형식`으로 제공됩니다. 이 값들은 안전하게 저장되어 Claude 세션에 전달됩니다.", button `완료`.
- `클라우드`: `이름` (placeholder `내 앱`), `네트워크 액세스` (a select, default `제한됨`), a card `허용된 도메인` with a mono text area (placeholder `example.com` / `anthropic.com`), the note "도메인 목록입니다(URL 아님). 와일드카드에는 * 기호를 사용하세요. 더 알아보기 · 아티팩트 도메인 추가", a checkbox `패키지 관리자 허용` (checked); more below (the dialog scrolls). Buttons `취소`, `환경 추가` (grey until a name is typed).
- `SSH`: `이름` (placeholder `내 앱`), `호스트` (a select `호스트 선택`), `폴더` with `+ 추가` (disabled) and a dashed box "먼저 호스트를 선택하세요.", the `환경 변수` card, buttons `취소`, `환경 만들기` (grey).
- `Remote Control`: a card with an icon, `remote control 설정`, "여기서 코딩하려면 내 컴퓨터에서 `claude rc`를 실행하세요.", the same off notice with a link `Remote Control 설정 열기`, and a button `더 알아보기 ↗`; only `취소`.

**Edit dialog** (gear on a saved row): title `환경 편집`, `이름`, `폴더 (선택 사항)` with `+ 추가`, the folder rows (a non-git folder shows `Git 저장소가 아님`), the `환경 변수` card; footer: a red text button `환경 삭제` at the left, `취소` and `저장` at the right.

**Session screen:**
- The grey line above the first message: `시작 위치: 이 컴퓨터 · forge-observe-git · 환경으로 저장` (the last part is an underlined link). It opens the same `새 환경` dialog as the card's `저장`: name prefilled and selected, the folder with branch `master` and the `워크트리` checkbox checked, `환경 변수`.
- The side panel (first icon of the header toolbar) has the card `환경 ⌄` (the chevron folds it) with rows: `이 컴퓨터` + a link `저장` at the right (hover tooltip: `로컬 세션` / the computer's host name; the row is not a menu), the folder `forge-observe-git · /private/tmp` (menu: `Finder에서 보기`, line, `경로 복사`, `폴더 추가…`, line, `터미널에서 열기`), the branch `claude/acknowl…ion-75511b` + tag `워크트리` (menu: `브랜치 이름 복사`, line, `터미널에서 열기`), and `Remote Control` with a switch (off). Then the cards `컨텍스트 ›` (8% and a small bar), `커넥터 ›` (`재연결까지 2`).
- Header `⋮` menu in a session: `사이드 채팅` ⌘;, `iOS 시뮬레이터`, a switch `컴퓨터 절전 모드 방지` / `이 세션에서만`.

**Settings:** the search finds nothing for `환경`, `environment` or `변수`. `SSH` finds `Claude Code › SSH 세션`; `Remote` finds `Claude Code › 새 세션을 Remote Control에 연결`. The Claude Code page (after `일반`, `코드 모양`, `화면 모드`) has a section **Remote Control**: switches `새 세션을 Remote Control에 연결` ("이 컴퓨터에서 시작한 세션은 자동으로 Remote Control에 연결되므로 터미널이나 claude.ai/code에서 계속 이어갈 수 있습니다."), `휴대폰 및 claude.ai에서 여기에 세션을 시작할 수 있도록 허용` (sends the computer name and each folder's name, path, branch and repo URL to Anthropic), `Remote Control 사용을 위해 이 컴퓨터를 절전 모드로 전환하지 않음` (tag `추천`), and `폴더 (15개)` ("Claude Code를 가장 많이 사용하는 6개 폴더를 표시…") with a button `폴더 추가…` and `15개 중 5개 표시됨 · 폴더 표시`. All three switches were off. The `SSH 세션` row did not scroll into view (the jump landed on the same place); not seen.

**Sidebar filter** (round 3, above): `환경` submenu with `모든 환경`, `로컬`, `클라우드`, `원격 제어`, `SSH`, `Slack`.

**Not seen:** the `SSH 세션` settings row; the cloud tab below `패키지 관리자 허용`; a saved environment's session screen (the `환경` card with a saved environment's name); what the star does exactly; `환경 만들기` and `저장` results (not pressed); the `기본값` cloud entry; `원격 제어` and `Slack` filter rows with real sessions.

**Round 21, part 2 (2026-10-19): folders in an environment**
- `+ 추가` in the `새 환경` dialog opens a popover with a search field, the known folders (recent list, current folder's worktrees included) and `선택…` (the system folder dialog). Picking `mywork` added a second row under the first: folder icon, name, and `Git 저장소가 아님` (a git folder shows `master ⌄` and the `워크트리` checkbox). On hover a row shows `×` at its right end (remove). No primary marker, no reorder handle, no limit seen. The dialog grows; `환경 변수` and the buttons stay under the rows. Cancelled; nothing saved.
- The `이 컴퓨터` row has only a star on hover (no gear): the docs' "hover Local, click the gear for the local environment editor" does not match this version. The docs say env variables saved there "apply to every local session and preview server" and are "stored encrypted on your machine"; the dialog says a saved environment's variables apply to sessions started in that environment.
- On disk the desktop app keeps saved environments in `~/Library/Application Support/Claude/ccd-session-presets.json`: `byAccount.<account id>.presets[]` with `id`, `name`, `createdAt`, `lastUsedAt`, `target: { kind: "local", cwd, wslDistro? }` (one `cwd`; `ssh` has `sshConfigId`), plus `builtIns` and `seededFromDraft`. Limits in the code: 200 presets, 512 KiB. The two presets there were seeded from the folders of past sessions. This file has no multi-folder or env-variable fields, so the new dialog's folders and variables are stored somewhere else or by a newer format; the UI itself is served from the web, not from `app.asar`, so the code cannot be read.
- Not seen: how a session started in a two-folder environment uses the second folder (extra directory, or second working copy), and where the variables are kept.

**Round 21, part 3 (2026-10-19): a two-folder environment, with one real message** (the user allowed it; two temp folders `/private/tmp/forge-obs-multi-a` and `-b`, both removed afterwards, the environment `obs-multi` deleted; its session `Acknowledge instruction` was left, under the group `forge-obs-multi-a`)
- **Making it:** the name field is prefilled from the current folder (a click is needed before typing); removing the prefilled folder shows a dashed empty box `세션을 시작할 폴더를 선택하세요.`; `선택…` opens the system dialog; folders without a git repo show a `워크트리` checkbox here (checked by default for the first, unchecked for the next) while a later edit shows `Git 저장소가 아님` for them. Closing a dirty dialog asks `이 환경을 만드시겠습니까?` with `삭제` (red), `계속 편집`, `환경 만들기`. `환경 만들기` asks `이 워크스페이스를 신뢰하시겠습니까?` once **per folder** (path shown, `취소` / `작업 공간 신뢰`), and only then closes.
- **Picking it:** the new-session list card gets the environment on top with a check; the chip becomes `obs-multi`, the folder chip is gone, the mode stays `자동` (so the earlier `수동` switch was something else). A used environment moves to ⌥⌘2 and the older one drops into `더 보기` (most recently used first; pinning not tried).
- **Session screen:** the `환경` card lists `obs-multi` (no `저장` link: it is already saved), then **every folder as its own row** (`forge-obs-multi-a · /private/tmp`, `forge-obs-multi-b`), then `Remote Control`. The grey `시작 위치` line was not there. The sidebar group is the first folder's name. The `환경 편집` of it shows the folders without worktree choices.
- **How a second folder is used (answer to the open question):** the session file `claude-code-sessions/.../local_<id>.json` has `cwd` and `originCwd` = the first folder, `requestedAdditionalFolders: [{path: <second>}]`, `sessionPermissionUpdates: [{type: "addDirectories", directories: [<second>], destination: "session"}]`, `startedFromEnvironmentId` and `envScopeId` = the environment's id. So the first folder is the working directory and the rest are added directories (the SDK's `additionalDirectories`), for this session only. The Claude transcript went under the first folder's project.
- **How it is saved** (`ccd-session-presets.json`): `{id, name, target: {kind: "local", cwd, additionalFolders: [..]}, settings: {modelId, effort, worktree}, createdAt, lastUsedAt}`. So a saved environment also keeps the model, the effort and one `worktree` flag (false here, set from the first folder's checkbox); the mode is not kept. No env variables were set, so where they are kept is still unseen.
- **Delete:** `환경 삭제` in the edit dialog asks `환경을 삭제하시겠습니까?` — "“obs-multi” 환경이 영구적으로 삭제됩니다. 이 환경에서 시작된 세션 1개는 유지하거나 함께 삭제할 수 있습니다." with `취소`, `환경 삭제` (keeps the sessions) and a red `세션 1개도 삭제`.

## Round 22 (2026-10-20): folders in an environment, and "no folder"

Desktop app, one real message (allowed by the user) with the folder menu on `폴더 없음`; everything made was removed again (the test environment deleted, the session archived).

- **Second and later git folders:** in `새 환경` and in `환경 편집` the first git folder shows `master ⌄` and a checked `워크트리` box; a second git folder (`forge`) shows only an unchecked `워크트리` box and no branch picker. The saved preset (`ccd-session-presets.json`) holds `target.additionalFolders` and one `settings.worktree` (true, the first folder's box); nothing per extra folder. The docs say a folder added to a running session is worked in "itself, with no worktree".
- **Delete confirmation without sessions:** `환경을 삭제하시겠습니까?` / "“my-app” 환경이 삭제됩니다. 이 환경에서 시작한 세션은 유지됩니다. 이 작업은 되돌릴 수 없습니다." with `취소` and a red `환경 삭제` (no third button when no session came from it).
- **`폴더 없음`:** the folder chip menu is: search `폴더 검색…`, `폴더 없음` (checked when none is chosen), `최근` with the recent folders, `폴더 열기…`. With none chosen the chip reads `선택…`. A session started that way runs in an empty folder the app makes: `~/Library/Application Support/Claude/scratch-workspaces/<account>/<org>/scratch-<YYYY-MM-DD>-<6 hex>` (`cwd` = `originCwd`; `envScopeId: "builtin_local"`; no `startedFromEnvironmentId`). The transcript went under `~/.claude/projects/-Users-james-Library-Application-Support-Claude-scratch-workspaces-…`. In the sidebar the session sits in a group named `이 컴퓨터` (with `+`). Its `환경` card shows `이 컴퓨터`, a row `폴더 선택…` (to choose a folder afterwards) and the Remote Control switch.

## Round 23 (2026-10-22): the `환경` card in a saved environment's session (stage B)

Desktop app 3.1623.1, an existing session in the saved environment `mywork` (nothing sent, nothing changed; dialog cancelled).

- **Card:** `환경 ⌄`, then the environment name `mywork` with a laptop icon and a **gear** at the right end (always visible, not only on hover), then the folder row `mywork · ~/Developer`, then `Remote Control` with a switch. No `저장` link (already saved).
- **Gear:** opens `환경 편집` (name, `폴더` with `+ 추가`, the folder row with `Git 저장소가 아님`, the `환경 변수` card `이 환경에는 없음`). In this copy the footer has only `취소` and `저장`: no `환경 삭제` (Round 21 saw it from the chip menu's gear).
- **Folder row menu:** `Finder에서 보기`, line, `경로 복사`, `폴더 변경…`, `폴더 추가…`, line, `터미널에서 열기`. In a saved-environment session `폴더 변경…` is there; Round 21 saw only `폴더 추가…` in a plain session.
- Not seen this round: the `저장` link result and the grey `시작 위치 … 환경으로 저장` line (see Round 21); what `폴더 변경…` does.

**Round 23, part 2 (2026-10-22): saving an environment from a session** (the user allowed one real message; folder `/private/tmp/forge-observe-git`, a non-git folder; the environment `obs-r23` was deleted afterwards, the session `Confirmation response` was left under the group `forge-observe-git`)
- **Before saving:** in a session started from a chosen folder with no environment, the grey line above the first message reads `시작 위치: 이 컴퓨터 · forge-observe-git · 환경으로 저장` and the `환경` card has `이 컴퓨터` with a `저장` link at the right. Both open the same `새 환경` dialog: the name is the folder name, selected; the folder row shows `Git 저장소가 아님` (a git folder would show branch and `워크트리`); `환경 변수` `이 환경에는 없음`; buttons `취소`, `환경 만들기`.
- **After `환경 만들기`** (name changed to `obs-r23`): no trust question (the folder was already trusted). The line becomes `시작 위치: 이 컴퓨터 · forge-observe-git · 저장된 환경: "obs-r23"` (plain text, no link) and the card row becomes `obs-r23` with a gear at the right (always visible); the folder row stays. The dialog closed at once; the session did not move in the sidebar.
- **What is stored** (`ccd-session-presets.json`): `{id, name, target: {kind: "local", cwd}, settings: {modelId: "claude-sonnet-5-5", effort: "medium"}, createdAt, lastUsedAt}`. So saving from a session takes the session's current model and effort; no `worktree` key for a non-git folder; no mode. The session file still has `startedFromEnvironmentId: null` and `envScopeId: null`: the line and the card are worked out from the folder, not from a stored link.
- **Proof of that:** after deleting the environment, reopening the session shows `이 컴퓨터` + `저장` and the `환경으로 저장` line again.
- **New-session screen:** the saved environment goes to the top of the list card, is checked at once, and the chip reads `obs-r23` (the folder chip disappears). The chip menu lists `이 컴퓨터` ⌥⌘1, `obs-r23` ⌥⌘2, `더 보기 2`, `클라우드`, `Remote Control`, `새 환경` ⌥⌘0. A click in the card list selects, it does not open a menu.
- **Chip menu row on hover** (needs a real mouse move first; a click alone only selects): gear, star, check, shortcut. The gear opens `환경 편집` with `환경 삭제` at the left and the folder label `폴더 (선택 사항)`. The session card's gear opens the same dialog without `환경 삭제` and with the label `폴더` only (folder required).
- **Delete confirmation:** `환경을 삭제하시겠습니까?` / "“obs-r23” 환경이 삭제됩니다. 이 환경에서 시작한 세션은 유지됩니다. 이 작업은 되돌릴 수 없습니다." with `취소` and a red `환경 삭제`; no session count, because the session was not linked.

**Round 24 (2026-10-22): `폴더 추가…` and `폴더 변경…` in the card**

Desktop app 3.1623.4 (CLI 2.1.308). Done on the leftover Round 23 session `Confirmation response` (a plain `이 컴퓨터` session, one message already sent, folder `/private/tmp/forge-observe-git`, not in a saved environment). Folders `/private/tmp/forge-obs-r24-a`, `-b` (empty, not git). No message sent; the folder was set back at the end and the temp folders were removed.

- **Folder row menu** (click the row): `Finder에서 보기`, line, `경로 복사`, `폴더 변경...`, `폴더 추가...`, line, `터미널에서 열기` (the dots are three periods in this menu; no `…` character). So a plain session with messages already has `폴더 변경...` (Round 21 saw only `폴더 추가…` in a no-folder-chosen draft/earlier build).
- **`폴더 추가...`:** opens the plain macOS folder dialog (no popover, no recents), title `세션에 폴더 추가`, buttons `새로운 폴더`, `취소`, `열기`. After `열기`:
  - a new row `forge-obs-r24-a` (folder icon, name only, no path part) appears **under** the first row, above `Remote Control`; the first row keeps its name and path.
  - the composer gets a draft attachment card (the path `/private/tmp/forge-obs-r24-a` and a tag `폴더`), not sent: it is a chip the user can send with the next message. It stayed after the later changes.
  - **no trust question**, no toast, no line change. The grey `시작 위치` line stays on the first folder.
  - disk: only the session file `local_<id>.json` changed: `sessionPermissionUpdates: [{type: "addDirectories", directories: ["/private/tmp/forge-obs-r24-a"], destination: "session"}]`. `cwd`, `originCwd`, `startedFromEnvironmentId` untouched; `requestedAdditionalFolders` was NOT written (Round 21 saw it only for environment starts). `ccd-session-presets.json` unchanged (the environment is not touched, only the session).
- **Added (non-first) row menu:** only `Finder에서 보기`, line, `경로 복사`, line, `터미널에서 열기`. No `폴더 변경...`, no `폴더 추가...`, no remove item. The first row keeps `폴더 변경...` and `폴더 추가...` (also after a change, with an added row present).
- **`폴더 변경...`:** opens the system folder dialog, title `작업 디렉터리 변경`. After picking `forge-obs-r24-b` (an untrusted folder):
  - first the trust question `이 워크스페이스를 신뢰하시겠습니까?` (path in mono, link `보안 가이드`, buttons `취소` / `작업 공간 신뢰`), shown before anything changes. (Trusted folders: not seen without it; the set-back to the trusted `forge-observe-git` showed none.)
  - then a toast at the bottom right `세션이 /private/tmp/forge-obs-r24-b(으)로 이동되었습니다` (info icon, `×`).
  - the first row becomes `forge-obs-r24-b · /private/tmp`; the grey line becomes `시작 위치: 이 컴퓨터 · forge-obs-r24-b · 환경으로 저장`; the sidebar session moves to a new group `forge-obs-r24-b` (and back to `forge-observe-git` after setting it back). The added row `forge-obs-r24-a` stays; the session stays selected; no confirmation dialog; the chat history stays.
  - disk: `cwd` and `originCwd` become the new folder; `gitAnchors` (with `trustedAt`) is replaced by `gitAnchorsLookupOnly: true` and `gitAnchorsFolderRealpath`; `completedTurns` 1 to 2 and `lastActivityAt` bumped; `sessionPermissionUpdates` kept (the added folder stays). The Claude transcript is **moved** to `~/.claude/projects/-private-tmp-forge-obs-r24-b/<id>.jsonl` (the old project dir keeps only `memory`), with a record `{type: "relocated", relocatedCwd}` and a hidden meta user line (`isMeta`, transcript only, no model call, no reply): "The session's working directory has changed to <new> (by the user). All tool calls and relative paths now resolve from <new>. Project settings (permission rules, hooks), project MCP servers, and project skills now come from <new>; its CLAUDE.md, if any, follows below. Environment variables set by the previous directory's settings stay in effect for this process ... " So the next turn tells the model about the change.
  - `ccd-session-presets.json` unchanged.
- **Not seen:** `폴더 변경...` in a session started from a saved environment (does the environment's saved `cwd` change? the presets file stayed the same for a plain session; a saved-environment session needs a sent message to exist, so not made here; the Round 23 `mywork` session was not touched); the item set in a session with no message sent (the new-session screen has no card and a session exists only after a send, so not reachable under the no-message rule); the added row after a reopen or restart; the composer `폴더` chip's `×` and what sending it does; adding a folder that is already there or a worktree folder.

**Round 25 (2026-10-22): `폴더 변경...` / `폴더 추가...` in a saved-environment session, and the unsent folder chip**

Desktop app 3.1623.4 (CLI 2.1.308). The user allowed several real messages; two `ok` messages were sent in all. Throwaway environment `obs-r25` (folders `/private/tmp/forge-obs-r25-a`, `-b`, `워크트리` unchecked for both; each folder asked `이 워크스페이스를 신뢰하시겠습니까?` once at `환경 만들기`), folders `-c` and `-d` for the changes. The environment was deleted with the plain `환경 삭제` and the temp folders removed; the session was left (title `Session`, sidebar group `forge-obs-r25-c`; its Claude transcript is `~/.claude/projects/-private-tmp-forge-obs-r25-c/8b1a56e1-f7b2-464b-8208-6bb9668a4b41.jsonl`, moved there by the folder change).

- **Session from the environment** (message `ok`, new-session screen with `obs-r25` picked): the `환경` card shows `obs-r25` with a gear, then a row per folder (`forge-obs-r25-a · /private/tmp`, `forge-obs-r25-b`), then `Remote Control`. No grey `시작 위치` line at all (as in Round 21). Session file: `cwd`/`originCwd` = a, `startedFromEnvironmentId` = `envScopeId` = the environment id, `requestedAdditionalFolders: [{path: b}]`, `sessionPermissionUpdates: [addDirectories b]`. Saved preset: `target {cwd: a, additionalFolders: [b]}`, `settings {modelId, effort, worktree: false}`. The title stays `Session` (the model's answer was a plain question, the sidebar row showed `차단됨` and the question).
- **(1) `폴더 변경...` on the first row, pick `-c`:** system dialog `작업 디렉터리 변경`; the untrusted folder first asks the trust question; then the toast `세션이 /private/tmp/forge-obs-r25-c(으)로 이동되었습니다`. The card keeps the environment name `obs-r25` and its gear (no fall back to `이 컴퓨터` + `저장`); the first row becomes `forge-obs-r25-c · /private/tmp`; the extra row `forge-obs-r25-b` stays; the sidebar session moves to a new group `forge-obs-r25-c`. Session file: `cwd`/`originCwd` = c; `startedFromEnvironmentId`, `envScopeId`, `requestedAdditionalFolders` and `sessionPermissionUpdates` unchanged. **`ccd-session-presets.json` is byte-identical** (the environment's saved folder stays a): the change is per session.
- **(2) `폴더 추가...` on the first row, pick `-d`:** system folder dialog; no trust question here; a third row `forge-obs-r25-d` under `-b`; the card still shows `obs-r25`; the composer gets the draft card (path `/private/tmp/forge-obs-r25-d`, tag `폴더`). Session file: a second `sessionPermissionUpdates` entry `{addDirectories [d]}` appended; `requestedAdditionalFolders` still only b; presets file unchanged.
- **(3) New session of the same environment:** the new-session screen first showed `이 컴퓨터` + the folder chip `forge-obs-r25-a` (the last used folder, not the environment). After picking `obs-r25` in the list card and opening the chip menu gear, `환경 편집` shows the original folders `forge-obs-r25-a` and `-b` (both `Git 저장소가 아님`), footer `환경 삭제`, `취소`, `저장`. So the folder change and the added folder were per session only.
- **B (no message yet):** no session exists before the first send. The new-session screen (⌘N) has only the list card (environments, `환경 선택`, a card for a waiting session `막힘 Session 질문함`) and the bottom chips (`이 컴퓨터`/`obs-r25`, folder chip or none, `자동`). No `환경` card, no folder rows, so no `폴더 변경...` / `폴더 추가...` there; with an environment picked the folder chip disappears.
- **C (close and reopen):** after `폴더 변경...` and `폴더 추가...`, a click on another session and back showed the card unchanged (first row c, extra rows b and d, `obs-r25`); the unsent `폴더` draft card was still in the composer. After deleting the environment and reopening the session, the card shows `이 컴퓨터` + `저장`, the grey line `시작 위치: 이 컴퓨터 · forge-obs-r25-c · 환경으로 저장` appears, the rows c, b, d stay (the extra folders come from the session, not from the environment).
- **What sending the folder chip does** (message `ok` with the chip): the user bubble shows a `폴더` card `forge-obs-r25-d` plus `ok`; the transcript user line is `@"/private/tmp/forge-obs-r25-d/"` newline `ok`, plus an `attachment` of type `directory` (path, `displayPath ../forge-obs-r25-d`) that makes the app run `ls` on it for the model (the model then listed the empty folder in a Bash call: `Listing files in the added directory`). The next `environment` snapshot has `workingDirectory` c and `additionalWorkingDirectories: [b, d]`. So the added directory was already registered by `폴더 추가...` (`addDirectories`); the chip is only an `@` mention of it that makes the model look at it.
- **Folder change in the transcript:** a `relocated` record `{relocatedCwd: c}` and a queued meta user message "The session's working directory has changed to /private/tmp/forge-obs-r25-c (by the user)..." delivered with the next real message (not a model call by itself); the first message's environment block still names a (stale), the scratchpad dir name too.
- **Delete confirmation with a linked session:** `환경을 삭제하시겠습니까?` "“obs-r25” 환경이 영구적으로 삭제됩니다. 이 환경에서 시작된 세션 1개는 유지하거나 함께 삭제할 수 있습니다." with `취소`, `환경 삭제`, red `세션 1개도 삭제`.
- **Not seen:** `폴더 변경...` on a trusted folder in an environment session; a worktree-checked environment; whether a later new session from the environment uses a/b or the changed c (it showed a, b in the editor); restart of the app with the extra rows.

**Round 26 (2026-10-23): the `컨텍스트`, `커넥터`, `결과물` and `변경 사항` cards of the details panel**

Desktop app 3.6556.1. Two views: the old Round 25 sessions (folder deleted, so a dead session) and one new session in a throwaway git folder `/private/tmp/forge-obs-r26` (trusted at the dialog; one real message `Create a file b.txt containing the word hi…`; the session `File b.txt creation` and its worktree were left; the CLI wrote the project dirs `-private-tmp-forge-obs-r26` and `…--claude-worktrees-create-file-b-txt-6eeb0c`).

- **Card order** (top to bottom): `환경`, `컨텍스트`, `커넥터`, `결과물` (only once a file was made), `변경 사항` (only with changes). Each header has a `▼`/`▶` fold arrow; a folded card shows one line (`컨텍스트 ▶ 8% [switch]`, `커넥터 ▶`). The panel scrolls inside; the cards do not shrink.
- **Git folder default:** a session in a git folder starts in a worktree at once: the card gets a second row `claude/cre…txt-6eeb0c` + `워크트리` (branch icon) under the folder row. The model's answer said "in the worktree"; the changes panel header read `master → claude/create-file-b-txt-6eeb0c`.
- **`컨텍스트` before any message:** `컨텍스트 사용량을 확인하려면 메시지를 보내세요.` (no bar). A dead session with old messages shows `8% 사용됨`, `77k / 1M`, a thin bar and `메시지를 보내면 세부 내역을 확인할 수 있습니다.`
- **`컨텍스트` after a turn:** `8% 사용됨` and `78.1k / 1M`, a stacked bar, then rows `Messages 20.1k 2.0%`, `System tools 18k 1.8%`, `MCP tools 16.9k 1.7%`, `System prompt 10.3k 1.0%`, `Skills 8.6k 0.9%`, `MCP server instructions 4k 0.4%`, `Custom agents 241 0.0%`, `Autocompact buffer 33k 3.3%`, `Free space 888.9k 88.9%` (each with a colour square; the first four coloured, the rest greys), a gap, then deferred `MCP tools (deferred) 123.9k —` and `System tools (deferred) 24k —`, then two fold rows `MCP 도구 248` and `커스텀 에이전트 3`. Some names have a dotted underline (hover help not opened). The header has a drag handle (dots) at the right when expanded.
- **`커넥터` in a dead session** (folder gone): stays on a grey skeleton row for good. In the live session it lists, with a letter icon, `Context7 인증`, `M365 다시 연결` (both brown action text), `Figma`, then `더 보기`; `더 보기` adds `Playwright`, `Puppeteer`, `Google Drive` (its own coloured logo) and turns into `간략히 보기`.
- **Click a connector row** (Playwright): modal `MCP 서버` with the folder path, a `← 모든 서버` link and the server: `playwright`, `연결됨 · 도구 25개`, `Playwright 플러그인에서 제공됨`. `← 모든 서버` shows the table `서버` / `유형` / `상태`: `context7` (`웹`, tag `플러그인`, button `연결`), `m365` (`데스크톱`, `플러그인`, warning icon + `다시 연결`), `Google Drive` (`웹`, check), `figma` (`웹`, tag `사용자`, check), `playwright` (`데스크톱`, `플러그인`, check), `puppeteer` (`데스크톱`, `앱`, check); footer `커넥터 관리` (dark) and `완료`. Not clicked: `연결`, `다시 연결`, `인증`, `커넥터 관리`.
- **`결과물`:** `결과물 ▼` with a file row `b.txt` (document icon); a click opens the Files panel on that file (`b.txt` tab, the folder listing `a.txt`, `b.txt`).
- **`변경 사항`:** `변경 사항 ▼`, a row with the diff icon, green `+1`, red `−0` and a grey button `변경 사항 커밋` at the right (the same words as the chat's `변경 사항 커밋`; not clicked). A click on the row opens the Changes panel with the diff (`b.txt +1 −0`, `1 + hi`) and the file list at the right.
- **Chat side:** after the turn the model asked `Commit b.txt?` with buttons `변경 사항 커밋` / `Not now` (a question card), so the commit path is a chat prompt (Round 19/Plan 48).
- **Not seen:** the card order with several changed files, `결과물` with more than one file, the hover help on the underlined context rows, what the `컨텍스트` switch at the folded row does, the drag handle's use.

**Round 26, part 2 (2026-10-23): what part 1 did not see** (the same session `File b.txt creation`; a second real message: `Create c.txt … d.txt … change a.txt to say changed`; the model made two files and edited one)

- **`결과물` with several files:** one row per file, newest first: `a.txt` (the edited one), `d.txt`, `c.txt`, `b.txt` (so it lists edited files too, not only created ones). A hover shows a dark tooltip with the file name and its full path (`/private/tmp/forge-obs-r26/.claude/worktrees/create-file-b-txt-6eeb0c/b.txt`: the worktree's path). A click opens the Files panel on it (part 1).
- **`변경 사항` with several files:** still one row: a diff icon, `+4 −1` (all uncommitted changes of the folder together: b, c, d new lines and a's one changed line; not a row per file) and the grey `변경 사항 커밋` button. The chat's own summary line said `+3 −1` for that turn.
- **Context rows are links:** the names with a dotted underline are links; the others are plain. Hover tooltips: `MCP tools` → `커넥터 관리`; `Skills` → `Skills 관리`; `Custom agents` → `사용자 지정 에이전트 보기`; `MCP server instructions` carries a dotted underline too (its tooltip was not read cleanly; the one that showed was the previous row's). A click on `Skills` opens the **Settings window** on `사용자 지정 → Skills` (tabs `내 항목` / `탐색`, `내가 만듦 6`, `Anthropic 및 파트너 제공 22`).
- **Folded `컨텍스트`:** `컨텍스트 ▶`, then at the right `8%` and a small grey bar with a blue dot at its left end. Part 1 called it a switch; it is **not a switch**, it is a mini usage bar (the dot is the filled part).
- **`MCP 도구 248` expands:** a row per tool: `<server> · <tool name>` (cut with `…`) and its tokens (`Claude Docs · Mak… 152`, `Claude Docs · Cre… 232`, …); the chevron turns down. `커스텀 에이전트 3` was not opened (it is the same kind of row).
- **Drag handle:** an expanded card shows a grid of six dots at the right end of its header. Dragging it down moved `컨텍스트` below `커넥터`; dragging back restored it (done and undone in this round, so the user's panel order is as it was). So the cards can be reordered by hand; whether the order is kept after reopening was not checked.
- **Numbers moved after turn 2:** `82.6k / 1M`, `Messages 25.3k 2.5%`, `MCP server instructions 3.3k 0.3%`, `Free space 884.4k 88.4%`: the card follows each turn.

## Round 27 (2026-10-24): the same three tasks in the desktop app and in Forge

Method: one throwaway git folder (`/private/tmp/forge-observe/task-r27`, a small Python script with two failing tests), the same Korean prompt typed into the desktop app (3.6556, Sonnet 5.5, real account) and into Forge (harness build on `plan-55`, default model, real Claude). Both start in a worktree. The desktop app's side was driven with `scripts/observe/claude_app.py`, Forge's with `scripts/harness/`. Text only.

**Task 1: fix the failing tests, run the script, write a README (auto mode).** Result text was the same quality in both. Differences:

- Desktop: the title is generated in a few seconds (`pytest 실패 테스트 수정`); Forge keeps the first prompt cut with `...` although the transcript holds an `ai-title` line.
- Desktop opens the details panel by itself on the first send (환경 with a `워크트리` branch row and `Remote Control`, 컨텍스트 with `MCP tools`, `Skills`, `MCP server instructions`, `Custom agents`, 커넥터). Forge opens it only from the `≡` button and its 환경 card has no branch row; 컨텍스트 has fewer categories.
- Desktop folds the turn into `편집함 파일 1개, 실행됨 명령 1개, 생성됨 파일 1개 +25 −2` with a row per file (`편집됨 wordstat.py +2 −2`, `생성됨 README.md +23`). Forge's fold showed `파일 1개 생성, 명령 3개 실행 +23 −0` and left the edit out because the model edited with a `python -` script (the Changes panel did show `+25 −2`).
- Changes panel: desktop lists two files; Forge also listed two binary `__pycache__/*.pyc` entries with `표시할 텍스트 변경 사항이 없습니다`.
- The file card at the bottom of Forge's chat (`README.md +23 −0`) is a grey bar with centred text; the desktop shows a plain row.

**Task 2: plan mode, with questions** (`계획` mode; prompt: add `--top N` and stop words, plan it, ask me what I must decide).

- Question card: both have it. Desktop: `4개 중 1번째` pager with `< >`, radio rows with `(Recommended)`, `기타` as a radio row, `건너뛰기`, `다음 ⌘⏎` (last one `제출`). Forge: `1/4` badge, number keys that move on at once, an inline `기타` text field, `이전`, `건너뛰기`, `다음 ⌘↵`. After submitting, both fold the card into a question/answer list.
- Sidebar and chat while waiting: desktop shows a blue dot with `차단됨` and an orange `입력이 필요합니다` pill; Forge shows an orange `!` icon on the row.
- **The plan step is the big gap.** Desktop writes the plan file (`생성됨 wordstat-py-top-n-hashed-walrus.md +35`), opens a `계획` panel on the right with the plan rendered (a hint: `텍스트를 선택하여 Claude에게 댓글을 남기세요`; icons: open folder, copy, expand, close) and shows `Claude가 계획을 제안했습니다 · 계획 열기` with `거부`, `수정... Esc`, `수락 ⇧⌘⏎`, `수락 및 자동 모드 ⌘⏎ ▾`. Forge shows the generic permission card `Claude가 ExitPlanMode을(를) 사용하려고 합니다` with `거부 / 항상 허용 / 한 번 허용`; the plan text is nowhere in the window, so the person cannot read the plan before accepting.
- Forge: when the model starts a background Explore agent, the first turn ends; the UI shows a quiet composer and a `작업 1개 완료` pill, and the question card appears about 30 s later with no sign that work is going on.

**Task 3: manual mode: `sleep 25`, then write CHANGELOG.md, then run pytest.**

- `sleep 25` ran in the background in both and needed no permission. Desktop shows a row `Waiting 25 seconds 실시간 출력 보기 ▸` (later `Waited 25 seconds`) and keeps a todo card (checked / in progress / pending) above the composer. Forge showed `명령 2개 실행 (실패 1)` and a pill `백그라운드 작업 1개`; no todo card appeared (the model may not have used the todo tool; not compared).
- Write permission card: desktop `Claude가 쓰기 CHANGELOG.md하도록 허용하시겠습니까?`, the path once, a diff preview, `거부` and `항상 허용 ⇧⌘⏎ ▾` whose menu holds `한 번만 허용 ⌘⏎` and `허용하고 Auto 모드로 전환` (`이 세션의 나머지 작업을 자동으로 검토합니다`). Forge: `Claude가 CHANGELOG.md을(를) 생성하려고 합니다`, the same path shown four times, a diff, `거부 1 Esc`, `항상 허용 2`, `한 번 허용 3 ⌘↵ ▾`; the card fills most of the window. (Forge's ▾ menu was not opened.)
- Bash card: desktop `Claude가 실행 Run pytest하도록 허용하시겠습니까?` with the command; Forge `Claude가 Run pytest을(를) 실행하려고 합니다` with the command.
- End of turn: desktop shows an end card (a short English summary, the question `Fix the two failing wordstat tests?`, buttons `Fix them ⌘⏎` / `Not now`, a close X) and a row of message actions (copy, a branch/share icon, pin, read aloud, `지금`). Forge shows the answer, one copy icon, and the suggestion as grey text in the composer (`응, wordstat.py 고쳐줘`).

**Slash commands.** Typing `/` in the desktop opens a short list: `btw` (tooltip `세션에 추가하지 않고 간단한 질문하기`), `ultrareview`, `rewind`, `fork`, `subtask`, `mcp`, `plugin`, `feedback`, `model`, `effort`, `fast`, `plan`, `permissions`, and more. Forge's `/` lists everything the CLI reports, skills and plugin commands first (`/design`, `/dataviz`, `/code-review`, …), then `/mcp`, `/model`, `/compact`, `/heapdump`; it has no `/btw` or `/rewind`. Sending `/compact` in Forge: the transcript says `Compacted` (it worked), but the window showed only the `/compact` bubble and nothing after it.

**Other.** The harness log has a Dioxus warning: a Copy value created at `crates/forge-features/src/workspace.rs:2847` is used at `session_notices.rs:122` after its scope is gone.

**Not seen in this round:** image or file attach and `@` mentions, `⌘⏎` queueing and the stop button mid-turn, a restart with a blocked or running session, several sessions running at once, the dropdown of Forge's permission card, the desktop's behaviour when a background agent ends the turn.

## Round 28 (2026-10-24): office-worker task, attachments, mentions, queue, stop

Same method as round 27. Throwaway folder `/private/tmp/forge-observe/task-r28` (not a git folder): `매출.csv` (24 rows of sales), `회의록.txt` (meeting notes with owners, one task without an owner), `chart.png`. Desktop 3.6556, Sonnet 5.5; Forge harness build on `plan-55`, default model. Forge got a copy of the folder (`task-r28b`).

**Prompt (non-developer):** with the chart image attached: `Using the attached chart, 매출.csv and 회의록.txt, make a one-page 3rd-quarter sales summary for the team leader as an Excel file: totals by product and by month, one chart; also a table of to-dos per person from the minutes; ask me about anything with no owner.`

- **Non-git folder:** both skip the branch and worktree rows; the session starts in the folder itself. Forge's folder chip row is the same.
- **`+` menu:** same items (`파일 또는 사진 추가 ⌘U`, `폴더 추가`, `슬래시 명령어`, `커넥터 >`, `플러그인 >`). Desktop adds `GitHub 이슈 가져오기` (greyed: no remote).
- **Attached image:** desktop shows a large thumbnail inside the composer and above the user bubble after sending. Forge shows a small chip `chart.png ×`, and under the bubble after sending.
- **`@` mention (gap):** desktop's list holds other sessions (title, folder, branch), files of the folder (csv and txt icons) and open windows of other apps. Forge shows nothing after `@`.
- **Queue (gap):** desktop while a turn runs: composer hint `⏎ 키로 보내기, ⌘⏎ 키로 대기열에 추가하여 나중에 보내기`; `⌘⏎` puts the text in a card `메시지 1 개 대기 중 …` with three icons (send now, edit, remove) and sends it when the turn ends. Forge: `⌘↵` puts the text in the chat at once as a normal user bubble; there is no card, no edit, no remove, and nothing says it is waiting.
- **Running state:** desktop `실행 중 명령어 35초 ▸`, `1번째 줄 읽는 중 대기 10초 · 79 토큰` (the model's own description of the step) and a todo card above the composer. Forge `Pause 8 seconds (line 2) 18s` (also the tool description) and a spinner; both have a stop button.
- **Stop:** desktop folds the turn as `실행됨 명령 1개, 완료함 백그라운드 작업 1개 (중지됨 1개)`; the todo card stays with its unfinished items. Forge folds it as `명령 2개 실행, 파일 1개 읽기 (실패 1)` and shows a `작업 2개 완료` pill: a stop reads as a failure and as done.
- **Excel result (gap):** desktop opens the `.xlsx` in the Files panel as a spreadsheet: a grid with column letters, a formula bar `fx`, sheet tabs (`요약`, `할일`, `데이터`), the folder's files at the right with a filter box. Formula cells that were never calculated show empty (the model said LibreOffice was missing). Forge: the file row has a plain document icon and the view says `이 파일은 바이너리 데이터로 보여 텍스트로 미리 볼 수 없습니다` with `Finder에서 보기` and `그래도 텍스트로 보기`. In the chat Forge prints the result path as inline code, which does nothing when clicked; the desktop's file name is a link that opens the panel.
- **Questions to the person:** the desktop model asked its owner question as a plain end card (not a question card), `최팀장님 보고용 매출 요약 1장은 누가 담당하나요?`. Forge's model did not ask (model behaviour, not compared).
- **Panels:** opening Preview and Files together in a 1200 pt window floats both over the chat and hides it.
- **Restore after a crash (checked against the real Claude home, 2026-10-24):** Forge was killed with `kill -9` while `python3 -c 'import time; time.sleep(90)'` was running in an auto-mode session; after the restart the session is in the list with a generated title (`python3 time.sleep(90) 대기`; in the live view it had shown the first prompt cut off, so the title only updates after a restart), and opens with the question and the tool row `Sleep for 90 seconds using python3 (실패 1)`. Nothing says the run was cut by a crash; the mode is back to `수동` and the model to the default (the session was on `자동`). Forge's sessions started this way are also listed by the desktop app, which shares the folder.
- **A blocked command:** `sleep 60` alone was refused by the CLI's tool rules (it advises the Monitor tool); Forge shows the row as `Sleep for 60 seconds (실패 1)` and the model's explanation. Same text as the desktop would get (not compared).
- **Still not seen:** voice input, several sessions running at once, read-aloud and pin actions, `/btw` and `/rewind` behaviour, Dispatch.

## Round 29 (2026-10-24): the plan flow in detail (Plan 56)

Desktop 3.6556, the session `wordstat.py --top N과 불용어 제외` from Round 27 (a throwaway folder).

- **Plan card:** `Claude가 계획을 제안했습니다`, link `계획 열기` at the right; `거부`, `수정… [Esc]`, `수락 [⇧⌘⏎]`, dark `수락 및 자동 모드 [⌘⏎] | ⌄`. The `⌄` menu: `수락 및 편집 허용`, `수락 및 자동 모드`.
- **`수정…`:** the buttons turn into a text area (`무엇을 변경해야 하나요? (선택사항)`), `뒤로` and a dark `수정`.
- **Comment on the plan:** dragging over plan text in the `계획` panel selects it and shows a floating box `댓글 추가` with a send icon. After Enter the card switches to edit mode: a grey chip with the quoted text and the comment (X to remove it), the text area `더 추가할 내용이 있으신가요?` and `수정`. The quoted text gets an underline in the plan. Removing the chip returns the card to the four buttons.
- **`수락` (plain):** the composer's mode became `수동`; the next step (`wordstat.py` write) asked for permission with the usual card. The card folded to `계획 제안됨 ▸` and the plan panel stayed open; its hint line changed to `계획 모드에서 계획에 대해 의견을 남기거나, Claude가 승인을 요청할 때 의견을 남길 수 있습니다`.
- **Sidebar:** a session whose plan is ready shows `차단됨` and a blue dot; hovering the row shows a tooltip: the session title, `플랜 준비됨 · Ready to start?`, the first lines of the plan, `승인하려면 세션을 여세요`.
- **Plan panel header:** title `계획`, icons: open folder, copy, expand, close. The hint line at the top: `텍스트를 선택하여 Claude에게 댓글을 남기세요`.
- **Plan data:** the real transcript has a `Write` of `~/.claude/plans/<name>.md` and then `ExitPlanMode` with `input: { plan: "<the markdown>" }`.
- **Not clicked:** `수락 및 편집 허용`, `수락 및 자동 모드`, and the `수정` button (what Claude receives).

Plan 56A built and checked on 2026-10-24 (see the spec's "As built"); the three desktop buttons not clicked in Round 29 are still unclicked.

## Round 30 (2026-10-25): an office worker's report task (docx + xlsx in, docx + pptx out)

Same method. Throwaway folder `/private/tmp/forge-observe/task-r30`: `보고서_초안.docx` (heading, a 3×3 sales table, a bullet) and `월별매출.xlsx` (3 products × 3 months with SUM formulas), made with python-docx and openpyxl. Desktop 3.6556, Sonnet 5.5, auto mode. Forge: the 57B build on `plan-58` (master), opened on the files the desktop's Claude made (the harness cannot type Korean and ⌘V does not reach the page, so a real second run in Forge was not done; Forge's run-time behaviour is from rounds 27 to 29).

**Prompt (office worker):** `첨부한 월별매출.xlsx의 3분기 수치로 보고서_초안.docx의 제품별 매출 표와 요약 문장을 3분기 기준으로 갱신해서 새 파일(보고서_3분기.docx)로 저장해줘. 바뀐 숫자는 빨간색으로 표시하고, 끝나면 임원 보고용 PPT 3장도 만들어줘.` Then a follow-up: `보고서_3분기.docx의 제품별 매출 표에 '전분기 대비' 열을 추가해줘 … 같은 파일에 덮어써줘.`

- **Attaching Office files (`+` → `파일 또는 사진 추가`, system dialog; `⇧⌘G` types a path, `⌘A` selects both):** in the composer each file is a card with its path cut in the middle (`/private/tmp/for ge-observe/task-r…30/월별… .xlsx`) and a rounded type tag `XLSX` / `DOCX`. After sending, the user bubble has the two cards above it (name only, type tag). Forge shows a small chip `name ×` (round 28).
- **While it runs:** the session title is generated at once (`3분기 매출 보고서 갱신`, in the header and the sidebar; Forge only after a restart); the details panel opens by itself (`환경`, `컨텍스트`, `커넥터`); a todo card in the chat (`Inspect xlsx and docx`, `Update report with Q3 figures in red`, `Build 3-slide executive PPT`, `Verify outputs`) that stays in the transcript with all items checked after the end; a fold `실행된 명령 2개, 사용함 도구 1개 ▸`; the status line `269 토큰 · 생각 중`, later `실행함 python3… 9초 ▸`; the composer hint `⏎ 키로 보내기, ⌘⏎ 키로 대기열에 추가하여 나중에 보내기` and a stop button.
- **After it ends:** a message action row under the last answer (copy, share/branch, pin, read aloud, `지금`). The answer is plain markdown with the file names as links; a markdown table in the answer is drawn as a table. No end card and **no `결과물` card**: the files were made by a python script, not by a Write tool, so the desktop's card (Write-tool paths) does not list them. Forge's card (paths named in the last message) does.
- **The Files panel renders Office files (big gap):** a click on `보고서_3분기.docx` opens the Files panel with the document drawn as a **page**: the heading and section titles in blue, the changed numbers **in red** (what the prompt asked for), table borders, the page pill `Page 1 / 1`; the changed `3` in the title is red too. The `.pptx` opens as **slide images**: dark title slide with three stat boxes, a slide with a bar chart and three cards, a slide with a line chart and a bullet box, each with `Page N / 3`. A spinner shows for 5 to 10 seconds before each render. Forge (57B) shows a reading view: headings, bold/italic, lists, tables, but **no colours** (the red numbers look like the rest of the text), and the deck has text only (no charts, shapes, backgrounds or pictures).
- **The open tab refreshes by itself:** after the follow-up turn edited the same `.docx`, the open tab showed the new `전분기 대비` column without a click (it drew the page again). Not checked in Forge for Office files.
- **File list at the right of the panel:** every file of the folder with an icon by type (slides, document, table), a filter box; the more menu (`⋯`) holds only editor settings (file tree, tab size, auto save), nothing like `Word에서 열기`.
- **`@` list:** sessions, then the folder's files (the two attached ones only; the new outputs were not in it), then windows of other apps (Finder, iTerm2 …). Same as round 28.
- **Questions asked back:** none; the model noted what it did not verify (no LibreOffice, so no render) and marked its own assumptions (`"9월은 7월 대비 +10%" 등 내용은 … 제가 구성한 것이니 검토해 주세요`).
- **What this means for Forge:** the person's first need is to see that the changes they asked for are there (colours, the new column), and for a deck to see charts. A text-only view hides both. Cheap step: carry run colour, highlight, underline and strike through the docx reader and view (and heading colours). Larger step: draw pages and slides (a bundled renderer in the web view; a conversion through LibreOffice would not exist on the work PCs).

## Round 31 (2026-10-26): the 58B to 60 features in the desktop app

Same method. Throwaway folder `/private/tmp/forge-observe/task-r31` (`items.csv`, `memo.md`), auto mode, Sonnet 5.5, one new session driven through `scripts/observe/claude_app.py`. Goal: the things earlier rounds listed as not seen (`/btw`, `/rewind`, the `/compact` result, the queue while a turn runs, stop, the message row) now that Forge has built its own versions (Plans 58B to 60).

- **Queue (⌘⏎ while a turn runs):** a card above the box `메시지 1 개 대기 중` + the text in grey + three icons (send ↵, edit ✎, remove ×). Same as Forge. When the turn ended the queued text went out as a normal user bubble. Forge matches.
- **Usage warning (new):** a grey bar above the composer `주간 사용량 한도 초과 임박 · 10월 28일 (수) 오후 6:00에 초기화`, a `사용량 추가 구매` button and a close `×`. It stays across turns until closed. Forge shows nothing about usage limits in the chat.
- **`/btw 질문`:** not shown in the main chat at all. A **right-hand panel** opens (a tab named like the session, with `×` and `+`, expand and close at the top): the question as a bubble at the right, the answer as plain text, and at the bottom its own box `이 세션에서 답장하기…` (so the side chat can go on). The main conversation, the details panel and the context figure stay as they were. Forge's `/btw` is a closable card above the composer (no follow-up box).
- **`/rewind`:** a modal `되감기`, text `세션을 이전 메시지로 복원합니다.`, then the user messages **oldest first** (each cut with `…`, the hovered one grey). Picking one shows step 2, titled `이 메시지로 되돌리시겠습니까?`: the message in a grey box, a `복원` select with three choices `코드와 대화` (default), `대화만`, `코드만`, a line `파일 1개 · +0 −2`, a grey note `셸 명령으로 적용된 변경 사항은 되돌려지지 않습니다. 커밋은 그대로 유지됩니다.`, and buttons `취소` and a black `되감기`. Not pressed (the result was not seen: whether the same session is cut back, and what happens to the composer text). Forge: newest first, `파일 되돌리기` / `대화 열기` / `둘 다` buttons, and the conversation opens as a **new** session; no shell caveat.
- **`/compact`:** the command shows as a blue-text bubble `/compact`; while it runs a status `세션 압축 중 4초` with the dots; the result is one grey line `압축된 세션 · 63.6k개 토큰 절약` (tokens saved, not before and after). Forge: a divider `대화를 압축했습니다 · 11.4k → 1.3k 토큰`.
- **Message row (hover on a user bubble):** `✓✓` (delivered), `4분 전`, `·`, copy, `↺` (rewind, opens the `되감기` flow), and a fork icon. Forge: copy and ↺ only, no time, no fork.
- **Stop:** pressing the stop square mid-turn left no `중지됨` line; the fold line above the next message read `실행된 명령 1개, 완료함 백그라운드 작업 1개 (중지됨 1개)` (here the model had run the sleep in the background). A direct tool call refused or stopped reads `중지됨` after its name (round 26). Forge shows a grey `중지됨` line under the turn.
- **Title:** generated at once (`items.csv와 memo.md 요약`), as in round 30; Forge now does it too.
- **Answers:** the model put the summary in a code block with a copy icon at its corner; a file name in the text is a link that opens the Files panel.

**Round 31, `되감기` pressed (same session, first message, `코드와 대화`):** the **same session** goes back: the chat reads `아직 메시지가 없습니다`, the rewound message's text is put back in the composer (ready to edit and send again), the file the turn made (`summary.txt`) is deleted from the folder and the `결과물` card empties, the context card keeps its old figures with a note `마지막 업데이트: 56분 전. 새로고침하려면 메시지를 보내세요.`, and the `/btw` side panel stays open. No new session, no confirmation after the button. So Forge's `대화 열기` (a new session) differs: the desktop cuts the session itself back.

## Round 32 (2026-10-28): fork from a message, sidebar edit, the Preview panel menus

Same method (`scripts/observe/claude_app.py`, shown scale 1.39, window 1391 x 893 points), in the throwaway session `백그라운드 sleep 및 출력` (folder `task-r31`). Goal: the items the roadmap still lists as `partly`: what the message fork icon does, `사이드바 편집…`, and the Preview panel's own menus.

- **`사이드바 편집…` (SES-10):** the `더 보기` item of the sidebar holds only this one entry. It opens a small modal `사이드바 편집`, text `사이드바에 표시할 항목을 선택하세요.`, three checkboxes with their icons, all on: `아티팩트`, `예약됨`, `사용자 지정`, and a black `완료` button (and `×`). So it only hides or shows those three sidebar rows (`새로 만들기`, `검색`, `더 보기` cannot be hidden). Nothing was changed.
- **Message row, third icon = fork:** hover a user bubble: `✓✓ 그저께 · copy ↺ fork`. Pressing the fork icon on the SECOND message of the session made a **new session** at once and switched to it:
  - Title `<old title> (fork)` in the header and the sidebar; it keeps the folder (`task-r31`).
  - The fork holds the conversation **up to before the clicked message**: the first turn only. The clicked message's text sits in the composer, ready to send again (like `되감기`, but the old session is left whole).
  - A grey line at the top of the chat: `포크 원본 <old title as a link> 지금`. The link goes back to the original.
  - Sidebar: the new row sits under the original in the `오늘` group, indented with an elbow line `└`, and the original row got a `▾` (collapse the forks). Group by date puts the fork in `오늘`.
  - The details panel opened with the fuller context list (Messages, System tools, System prompt, Skills, Custom agents, Autocompact buffer, Free space) and the connector list (`Context7 인증`, `M365 다시 연결`, `Figma`).
  - No confirmation, no menu: one click.
- **Preview panel (globe icon):** first-run modal `사이트에 로그인 상태 유지` (cookie import: Chrome, Edge, Firefox icons; three ticks: only cookies and site data, you choose the sites, nothing leaves this computer; `다시 표시 안 함`, `나중에`, `쿠키 가져오기`). Empty state: a globe, `Claude로 브라우징하기`, `URL을 입력하거나 Claude에게 사이트를 열어달라고 요청하세요. Claude는 읽기, 클릭, 입력이 가능합니다.`, and `앱을 미리 볼까요? 개발 서버 감지`.
  - **`⋮` menu** (top right of the panel): `다음 브라우저에서 열기: Google Chrome`, `URL 복사`, `스크린샷 저장` (the three grey while no page is open), `HTML 파일 열기…`; a row `뷰포트 · 반응형` (grey); `기본 제공 브라우저에서 링크 열기` ✓; `Claude가 작성한 파일 자동으로 열기` ✓; `변경사항 자동 확인`; `허용된 사이트 관리…`; a `쿠키` group: `쿠키 가져오기…`, `쿠키 유지 · 종료 시까지 ›`, and a red `브라우징 데이터 삭제`.
  - **Layout icon** (left of `⋮`): a one-item menu `서버 추가` (a dev server, looks like the launch config). Not pressed.
  - **Element picker** (cursor icon at the right of the address bar): does nothing while no page is open.
  - The modal was dismissed with `나중에`; the panel was closed again.
- **State left in the desktop app:** one new session `백그라운드 sleep 및 출력 (fork)` in `task-r31` (delete it by hand if unwanted).
- **For Forge:** the Preview `⋮` menu is mostly built in Forge's own tab bar; the missing pieces are `HTML 파일 열기…`, `스크린샷 저장`, the viewport row, the `Claude가 작성한 파일 자동으로 열기` switch, `허용된 사이트 관리…` and the cookie group (cookie import stays out on the work PCs). Fork from a message needs the session copy Plan 19 already has, cut at a message, plus the original-link line and the nested row in the sidebar.

## Round 33 (2026-10-28): the desktop session `다음 로드맵` (the Plan 68B work), read top to bottom

Method: the user's own desktop session `다음 로드맵` (worktree `claude/daum-roadmap-c1e83b`, about 800 transcript lines) was read in the app with `scripts/observe/claude_app.py` (shown scale 1.39), scrolling from the first message to the end, and one fold line was opened. Nothing was sent. Compared with the code on `plan-67` merged with master 0d577e5.

What the desktop app shows that Forge does not:

- **Fold line wording is by kind of tool, not only files and commands.** Seen: `불러옴 메모리 1개, 실행됨 명령 2개` (a memory file read counts as `불러옴 메모리`, not `파일 읽음`), `Skill 실행됨  /superpowers:brainstorming` (the skill name as a grey chip), `실행됨 명령 2개, 로드된 도구, 앱 목록 조회됨` (a tool-search call is `로드된 도구`, a computer-use list call is `앱 목록 조회됨`), `실행됨 명령 6개, 완료함 백그라운드 작업 1개`, `완료함 백그라운드 작업 2개, 실행됨 명령 6개 (5개 실패)`. Forge (`crates/forge-entities/src/turn_view.rs` `summarize`) knows only created/edited/ran/read/`other tools`, and `(N failed)`; no memory, skill, tool-search or background-task count.
- **A fold line that holds one image read** is `읽음  forge-harness-shot.png ▸` (or `불러옴 메모리 1개, 실행됨 명령 1개, 읽음 <file>`) with a **small thumbnail of the image at the right end of the line**. Forge shows the file name only.
- **Opened fold (`완료함 백그라운드 작업 2개 … (5개 실패)`):** a bordered box of rows, one per call, each a grey line with its own state word in front: `실행 중 sleep 4… 실패 ▸`, `백그라운드 작업 · sleep 4 && python3 scripts/harness/capture.py 실패` (a background task row has its own label and the command, a red `실패`), `실행함 python3… ▸`, `실행함 grep… ▸`, `실행 중 명령어 실패 ▸`. The words are past tense (`실행함`, `실행됨`) when done, and the red `실패` comes after the name. Each row with `▸` opens its own input and output.
- **Reply footer** (last message of the turn): copy, share/branch, pin, speaker, `23분 전` (already in NEW-07; seen again, same four icons).
- **Code block** (the harness command in a ```bash block): a play button `▷` and a copy icon at the top right corner. The `▷` is not in Forge (copy only). Not pressed (it would run a command).
- **Feedback card** under the last reply of a finished session: `이번 세션에서 Claude는 어떤가요?` with three text buttons `나쁨 보통 좋음` and a close `×`. A bordered card in the chat column, above the composer. Forge has nothing like it. Not pressed.
- **Sidebar, collapsed parent:** the desktop row `백그라운드 sleep 및 출력 ▸` is followed by an indented grey line with an elbow, `└ 중첩된 세션 1개`, while the forks are folded. Forge (68B) shows `▸` only and no count line.
- **Header and composer:** title `다음 로드맵 ⌄` (a menu), a spinner at the right of the model label `Sonnet 5.5 중간 ◌` while the session is still alive, and the permission label `자동` at the left under the box.
- **Not seen in the finished transcript:** the plan list the model sent with `update_plan` (five items) and the `wrap_up` summary with two buttons (`그대로 커밋`, `창 확인 먼저`) leave **no card or line in the chat** once the turn is over. They may show only live (a progress card, an end card as in round 27). Forge has no handling of `mcp__ccd_turn__update_plan` or `mcp__ccd_turn__wrap_up` (no hit in `crates` or `sidecar`). Worth one more round with a live turn.
- **Tools the desktop app allows its session** (from the process arguments): `ccd_session` (spawn_task, dismiss_task, mark_chapter), `ccd_session_mgmt` (list/get session, set_session_title, send_message, search_session_transcripts, export_transcript), `ccd_sidebar` (create_group, set_view, create_section), `ccd_view` (show_pane, close_pane, set_transcript_view), `ccd_window` (open_session_in, close_split, set_sidebar_collapsed), `ccd_settings`, `ccd_connectors`, `ccd_pr`, `mcp-registry`, `scheduled-tasks`. Forge exposes none of these to the model. `mark_chapter` hints at a chapter feature in the transcript that was not seen here.

Already built and seen again (no change): the dock panels, the context and connector cards at the right (`컨텍스트 21% 사용됨 212.8k / 1M`, `커넥터 Context7 인증`), the fork nesting with `└`.

For the backlog: NEW-20 fold line kinds (memory, skill, tool search, background task) and per-row states; NEW-21 image thumbnail on a read line; NEW-22 code block `▷`; NEW-23 feedback card; NEW-24 `중첩된 세션 N개` line; NEW-25 `update_plan`/`wrap_up` (observe live first).

**Round 33b (same day): `update_plan` and `wrap_up` seen live** (NEW-25). A throwaway session in `forge-observe` (`/private/tmp/forge-observe`, mode `수동`) was sent: use only `update_plan` for three steps (조사, 정리, 마무리), one line of text per step, then `wrap_up` with a summary, a question and two buttons. The title was made at once (`3단계 계획 및 진행 관리`).

- **`update_plan` = an inline plan card.** While the turn runs, a white bordered card (the width of the chat column, rounded) sits in the chat at the place where the tool was called, between two narration lines. One row per item, an icon and the text: a filled grey circle with a check (done), an empty ring (waiting), a dashed ring (in progress). It followed the model's status calls. **When the turn ended the card was gone**: the finished chat shows only the narration lines (`조사 단계: …`, `정리 단계: …`, `마무리 단계: …`) and the answer, as in the `다음 로드맵` transcript. The right panel did not get a progress card (it showed 환경, 컨텍스트, 커넥터 only). Under the live card the usual `6초 · 46 토큰` line runs with the dots.
- **`wrap_up` = the end card** (the `Fix them / Not now` card of round 27, so NEW-07's card is this tool): after the last text, a bordered card the width of the column with a close `×` at the top right; grey recap text (the `recap` field), the question in black (`다음에 어떤 작업을 진행할까요?`, the `ask` field), then the buttons: the first is black with a white label and the key `⌘⏎` (`주제 지정 ⌘↵`), the second is light grey (`여기서 종료`). They are the `actions` labels in order.
- **Pressing a button** (`주제 지정`, the first): the text of its `message` field is sent at once as a normal user bubble (here the same as the label, `주제 지정`), the session answers as usual, and **the card stays in the chat as a spent copy**: narrower (about 70% of the column), the recap and the question in grey, no buttons, only the chosen label in grey text under the question, no `×`. It is no longer clickable. `⌘⏎` with the composer empty is the key of the first button. When no new turn is sent the card stays live; the old card of the `다음 로드맵` session was not shown after more turns (a later card replaced it).
- **For Forge:** both tools are MCP tools of the desktop app (`mcp__ccd_turn__update_plan`, `mcp__ccd_turn__wrap_up`), not model-side features, so Forge's sidecar would have to offer an MCP server with these two tools to the SDK, then draw (1) the live plan card from the tool input (`items` with `content` and `status`, `scope` seen as `continuation`) and (2) the end card from `recap`, `ask`, `actions[{label, message}]`, with the spent look after a click. Without that server the model never calls them, so Forge's end of turn stays the grey composer suggestion.
- **State left in the desktop app:** the session `3단계 계획 및 진행 관리` in `forge-observe` (delete it by hand if unwanted); its second turn (after `주제 지정`) was still answering when the last capture was taken.

## Round 34 (2026-10-29): the code block `▷` pressed (NEW-22)

Method: the throwaway session `3단계 계획 및 진행 관리` in `forge-observe` (`/private/tmp/forge-observe`, mode `수동`). With the user's go-ahead one message was sent: `도구는 쓰지 말고, 아래 내용의 bash 코드 블록 하나만 답해 주세요: echo forge-observe-hello`. The reply was a bash block with that one line. `▷` was pressed once. Shown scale 1.39.

- **The code block header** has two icons at the right end of the block's own row: `▷` (run) then the copy icon. The row is one line for a one-line block; no language label.
- **Pressing `▷` runs the text in the session's own terminal** (the dock terminal tab `터미널 forge-observe`, cwd = the session folder, here `(base)` conda prompt included). No permission card, no confirmation, even in mode `수동`; the model was not involved in the run.
- **A result box appears at once under the code block** (same width, thin border, grey monospace): the command's output (`forge-observe-hello`), then the shell prompt lines that followed it (`(base)` and the cwd in blue), a faint time `21:38:44` at the right, and a small terminal icon `>_` at the top right corner. The box stays after the reply goes on.
- **The `>_` icon opens the dock terminal** (header `터미널 forge-observe`, `+`, `…`, expand, close); the terminal shows the command line `❯ echo forge-observe-hello` and its output, so the same shell that the dock `>_` button opens did the run (the dock terminal also replayed older buffered output with `[shell reconnected — replaying buffered output]`).
- **The output is sent to the model**: with no user bubble and nothing typed, a new turn started (the stop square showed in the composer, the placeholder changed to `⏎ 키로 보내기, ⌘⏎ 키로 대기열에 추가하여 나중에 보내기`), and the model answered under the box: ``echo forge-observe-hello`를 실행했고 `forge-observe-hello`가 출력됐습니다. 오류는 없었습니다.`` So `▷` = run in the terminal, show the result in the chat, and tell the model what happened.
- **Not checked:** a command that fails or never ends; a multi-line block (one run or one per line); whether the box is saved in the transcript after a restart (nothing was seen in the transcript file); what `▷` does in a block that is not shell (python); long output (cut or scroll).
- **State left in the desktop app:** two more turns in `3단계 계획 및 진행 관리` and its terminal tab `forge-observe`.

## Round 35 (2026-10-31): new-session screen, desktop app vs Forge

Method: `scripts/observe/claude_app.py` (shown scale 1.24) on the desktop app, `scripts/harness/` on Forge (shown scale 1.20). The desktop app was on a session whose folder is gone; `새로 만들기` opened the new-session screen. Nothing was sent. The card's rows and footer were pressed (only the new-session draft changes).

**The card in the middle of the desktop screen is a quick ENVIRONMENT picker, not a recent-folders list.**

- Placement: centred in the column above the box with a gap, a pixel mascot on its top edge. Under it a second card `Claude Code CLI 세션 2개 가져오기`.
- Rows: a laptop icon and an environment name (`forge`, `forge-observe`, `mywork`); hover shows the path as a tooltip (`~/Developer/forge`). It need not equal what the box currently has: before pressing, the line under the box said `이 컴퓨터 · task-r31` and no row was checked.
- Pressing a row checks it and the line under the box becomes the environment: `forge` plus a `워크트리` checkbox (the folder chip and `이 컴퓨터` are gone). The box takes focus.
- Footer `환경 선택 ⌄` opens the full environment menu: a search field `환경 검색…`, group `로컬` (`이 컴퓨터`, `forge` checked, `forge-observe`, `더 보기 1 ›`), `클라우드 ›`, `+ 새 환경`. The card's three rows are the first saved environments of this list (`mywork` is the one under `더 보기`).
- Box: `+` left, mic right; under it `이 컴퓨터`/folder at the left, `Sonnet 5.5 중간`, `자동`, a ring at the right.

Forge already has the whole environment concept (`EnvironmentChip`: search, `이 컴퓨터`, saved environments, `더 보기`, `새 환경`). What it lacked is the shortcut card above the box.

Mistake recorded: a first version (PR #6 before the fix) built the card from recent FOLDERS after one screenshot, without pressing it. The screenshot already contradicted that (box said `task-r31`, card did not list it; laptop icons; footer said `환경 선택`).

Not ported: CLI session import (Forge reads `~/.claude`), mic, mascot. Not checked: the cloud group, where the desktop menu opens relative to the card.

Follow-up (same day): the user asked for Forge's card to sit in the middle of the screen like the desktop app's, and the usage box on the new-session screen to go. Both done: the card is centred between the greeting and the box, and the usage box (`UsageCard` widget) is removed from that screen (its `.usage-*` styles and the data code in `forge-entities` are left, now unused by this screen).

## Round 36 (2026-10-31): a todo-app project built in the desktop app

Method: `scripts/observe/claude_app.py` (shown scale 1.24). The user allowed real sends and every feature, but no Opus. A new session in the saved environment `forge-observe` (folder `/private/tmp/forge-observe`), model Sonnet 5.5, asked for a desktop todo app in `todo-app/`: first a technology proposal and plan, then "go ahead with Tauri, steps 1-3, show progress as a task list". Session title (auto): `데스크톱 Todo 앱 기술 선택`. The app was built (Tauri 2 + React-TS, `useTodos.ts`, `App.tsx`, `TodoItem.tsx`, `App.css`); it was not run in a window.

What I pressed, and what it did:

- **Choosing the environment `forge-observe` on the new-session screen** changed three things at once: the `워크트리` checkbox disappeared (the folder is not a git repo); the mode under the box went from `자동` to amber `수동`; the card under the environment card gained a row `자동 모드 켜기` (bolt icon) next to `Claude Code CLI 세션 N개 가져오기`. So the default mode follows the folder, and the card offers a shortcut to Auto.
- **Permission card** (manual mode): header `Claude가 이 명령을 실행하도록 허용하시겠습니까?`, a grey line with the tool's own description (`Check installed dev toolchains`), a second grey line with the reason when the check cannot be done (`A variable in this command can't be checked before it runs`), the command in a code box, then `거부` and a black `한 번만 허용 ⌘⏎`. Above the card a pill `입력이 필요합니다` with an orange ring; the sidebar row has an orange dot. For the second command (`npm create ...`) the black button grew a split `⌄`. Its menu: `항상 허용` (`⇧⌘⏎`, "로컬 프로젝트 설정에 저장됩니다") and `허용하고 Auto 모드로 전환` ("이 세션의 나머지 작업을 자동으로 검토합니다"). Pressing the second one allowed the command, switched the mode label to `자동`, and no further card appeared for the rest of the work (npm install, file writes, cargo check).
- **Task list**: while a turn runs, a white card with the plan rows (done check, spinner, dashed circle) sits above the activity line, at the bottom of the chat, not in the middle of the text. After `영속화 진행` the list was replaced by the new step's rows.
- **Activity line**: `Installing deps and inspecting template 1분 6초 ▸` with orange dots at its left (the dots animate); `생성 중 9초 ▸` while the model writes. Composer placeholder while running: `⏎ 키로 보내기, ⌘⏎ 키로 대기열에 추가하여 나중에 보내기` (empty box) and `⌘⏎ 키를 눌러 대기열에 추가...` (when narrow).
- **Right column**, the default Details panel (환경, 컨텍스트, 커넥터): `컨텍스트` expands to a table by kind (Messages, System tools, MCP tools, System prompt, Skills, MCP server instructions, `마지막 업데이트 이후` (new since the last count), Autocompact buffer, Free space) with colored squares, tokens and percent; below it `MCP tools (def…` and `System tools (de…` with `—` and `MCP 도구 248`. New cards appear when there is something to show: `백그라운드 작업` (the install command, seconds, a stop square) and `결과물` (`App.css` with a code icon).
- **Next-step card** (`wrap_up`): recap, question, `영속화 진행 ⌘⏎` and `먼저 실행해 보기`, and a `×`. Pressing the first sends its label as a user bubble; the card stays in the chat greyed out with the chosen label under the question.
- **Shell block** under the reply: `▷` and copy at its right; the path in it is a plain code line.
- **Reply footer** after the last reply: copy, share, pin, read aloud, `지금`.
- **Changes panel in a folder that is not a git repo**: header `변경 사항`, centre text `변경 사항을 보려면 Git 저장소가 필요합니다` and grey `이 세션에서 찾은 항목이 없거나 세션 폴더에 접근할 수 없습니다.`
- **Files panel** opened under Changes: panels stack top to bottom in the right column, each with its own header (title, `...`, expand, close). The Files header has a search icon; the tree sits at the right of the panel, the open-file area at the left (`열린 파일이 여기에 표시됩니다`, `트리에서 파일을 선택하거나, 대화 내의 파일 경로를 클릭하세요.`); filter field `필터링하거나 ?를 입력해 파일 …`. The tree's root showed the children of `/private/tmp/forge-observe` (`todo-app` among them), not of `todo-app`.
- With a panel open the chat header's icons shrink to tasks, changes, globe, folder, `...`; the `>_` terminal icon is not there.

Not checked: `항상 허용` (it writes the user's local project settings), `▷` on this block, `먼저 실행해 보기`, running `tauri dev`, the Preview panel, the `?` filter in Files, the `자동 모드 켜기` row (not pressed), the `결과물` card (only its first row was seen).

Compared with Forge by string search only (not run): `항상 허용` exists; `허용하고 Auto 모드로 전환`, the `자동 모드 켜기` row, the `마지막 업데이트 이후` context row and the non-git sentence of the Changes panel are not in `ko.rs`. Backlog rows NEW-26..28.

### Round 36, part 2 (same session, after step 4): menus and panels

Same method. The session had reached step 4 (`todos.json`, `plugin-store`); a next-step card `5단계 진행 ⌘⏎ / 먼저 실행해 보기` stood at the end.

- **Folder tab in Files**: pressing a folder row in the tree opens an italic (preview) tab named after the folder, showing the folder's `README.md` rendered, under a header `todo-app` with four icons: paperclip, `>_`, a folder, `…`. `…` menu: `컨텍스트로 첨부`, `복사 ›` (`절대 경로`, `상대 경로`, `파일 이름`), `다음에서 열기 ›` (only `VS Code`), `Finder에서 보기`, `터미널에서 열기`. Pressing `컨텍스트로 첨부` puts a blue mention token `todo-app` at the start of the composer (like an `@` mention) and the next-step card's `⌘⏎` hint disappears while the box has text. (I cut the text again with ⌘A ⌘X; nothing was sent.)
- **Composer `+` menu**: `파일 또는 사진 추가 ⌘U`, `폴더 추가`, `슬래시 명령어`, `커넥터 ›`, `플러그인 ›`. The connector submenu: `커넥터 둘러보기`, `커넥터 관리`, then one row per connector with an on/off switch (here Google Drive, Claude in Chrome, Filesystem, puppeteer, all on): a per-session switch. The plugin submenu: one row per plugin (`M365 connect ›`, `Claude hud ›`, `Compound engineering ›`), `플러그인 관리`, `플러그인 탐색`. A plugin row opens a tall list of that plugin's skills, each with its name and the full description (scrolls).
- **Mode menu** (press `자동`): title `모드`; `자동` (`Claude가 권한 결정을 처리합니다`, key 1, checked), `수동` (`변경하기 전에 항상 확인`, 2), `편집 자동 수락` (`모든 파일 편집을 자동으로 수락`, 3), `계획` (`변경하기 전에 계획 세우기`, 4), and `권한 무시` with a text button `활성화` (not pressed).
- **Model menu** (press `Sonnet 5.5 중간`): `Opus 5.5` (1), `Sonnet 5.5` with a `기본값` pill (checked), `Fable 5.1` with a pill `ⓘ 사용 크레딧 필요` (3), `Haiku 5.5` (4); then `노력 중간 ›` and `더 많은 모델 ›`. `더 많은 모델`: `Haiku 4.5`, `Sonnet 5`, `Opus 5`, `Fable 5` (credit pill), `Opus 4.8`, `Opus 4.7`, `Opus 4.6`, `Sonnet 4.6`. `노력` submenu: a grey sentence on top (`더 높은 노력은 더 철저한 응답을 의미하지만, 시간이 더 오래 걸리고 한도를 더 빨리 사용합니다.`), `낮음`, `중간` (`추천`, checked), `높음`, `엑스트라`, `최대 ⓘ`, a line, and a switch `Ultracode`. I did not choose any model (the user said no Opus).
- **Chat header**: the title chevron menu is the session menu already in SES (`다음에서 열기 ›`, `이름 변경 R`, `포크 F`, `트랜스크립트 보기 ›`, `출력 스타일 ›`, `보관 A`, red `삭제 D`). The `…` menu at the right of the header lists panels: `터미널 ⌘J`, `사이드 채팅 ⌘;`, `백그라운드 작업`, `iOS 시뮬레이터`, and a switch row `컴퓨터 절전 모드 방지` / `이 세션에서만`.
- **iOS simulator panel** (mac only): a third column with tabs `편집 ⌄`, `기기 ⌄`, a header `시뮬레이터 선택 ⌄`, a phone frame saying `Claude가 앱을 볼 수 있도록 시뮬레이터를 연결하세요` (`Claude가 이 시뮬레이터를 제어하고 전체 화면의 스크린샷을 촬영합니다. 종료된 기기는 자동으로 부팅됩니다.`), a check row `Xcode 및 시뮬레이터 설치됨`, a black button `시뮬레이터 연결 ⌄`, and a bottom bar (home, reload, camera, video, pencil, terminal). Not connected. Once opened, its phone icon stays in the header row.
- **Terminal panel** (⌘J): a third column headed `터미널` with the session folder's shell as a tab named after the environment (`forge-observe`), a `+` for another tab, `…`, expand, close. The shell started in the session folder and showed the user's own shell greeting (not recorded here).
- **Preview panel** (globe): header `새 탭` + `+`, a layout icon, `…`, expand, close; address row with back, reload, `URL 입력`. Empty text: `Claude로 브라우징하기` / `URL을 입력하거나 Claude에게 사이트를 열어달라고 요청하세요. Claude는 읽기, 클릭, 입력이 가능합니다.` and a line `앱을 미리 볼까요? 개발 서버 감지`. Pressing `개발 서버 감지` showed a spinner `프로젝트 파일을 읽는 중입니다. 잠시 시간이 걸릴 수 있습니다.` and it was still spinning after 20 seconds (no dev server was running; the result was not seen).
- **Layout**: with three panels open the chat column shrinks to about a quarter of the window and the header icons shrink to the tasks icon, one panel icon and `…`; the code block wraps and the next-step card buttons stack.

Compared with Forge by string search in `ko.rs` only (not run): none of `더 많은 모델`, `Ultracode`, `개발 서버 감지`, `iOS`, `사이드 채팅` were found; `Attach as context`, `Accept edits`, `Bypass permissions`, `Add folder`, `Absolute path` exist. Backlog NEW-29..33.

Not checked: `Ultracode` switch, `권한 무시 활성화`, `사이드 채팅`, `백그라운드 작업` (panel), the plugin skill rows (do they insert `/skill`?), `…` of the Preview header and its layout icon, what ends the dev-server spinner.

### Round 36, part 3: dev server detection and a run of the app

- **`먼저 실행해 보기`** (second button of the next-step card) sent its label as a user bubble; the model ran `npm run tauri dev` in the background (activity line `Running Tauri dev app in backgr…` then `Waiting for Rust build … 46초`, reply `첫 Rust 빌드가 진행 중입니다(353개 크레이트). 끝날 때까지 기다립니다.`). A real Tauri window opened on screen (processes `vite`, `target/debug/todo-app`). The model then told me what to check (five numbered items) and said the server keeps running in the background.
- **The next card's buttons change with the situation**: after the run the card asked `동작 확인 후 5단계(데스크톱 마무리)로 갈까요?` with `5단계 진행 ⌘⏎` and `문제 있음`. The model writes both labels.
- **Preview `개발 서버 감지`** finished in about a minute: text `개발 서버가 감지되었습니다. 이 명령은 미리보기가 시작될 때 셸에서 실행됩니다.`, a card `Vite · 포트 1420` / `npm run dev` with a button `이것 사용`, a grey line `선택 사항이 이 프로젝트의 .claude/launch.json에 저장되었습니다.` and a link `← 탐색으로 돌아가기`. So the app reads `package.json`, proposes the server command and port, and keeps the choice in `.claude/launch.json` of the project. Pressing `이것 사용` here showed a red line `이 구성을 저장할 수 없습니다. 다시 시도하세요.` and no `.claude` folder was created in `/private/tmp/forge-observe` or `todo-app` (reason unknown; the folder is a `/private/tmp` path with no git repo). The preview did not start. Not checked: the success path, what the preview shows afterwards, the layout icon in the header.
- **Narrow window**: with three columns open, the details panel (환경, 컨텍스트, 커넥터, 결과물) opens as a floating popover over the chat column under its header icon and scrolls inside; it did not show a `백그라운드 작업` card in that view.
- Cleanup: the dev server and its window were stopped by killing the processes of `forge-observe/todo-app` from the shell (not through the app's stop button).

Backlog NEW-34 (dev-server detection) and NEW-35 (card buttons written by the model, already how NEW-25 works).

### Round 36, part 4: side chat, bypass-permissions switch, step 5

- **Side chat (⌘;)** opened a panel at the bottom of the right column (below Files), tab title `사이드 채팅`, a `+` for another tab, expand and close. Empty text `아직 메시지가 없습니다`; box `간단한 질문하기` with a return icon. I asked `todo 데이터는 어느 파일에 저장돼? 한 줄로만 답해줘.` and the reply came in about 15 seconds; the tab title became the first words of the question (`todo 데이터는 어느 파일…`); the box placeholder changed to `이 세션에서 답장하기...`. Nothing from the side chat appeared in the main chat. The answer used the session's context: it named the macOS app-data path and said it came from plugin-store's default behaviour, "not checked by code" (so the side chat answers from what the session knows and does not run tools). Not checked: whether the side chat shows in the sidebar or in the transcript file, closing it, the `+` tab.
- **Mode menu `권한 무시` → `활성화`** does NOT switch the mode. It opens the Settings window on the `Claude Code` page, scrolled to a highlighted (amber) row `권한 우회 모드 허용` with a switch (off): `모든 권한 확인을 우회하고 Claude가 중단 없이 작업하도록 합니다. 이는 린트 오류 수정이나 보일러플레이트 코드 생성과 같은 워크플로에 적합합니다. Claude가 임의의 명령을 실행하도록 허용하는 것은 위험하며 데이터 손실, 시스템 손상 또는 데이터 유출(예: 프롬프트 인젝션 공격)을 초래할 수 있습니다.` and a link `안전한 사용을 위한 모범 사례 보기`. So the mode only appears after the setting is on. I did not turn it on. The same page lists, under `세션`: `동적 워크플로우` (on), `알림에 주의 끌기` (off, Dock icon bounce), `Claude가 작업하는 동안 컴퓨터 절전 모드 방지` (on) with a sub-row `배터리 전원 사용 시 절전 모드 방지` (on), and above them a Remote Control block (switch, `Remote Control 사용을 위해 이 컴퓨터를 절전 모드로 전환하지 않음` with a `추천` pill) and `폴더 (23개)` with `폴더 추가...` and `23개 중 3개 표시됨 · 폴더 표시`. Settings left pages: 일반, 계정, 개인정보보호, 결제, 사용량, 기능, 메모리, 디자인 시스템, Claude Code, Claude in Chrome; 이 컴퓨터: 시스템, 확장 프로그램, 개발자; 사용자 지정: Skills, 커넥터, 플러그인. A search field sits on top.
- **Step 5 (`5단계 진행`)**: the button sent its label as a user bubble; a `↓` round button appeared above the box while the new turn was off screen (scroll to bottom). The model made the changes (app name, shortcuts ⌘/Ctrl+N and ⌘/Ctrl+1~3, new icon set), ran typecheck, build and `cargo check`, and wrote a spent grey card plus a new one for step 6 (`6단계 진행` / `먼저 실행해 보기`). The model says plainly what it could not check ("단축키와 아이콘이 실제 창에 어떻게 보이는지는 확인하지 못했습니다").

Backlog NEW-36 (side chat), NEW-37 (bypass switch lives in Settings).

### Round 36, part 5: the `/` command list

Typing `/` in an empty box opens a list above the box, filter placeholder `입력하여 필터링` (return icon at the right of the box). Order, top to bottom: `btw`, `ultrareview`, `rewind`, `fork`, `subtask`, `mcp`, `plugin`, `feedback`, `model`, `effort`, `fast`, `plan`, `permissions`, `skills`, `output-style`, `rename`, `add-dir`, `cd`, `diff`, `tasks`, `workflows`, `status`, `usage`, `stats`, `usage-credits`, `autocompact`, `config`, `storage`, `sandbox`, `theme`, `auto-mode-setup`, `resume`, `advisor`, `login`, `help`, `copy`; then a second group with `__remote-workflow`, `advisor` (right label `사용자 지정 명령`), `agents`, `algorithmic-art`, and one row per skill (`anthropic-skills:built-in-browser`, `:chrome-browser`, `:computer-use`, `:deep-research`, …). Names are shown without the slash, in English.

- The highlighted row (mouse hover or arrow keys) shows a black tooltip to its right with the description: `btw` = `세션에 추가하지 않고 간단한 질문하기`; `plugin` = `Claude Code 플러그인 관리`; `workflows` = `워크플로 기록 탐색 (실행 중 및 완료됨)`; `auto-mode-setup` = `자동 모드에 환경 정보를 알려주고, 선택적으로 규칙을 조정하세요.`; a skill row shows the skill's full description in English, cut after about 10 lines with `…`.
- Arrow keys move the highlight and scroll the list; the list is about 13 rows tall; mouse wheel scrolling over it moved it by only a few pixels.
- The command names that appear here and not in Forge's list were not compared row by row (only these strings were grepped: `ultrareview`, `subtask`, `auto-mode-setup`, `usage-credits`, `output-style`, `add-dir`; results in the backlog note). The user-defined commands and skills of the account are mixed in with the built-ins, with a right-hand label only for user commands.
- Not checked: what each command does when run (`subtask`, `advisor`, `autocompact`, `sandbox`, `storage`, `stats`, `usage-credits`); the tooltip on the first rows other than the ones above.

### Round 36, part 6: `/tasks` opens the background-task panel

- Typing `/tasks` turns it into a blue token in the box (Return once picks it from the list, Return twice runs it; same as the other commands). No user bubble and no model turn: the command only opened a panel `백그라운드 작업` as a FIFTH column at the right edge. Header: title, a trash icon, expand, close. The column list is the same panel as the header `…` item `백그라운드 작업`.
- Inside: a fold `완료 5` (open) with one row per background task of the session, each a grey card with a triangle, the task's description (the model's own words: `Run Tauri dev app in back…`, `Wait for Rust build to finish`, `Typecheck, build frontend, ca…`, `Check Rust side compiles`, `Install deps and inspect temp…`), and a red `실패` at the right of the one I killed. Pressing a row opens it in place: `$ <command>` in a grey code block with the colored console output (cargo's `Building [====>] 352/353`, `Finished`, `Running`) and a last line `[exited with code 144]`. Wide lines wrap with no horizontal scroll.
- With five columns open the chat column shrinks to about 190 px and is clipped (no minimum width); the header of the chat column shows only a short title and `…`. Nothing in the layout hid a panel to make room.
- Not pressed: the trash icon (clear finished tasks?), a running task's stop control (the earlier `백그라운드 작업` card in the Details panel had a stop square).

Backlog NEW-39.

### Round 36, part 7: panel reflow, fold detail, wide chat

- **Panels re-flow when one is closed.** With Changes, Files and Preview stacked in the right column and a side chat under Preview, closing Preview made the side chat take the full height of that column; its tab then showed the whole exchange (the question as a grey bubble at the right, the answer below). Closing the side chat later brought back the earlier two-column layout and the chat column grew to about 600 px (all seven header icons returned: tasks, terminal, changes, globe, phone, folder, `…`).
- **Fold line `실행된 명령 4개 (1개 실패)`** (same wording family as NEW-20) opens to one grey row per command; each row's TITLE is the description the model wrote for the call, in English (`Generated app icon set from drawn source`, `Removed unused mobile icon folders`, `Typecheck, build, cargo check`, `Ran cargo check with absolute path`); a failed one carries a red `실패` after its title. Opening a row shows `$ <command>` in a grey code block (long lines wrap), then a red line `종료 코드 1`, then the output (`✓ built in 96ms`, `(eval):cd:1: no such file or directory: src-tauri`). Rows open independently. The fold line's `실패` count counts only the rows that failed.
- **A `↓` button** (round, white, shadow) floats in the chat at the bottom centre whenever the view is above the last message.
- **Reply footer** shows `4분 전` (relative time) after copy, share, pin and read-aloud icons; it said `지금` for the newest reply.
- The scroll helper moves the chat by only a few lines per call; six calls of 60 lines moved about one screen.

### Round 36, part 8: a background task's exit starts a model turn by itself

- When I killed the dev server's processes from my shell (the `npm run tauri dev` background command of the session ended with code 144), the app put a notification into the session and the model answered WITHOUT any user message: `백그라운드 작업이 코드 144로 끝났다고 알림이 왔는데, 앱은 꺼지지 않았습니다. 방금 확인해 보니 todo-app 프로세스와 Vite 개발 서버가 둘 다 계속 실행 중입니다. 알림은 이 명령을 실행한 셸 래퍼가 종료됐다는 뜻으로 보이고…` and later `이 알림은 자동 이벤트라서 앱 동작을 확인했다는 뜻이 아닙니다.` So a finished background command is delivered to the model like a new message and starts a turn (the model even ran a check, `ps`, in that turn). The chat shows no user bubble for it; the reply starts under a fold line `완료함 백그라운드 명령어 1개, 실행됨 명령 1개 (1개 실패) ▸` (same wording family as NEW-20). Not seen: whether the notification can be read in the chat (the fold is the only trace) and whether it fires when the app has another session open.
- The model's own tool-call titles are English even in a Korean chat (`Waited for Rust build to finish`, `Running Tauri dev app in backgr…`); the fold lines and the buttons are Korean.
- Hovering the user bubble `먼저 실행해 보기` shows a row under it at the right: `✓✓ 13분 전 · ⧉ (copy) · ↺ (rewind) · fork icon`; the time is relative (`13분 전`), same as round 31.

Backlog NEW-40.

### Round 36, part 9: `/usage` card in the chat

`/usage` (Return twice) answers with a card in the chat, no model turn and no user bubble. Structure, top to bottom (numbers left out; they are the user's account):

1. Title `사용량` and a copy icon at the right.
2. Two meters: `5시간 제한` (reset time, percent, a blue bar) and `주간 · 모든 모델` (reset day and time, percent, bar); then `클라우드 세션 크레딧` (`US$100 중 US$100 남음`, an empty bar) and `만료: <date> GMT+9`.
3. Block `이 세션`: a 3-column grid, row 1 `비용 US$…`, `API <m>분 <s>초`, `활성 시간 …`; row 2 `줄 +488 −18` (green and red), `Sonnet 100%`, `캐시 히트 98%`.
4. Table `세부 내역` with the model name at the right: `입력`, `출력`, `캐시 읽기`, `캐시 쓰기` (74, 4.1k, 2.9M, 63k).
5. Block `무엇이 사용량 제한을 소모하고 있나요?` with two small selects, `24h ⌄` and `전체 ⌄`; a grey line `최근 24시간 · 근사치, 중복 측정 · 이 기기만 해당, claude.ai 제외`; three rows with a percent and a dotted-underlined label (`150k 컨텍스트 초과 실행`, `서브에이전트 집약적 세션에서 발생`, `always on context`); then rows of a name, a kind (`플러그인`, `Skill`, `서브에이전트`), a blue bar and a percent, five shown, link `10개 더 보기`.
6. `도움말` with a sentence on long sessions and cache cost, and two buttons `이 세션 압축하기` and `새 세션`.

Facts: the card is part of the chat transcript (it stays above the next-step card and scrolls with it); the bars are the only blue besides links. Forge has `crates/forge-entities` code for usage (round 35 says the usage box data is still there but unused on the new-session screen), but none of the items 3-6 were found by name. The usage-limit bar above the composer (round 31) was not visible in this session.

Not checked: the two selects (`24h`, `전체`), `10개 더 보기`, `이 세션 압축하기` (it would compact the session), `새 세션`, what the dotted underline shows on hover.

Backlog NEW-41.

### Round 36, part 10: `/status` card in the chat

`/status` answers with a card in the chat (no model turn). Groups, each a small grey heading with label and value rows (value right-aligned):

- `세션`: `Claude Code` (CLI version), `데스크톱 앱` (app version · OS version and arch), `모델` (`Sonnet 5.5 · 보통`), `권한` (`자동`), `시작` (`나 · 26분 전`: who started it and how long ago), `세션 ID` (first 8 characters + `…` and a copy icon).
- `환경`: `폴더` (full path), `MCP 서버` (`4개 연결됨 (figma, playwright, puppeteer, Google Drive) · 1개 실패 (m365)` with a link `관리` after the failure count), `샌드박스` (`꺼짐`).
- `계정`: `로그인 계정` (email and plan), `조직`.

So the model line says `보통` here while the box says `중간` for the same effort: the card and the picker word the level differently (picker `중간`, status `보통`). The card is the place where the CLI version and the account are visible. Forge's equivalents: no `/status`; the account and plan appear in the sidebar footer and Settings; the version is in the About/Developer pages. Not checked: the `관리` link target (probably Settings → 커넥터), the copy icon.

Backlog NEW-42.

### Round 36, part 11: `/stats` card and the feedback card

- **`/stats`** shows a card titled `통계` (copy icon at the right). While loading it shows grey skeleton blocks (one wide block, a 2x3 grid of bars, a second wide block, a thin line) for about 8 seconds, then fills. Under the title two selects: `개요 ⌄` (menu: `개요` checked, `모델`) at the left and `전체 기간 ⌄` (menu: `전체 기간` checked, `최근 7일`, `최근 30일`) at the right. `개요` view: a GitHub-style activity heat map (columns are weeks, month names above, weekday letters `월 수 금` at the left, 7 rows, five blue shades with a legend `적음 ▪▪▪▪▪ 많음`); a 2-column grid of facts (`즐겨 사용한 모델`, `세션`, `활성 일수 29/174`, `총 토큰`, `최장 세션 37일 20시간`, `가장 활발했던 날`); a table `토큰` with `입력`, `출력`, `캐시 읽기`, `캐시 쓰기`, `총계` for the chosen period; and a grey fun line comparing the longest session with the time to listen to a famous album (`가장 긴 세션은 … 앨범을 듣는 시간보다 약 N배 깁니다`). The data covers all sessions of the account on this computer, not only this one. Not opened: the `모델` view.
- **Feedback card** (NEW-23) appeared by itself under the last card once the session had been idle after a turn: a one-line card `이번 세션에서 Claude는 어떤가요?` with three plain text buttons `나쁨` `보통` `좋음` and a `×`, with the footer row of the last message (pin, `지금`) above it. It appeared after `/status` and `/stats`, not after the first turns. Not pressed (it would send a rating from the user's account).
- With a slash card as the last item the reply footer shows a pin icon and `지금`, i.e. the card counts as a message.

Backlog NEW-43; NEW-23 gets "seen again, shows after an idle moment" (below).

### Round 36, part 12: `/diff` is a panel toggle

`/diff` (Return twice) adds no card and no bubble to the chat: it shows or hides the `변경 사항` panel. First run: the panel was open and closed (the Files panel took its place and filled the whole right column, tree at the right); second run: the panel opened again at the top of the column. In a folder that is not a git repo the panel only says `변경 사항을 보려면 Git 저장소가 필요합니다`. So of the slash commands tried, `/tasks`, `/diff` open or toggle panels; `/usage`, `/status`, `/stats` add cards to the chat; `/btw` opens the side chat; none of the six sent a message to the model. Backlog NEW-38 (slash list) should record these three kinds of result.

### Round 36, part 13: steps 6 and 7, the ending, path links

- **Step 6 (tests)** sent by typing a message: the task list card appeared right under my bubble at once (four rows: `Vitest·Testing Library 설치 및 설정`, `useTodos 훅 테스트 작성`, `TodoItem 컴포넌트 테스트 작성`, `테스트 실행 및 결과 확인`; first row with a ring spinner, the others dashed circles) and the activity line below it (`Installing test dependencies and view configs 13초`). The composer placeholder while running: `⌘↵ 키를 눌러 대기열에 추가하세요` with a black stop square at the right. No permission card appeared in `자동` mode for any of installs, file writes or test runs. The model hit a type error in its own test (`Array.prototype.at` with the ES2020 lib), fixed it and said so under `수정한 점`, and listed `테스트하지 않은 부분`. End card: `7단계 진행 ⌘⏎` and `나중에`.
- **Step 7 (release build)**: task list of two rows, activity line `Building release app bundle 4분 30초`; the whole step took about 7 minutes. The reply has headings (`배포 전에 알아둘 점`, `전체 진행 상황`) and bullets. End card: `앱 실행 확인 ⌘⏎` and `이대로 끝`; the footer under it shows copy, fork, pin, read-aloud and `지금`; the one-line feedback card sits below.
- **`이대로 끝`** sent the label as a user bubble and the model gave a closing summary: where the project is, the two output files as path links, what is still unchecked (three bullets), a hand-over note. No end-of-session screen, no automatic archive; the session stays active.
- **Path links in the summary**: a path with an extension (`…/Todo_0.1.0_aarch64.dmg`) is a blue underlined link; the `.app` path was a link too. Pressing the `.app` link did nothing visible (no panel opened, no error) and the link turned into plain code style afterwards, so a link is checked against the disk on press and falls back to plain text when it is a folder or missing. The `.dmg` link was not pressed (a binary).
- Files panel facts: the folder `todo-app` stays open in the tree after the build, and `target`/`dist` are listed like any folder; no change counters (not a git repo).

Backlog NEW-44 (links that fall back to code style), NEW-45 (no permission cards in auto mode: Forge already has auto, check parity of what auto allows).

### Round 36, part 14: account menu and the Settings window pages

Method as before. Account menu (press the name at the bottom left): the email at the top (grey), `설정 ⌘,`, `사용량`, `언어`, `도움 받기`; a line; `플랜 업그레이드`, `앱 및 확장 프로그램 다운로드`, `게스트 패스 공유`, `변경 로그 보기`, `더 알아보기 ›`; a line; `API 키 받기` (external-link icon), `로그아웃`. `더 알아보기 ›` opens a submenu: `Anthropic에 대하여 ↗`, `Claude Academy`, `사용 정책 ↗`, `개인정보 처리방침 ↗`, `이용약관 ↗`, `글꼴 라이선스 ↗`, `개인정보 보호 선택`, a line, `단축키 ⌘/`. `사용량` in that menu opens the Settings window on the `사용량` page (not a popover). `⌘,` is not reachable with the driver's key list (no `comma`).

Settings window: a modal over the app (about 83% of the window wide), title `설정` and `×` at the top, a search field `검색`, a left list in three groups: `환경설정` (일반, 계정, 개인정보보호, 결제, 사용량, 기능, 메모리, 디자인 시스템, Claude Code, Claude in Chrome), `이 컴퓨터` (시스템, 확장 프로그램, 개발자), `사용자 지정` (Skills, 커넥터, 플러그인). Pages seen now (Claude Code, 커넥터, 플러그인, Skills, 개발자 were seen in rounds 32-33, 38):

- **일반**: `화면 모드`: `테마` (three icon buttons: system, sun, moon), `인터페이스 글꼴` (select, `기본값 · 시스템`), `응답 글꼴` (`인터페이스와 맞추기`), `트랜스크립트 텍스트 크기` (`보통`), `트랜스크립트 너비` (`좁게`), `모션` (`시스템`). `작업`: `신뢰할 수 있는 폴더` + `관리` (text says trusting a folder does not add it to new sessions; re-add it here when a prompt asks), `기본 브라우저` (`내장 브라우저`). `음성`: `언어` (`한국어`), `스타일` (`선택`), `속도` (`보통`). `알림`: switches `응답 완료`, `예약된 작업`, `Code 알림`, `Code 권한 요청` (push when a command needs approval), `Claude Code 클라우드 세션 이메일`.
- **사용량** (`Pro` pill): a bold warning line (`이 속도라면 수요일 초기화 전에 화요일 오전에 사용량이 모두 소진됩니다`), buttons `플랜 업그레이드` (black) and `사용 크레딧 구매`; meters `현재 세션` (reset time, amber bar at 83%, `83% 사용됨`; the bar is AMBER at high use, blue otherwise) and `이번 주` (blue, `52% 사용됨`); `제한 초기화` (`현재 초기화 없음`); `클라우드 세션 크레딧` with `?`; `포함된 크레딧` bar; `사용 크레딧 US$0` with a switch to keep going past the plan limit.
- **기능**: `일반` (`도구 액세스 모드` select `필요할 때 도구 로드`, switches `커넥터 검색`, `메시지가 플래그 지정되면 모델 전환`), `비주얼` (`아티팩트` forced on with the note `코드 실행 및 파일 생성에 필요`, `AI 기반 아티팩트`, `채팅 내 시각화`), `코드 실행 및 파일 생성` (`클라우드에서 코드 실행`), more below.
- **메모리**: switches `채팅 검색 및 참조`, `채팅에서 메모리 생성`, `민감한 주제를 메모리에 포함`; `다른 AI 제공업체에서 메모리 가져오기` + `가져오기 시작`; `메모리 내보내기` + `내보내기` (zip of Markdown files); then the memory itself as lists `나` / `주제` (name, one-line description, `<date> 업데이트됨`), and a box at the bottom `Claude에게 변경할 내용을 알려 주세요` with a send arrow: memory is edited by talking to Claude.
- **디자인 시스템**: title, a sentence, `+ 새 디자인 시스템 ⌄`, a search `이름으로 검색`, table headers `이름` / `사용 가능 대상 ⓘ` / `게시일`, empty text `아직 디자인 시스템이 없습니다…`.
- **Claude in Chrome**: switch `Claude in Chrome 사용 설정`; `사이트 권한` (`이 권한은 Claude in Chrome과 Claude 데스크톱 앱의 내장 브라우저에 적용됩니다`), `모든 사이트 기본값` select `기본 정책 선택`.
- **시스템**: `데스크톱 앱 버전`, `시작 시 실행`, `빠른 액세스 바로가기` (select `사용자 설정…` plus a key field showing `⇧⌘ Space` with a clear `×`), `음성 단축키`, `음성 입력 단축키` (`Command+D`), `메뉴 바`, `컴퓨터 절전 모드 방지` (long text, display may still sleep, closing the lid still sleeps), `저장 공간` + `관리`, `저장 폴더` (path link, button `권장 폴더 사용` with a warning icon, button `변경`), `Dispatch` `베타` (phone sends tasks to this computer).
- **확장 프로그램**: `확장 프로그램 찾아보기`; `컴퓨터에 설치됨` list (one row: icon, name, `구성`, `…`); `고급 설정` button; a hint `.MCPB 또는 .DXT 파일을 여기에 드래그해서 설치하세요`.

String search of Forge's `ko.rs` (English keys) found none of: `Transcript width`, `Transcript text size`, `Response font`, `Interface font`, `Motion`, `Menu bar`, `Quick access`, `Dispatch`, `Default browser`, `Tool access mode`, `Connector search`, `Memory`, `Design system`; only `Trusted folders` exists. Many of these are the account/Claude.ai side and have no Forge meaning (billing, memory, design systems, Dispatch, Chrome).

Backlog NEW-46 (display settings that may apply to Forge: transcript text size, transcript width, fonts, motion), NEW-47 (memory page).

### Round 36, part 15: Settings search, privacy page, close

- **Settings search**: typing `단축키` in the left search field opens a result list under the field, grouped by page: a page row with its icon (`시스템`, highlighted) and indented setting rows under it (`음성 단축키`, `음성 입력 단축키`) with the matched letters in blue. The page behind did not change while typing. Pressing a setting row opened the page (`시스템`) and showed it from the top (no scroll to the row and no highlight that I could see; the left list stayed on `일반` — the left highlight did not follow). The search field cleared. `Esc` closed the whole Settings window (the focus then sat on the account row at the bottom left with a blue ring).
- **개인정보보호** page: a paragraph with links, two rows with `›` (`데이터를 보호하는 방법`, `데이터 사용 방법`), `환경설정`: `위치 메타데이터` (switch on) and `Help improve our AI models` (English, switch off); `내 데이터`: rows with a button each: `데이터 내보내기`, `공유된 채팅`, `공유된 아티팩트`, `업로드한 파일`, `내 피드백`. Account-side, not for Forge.
- **Page-level facts**: the Settings left list highlight after a search jump is not updated (it kept `일반` while the content was `시스템`).

Backlog NEW-48 (Settings search grouped by page).

### Round 36, part 16: `자동 모드 켜기` answered, the environment menu and `새 환경` dialog

- **`자동 모드 켜기`** (the row under the environment card, NEW-27) pressed: the mode label under the box changed from `수동` to `자동` and the row disappeared at once; the card kept one row `Claude Code CLI 세션 3개 가져오기`. No dialog, nothing sent. So the row is a one-press shortcut for the same mode setting as the mode menu, and it exists only while the mode is not auto. The default mode did follow the folder earlier (round 36 part 1: `수동` for a folder that is not a git repo; here a new session in the same environment started as `수동` again).
- **`⌘N`** opens the new-session screen with the LAST used environment checked (`forge-observe`) and the mode back to `수동`: the mode is not remembered across new sessions.
- **Full environment menu** (`환경 선택 ⌄`): search field `환경 검색…`; group `로컬`: `이 컴퓨터`, `forge-observe` (checked, blue), `forge`, `더 보기 | 1 ›`; a line; `클라우드 ›` and a line; `+ 새 환경`. `클라우드 ›` opens a side menu: `기본값` (cloud icon) and `클라우드 환경 추가…`.
- **`새 환경` dialog** (a modal over a dimmed window, `×`): four tabs at the top, `로컬` (default), `클라우드`, `SSH`, `Remote Control`; footer `취소` and a primary button (disabled until valid).
  - `로컬`: `이름` (placeholder `내 앱`), `폴더 (선택 사항)` with a `+ 추가` button and a dashed empty box `세션을 시작할 폴더를 선택하세요.`, a row `환경 변수` (`이 환경에는 없음`, `›`); button `환경 만들기`. `+ 추가` opens a list of recent folders by name (`forge-observe`, `develop`, `forge`, `task-r31`, …; no paths) and a last row `📁 선택…` (the file dialog).
  - `클라우드`: `이름`, then four rows with `›`: `네트워크 액세스` (`제한됨 · 도메인 없음 · 패키지 관리자 켜짐`), `네트워크 시크릿` (`없음`), `설정 스크립트` (`설정되지 않음`), `환경 변수`; button `환경 추가`.
  - `SSH`: `이름`, `호스트` (select `호스트를 선택하세요`, from ssh config presumably), `폴더` with `+ 추가` disabled and `먼저 호스트를 선택하세요.`, `환경 변수`; button `환경 만들기`.
  - `Remote Control`: a card `remote control 설정`: `여기서 코딩하려면 내 컴퓨터에서 claude rc를 실행하세요.` and `이 컴퓨터에서 Remote Control 기능이 꺼져 있으므로 …` with a link `Remote Control 설정 열기` and a button `더 알아보기 ↗`; only `취소` in the footer.
  - I pressed `취소` at the end; nothing was created.

Forge side (string search of `ko.rs`): `Environment variables` exists; `Remote Control`, `Network access`, `Setup script`, `Network secrets`, `SSH`, `Add cloud environment` do not. Forge's own `새 환경` (Plan 53) has the local kind with name, folders and variables; the other three tabs are cloud, remote and phone features that do not apply, except that the recent-folder list in `+ 추가` and the empty-state texts may be worth matching. Backlog NEW-49.

Not checked: the file dialog behind `선택…`, `더 보기 1 ›`, the `네트워크 액세스` row dialog, `클라우드 환경 추가…`.

### Round 36, part 17: sidebar after the app update, filter menu

The desktop app is version 4.4212.0 (Settings → 시스템). The sidebar differs from rounds 3 and 32:

- **Sidebar top** now reads `새로 만들기`, `검색`, `프로젝트`, `아티팩트`, `예약됨`, `Dispatch 베타`, `사용자 지정` in the Chat (left of the two top-left mode buttons) mode; the Code mode shows `새로 만들기`, `검색`, `아티팩트`, `예약됨`, `사용자 지정`, `더 보기` (a `…` row). The two top-left buttons are a mode switch: a chat-bubble-with-lens (Chat/Cowork home) and `</>` (Code); pressing the first opened a home page `Hyunseung님, 다시 돌아왔군요` with a big box `오늘 어떤 도움을 드릴까요?`, `프로젝트 또는 폴더 ⌄`, `Sonnet 5.5 중간`, `수동`, a `최근 아티팩트` row (three cards with a thumbnail, title and date; `모두 보기`) and company legal text at the bottom. The sidebar then shows `고정됨` and `최근 항목` groups. Pressing `</>` returned to Code mode.
- **`더 보기`** (Code mode) opens a small popover to the right of the row: one item, `사이드바 편집…` (round 32: three checkboxes).
- **Project group header filter menu** (hover the header: a filter icon and a `+` appear at its right; the header row gets a grey background): `그룹화 기준 환경 ›`, `정렬 기준 마지막 업데이트 ›`, `표시 ›`, a line, `새 섹션`, a line, `기본값으로 재설정`, `모두 보기`.
  - `그룹화 기준` submenu: `날짜`, `상태`, `폴더`, `환경` (checked, blue), a line, `없음`. (`사용자 지정 그룹` of round 3 is gone; `새 섹션` replaces it, and `환경` is the new default.)
  - `정렬 기준` submenu: `마지막 업데이트` (checked), `생성 날짜`, `이름`; a line; `내림차순` (checked), `오름차순`.
  - `표시` submenu: `메타데이터` (checked), a line, `사이드 채팅`, `빈 그룹` (both unchecked). So side chats are listed in the sidebar only when this is on, and the old `PR 상태 표시` is now part of `메타데이터`.
  - `새 섹션` adds a user-named group; `기본값으로 재설정` resets group, sort and show; `모두 보기` shows all items in the list (the list is cut to a count with `더 보기` otherwise).
- Group headers show the environment name (`claude-code-study`, `features`, `forge-obs-r26`, a branch line `claude/create-file-b-txt-6eeb0c` under a session in a worktree), with a triangle `▾` at the right of the name.

Compared with Forge by string search: only part of this was checked (see the backlog row). Backlog NEW-50.

### Round 36, part 18: recovery screen of a session whose folder and transcript are gone (SES-09)

My third click in the `표시` menu missed and opened the session `Debug and identify error cause` (an old session in `claude-code-study`; the `사이드 채팅` toggle was NOT confirmed, so its effect on the sidebar is still unseen). The page shows what SES-09 describes:

- Middle: `디스크에서 세션을 찾을 수 없음` (bold) and a grey line `이 디렉터리에서 새로 시작하려면 메시지를 보내세요.`, then three text buttons: `CLI 세션 가져오기` (grey fill), `보관`, `삭제`.
- Where the box should be, a grey bar instead: `작업 폴더가 더 이상 존재하지 않습니다. 계속하려면 다른 폴더를 선택하세요.` with buttons `폴더 선택` and `보관`. There is no composer, no model or mode row.
- The Details panel opened by itself at the right (the header's first icon highlighted) with the new layout of `환경`: a row `이 컴퓨터` with a link `저장` (save this environment?) at the right, a row with the folder icon `claude-code-study · ~/Developer` (path cut), and a row `Remote Control` with a switch (off); `컨텍스트` says `컨텍스트 사용량을 확인하려면 메시지를 보내세요.`; `커넥터` shows a grey loading bar.
- The chat header shows only title and the same panel icons (details, terminal, changes, globe, folder, `…`).
- Not pressed: all five buttons (`삭제` is destructive; `보관` changes the user's list; `CLI 세션 가져오기` and `폴더 선택` open dialogs). Forge treats such a session as view-only (SES-09 is "won't" in the backlog); this screen shows what the desktop app offers in its place.

Note for NEW-50: `환경` in the Details panel now carries a `저장` link and a `Remote Control` switch; Forge's environment card (Plan 53) has neither.

### Round 36, part 19: `저장` in the Details panel saves the folder as an environment

Pressing `저장` (the link at the right of `이 컴퓨터` in the Details panel's `환경` card, shown when the session's folder is not yet a saved environment) opened the `새 환경` dialog already filled in for the local kind (no tabs this time, title `새 환경`): `이름` holds the folder name selected (`claude-code-study`, text highlighted so typing replaces it), `폴더` lists that folder with a grey tag at the right (`Git 저장소가 아님`; for a git repo it would say the branch or `Git`), `환경 변수` (`이 환경에는 없음`), and a BLACK enabled button `환경 만들기`. Pressing `취소` closed it; no environment was created.

So `저장` answers where saved environments come from: a folder first used in a session is a bare path, and this link promotes it to a named environment with one dialog (the same dialog as `새 환경`, prefilled). The environment card's rows in later sessions then show it by name. Forge's stage C (Plan 53) has `폴더 추가…`/`폴더 변경…`; it has no one-press `저장` for the current folder (to check). Backlog NEW-51.
