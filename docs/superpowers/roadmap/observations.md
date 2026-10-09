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
