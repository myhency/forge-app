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
