---
title: 작성창 통일과 데스크톱 앱 관찰 정리 - Plan
type: feat
date: 2026-10-31
topic: composer-unify-desktop-parity
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-plan-bootstrap
execution: code
---

# 작성창 통일과 데스크톱 앱 관찰 정리 - Plan

## Goal Capsule

- **Objective:** 새 세션 화면과 세션 안의 작성창이 같은 모양과 같은 자리 구성이어서, 화면을 옮겨 다녀도 입력창이 다른 물건처럼 보이지 않는다. 그 밖의 화면도 Claude 데스크톱 앱과 나란히 놓고 달라진 곳을 찾아, 확인된 차이만 고친다. 결과가 "AI가 만든 UI" 같은 장식(여러 겹의 알약, 굵은 색 면, 이유 없는 그림자)으로 보이지 않는다.
- **Means:** 두 작성창이 하나의 공용 셸을 쓴다(KTD1). 컨트롤은 상자 밖 아래 줄로 내린다(KTD2). 나머지 화면은 후보를 두 앱 화면으로 다시 비교해 확인된 것만 고친다(KTD4).
- **Product authority:** 이 대화의 사용자 요청("새 세션의 message input과 session 내의 message input의 UI 차이가 나는데 맞춰줘", "desktop app 관찰을 통해서 개선", "ai가 만든 것처럼 보이는 ui로 만들면 안돼"). 데스크톱 앱과 같은 상호작용을 유지한다는 기존 방침(UI parity).
- **Target repo:** Forge 저장소, 현재 브랜치(`myhency/develop`)에서 갈라지는 기능 브랜치. 아래 경로는 모두 저장소 기준이다.
- **Execution profile:** 하나의 PR. 유닛 순서대로 커밋하고 각 유닛 뒤에 앱이 빌드되고 테스트가 통과한다.
- **Finish:** 구현, 검증, PR 마무리는 호출한 파이프라인(`lfg`)이 한다.
- **Open blockers:** 없음.

---

## Product Contract

### Summary

세션 안의 작성창(`.composer` 카드: 큰 입력칸, 안쪽 아래줄의 컨트롤, 글자 버튼 "보내기")과 새 세션의 작성창(맨 입력칸 + 안쪽 보내기 아이콘, 위에 테두리 알약 칩)을 데스크톱 앱처럼 하나로 합친다. 한 줄 알약 상자에 왼쪽 `+`, 가운데 입력, 오른쪽 보내기(또는 멈춤) 아이콘을 두고, 모델·노력·모드·컨텍스트 링은 상자 바로 아래 오른쪽에 테두리 없는 작은 회색 글자로 둔다. 새 세션의 환경과 폴더는 같은 줄 왼쪽에 아이콘+글자로 둔다. 기능, 단축키, IME, 큐, 곁질문, 멘션, 제안 글, 카드는 그대로다.

### Problem Frame

같은 일(메시지를 쓴다)을 하는 입력창이 두 곳에서 다르게 생겼다. 새 세션은 위쪽에 테두리 있는 알약 칩 둘, 맨 입력칸, 칸 안쪽 모서리의 작은 보내기 아이콘, 칸 아래의 컨트롤 줄이다. 세션 안은 하나의 카드 안에 입력과 컨트롤과 빨간 "보내기" 글자 버튼이 모두 들어 있다. 같은 앱의 두 화면이 서로 다른 문법을 쓴다. 데스크톱 앱은 두 화면 모두 한 줄 알약이고 컨트롤은 밖에 있다.

### Observed facts (이번 실행에서 두 앱을 직접 본 것)

데스크톱 앱(Claude.app, 한국어, 새 세션과 세션 안 둘 다 확인, 글을 쓴 상태도 확인):

- 상자: 한 줄 높이의 알약. 1px 옅은 테두리, 큰 둥근 모서리, 거의 그림자 없음. 왼쪽 `+`, 가운데 입력(새 세션은 "작업을 설명하거나 질문하세요", 세션 안은 "/ 입력 시 명령어"), 오른쪽 마이크 아이콘. 글을 쓰면 마이크 왼쪽에 `↵` 모양의 보내기 아이콘이 생긴다. 색 면 버튼이 없다.
- 상자 아래 한 줄: 오른쪽에 `모델 이름 · 노력 · 모드 · 링`이 테두리 없는 작은 회색 글자(링은 작은 원). 새 세션에서는 같은 줄 왼쪽에 `이 컴퓨터`와 폴더 이름이 아이콘+글자로 있다. 알약 테두리가 없다.
- 새 세션 상자는 화면 아래쪽에 가운데 정렬된 열이고 세션 안 상자와 같은 폭이다. 환경 선택 목록은 상자 위로 열린다.
- 사이드바: 날짜 열이 없고 제목만(브랜치가 있으면 둘째 줄에 브랜치). 선택 행은 옅은 회색 면. 그룹 제목은 작은 회색 글자와 ▾이며, 그룹에 마우스를 올리면 오른쪽에 필터와 `+`가 나온다. 크고 색이 진한 "새 세션" 버튼은 없다.
- 세션 머리말: 제목과 ⌄ 하나. 폴더 알약은 없다(폴더와 환경은 오른쪽 패널의 환경 카드).

Forge 현재(같은 하네스 화면에서 확인): 위 "Problem Frame"의 모양, 사이드바에 날짜 열과 굵은 제목, 크고 빨간 "+ 새 세션" 버튼, 머리말에 폴더 알약, 위쪽에 `일반/사고 과정/자세히` 세그먼트.

### Requirements

**작성창 통일**

- R1. 새 세션과 세션 안의 작성창은 같은 셸(같은 마크업과 CSS 클래스)을 쓴다. 두 화면의 상자는 폭, 높이, 테두리, 모서리, 그림자, 안쪽 여백이 같다.
- R2. 상자 안에는 왼쪽 `+`(기존 `PlusMenu`), 입력, 오른쪽 동작 아이콘만 있다. 동작 아이콘은 실행 중이 아닐 때 보내기 아이콘(글이 있거나 첨부가 있어 보낼 수 있을 때만 활성), 실행 중일 때 멈춤 아이콘이다. 색 면 글자 버튼 "보내기"는 없어진다. 접근성 이름("보내기", "중지")과 단축키는 그대로다.
- R3. 입력칸은 한 줄로 시작하고 글이 길어지면 지금처럼 늘어난다(최대 높이와 스크롤 포함). 알약 모서리는 한 줄일 때 알약이고 여러 줄이면 같은 반지름의 둥근 사각형으로 이어진다.
- R4. 모드, 모델, 노력, 컨텍스트 링은 상자 밖, 바로 아래 줄 오른쪽에 둔다. 테두리와 배경이 없는 작은 회색 글자이고 순서는 모델, 노력, 모드, 링이다. 각 컨트롤의 메뉴와 동작은 그대로다. 이 줄의 모양은 두 화면에서 같다.
- R5. 새 세션의 환경, 폴더, 브랜치, 폴더 추가는 상자 위 알약이 아니라 같은 아래 줄의 왼쪽에 아이콘+글자로 둔다. 테두리와 배경 알약은 없다. 메뉴는 지금처럼 위로 열리고 선택, 단축키, 닫힘 후 입력창 포커스 복귀(`refocus_composer`)는 그대로다.
- R6. 두 화면의 상자 열 폭은 같다(`--chat-column`). 새 세션의 상자는 화면 아래쪽에 붙고, 창이 좁거나 낮을 때 지금의 줄어듦과 스크롤 규칙(`.chat-narrow`, `.new-session-pane`)이 유지된다.
- R7. 입력창 위에 붙는 카드(권한, 질문, 대기열, 칩, 신뢰, 곁질문, 멘션 목록)와 오류 배너는 지금 자리와 폭을 유지한다.
- R8. 포커스 표시는 PR #2와 PR #3의 기준을 유지한다(상자 `:focus-within`에서 얇은 accent 테두리와 옅은 번짐, 고대비 모드 윤곽선).

**나머지 화면**

- R9. 다음 후보는 두 앱의 같은 화면을 나란히 다시 본 뒤 "데스크톱 앱이 다르게 한다"가 확인된 것만 고친다. 확인되지 않거나 Forge가 의도해서 둔 기능이면 고치지 않고 PR 설명에 근거와 함께 적는다: (a) 사이드바 날짜 열과 굵은 제목, (b) 크고 빨간 "+ 새 세션" 버튼, (c) 세션 머리말의 폴더 알약, (d) 위쪽 `일반/사고 과정/자세히` 세그먼트. 후보 목록에 없는 차이도 같은 방식으로 찾아 같은 기준으로 처리한다.
- R10. 라이트와 다크 모두에서 확인한다. 새 색이나 그림자 값을 만들지 않고 기존 토큰을 쓴다.
- R11. 기능 동작, 단축키, 메뉴 구조, 번역 문장 표(한국어와 영어)는 바뀌지 않는다. 글자 모양을 단정하던 테스트가 있으면 새 모양에 맞게 고친다.

### Acceptance Examples

- AE1. **Covers R1, R2, R4, R6.** 새 세션 화면과 세션 안 화면을 같은 창 크기로 캡처해 겹쳐 보면, 상자의 가로 위치, 폭, 높이, 테두리, 모서리가 같고 아래 줄의 모델, 노력, 모드, 링이 같은 자리에 같은 모양으로 있다.
- AE2. **Covers R2.** 글이 없을 때 보내기 아이콘은 비활성(옅은 색)이고, 글을 쓰면 활성이 된다. Enter로 보내진다. 실행 중에는 같은 자리에 멈춤 아이콘이 나오고 누르면 중단된다.
- AE3. **Covers R5.** 새 세션에서 `이 컴퓨터`와 폴더 이름이 알약 테두리 없이 아래 줄 왼쪽에 있고, 누르면 목록이 위로 열린다. 항목을 고르면 입력창에 포커스가 돌아온다.
- AE4. **Covers R3.** 긴 글을 붙여 넣으면 상자가 위로 자라고 최대 높이에서 스크롤한다. 지웠을 때 한 줄로 돌아온다.
- AE5. **Covers R9.** 후보 (a)~(d) 각각에 대해 PR 설명에 데스크톱 앱과 Forge 화면 비교와 "고침" 또는 "고치지 않음과 이유"가 있다.

### Scope Boundaries

- 마이크(음성 입력) 아이콘은 만들지 않는다. 데스크톱 앱의 기능이고 Forge에는 해당 기능이 없다.
- 기능 추가나 제거는 하지 않는다. 데스크톱 앱에 없는 Forge 기능(곁질문, 큐, 제안 글 등)은 모양만 새 셸에 맞춘다.
- 색 토큰, 글꼴, 간격 토큰의 값은 바꾸지 않는다.
- 오른쪽 패널(환경, 컨텍스트, 커넥터 카드)과 새 화면의 추가는 이 계획에 없다.
- Windows 확인은 하지 않는다(맥에서만 확인, PR에 적는다).

### Assumptions

- "새 세션과 세션 안의 UI 차이를 맞춰달라"는 방향은 데스크톱 앱 모양(한 줄 알약 + 밖의 컨트롤 줄)으로 합치는 것이다. 근거: 사용자가 데스크톱 앱 관찰을 기준으로 삼으라고 했고, 관찰에서 두 화면이 그렇게 같다. 두 Forge 모양 중 하나를 고르는 것이 아니다.
- 사이드바와 머리말 후보는 사용자가 "나머지 부분도 관찰해서 개선"이라고 요청한 범위에 든다. 단 제거가 기능 접근성을 줄이면 R9의 "고치지 않음"으로 처리한다.

---

## Key Technical Decisions

- **KTD1. 공용 셸 컴포넌트.** `crates/forge-features/src/composer_shell.rs`(새 파일)에 `ComposerShell`을 둔다. 슬롯은 `leading`(`+`), `input`(`CommandTextArea`), `action`(보내기 또는 멈춤 아이콘), `below_start`(새 세션의 환경, 폴더 등), `below_end`(모델, 노력, 모드, 링). 두 화면은 이 셸만 쓰고 자기 클래스(`.composer`, `.composer-box`)를 따로 스타일하지 않는다. 이유: 같은 모양을 CSS 두 벌로 유지하면 다시 어긋난다. 기각한 안: 두 화면의 CSS만 맞추기. 이유: 마크업이 달라 안쪽/바깥 컨트롤 구조가 맞지 않는다.
- **KTD2. 컨트롤은 상자 밖.** 데스크톱 앱이 그렇게 하고, 한 줄 알약이 성립하려면 컨트롤이 안에 없어야 한다. 기존 `SendMessageBox`의 `children`을 `leading`과 `below_end`로 나누어 호출부(`chat_window.rs`, `new_session_panel.rs`)가 슬롯별로 넘긴다. 컨트롤 줄에서 메뉴는 위로 열린다(지금과 같은 `.picker-menu` 위치 규칙, 줄이 상자 밖이어도 메뉴가 상자를 가리지 않도록 확인).
- **KTD3. 보내기는 아이콘.** `new_session.rs`의 `send-icon` 방식(기존 `IconKind::Send`)을 두 화면이 쓴다. 활성 판정은 기존 `send_ready`와 `can_send`를 그대로 쓴다. 세션 안의 `Button { Primary }` 보내기는 없앤다. 멈춤은 기존 `stop-button`의 모양을 같은 크기의 아이콘 버튼으로 맞춘다.
- **KTD4. 관찰 우선, 확인된 것만 수정.** 후보 목록은 가설이다. 구현자는 각 후보를 두 앱 화면으로 다시 확인하고(Forge: `scripts/harness/capture.py`; Claude.app: `screencapture -o -x -l<윈도우ID>`, 윈도우 ID는 Quartz `CGWindowListCopyWindowInfo`로 찾는다) 차이가 확인된 것만 고친다. 기능 접근성을 줄이는 제거(예: 큰 새 세션 버튼)는 대체 경로(⌘N, 그룹의 `+`)가 Forge에 이미 있는지 코드에서 확인한 뒤에만 한다. 없으면 모양만 낮춘다.
- **KTD5. 순서.** U1(셸, 새 세션과 세션 안 동시 전환) → U2(아래 줄 컨트롤과 환경 칩) → U3(나머지 화면 관찰과 수정). U1과 U2는 한 화면에서 같이 보이므로 U1 커밋 시점에 두 화면 모두 깨지지 않아야 한다.

---

## Implementation Units

### U1. 공용 작성창 셸

- **Goal:** R1, R2, R3, R6, R8. 두 화면의 상자가 같은 셸이 된다.
- **Files:** `crates/forge-features/src/composer_shell.rs`(새), `crates/forge-features/src/lib.rs`(내보내기), `crates/forge-features/src/send_message.rs`, `crates/forge-features/src/new_session.rs`, `crates/forge-shared/src/theme.css`(`.composer`, `.composer-box`, `.composer .textarea`, `.send-icon`, `.stop-button`, `.chat-narrow .composer`, `.composer:focus-within` 규칙), `crates/forge-widgets/src/chat_window.rs`, `crates/forge-widgets/src/new_session_panel.rs`.
- **Approach:** 슬롯 구조는 KTD1. `.composer`는 가로 flex(`+`, 입력, 동작), 입력은 `flex: 1`에 테두리 없는 투명 배경. 알약 반지름은 한 줄 높이의 절반, 여러 줄이면 같은 값을 유지해 둥근 사각형이 되게 한다(상자가 자라면 `align-items: flex-end`로 `+`와 동작 아이콘이 아래에 붙는다). 실행 중 멈춤과 보내기는 같은 자리의 같은 크기. 기존 `onkeydown`(⌘U, 제안 수락), 카드와 칩 슬롯의 위치(R7), 포커스 규칙(R8)은 그대로 옮긴다.
- **Patterns to follow:** 기존 `CommandTextArea` 사용법(`send_message.rs`, `new_session.rs`), `send-icon`/`stop-button` 마크업, PR #3의 `:focus-within` 규칙.
- **Test scenarios:** 보내기 활성 판정은 기존 `send_ready` 단위 테스트와 `can_send` 분기를 그대로 통과한다. 셸이 슬롯을 받는 순수 함수(예: 동작 아이콘 종류를 `running`, `ready`에서 고르는 함수)가 있으면 단위 테스트: 보낼 수 없음→비활성 보내기, 보낼 수 있음→활성 보내기, 실행 중→멈춤. 하네스 확인: AE1, AE2, AE4.
- **Verification:** `cargo test`로 두 crate 통과. 하네스 스크린샷 두 장(새 세션, 세션 안)을 같은 창 크기로 비교.

### U2. 아래 줄 컨트롤과 환경 칩

- **Goal:** R4, R5. 컨트롤이 상자 밖 아래 줄로 가고 환경 알약이 사라진다.
- **Files:** `crates/forge-widgets/src/new_session_controls.rs`, `crates/forge-widgets/src/environment_chip.rs`, `crates/forge-widgets/src/composer_controls.rs`(모드, 모델, 노력, 링의 버튼 모양과 순서), `crates/forge-widgets/src/chat_window.rs`, `crates/forge-widgets/src/new_session_panel.rs`, `crates/forge-shared/src/theme.css`(`.env-chips`, `.picker-button`, `.chip-button`, `.picker-end`, `.composer-row`).
- **Approach:** 아래 줄은 `below_start`와 `below_end`를 양 끝에 둔 한 줄. 환경과 폴더 선택 버튼은 `.picker-button`의 기본 모양(테두리 없음, 회색 글자, 호버 때만 면)을 쓰고 `.env-chips`의 알약 덮어쓰기를 지운다. 폴더와 환경 앞에 SVG 아이콘(기존 `forge_shared::Icon`, 필요하면 `IconKind` 추가는 기존 선 스타일을 따른다). 모델, 노력, 모드, 링 순서로 정렬한다. `.picker-end .picker-menu`와 메뉴가 열릴 때 가려지는 영역을 확인한다.
- **Test scenarios:** 기존 폴더 메뉴 행 구성(`folder_menu_rows`)과 `PickerItem` 테스트는 그대로 통과한다. 하네스: AE3, 환경 선택 목록이 위로 열리고 닫힌 뒤 입력창에 포커스가 있다(`refocus_composer`).
- **Verification:** 하네스 스크린샷(라이트와 다크), 메뉴를 연 상태 포함.

### U3. 나머지 화면 관찰과 수정

- **Goal:** R9, R10. 후보 (a)~(d)와 새로 찾은 차이를 확인된 것만 고친다.
- **Files:** 후보별로 확인한 뒤 정한다. 예상: `crates/forge-widgets/src/sidebar*.rs`(행 모양), `crates/forge-widgets/src/chat_window.rs`(머리말), `crates/forge-shared/src/theme.css`(`.sidebar-item`, `.pane-tag`, `.view-switch`, 새 세션 버튼 규칙).
- **Approach:** KTD4. 후보마다 (1) 데스크톱 앱 화면, (2) Forge 화면, (3) 차이, (4) 고침 또는 고치지 않음과 이유를 PR 설명에 적는다. 구현 전에 각 후보가 Forge의 의도된 결정인지 `docs/superpowers/roadmap/`, `docs/plans/`에서 확인한다. 사이드바의 날짜 열 제거 같은 변경은 정렬 기준(최근 활동)을 보여 주는 정보를 없애는지 확인하고, 대체 표시(호버 툴팁 등)가 필요한지 판단한다.
- **Test scenarios:** 행 모양을 단정하던 기존 테스트(있으면)를 새 모양에 맞게 고친다. 하네스 비교 스크린샷.
- **Verification:** 라이트와 다크 스크린샷 비교, `cargo test`, 번역 표 테스트.

---

## Risks and Mitigations

- **알약이 여러 줄로 자라는 동작**이 CSS만으로 어색할 수 있다. 완화: 반지름을 고정하고 상자가 자랄 때 `+`와 동작을 아래에 붙인다. AE4로 확인한다.
- **컨트롤 줄로 내린 메뉴**가 상자나 창 아래에 잘릴 수 있다. 완화: 메뉴는 위로 열린다. 새 세션이 창 아래에 붙어 있을 때 하네스로 확인한다.
- **좁은 창**에서 아래 줄(환경, 폴더, 모델, 노력, 모드, 링)이 넘칠 수 있다. 완화: 줄바꿈을 허용하고 `.chat-narrow`에서 확인한다. 기존 반응형 규칙(Plan 26)을 깨지 않는다.
- **IME**(한국어 조합 중 Enter)는 `ime-guard.js`와 `CommandTextArea`에 달려 있으므로 입력칸 마크업을 바꿀 때 그 요소와 속성을 그대로 유지한다.

---

## Sources / Research

- 두 앱의 직접 관찰(위 Observed facts): Claude.app(한국어)의 새 세션, 세션 안, 글을 쓴 상태. Forge는 `scripts/harness/`로 같은 창 크기에서 캡처.
- `crates/forge-features/src/send_message.rs`, `new_session.rs`, `crates/forge-widgets/src/chat_window.rs`, `new_session_panel.rs`, `new_session_controls.rs`, `crates/forge-shared/src/theme.css`(`.composer` 281행, `.composer-box` 823행, `.env-chips`, `.send-icon`, 포커스 규칙 1484행).
- 앞선 계획: `docs/plans/2026-10-31-0932-feat-ui-polish-design-system-plan.md`, `2026-10-31-1040-feat-softer-field-focus-and-icons-plan.md`(포커스와 아이콘 기준).
