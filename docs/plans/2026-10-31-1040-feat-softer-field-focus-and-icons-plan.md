---
title: 입력창 포커스 완화와 이모지 아이콘 교체 - Plan
type: feat
date: 2026-10-31
topic: softer-field-focus-and-icons
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-plan-bootstrap
execution: code
---

# 입력창 포커스 완화와 이모지 아이콘 교체 - Plan

## Goal Capsule

- **Objective:** 글자를 입력하는 칸에 포커스가 가도 화면이 시끄럽지 않고, 앱 어디에도 그림 문자(이모지와 기호 글자)가 아이콘 자리에 쓰이지 않아 아이콘이 모두 같은 모양과 두께로 보인다. 배치와 동작은 그대로다.
- **Means:** 입력칸은 두꺼운 포커스 링 대신 얇은 테두리 색 변화와 옅은 번짐으로 바꾸고(KTD1), 글자 아이콘은 공용 SVG 아이콘 컴포넌트로 바꾼다(KTD2).
- **Product authority:** 이 대화의 사용자 요청("input에 focus가 너무 강하게 잡혀", "이모지는 아이콘으로 다 바꿔"). 키보드 포커스가 보여야 한다는 PR #2의 접근성 조건은 유지한다.
- **Target repo:** Forge 앱 저장소, 브랜치 `feat/softer-focus-and-icons`(PR #2 브랜치 `feat/ui-polish-design-system` 위). 아래 경로는 모두 그 저장소 기준이다.
- **Execution profile:** 하나의 PR. PR #2가 병합되지 않았으므로 PR #2 브랜치를 기준(base)으로 연다. 유닛 순서대로 커밋하고 각 유닛 뒤에 앱이 빌드되고 테스트가 통과한다.
- **Finish:** 구현, 검증, PR 마무리는 호출한 파이프라인(`lfg`)이 한다.
- **Open blockers:** 없음.

---

## Product Contract

### Summary

텍스트 입력칸의 포커스 표시를 얇은 테두리 색 변화와 옅은 번짐으로 낮추고, 키보드로 이동할 때 다른 컨트롤의 포커스 링은 그대로 둔다. UI에 보이는 이모지 4종(📁, 👁, 🗑, 🗄)과 아이콘 자리에 쓰인 기호 글자(닫기 ✕, 검색 ⌕, 체크 ✓, 화살표, 쉐브론 등)를 하나의 SVG 아이콘 세트로 바꾼다.

### Problem Frame

PR #2가 모든 포커스 가능 요소에 2px 포커스 링을 넣었다. 버튼에는 알맞지만 글자를 입력하는 칸은 클릭만 해도 `:focus-visible`이 켜져서 입력창마다 굵은 주황 선이 둘러진다(PR #2 이전에는 `accent-soft` 외곽선이라 은은했다). 아이콘은 두 가지가 섞여 있다. 파일 아이콘, 세부 패널 탭, 댓글 아이콘은 SVG인데, 나머지는 글자다. 글자 아이콘은 이모지(📁 👁 🗑 🗄)와 OS 글꼴에 따라 모양과 굵기가 달라지는 기호(✕ ✦ ◷ ⌕ ☰ ⇅ ✎ ✓ ▾ ▸ ⌄ › ⋮ ◎ 등)다.

### Requirements

**Input focus**

- R1. 글자를 입력하는 칸(작성창, 검색 팔레트, 찾기와 이동 바, 설정 검색과 텍스트 입력, 주석과 댓글 입력, 계획 카드 입력, 스킬과 루틴 폼의 텍스트 입력)은 포커스 때 2px 포커스 링을 쓰지 않고, 테두리를 accent 색으로 바꾸고 옅은 번짐 하나만 더한다. 테두리가 없는 칸은 그 칸을 둘러싼 상자(또는 하단선)가 같은 변화를 맡는다.
- R2. 입력칸의 포커스 표시는 인접한 면에 대해 3:1 이상이어서 키보드로 이동해도 현재 위치가 보인다. Windows 고대비(강제 색상) 모드에서도 윤곽선이 보인다.
- R3. 버튼, 메뉴 항목, 링크, 드롭다운(`.select`) 같은 입력칸이 아닌 컨트롤의 포커스 링은 지금 그대로다. 이름 바꾸기 입력(`.rename-input`)은 이미 accent 테두리를 가지고 있어 지금 모양을 유지한다. 찾기 바의 오류 상태(빨간 테두리)는 포커스 중에도 빨갛게 보인다.

**Icons**

- R4. UI에 그려지는 이모지(📁, 📁+, 👁, 🗑, 🗄)는 SVG 아이콘으로 바뀐다. 소스 보기 전환의 `</>` 글자도 같은 버튼 자리의 아이콘이므로 코드 아이콘으로 바뀐다.
- R5. 아이콘 자리에 쓰이던 기호 글자(닫기, 검색, ✦, ◷, ☰, ⇅, ✎, 화살표 버튼, 보내기 버튼의 ↵, ⋮, ◎, ≡, 체크 표시, 쉐브론, 코드 블록 실행 버튼의 ▷, 실행 중 표시의 ◌)도 같은 SVG 아이콘 세트로 바뀐다. 아이콘은 `currentColor`를 쓰고, 기본과 작은 두 크기 단계, 한 선 두께를 공유한다.
- R6. 글자 안에 끼어 있는 타이포그래피 문자는 바꾸지 않는다: 단축키 표기 글자(단축키 목록과 안내 문구에 쓰인 ⌘ ⇧ ⌥ ⌃와 화살표 키 표기), 번역 문장 속 화살표("← Skills", `base → head`), 말줄임표, 터미널 화면에 그려지는 `❯`.
- R7. 아이콘의 접근성 이름은 유지된다. 이미 `title`이나 `aria-label`이 있는 버튼은 그대로 두고, 이름이 글자 아이콘뿐이던 곳에는 같은 뜻의 `aria-label`을 둔다.

**Parity guard**

- R8. 레이아웃, 메뉴 구조, 단축키, 동작은 바뀌지 않고 기존 테스트가 통과한다. 단, 글자 모양을 단정하던 테스트(U3가 가리키는 세션 제목 표시 테스트)는 아이콘 없는 모양으로 고친다. 한국어와 영어 문장 테이블(`forge-shared` i18n)은 어긋나지 않는다.

### Acceptance Examples

- AE1. **Covers R1, R2.** Given 라이트나 다크에서, When 작성창을 클릭하거나 Tab으로 들어가면, Then 두꺼운 주황 링 없이 1px accent 테두리와 옅은 번짐만 보이고, 테두리 색이 주변 면에 3:1 이상이다.
- AE2. **Covers R3.** Given 사이드바의 버튼이나 메뉴 항목에 Tab으로 이동하면, Then 지금과 같은 2px 포커스 링이 보인다. 찾기 바에 잘못된 검색어를 넣고 포커스가 있어도 테두리는 빨갛다.
- AE3. **Covers R4, R5.** Given 앱의 홈, 채팅, 사이드바, 설정, 변경, 파일 패널과 코드 블록을 둘러볼 때, Then 이모지나 기호 글자 아이콘이 보이지 않고 같은 선 두께의 아이콘이 보인다.
- AE4. **Covers R6.** Given 단축키 목록과 "← Skills" 같은 뒤로 가기 문구를 열면, Then 글자는 그대로 보인다.

### Scope Boundaries

- 아이콘의 새 모양을 디자인하는 일(필요한 모양은 기존 SVG 아이콘의 선 스타일을 따라 그린다), 외부 아이콘 라이브러리나 폰트 추가, 이미 SVG인 아이콘의 교체는 하지 않는다.
- 단축키 표기와 번역 문장 속 글자는 바꾸지 않는다(R6).
- 포커스 링의 색, 두께, 오프셋 토큰과 버튼의 링은 바꾸지 않는다(R3).
- 입력칸이 아닌 컨트롤의 호버, 눌림, 등장 모션은 바꾸지 않는다.

### Assumptions

- "input"은 글자를 입력하는 모든 칸이다. 체크박스, 스위치, 슬라이더, 드롭다운은 입력칸이 아니라고 보고 지금 링을 유지한다.
- "이모지"는 그림 문자(📁 👁 🗑 🗄)와 아이콘 자리에 단독으로 쓰인 기호 글자를 모두 가리킨다. 글자 속 타이포그래피 문자는 포함하지 않는다(R6). 이 경계는 코드에서 확인한 근거로 정했고, 더 좁게 읽으면 R5가 줄어든다.
- 기존 SVG 아이콘 방식(16x16 viewBox, `currentColor`, 인라인)을 따르고 새 의존성은 넣지 않는다.

### Sources / Research

- `crates/forge-shared/src/theme.css`: 전역 `:focus-visible`(명시도 0,1,0), PR #2가 넣은 텍스트 입력칸의 포커스 규칙(`.text-input:focus-visible`, `.composer:focus-within` 등), `.rename-input`의 accent 기본 테두리, `.cm-find-bar input.invalid`의 빨간 테두리.
- 기존 SVG 아이콘: `crates/forge-widgets/src/files_icons.rs`, `details_panel.rs`, `changes_comment.rs`.
- 웹뷰와 HTML 문자열에 그려지는 글자 아이콘: `crates/forge-entities/src/markdown.rs`의 코드 블록 실행 버튼(▷를 담은 HTML 문자열), `crates/forge-widgets/src/code_run_watcher.rs`의 `◌`(웹뷰에 주입되는 자바스크립트).
- `crates/forge-features/src/account_menu.rs`의 `" ✓"`는 `#[cfg(test)]` 안의 테스트용 줄 포맷이다. 모델은 이미 `check: Option<bool>`을 가지고 있다.
- 글자 아이콘 사용처는 U3~U5의 파일 목록에 있다.

---

## Planning Contract

### Key Technical Decisions

- KTD1. **입력칸은 `outline: none`과 `border-color`, `box-shadow`로 포커스를 보인다.** 입력칸 전용 규칙이 전역 `:focus-visible` 링(명시도 0,1,0)을 덮으려면 같은 규칙이 `outline: none`을 선언해야 한다. 규칙은 `:focus-visible`(칸 자신)과 `:focus-within`(바깥 상자)을 받아 테두리를 `--field-focus-border`(= accent)로, 번짐을 `0 0 0 var(--field-focus-halo) var(--accent-soft)`로 바꾼다. 테두리가 3:1을 맡고 번짐은 장식이다. 테두리가 없는 칸(`.palette-input`, `.cd-search input`)은 바깥 상자가 맡고, 하단선만 있는 `.cd-menu-search`는 하단선을 2px accent로 바꾼다. 윤곽선을 없애므로 `@media (forced-colors: active)`에서 같은 선택자에 `outline: 2px solid Highlight`를 돌려준다. Governs R1, R2, R3.
- KTD2. **아이콘은 `forge-shared`의 `Icon` 컴포넌트 하나와 `IconKind` 열거형으로 모은다.** `files_icons.rs`가 이미 쓰는 방식(16x16 viewBox, `currentColor`, 둥근 끝, `aria-hidden`)을 따르고, 크기는 기본(16px)과 작음(12px, 쉐브론과 체크용) 두 단계, 선 두께는 하나(1.5)로 `--icon-size`, `--icon-size-sm`, `--icon-stroke` 토큰에 둔다. `size`를 자유 값으로 받지 않고 `IconSize::{Normal, Small}`만 받는다. 모든 크레이트가 `forge-shared`를 이미 쓰므로 새 의존성이 없다. 기존 SVG 아이콘은 건드리지 않고, 새 모양은 그 선 스타일을 따라 그린다. 쉐브론은 오른쪽 방향 하나만 두고 펼친 상태는 CSS 회전 클래스로 보인다. HTML 문자열이나 웹뷰 자바스크립트에 들어가는 아이콘(코드 블록 실행 버튼의 ▷, ◌)은 앱 스타일시트가 없으므로 `width`, `height`, `stroke`, `currentColor`를 요소 속성으로 담은 SVG 문자열 상수로 `forge-shared`에 두고, `Icon`의 Rust 열거형에는 넣지 않는다. Governs R4, R5.
- KTD3. **회귀 방지는 소스를 훑는 테스트로 한다.** `forge-shared`에 훑기 도구 하나를 두고(주석과 `#[cfg(test)]` 모듈 본문을 제거한 뒤 이모지 블록, 다이아모니, 도형 기호 범위를 찾는다), 훑을 파일 목록을 인자로 받는다. 각 유닛은 자기 파일에 대해, U6은 전체 소스에 대해 단정한다. 예외는 파일 단위로 단축키 표기(`shortcut.rs`)와 번역 테이블(`ko.rs`)과 터미널 화면(`sign_in.rs`, `sign_in_run.rs`)만이고, 나머지는 줄 끝 `// icon-ok` 표시로 둔다. 도구 자체는 이모지 줄은 잡고 허용 표기 줄은 통과시키는 고정 예제로 시험한다. Governs R4, R5, R6.
- KTD4. **`forge-features`의 `account_menu.rs`는 바꾸지 않는다.** 그 ✓는 테스트 전용 줄 포맷이고 모델은 이미 `check: Option<bool>`을 가진다. 화면은 `forge-widgets`의 `account_menu.rs`가 그리므로 위젯만 바꾼다. Governs R5, R8.

### Alternatives Considered

- 입력칸에서 포커스 표시를 아예 끄는 방식은 키보드 위치가 보이지 않아(R2) 제외했다.
- 외부 아이콘 폰트나 라이브러리는 의존성이 늘고 번들이 커져서 제외했다.
- 글자 아이콘을 CSS 마스크 이미지로 바꾸는 방식은 `currentColor`와 접근성 이름 처리가 불편해서 제외했다.

### Risks

- 글자 아이콘은 폭이 글자 폭이라, 같은 자리에 SVG를 넣으면 버튼 크기나 정렬이 1~2px 달라질 수 있다. 아이콘 크기를 기존 글자의 폭과 높이에 맞추고 실제 창에서 확인한다.
- 기존 SVG 아이콘은 선 두께가 제각각이다(`files_icons.rs` 1.3, `changes_comment.rs` 1.5). 새 아이콘은 1.5로 맞추고 기존 것은 건드리지 않으므로 앱 전체 두께가 완전히 같지는 않을 수 있다.

---

## Implementation Units

### U1. 입력칸 포커스를 가볍게

- **Goal:** 글자 입력칸의 포커스를 얇은 테두리와 번짐으로 바꾼다.
- **Requirements:** R1, R2, R3. KTD1.
- **Dependencies:** 없음.
- **Files:** `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs` (테스트).
- **Approach:**
  1. 토큰 `--field-focus-border`(= `var(--accent)`)와 `--field-focus-halo`(3px)를 `:root`에 둔다.
  2. 입력칸 목록은 `widgets`와 `features` 소스에서 `input`과 `textarea`를 그리는 클래스를 훑어 확정한다(`.text-input`, `.textarea`, `.settings-search`, `.cm-find-bar input`, `.cm-goto-bar input`, `.plan-card-text`와 바깥 상자 `.composer`, `.palette`, `.cd-search`, `.cm-comment-box`, `.cd-comment`, 스킬과 루틴 폼이 쓰는 클래스). `.select`와 `.rename-input`은 목록에서 뺀다.
  3. 입력칸 전용 규칙 하나에 `outline: none`, `border-color: var(--field-focus-border)`, `box-shadow: 0 0 0 var(--field-focus-halo) var(--accent-soft)`를 선언한다. 하단선만 있는 `.cd-menu-search`는 `border-bottom-color`와 2px 하단선으로 따로 처리한다.
  4. PR #2가 입력칸에 준 `outline` 기반 규칙(`.text-input:focus-visible, .textarea:focus-visible, .select:focus-visible`와 `.composer:focus-within` 등의 `calc(var(--focus-ring-offset) * -1)` 규칙)을 위 규칙으로 바꿔 없애고, `.select:focus-visible`과 `.rename-input`의 현재 모양은 그대로 유지한다.
  5. `.cm-find-bar input.invalid:focus-visible { border-color: var(--danger); }`를 입력칸 규칙 뒤에 둬 오류 테두리가 이긴다.
  6. `@media (forced-colors: active)` 블록에서 입력칸 선택자에 `outline: 2px solid Highlight`를 돌려준다.
  7. 전역 `:focus-visible`(버튼 등)은 그대로 둔다.
- **Patterns to follow:** PR #2의 `keyboard_focus_shows_everywhere...` 테스트와 포커스 링 대비 테스트(`the_focus_ring_stands_out_from_every_surface_in_both_themes`).
- **Test scenarios:**
  - Covers AE1. 입력칸 규칙이 `outline: none`, `border-color: var(--field-focus-border)`, `box-shadow`를 선언한다.
  - `--field-focus-border`의 색이 라이트와 다크에서 `surface-0`~`surface-3`에 대해 3:1 이상이다.
  - Covers AE2. 전역 `:focus-visible` 규칙이 그대로 남아 있고, `.select`는 링 규칙을 유지하며, `.rename-input`은 입력칸 규칙의 선택자 목록에 없다. 기존 `keyboard_focus_shows_everywhere...` 테스트가 계속 통과한다.
  - Covers AE2. `.cm-find-bar input.invalid:focus-visible`이 입력칸 규칙보다 뒤에 있고 `--danger` 테두리를 선언한다.
  - `forced-colors: active` 블록이 존재하고 입력칸 선택자에 `outline`을 선언한다.
  - 입력칸 규칙이 `.button`이나 `.sidebar-item` 같은 입력칸이 아닌 선택자를 포함하지 않는다.
- **Verification:** `cargo test -p forge-shared` 통과. 실제 창에서 작성창, 검색 팔레트, 설정 검색을 클릭하거나 Tab으로 들어가면 굵은 링이 없고 테두리와 번짐(하단선)만 보인다.

### U2. 공용 아이콘 컴포넌트와 훑기 도구

- **Goal:** `Icon` 컴포넌트, 아이콘 목록, 그림 문자 훑기 도구를 만든다.
- **Requirements:** R4, R5, R7. KTD2, KTD3.
- **Dependencies:** 없음.
- **Files:** `crates/forge-shared/src/components.rs`, `crates/forge-shared/src/lib.rs`, `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`와 훑기 도구용 테스트 모듈(`crates/forge-shared/src/` 아래).
- **Approach:**
  1. `IconKind` 열거형: 닫기, 검색, 반짝임(✦), 시계(◷), 폴더, 폴더와 더하기, 눈, 코드, 휴지통, 보관함, 목록(☰와 ≡), 정렬(⇅), 연필, 위·아래·왼쪽·오른쪽 화살표, 보내기(↵), 점 세 개 세로(⋮), 원(◎), 쉐브론(오른쪽 하나), 체크를 둔다. 코드 모양은 `files_icons.rs`의 `FileIcon::Code` 경로를 쓴다.
  2. `Icon { kind, size }`의 `size`는 `IconSize::{Normal, Small}`. 속성은 `files_icons.rs`와 같다(`view_box: "0 0 16 16"`, `fill: none`, `stroke: currentColor`, 둥근 끝, `aria-hidden`). 쉐브론의 펼침은 `.icon.open` 회전 규칙으로 보인다.
  3. CSS: `.icon { width: var(--icon-size); height: var(--icon-size); stroke-width: var(--icon-stroke); flex: none; vertical-align: middle; }`, `.icon.sm { width: var(--icon-size-sm); height: var(--icon-size-sm); }`, 토큰은 `--icon-size: 16px`, `--icon-size-sm: 12px`, `--icon-stroke: 1.5`.
  4. HTML 문자열과 웹뷰 자바스크립트용 SVG 문자열 상수(코드 블록 실행 ▷, 실행 중 ◌)를 같은 모듈에 둔다.
  5. 훑기 도구(KTD3): 파일 목록을 받아 주석과 `#[cfg(test)]` 본문을 제거하고 그림 문자 범위를 찾아 돌려준다.
- **Patterns to follow:** `files_icons.rs`의 `FileTypeIcon`, 기존 i18n 소스 훑기 테스트(`crates/forge-shared/src/i18n.rs`).
- **Test scenarios:**
  - `IconKind`의 모든 값이 비어 있지 않은 경로를 갖는다(열거형을 훑는 테스트).
  - `.icon`이 `--icon-size`와 `--icon-stroke`를 쓰고 색 리터럴이 없다(기존 `no_rule_carries_a_color_literal` 테스트가 통과한다).
  - 훑기 도구가 이모지가 든 고정 예제는 잡고, 허용 표기(`// icon-ok` 줄, 주석, `#[cfg(test)]` 본문)는 통과시킨다.
  - SVG 문자열 상수가 `width`, `height`, `stroke`, `currentColor`를 요소 속성으로 가진다.
- **Verification:** `cargo test -p forge-shared` 통과.

### U3. 이모지를 아이콘으로

- **Goal:** 그림 문자 📁, 📁+, 👁, 🗑, 🗄와 소스 보기 `</>`를 아이콘으로 바꾼다.
- **Requirements:** R4, R7, R8.
- **Dependencies:** U2.
- **Files:** `crates/forge-features/src/composer_state.rs`, `crates/forge-widgets/src/new_session_controls.rs`, `crates/forge-widgets/src/settings_overlay.rs`, `crates/forge-widgets/src/environment_dialog.rs`, `crates/forge-widgets/src/files_panel.rs`, `crates/forge-widgets/src/tasks_panel.rs`, `crates/forge-widgets/src/session_row.rs`.
- **Approach:**
  1. 📁(작성창 칩, 설정 폴더 줄, 환경 대화상자의 폴더 줄)는 폴더 아이콘으로, `📁+`는 폴더와 더하기 아이콘으로 바꾼다.
  2. 👁(소스 보기 전환)은 눈 아이콘으로, 같은 버튼의 `</>`는 코드 아이콘으로 바꾼다.
  3. 🗑(작업 삭제)는 휴지통 아이콘으로 바꾼다. 이미 있는 `title`을 유지한다.
  4. 🗄(보관된 세션 제목 앞 글자)는 제목 문자열에 글자를 붙이던 `format!("🗄 {}", ...)`을 보관함 아이콘 요소로 바꾼다. 제목 문자열을 쓰는 테스트는 아이콘 없는 제목으로 단정한다.
- **Execution note:** 바꾸기 전에 해당 파일의 테스트가 이 글자를 단정하는지 먼저 확인한다.
- **Test scenarios:**
  - Covers AE3. 위 파일들을 U2의 훑기 도구로 훑으면 그림 문자가 없다.
  - 보관된 세션의 제목 표시용 문자열이 아이콘 글자 없이 제목만 낸다(`session_row.rs`의 제목 단위 테스트).
  - 아이콘만 있는 버튼에 `title`이나 `aria-label`이 있다(렌더된 속성을 확인하는 컴포넌트 테스트가 있는 곳).
- **Verification:** `cargo test -p forge-shared -p forge-features -p forge-widgets` 통과. 실제 창에서 폴더 칩, 환경 대화상자, 소스 보기 전환, 작업 삭제, 보관된 세션 줄을 확인한다.

### U4. 버튼 자리의 글자 아이콘

- **Goal:** 단독 버튼과 머리글에 쓰인 기호 글자(쉐브론과 체크는 U5)를 아이콘으로 바꾼다.
- **Requirements:** R5, R7, R8.
- **Dependencies:** U2.
- **Files:** `crates/forge-widgets/src/panel_frame.rs`, `details_panel.rs`, `settings_overlay.rs`, `shortcuts_dialog.rs`(닫기 ✕), `sidebar.rs`(← → ⌕ ✦ ◷), `customize_skills.rs`(✦), `changes_panel.rs`(⌕), `files_panel.rs`(☰), `filter_menu.rs`(⇅), `chat_window.rs`(↓), `changes_comment.rs`(↑), `session_tools.rs`(◎ ⋮), `files_menu.rs`(⋮), `changes_header.rs`(⋮), `customize_plugins.rs`(⋮), `customize_dialogs.rs`(⋮), `code_run_watcher.rs`(◌), `crates/forge-entities/src/markdown.rs`(코드 블록 실행 ▷), `crates/forge-features/src/queued_card.rs`(↑ ✎), `new_session.rs`(보내기 ↵).
- **Approach:**
  1. 각 사용처를 `Icon { kind: ... }`로 바꾼다. 이미 `title`이 있는 버튼은 그대로 두고, 글자 아이콘뿐이던 버튼에는 같은 뜻의 `aria-label`을 번역 함수로 준다(새 문장은 한국어와 영어 테이블에 함께 추가한다). 아이콘은 `currentColor`를 쓰므로 버튼의 호버와 비활성 색을 그대로 따른다.
  2. `markdown.rs`의 코드 블록 실행 버튼은 HTML 문자열에 SVG 상수(KTD2)를 넣는다. `markdown.rs`의 기존 마크업 테스트는 그 버튼의 새 마크업으로 고친다.
  3. `code_run_watcher.rs`의 `◌ `는 웹뷰 자바스크립트에서 `textContent`로 넣던 것을 SVG 상수를 `innerHTML`(또는 DOM 생성)로 넣는 방식으로 바꾼다. 그 페이지에는 앱 스타일시트가 없으므로 상수의 속성이 크기와 색을 정한다. `▷`는 이 파일에 주석으로만 있어 바꾸지 않는다.
  4. 기존 글자 크기 기준으로 잡혀 있던 버튼의 폭은 `Icon`의 크기와 `.icon-button` 안에서의 정렬을 맞춰 바뀌지 않게 한다.
- **Test scenarios:**
  - Covers AE3. 위 파일들을 U2의 훑기 도구로 훑으면(쉐브론과 체크용 파일은 U5에서) 그림 문자가 없다.
  - Covers R7. 새로 `aria-label`을 단 번역 문장이 `ko.rs`에 있다(기존 `every_sentence_in_the_sources_has_a_korean_text` 테스트가 통과한다).
  - `markdown.rs`의 코드 블록 마크업 테스트가 새 SVG 버튼을 단정한다.
- **Verification:** `cargo test -p forge-widgets -p forge-entities -p forge-features -p forge-shared` 통과. 실제 창에서 사이드바 머리글, 패널 닫기, 설정 닫기, 파일 패널 머리글, 변경 패널 검색, 세션 도구줄, 코드 블록의 실행 버튼을 라이트와 다크로 확인한다.

### U5. 메뉴의 표시와 쉐브론

- **Goal:** 체크 표시와 접힘 쉐브론을 아이콘으로 바꾼다.
- **Requirements:** R5, R8. KTD4.
- **Dependencies:** U2.
- **Files:** `crates/forge-widgets/src/composer_controls.rs`, `plus_menu.rs`, `files_menu.rs`, `changes_header.rs`, `filter_menu.rs`, `account_menu.rs`, `customize_plugins.rs`, `customize_skills.rs`, `tasks_panel.rs`, `routines_page.rs`, `sidebar.rs`, `session_row.rs`, `files_panel.rs`, `session_title_menu.rs`, `plan_card.rs`, `question_panel.rs`, `permission_panel.rs`.
- **Approach:**
  1. 체크 ✓는 체크 아이콘(작은 크기)으로, 쉐브론 ▾ ▸ ⌄ ›는 쉐브론 아이콘으로 바꾸고 펼친 상태는 `.icon.open` 클래스로 회전시킨다. 선택된 항목에서만 체크를 그리는 지금의 조건은 그대로 두고, 체크가 없을 때 같은 크기의 빈 자리를 둬 정렬을 유지한다.
  2. `routines_page.rs`의 `"✓ {label}"`처럼 글자 앞에 체크를 붙인 문자열은 체크 아이콘 요소와 라벨로 나눈다.
  3. `forge-features`의 `account_menu.rs`는 테스트 전용 문자열이므로 바꾸지 않는다(KTD4).
- **Test scenarios:**
  - Covers AE3. 위 파일들을 U2의 훑기 도구로 훑으면 그림 문자가 없다.
  - 체크 자리가 고정 폭이라는 CSS 규칙이 있다.
  - `.icon.open`이 회전을 선언한다.
- **Verification:** `cargo test -p forge-widgets -p forge-shared` 통과. 실제 창에서 작성창 모델 메뉴, 계정 메뉴, 필터 메뉴, 사이드바 접기와 펼치기를 확인한다.

### U6. 그림 문자 회귀 방지

- **Goal:** 화면 코드에 그림 문자가 다시 들어오지 않게 한다.
- **Requirements:** R4, R5, R6. KTD3.
- **Dependencies:** U3, U4, U5.
- **Files:** `crates/forge-shared/src/` 아래 U2의 테스트 모듈.
- **Approach:** U2의 훑기 도구로 `crates/*/src/**/*.rs` 전체를 훑어 허용 목록(KTD3) 밖의 그림 문자가 없음을 단정한다. 허용 목록의 파일이 모두 실제로 존재하는지도 확인한다.
- **Test scenarios:**
  - 허용 목록 밖의 화면 코드에 그림 문자가 없다.
  - 허용 목록의 각 파일 경로가 실제로 존재한다(오래된 예외 방지).
- **Verification:** `cargo test -p forge-shared` 통과, 위 유닛들이 끝난 뒤 실패하는 항목이 없다.

---

## Verification Contract

| 확인 | 방법 | 기준 |
|---|---|---|
| 단위 테스트 | `cargo test -p forge-shared`, `-p forge-entities`, `-p forge-features`, `-p forge-widgets` | 모두 통과 |
| 전체 테스트 | `cargo test --workspace` | 통과(`forge-adapter-claude-plugins`의 `round_trip`은 이 머신의 실제 플러그인 목록에 기대 가끔 실패하므로 단독 재실행) |
| 컴파일 | `cargo build -p forge-app` | 경고와 오류 없음 |
| 실제 창 | `scripts/harness/launch.sh`, `capture.py`, `click.py`로 홈, 채팅, 사이드바, 설정, 변경, 파일 패널, 코드 블록을 라이트와 다크로 캡처 | 이모지와 기호 글자 아이콘이 없고, 입력칸 포커스가 테두리와 번짐으로만 보이며, 버튼 포커스 링은 그대로 |
| 다크 확인 | 하니스의 `FORGE_CONFIG_PATH` 설정에서 테마를 다크로 바꿔 다시 캡처 | 위와 같음 |
| 고대비 모드 | 실제 창에서 확인하지 못하는 환경이면 `forced-colors` 블록이 있다는 테스트로 대신한다 | 확인하지 못한 항목으로 보고 |

---

## Definition of Done

- R1~R8과 AE1~AE4가 위 검증으로 확인됐다.
- U1~U6이 순서대로 커밋됐고, 커밋마다 `cargo test -p forge-shared`가 통과한다.
- 화면 코드에 허용 목록 밖의 그림 문자가 없다(U6 테스트).
- 입력칸의 포커스 표시가 `outline`이 아닌 테두리와 번짐이며 3:1 대비 테스트가 있고, 고대비 모드용 `forced-colors` 규칙이 있다.
- 시도하다 버린 코드와 쓰이지 않는 아이콘 종류가 남아 있지 않다.
