---
title: Forge UI 마감 개선 - Plan
type: feat
date: 2026-10-31
topic: ui-polish-design-system
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-brainstorm
execution: code
---

# Forge UI 마감 개선 - Plan

## Goal Capsule

- **Objective:** Forge를 쓰는 사람이 라이트와 다크 어느 쪽에서나 마감이 고른 화면을 보고, 키보드로 써도 현재 위치를 알 수 있다. 배치와 동작은 지금과 같다.
- **Means:** 토큰 체계를 먼저 정하고, 모든 화면의 하드코딩 값을 그 토큰으로 바꾼다 (KTD1).
- **Product authority:** 사용자가 이 대화에서 정한 범위. 구조, 메뉴, 단축키는 Claude Code 데스크톱 앱 패리티 기준을 그대로 따른다.
- **Target repo:** Forge 앱 저장소의 `master` 코드. 이 계획서가 있는 워크트리에는 코드가 없으므로, 구현은 `master`에서 딴 새 브랜치에서 한다. 아래 파일 경로는 모두 그 저장소 기준이다.
- **Execution profile:** 하나의 PR. 유닛 순서대로 커밋하고, 각 유닛이 끝날 때마다 앱이 빌드되고 기존 테스트가 통과한다.
- **Finish:** 구현, 검증, PR 마무리는 호출한 파이프라인(`lfg`)이 한다.
- **Open blockers:** 없음.

---

## Product Contract

### Summary

배치, 메뉴 구조, 단축키, 동작은 그대로 두고 시각 층만 새로 만든다. 간격, 글자, 반경, 표면, 그림자, 모션, 색을 토큰으로 정리하고, 질감과 전환, 상태 화면, 다크 모드, 접근성을 그 위에서 한 번에 맞춘다.

### Problem Frame

Forge는 Claude Code 데스크톱 앱의 배치와 동작을 따라 만들어졌고, 구조는 맞다. 시각 층은 `crates/forge-shared/src/theme.css`(약 1,400줄) 한 파일에 쌓여 있다. 이 파일에는 색, 반경, 그림자 토큰만 있고 간격, 글자 크기, 모션 토큰은 없다. 전환(`transition`)은 6곳뿐이다. 빈 상태, 로딩, 오류 화면은 화면마다 모양이 다르다. 다크 모드는 라이트 값의 짝으로만 존재하고, 흐린 글자색이 대비 기준을 넘는지 확인된 적이 없다.

### Requirements

**Design tokens**

- R1. 간격(4px 단위), 글자 크기와 행간과 굵기(역할별 단계), 반경, 표면 단계, 그림자 단계, 모션 길이와 곡선을 이름 붙은 토큰으로 정의한다.
- R2. 색 단계는 OKLCH로 다시 만들고, 주황 accent의 색상은 유지한 채 밝기와 채도 단계를 고르게 맞춘다.
- R3. 기존 CSS의 하드코딩된 간격, 크기, 색, 그림자 값은 R1과 R2의 토큰을 쓰도록 바꾼다. 화면 코드의 인라인 스타일도 포함한다.
- R4. 채팅 카드 안의 생성형 UI 킷(`crates/forge-entities/src/ui_kit.css`)과 터미널 색은 같은 토큰을 쓰게 맞추되, 카드 안 콘텐츠의 디자인은 바꾸지 않는다.

**Surface and depth**

- R5. 표면은 배경, 카드, 팝오버 세 단계 이상으로 나뉘어 명도 차이로 구분된다.
- R6. 카드와 팝오버에는 겹친 그림자와 1px 안쪽 하이라이트 테두리를 쓰고, 팝오버와 오버레이에는 은은한 블러를 쓴다.

**Motion**

- R7. 버튼, 목록 항목, 메뉴 항목의 hover, press, focus에 R1의 모션 토큰을 쓰는 짧은 전환을 둔다.
- R8. 메뉴, 팝오버, 카드는 등장할 때만 짧게 움직이고, 반복되거나 장식뿐인 모션은 두지 않는다.
- R9. 시스템의 모션 줄이기 설정이 켜져 있으면 R7과 R8의 움직임을 모두 끈다.

**States**

- R10. 빈 상태, 로딩, 오류는 앱 전체에서 각각 하나의 모양을 공유한다. 로딩은 스피너 대신 콘텐츠 모양의 스켈레톤으로 표시한다.

**Dark mode and accessibility**

- R11. 다크 모드는 라이트 값을 뒤집지 않고 R5의 표면 단계, 다크 전용 테두리와 그림자, 순백이 아닌 글자색으로 따로 맞춘다.
- R12. 모든 글자와 컨트롤 색 조합은 라이트와 다크 양쪽에서 WCAG AA 대비(본문 4.5:1)를 넘긴다. 흐린 글자색이 넘지 못하면 기준을 넘는 쪽으로 고정한다.
- R13. 키보드로 포커스할 수 있는 모든 요소에는 `:focus-visible`로 통일된 포커스 링이 보인다.

**Typography**

- R14. 글자 크기와 행간은 R1의 역할별 단계에서만 고르고, 숫자가 나란히 놓이는 곳(사용량, 토큰 수, 시각)은 같은 폭의 숫자(tabular-nums)로 정렬한다. 한글 줄바꿈(단어 단위)은 지금처럼 유지한다.

**Parity guard**

- R15. 레이아웃, 메뉴 구조, 단축키, 동작은 바뀌지 않는다. 기존 동작 테스트가 계속 통과한다.

### Key Decisions

- **시각 층만 바꾼다.** (session-settled: user-directed — chosen over 핵심 화면 구조 재설계 and 패리티 무시: 패리티 기준과 기존 동작 테스트를 깨지 않기 위해.) Governs R15.
- **accent 색상은 유지하고 OKLCH로 밝기와 채도 단계만 고르게 한다.** (session-settled: user-approved — chosen over accent 색상 자체 변경: 브랜드 느낌 유지.) Governs R2.
- **구현은 `master` 코드를 기준으로 새 브랜치에서 한다.** (session-settled: user-approved — chosen over 코드가 없는 이 워크트리에서 바로 구현: 앱 코드가 `master`에만 있다.)

### Key Flows

- F1. 토큰 먼저, 화면 순서로 적용
  - **Trigger:** 구현 시작.
  - **Steps:** R1, R2 토큰을 정의한다. 화면 한 곳씩 R3으로 바꾼다. 화면마다 R5에서 R14를 적용한다. 화면마다 라이트와 다크 실제 창에서 확인한다.
  - **Outcome:** 모든 화면이 같은 토큰을 쓰고, 화면별 예외 값이 남지 않는다.
  - **Covered by:** R1, R2, R3, R5, R12

### Acceptance Examples

- AE1. **Covers R12, R13.** Given 라이트와 다크 어느 쪽이든, When Tab으로 사이드바, 채팅, 도크의 컨트롤을 차례로 이동하면, Then 각 컨트롤에 포커스 링이 보이고 모든 글자와 컨트롤의 대비가 4.5:1 이상이다.
- AE2. **Covers R9.** Given 시스템의 모션 줄이기가 켜져 있을 때, When 메뉴나 팝오버를 열면, Then 움직임 없이 바로 나타난다.
- AE3. **Covers R10.** Given 목록 데이터가 아직 불러와지는 중일 때, When 화면이 열리면, Then 스피너 대신 목록 모양의 스켈레톤이 보이고, 데이터가 없으면 안내 한 줄이 있는 빈 상태가 보인다(그 화면에 주된 동작이 있으면 그 동작 하나가 함께 보인다).
- AE4. **Covers R15.** Given 개선 전후의 같은 세션, When 같은 조작을 하면, Then 배치와 동작이 같고 기존 테스트가 통과한다.

### Success Criteria

- 라이트와 다크 실제 창 스크린샷에서 모든 주요 화면(사이드바, 채팅, 도크 패널, 설정, 홈)이 같은 토큰 체계로 보인다.
- 새 CSS에는 R1과 R2 토큰 밖의 하드코딩 간격, 크기, 색이 남지 않는다.

### Scope Boundaries

- 아이콘 교체, 폰트 교체, 레이아웃과 정보 구조 변경은 하지 않는다.
- 생성형 UI 킷 안 콘텐츠와 터미널의 콘텐츠 디자인은 바꾸지 않는다.
- accent의 색상(hue) 변경은 하지 않는다.
- 사용자가 밀도나 모션 강도를 고르는 설정 UI는 만들지 않는다.

### Dependencies / Assumptions

- 이 워크트리(`myhency/develop`)에는 코드가 없다. 앱 코드는 `master` 브랜치에 있고, 구현은 `master`에서 새 브랜치를 따서 한다.
- 워크트리는 Dioxus 데스크톱 앱(`crates/forge-app`)이다. 확인은 실제 창 스크린샷(라이트와 다크)으로 한다.
- Windows에서의 확인은 이번 범위에 넣지 않는다.

### Outstanding Questions

**Deferred to Planning**

- 토큰을 `theme.css` 안의 변수로 둘지 파일을 나눌지.
- 화면 적용 순서와 PR을 몇 개로 나눌지.

### Sources / Research

- `crates/forge-shared/src/theme.css`: 토큰은 색, 반경, 그림자뿐이고 `transition`은 6곳, `box-shadow`는 38곳이다 (master, 2026-10-31 확인).
- `crates/forge-entities/src/ui_kit.css`: 생성형 UI 킷 스타일(52줄).
- 웹 리서치(블로그와 스킬 문서 중심, 근거 강도 약함): 토큰 먼저 정의, 상태 화면 누락, 다크 재설계와 표면 3단계, AA 대비, 모션 줄이기.

---

## Planning Contract

Product Contract preservation: unchanged R-IDs and meaning. Planning added the Key Decisions section, which records choices the user settled during brainstorming, and clarified AE3: the empty state's action appears where the screen already has a primary action.

### Key Technical Decisions

- KTD1. **토큰 이름 층을 새로 만들고, 기존 변수 이름은 별칭으로 남긴다.** `--bg`, `--bg-card`, `--border`, `--accent` 같은 기존 이름은 새 토큰을 가리키게만 바꾼다. 선택자가 1,400줄에 퍼져 있고 `crates/forge-shared/src/theme.rs`의 테스트가 선택자와 변수 이름을 고정하고 있어서, 이름을 바꾸면 변경량만 늘고 얻는 게 없다. Governs R1, R3.
- KTD2. **간격과 반경 토큰화는 픽셀이 변하지 않게 한다.** 간격 단계에 지금 `gap`, `padding`, `margin`이 쓰는 값(1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 18, 20, 24, 28px)을 그대로 두어 값이 그대로 토큰이 된다. 32px 이상의 한 번만 쓰이는 레이아웃 치수(패널 너비 등)는 리터럴로 남긴다. 시각 변화는 표면, 그림자, 색, 모션에서만 생기므로 R15의 배치 유지가 지켜진다. 글자 크기 단계는 10, 11, 12, 13, 14, 16, 18, 20, 22, 24px이고, 단계에 없는 값은 글자가 커지지 않게 내림한다(12.5→12, 15→14, 17→16px). Governs R1, R3, R14, R15.
- KTD3. **다크 팔레트는 지금처럼 두 블록(시스템 설정, 강제 지정)에 두되, 두 블록이 같은 선언인지 테스트로 지킨다.** 테마 설정이 라이트, 다크, 시스템 세 값이라 두 블록이 필요하고, 지금은 값이 손으로 복사돼 있어 어긋날 수 있다. `VARIABLE_BLOCK` 상수에 기대는 기존 테스트는 구조를 바꾸지 않는 선에서 유지한다. Governs R11.
- KTD4. **OKLCH 값을 기존 라이트, 시스템 다크, 강제 다크 변수 블록에 그대로 쓰고, hex 폴백은 두지 않는다.** `theme.css`와 `ui_kit.css`는 이미 `color-mix(in srgb, ...)`와 `backdrop-filter`를 쓰고 있어 앱이 돌아가는 웹뷰는 OKLCH도 지원한다(`color-mix`가 더 새 엔진을 요구한다). 블록 구조가 그대로라 `VARIABLE_BLOCK` 기반 기존 테스트도 바뀌지 않는다. Governs R2.
- KTD5. **대비 검사는 Rust 단위 테스트로 한다.** 테스트가 `theme.css`에서 토큰 선언을 읽고 OKLCH를 sRGB로 바꿔 라이트와 다크 양쪽의 글자·배경 쌍의 대비비를 계산한다. 눈으로 한 번 확인하는 방식은 토큰이 바뀔 때마다 다시 깨질 수 있다. Governs R12.
- KTD6. **하드코딩 색 금지는 테스트로 지킨다.** 토큰 블록 밖의 `theme.css`에 hex나 `rgb` 리터럴이 없어야 하고, 오피스 문서 미리보기의 문서 색(`data-lum` 계열)은 허용 목록으로 예외 처리한다. `theme.rs`에 이미 같은 방식의 테스트(`.effort`, `.usage` 규칙)가 있다. Governs R3.
- KTD7. **모션 줄이기는 하나의 전역 규칙으로 한다.** `:root[data-motion="reduced"]`가 이미 전환과 애니메이션을 0으로 만들므로, 같은 규칙에 `@media (prefers-reduced-motion: reduce)`를 더한다. 새 전환마다 개별 대응을 두지 않는다. Governs R9.
- KTD8. **생성형 UI 카드 안의 팔레트는 별도 문자열이라 따로 맞춘다.** 카드는 샌드박스 iframe이라 앱의 변수를 볼 수 없고, `crates/forge-entities/src/generated_ui.rs`가 자체 변수 문자열을 주입한다. 이 문자열의 값을 새 토큰 값과 같게 하고, 클래스 이름과 변수 이름(`--surface`, `--line`, `--muted`, `--c1`~`--c6`)은 바꾸지 않는다. 모델이 쓰는 안내 문서(`sidecar/src/ui_examples/kit.md`)와 맞물려 있기 때문이다. Governs R4.

### Alternatives Considered

- 화면마다 따로 손보는 방식은 일관성이 깨지고 되돌리기 어려워 제외했다.
- 변수 이름을 전부 새 이름으로 바꾸는 방식은 KTD1의 이유로 제외했다.
- OKLCH의 `@supports` hex 폴백은 만들지 않았다. 폴백이 필요한 웹뷰는 이미 `color-mix`를 못 써서 앱이 지원되지 않는 환경이고, 구조가 달라져 기존 블록 테스트가 깨진다. 지원 하한이 `color-mix` 아래로 내려가는 증거가 나오면 다시 판단한다.
- CSS 전처리기나 유틸리티 프레임워크 도입은 빌드 구성을 바꾸고 패리티와 무관해서 제외했다.

### Risks

- `theme.rs`의 기존 테스트가 선택자와 블록 문자열을 고정하고 있어, 구조를 건드리면 테스트가 깨진다. 유닛마다 해당 테스트를 먼저 읽고 의도를 유지한 채 고친다.
- 글자 크기를 단계에 맞추면 일부 줄바꿈 위치가 바뀔 수 있다. 한글 줄바꿈은 `word-break: keep-all`이라 길이 변화에 민감하다. U7에서 긴 텍스트가 있는 화면을 실제 창으로 확인한다.

### Dependencies / Prerequisites

- 구현은 `master`에서 딴 브랜치에서 한다 (Key Decisions 참조).
- 실제 창 확인에는 `scripts/harness/`(실행, 캡처, 클릭)와 Screen Recording, Accessibility 권한이 필요하다. 이미 부여돼 있다는 기록이 있다.

---

## Implementation Units

### U1. 토큰 층과 다크 블록 정리

- **Goal:** 간격, 글자(크기, 행간, 굵기), 반경, 표면 단계, 그림자 단계, 모션 길이와 곡선 토큰을 정의하고 기존 변수를 별칭으로 연결한다.
- **Requirements:** R1, R5, R11 (구조 부분). KTD1, KTD2, KTD3.
- **Dependencies:** 없음.
- **Files:** `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs` (테스트).
- **Approach:**
  1. `:root` 맨 위에 새 토큰 블록을 둔다: `--space-*`, `--text-*`와 `--leading-*`와 `--weight-*`, `--radius-*`, `--surface-0`~`--surface-3`, `--shadow-1`~`--shadow-3`, `--dur-fast`, `--dur-base`, `--ease-out`.
  2. 기존 `--bg`, `--bg-sidebar`, `--bg-card`, `--bg-hover`, `--radius`, `--radius-lg`, `--shadow`를 새 토큰의 별칭으로 바꾼다. 값은 이 유닛에서는 바꾸지 않는다.
  3. 다크 두 블록의 선언이 같은지 확인하는 테스트를 추가한다.
- **Patterns to follow:** `theme.rs`의 `THEME_CSS.contains` 계열 테스트와 `VARIABLE_BLOCK`을 쓰는 다크 팔레트 테스트.
- **Test scenarios:**
  - 새 토큰 이름이 모두 `:root`에 정의돼 있다.
  - 다크 팔레트의 두 블록(시스템 설정, 강제 지정)의 변수 선언 집합이 같다.
  - 기존 선택자 테스트(`.setting-row`, `.palette` 등)가 계속 통과한다.
- **Verification:** `cargo test -p forge-shared`가 통과하고, 앱을 띄웠을 때 화면이 이전과 같다.

### U2. OKLCH 팔레트와 대비 테스트

- **Goal:** 색 단계를 OKLCH로 다시 만들고 accent 색상은 유지하며, 모든 글자와 컨트롤 색 쌍이 AA를 넘게 한다.
- **Requirements:** R2, R12. KTD4, KTD5, Key Decision(accent 색상 유지).
- **Dependencies:** U1.
- **Files:** `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs` (테스트).
- **Approach:**
  1. 현재 hex 색(`--fg`, `--fg-muted`, `--accent`, `--warning`, `--danger`, 코드 강조와 diff 색, `--link`)을 OKLCH로 옮기고, 밝기와 채도 단계를 고르게 정리한다. accent의 색상 각도는 그대로 둔다.
  2. 값은 KTD4대로 기존 변수 블록에 OKLCH로 직접 쓴다.
  3. 대비 계산 테스트를 쓴다. 글자 토큰(`--fg`, `--fg-muted`, `--accent`, `--danger`, `--link`, `--hl-*`, diff 글자색)을 그것이 놓이는 모든 면(`--bg`, `--bg-sidebar`, `--bg-card`, `--bg-hover`, U4의 표면 단계 전부와 팝오버 표면, diff 배경)에 대해 라이트와 다크 블록에서 검사하고, `--accent-fg`/`--accent`와 `--warning-fg`/`--warning`도 포함한다. 기준 4.5:1 미만이면 muted 글자색 쪽을 진하게(라이트) 또는 밝게(다크) 고정한다.
  4. 포커스 링 토큰(색, 2px 두께, 오프셋)을 하나 정의하고(U5가 사용), 링 색이 `--bg`, `--bg-card`, 팝오버 표면, `--accent`에 대해 3:1 이상인지도 같은 테스트로 검사한다.
- **Execution note:** 대비 테스트를 먼저 쓰고 실패하는 쌍을 확인한 뒤 색을 고친다.
- **Test scenarios:**
  - Covers AE1. 라이트와 다크 양쪽에서 위 글자와 면의 모든 조합의 대비비가 4.5 이상이다.
  - 포커스 링 색이 네 가지 면에 대해 3:1 이상이다.
  - OKLCH를 sRGB로 바꾸는 계산이 알려진 값(흰색, 검정, 중간 회색)에서 맞다.
  - `theme.rs`의 `.effort`, `.usage` 규칙에 하드코딩 색이 없다는 기존 테스트가 통과한다.
- **Verification:** 테스트가 통과하고, 라이트와 다크 실제 창에서 accent 주황의 인상이 이전과 같다.

### U3. 하드코딩 값을 토큰으로 바꾸기 (CSS)

- **Goal:** `theme.css`의 간격, 반경, 글자 크기, 색 리터럴을 토큰으로 바꾼다.
- **Requirements:** R3, R14 (단계 맞추기). KTD2, KTD6.
- **Dependencies:** U1, U2.
- **Files:** `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs` (테스트).
- **Approach:**
  1. 글자 크기 13종을 KTD2의 `--text-*` 단계로 옮기고, 단계에 없는 12.5, 15, 17px은 각각 12, 14, 16px로 내림한다.
  2. 반경 값(2, 3, 4, 5, 6, 8, 10, 12, 14, 999px, 50%)을 `--radius-*`로, `gap`과 `padding`과 `margin`의 값을 `--space-*`로 바꾼다. 픽셀은 변하지 않아야 한다. 32px 이상의 한 번만 쓰이는 레이아웃 치수는 리터럴로 둔다.
  3. 토큰 블록 밖의 hex와 `rgb` 리터럴 94곳을 토큰이나 `color-mix`로 바꾼다. 오피스 문서 색은 허용 목록에 둔다.
  4. 하드코딩 금지 테스트를 추가한다.
- **Patterns to follow:** 이미 토큰만 쓰는 `.effort*`, `.usage*` 규칙.
- **Test scenarios:**
  - 토큰 블록 밖의 `theme.css`에 허용 목록 밖의 hex나 `rgb` 리터럴이 없다.
  - `font-size`가 `var(--text-*)`가 아닌 `px` 값으로 남은 규칙이 없고, `gap`, `padding`, 반경에 KTD2 단계 값이 리터럴로 남은 곳이 없다(허용 목록 예외만).
  - 기존 선택자 테스트가 통과한다.
- **Verification:** 테스트 통과. 대표 화면(사이드바, 채팅, 설정)의 전후 스크린샷에서 간격이 같다.

### U4. 표면과 깊이감, 다크 재설계

- **Goal:** 표면 단계 값, 겹친 그림자, 안쪽 하이라이트, 팝오버 블러를 적용하고 다크를 따로 맞춘다.
- **Requirements:** R5, R6, R11. KTD3.
- **Dependencies:** U1, U2, U3.
- **Files:** `crates/forge-shared/src/theme.css`.
- **Approach:**
  1. 표면 단계 값을 정한다: 배경, 사이드바, 카드, 팝오버 순으로 명도가 구분되게 하고, 다크에서는 순백 글자를 쓰지 않는다.
  2. 그림자 단계를 겹친 그림자로 바꾸고, 카드와 팝오버에 1px 안쪽 하이라이트를 더한다. 다크에는 다크 전용 테두리와 그림자 값을 둔다.
  3. 팝오버와 오버레이(`.row-menu`, `.filter-menu`, `.palette`, `.picker-menu`, `.usage-popover`, `.settings-backdrop`, `.mcp-backdrop`)에 은은한 블러를 적용한다. `backdrop-filter`는 이미 두 곳에서 쓰고 있다.
- **Test scenarios:**
  - Test expectation: none -- 값 조정이라 단위 테스트가 줄 정보가 없다. U2의 대비 테스트가 새 표면 단계와 팝오버 표면 전부에 대해 글자색을 검사한다.
- **Verification:** 라이트와 다크 실제 창에서 배경, 카드, 팝오버가 서로 구분되고 메뉴 뒤가 은은하게 흐려진다.

### U5. 모션과 포커스 링

- **Goal:** 통일된 전환, 등장 모션, 모션 줄이기 규칙, `:focus-visible` 링을 적용한다.
- **Requirements:** R7, R8, R9, R13. KTD7.
- **Dependencies:** U1, U3.
- **Files:** `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs` (테스트).
- **Approach:**
  1. 버튼, 목록 항목, 메뉴 항목, 아이콘 버튼의 공통 전환(배경, 색, 테두리, 그림자)을 모션 토큰으로 둔다.
  2. 메뉴와 팝오버가 열릴 때, 그리고 진행 중인 턴에서 새로 생긴 카드에만 한 번 움직이는 짧은 키프레임을 둔다(카드는 수식 클래스로 구분하고, 세션을 열거나 전환하거나 분할 창을 바꿔 다시 그려지는 카드에는 적용하지 않는다). 반복이나 장식 모션은 추가하지 않는다.
  3. `:root[data-motion="reduced"]` 규칙에 `@media (prefers-reduced-motion: reduce)`를 같은 선언으로 더한다.
  4. U2가 정한 포커스 링 토큰으로 전역 `:focus-visible` 링을 정의하고, 개별 `outline` 규칙(`.text-input:focus` 등)을 같은 링으로 모은다.
- **Execution note:** 기존 `data-motion` 설정이 동작하는지 먼저 실제 창에서 확인한 뒤 바꾼다.
- **Test scenarios:**
  - Covers AE2. 모션 줄이기 규칙이 `data-motion="reduced"`와 `prefers-reduced-motion` 양쪽에 있고 `transition-duration`과 `animation-duration`을 0으로 만든다.
  - 전역 `:focus-visible` 규칙이 존재하고 outline을 없애는(`outline: none`) 규칙이 짝이 되는 포커스 스타일 없이 남아 있지 않다.
  - 새 전환과 키프레임이 모션 토큰을 쓴다(`transition: ... 0.12s` 같은 리터럴 시간이 없다).
- **Verification:** Covers AE1. Tab으로 사이드바, 채팅, 도크를 지날 때 포커스 링이 모두 보이고, 설정에서 모션 줄이기를 켜면 메뉴가 바로 나타난다.

### U6. 상태 화면 통일

- **Goal:** 빈 상태, 로딩, 오류가 각각 하나의 모양을 공유한다.
- **Requirements:** R10.
- **Dependencies:** U3, U4, U5.
- **Files:** `crates/forge-shared/src/theme.css`, `crates/forge-widgets/src/chat_window.rs`, `crates/forge-widgets/src/search_palette.rs`, `crates/forge-widgets/src/mcp_servers_dialog.rs`, `crates/forge-widgets/src/environment_dialog.rs`, `crates/forge-widgets/src/files_office.rs`, `crates/forge-widgets/src/tasks_panel.rs`, `crates/forge-entities/src/turn.rs`, `crates/forge-entities/src/message.rs`, `crates/forge-widgets/src/customize_plugins.rs`, `crates/forge-widgets/src/customize_skills.rs`, `crates/forge-widgets/src/customize_connectors.rs`, `crates/forge-widgets/src/customize_dialogs.rs`, `crates/forge-widgets/src/customize_connector_dialogs.rs`.
- **Approach:**
  1. 공통 클래스를 정의한다: 빈 상태(안내 한 줄과, 그 화면에 이미 있는 주된 동작이 있으면 그 동작 하나), 오류(`--danger` 기반), 스켈레톤(모션 토큰을 쓰는 은은한 반짝임, 모션 줄이기에서는 정지).
  2. 지금 따로 있는 `.empty-state`, `.palette-empty`, `.mcp-empty`, `.env-empty`, `.office-slide-empty`, `.task-error`, `.tool-error`를 공통 클래스로 옮긴다. 선택자 이름을 지우는 경우 `theme.rs`의 선택자 테스트를 함께 고친다.
  3. 스피너를 쓰는 로딩 중 목록 자리를 행 모양 스켈레톤으로 바꾼다. 대상은 `crates/forge-widgets/src/customize_plugins.rs`, `customize_skills.rs`, `customize_connectors.rs`, `customize_dialogs.rs`, `customize_connector_dialogs.rs`의 목록 로딩이다. 모양은 목록 행 하나와 같은 높이의 막대 3줄이다. `crates/forge-entities/src/turn.rs`의 스피너는 진행 중 표시라 그대로 둔다.
- **Test scenarios:**
  - Covers AE3. 공통 빈 상태, 오류, 스켈레톤 클래스가 `theme.css`에 있고 스켈레톤 애니메이션이 모션 줄이기에서 멈춘다.
  - 옮긴 컴포넌트가 공통 클래스를 쓴다(컴포넌트 렌더 테스트가 있는 곳은 클래스 이름을 확인한다).
  - 오류 상태의 글자와 배경 쌍이 U2의 대비 기준을 넘는다.
- **Verification:** 실제 창에서 빈 목록, 오류 카드, 로딩 중 화면이 서로 같은 모양이다.

### U7. 타이포그래피와 인라인 스타일

- **Goal:** 글자 단계 적용을 마무리하고 숫자 정렬을 맞추며, Rust 코드의 정적 인라인 스타일을 클래스와 토큰으로 옮긴다.
- **Requirements:** R3, R14.
- **Dependencies:** U3.
- **Files:** `crates/forge-shared/src/theme.css`, `crates/forge-features/src/settings.rs`, `crates/forge-widgets/src/settings_overlay.rs`, `crates/forge-widgets/src/details_panel.rs`, `crates/forge-widgets/src/dock.rs`, `crates/forge-widgets/src/files_panel.rs`, `crates/forge-widgets/src/files_menu.rs`, `crates/forge-widgets/src/context_menu.rs`, `crates/forge-widgets/src/session_row.rs`, `crates/forge-widgets/src/changes_header.rs`.
- **Approach:**
  1. 숫자가 나란히 놓이는 곳(사용량, 토큰 수, 시각, 변경 줄 수)의 `font-variant-numeric: tabular-nums` 누락을 찾아 채운다.
  2. 정적 인라인 스타일(`margin-top: 12px`, `margin-left: auto` 같은 값)을 토큰을 쓰는 클래스로 옮긴다. 사용자 조작이나 계산에서 나오는 동적 값(너비, 좌표, 들여쓰기)은 인라인에 남긴다.
  3. 긴 한글 텍스트가 있는 화면(채팅, 설정, 환경)에서 줄바꿈이 어색해지지 않는지 확인한다.
- **Test scenarios:**
  - Test expectation: none -- 정적 값을 클래스로 옮기는 작업이다. 클래스 선택자 존재 여부는 U3에서 추가한 테스트 방식으로 확인한다.
- **Verification:** 실제 창에서 설정, 홈 사용량, 변경 목록의 숫자 열이 정렬되고, 긴 한글 문장의 줄바꿈이 이전과 같거나 더 낫다.

### U8. 생성형 UI 카드 팔레트와 터미널 색

- **Goal:** 카드 안 iframe의 팔레트 값과 UI 킷 스타일이 새 토큰 값과 같게 하고, 터미널 배경과 글자 색도 맞춘다.
- **Requirements:** R4. KTD8.
- **Dependencies:** U2, U4.
- **Files:** `crates/forge-entities/src/generated_ui.rs`, `crates/forge-entities/src/ui_kit.css`, `crates/forge-shared/assets/xterm.css`.
- **Approach:**
  1. `generated_ui.rs`의 팔레트 문자열(`--bg`, `--fg`, `--muted`, `--line`, `--accent`, `--surface`, `--c1`~`--c6`, 라이트와 다크)에 U2와 U4가 정한 값을 넣는다. 변수 이름은 그대로 둔다.
  2. `ui_kit.css`는 클래스 이름과 카드 안 콘텐츠의 모양을 바꾸지 않는다. `.fx-btn`의 `#fff` 글자색은 그대로 두고, 대신 `--accent`가 흰 글자와 4.5:1을 넘도록 팔레트의 accent 값을 맞춘다.
  3. 터미널(`xterm.css`의 `#000`)과 터미널 테마 색이 있다면 같은 표면 토큰과 어울리게 맞춘다.
- **Test scenarios:**
  - 팔레트 문자열에 필요한 변수(`--surface`, `--c1`, `--c6`, `data-forge-theme=dark`)가 모두 있다는 기존 테스트가 통과한다.
  - `ui_kit.css`의 클래스가 `sidecar/src/ui_examples/kit.md`와 맞물리는 기존 테스트가 통과한다.
  - 팔레트 문자열의 `#fff`/`--accent`(`.fx-btn`), `--fg`/`--bg`, `--muted`/`--bg`, `--muted`/`--surface`, `--fg`/`--surface` 쌍이 라이트와 다크에서 대비 4.5 이상이다.
- **Verification:** 실제 창에서 생성형 UI 카드를 열었을 때 카드 안 색이 앱 본문 색과 어울린다.

---

## Verification Contract

| 확인 | 방법 | 기준 |
|---|---|---|
| 단위 테스트 | `cargo test -p forge-shared`, `cargo test -p forge-entities`, `cargo test -p forge-features`, `cargo test -p forge-widgets` | 모두 통과 (기존 테스트의 의도를 유지한 채 고친 것만 변경) |
| 전체 테스트 | 저장소 루트에서 `cargo test --workspace` | 통과 |
| 컴파일 | `cargo build -p forge-app` | 경고와 오류 없음 |
| 하드코딩 값 | U3의 CSS 하드코딩 금지 테스트 | 허용 목록 밖의 hex와 `rgb` 없음 |
| 대비 | U2의 대비 테스트 | 라이트와 다크 모든 쌍 4.5:1 이상 |
| 실제 창 | `scripts/harness/launch.sh`, `capture.py`, `click.py`로 주요 화면(사이드바, 채팅, 도크, 설정, 홈, 생성형 UI 카드)을 라이트와 다크 양쪽에서 캡처 | AE1~AE4가 보이고 배치와 동작이 개선 전과 같음 |
| 포커스와 모션 | 실제 창에서 Tab 이동, 설정의 동작 줄이기(앱 설정 이름) 켜고 끄기 | 포커스 링이 모든 컨트롤에 보이고, 동작 줄이기에서는 움직임이 없음 |

실제 창 확인은 구현자가 직접 한다. 다크 모드는 하니스의 `FORGE_CONFIG_PATH` 설정에서 테마를 다크로 바꿔 띄우고 같은 화면을 다시 캡처해 확인한다. 캡처한 스크린샷은 PR 본문에 첨부한다. IME 입력은 이번 변경과 무관해 확인 대상이 아니다.

---

## Definition of Done

- R1~R15와 AE1~AE4가 위 검증으로 확인됐다.
- U1~U8이 순서대로 커밋됐고, 커밋마다 `cargo test -p forge-shared`가 통과한다.
- `theme.css`에는 토큰 블록 밖의 하드코딩 색 리터럴이 없고(허용 목록 예외만), 글자 크기와 간격과 반경이 토큰을 쓴다.
- 라이트와 다크 양쪽의 모든 글자와 컨트롤 색 쌍이 AA 대비를 넘는다.
- 시도했다가 버린 CSS와 코드가 남아 있지 않다.
- 개선 전후 같은 조작의 배치와 동작이 같다.
