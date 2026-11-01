---
title: 새 세션 화면의 최근 폴더 카드 - Plan
type: feat
date: 2026-10-31
topic: new-session-recent-environments
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-plan-bootstrap
execution: code
---

# 새 세션 화면의 환경 바로가기 카드 - Plan

> **정정 (같은 날):** 이 계획의 처음 판은 카드를 "최근 폴더 목록"으로 잘못 읽었다. 데스크톱 앱을 직접 눌러 보니 카드는 **저장된 환경**의 바로가기다(행: 노트북 아이콘 + 환경 이름, 누르면 그 환경이 선택되어 입력창 아래 줄이 환경 이름으로 바뀜, 푸터 `환경 선택`은 환경 전체 메뉴를 엶, 현재 선택과 같을 필요 없음). 아래 Product Contract와 유닛의 "폴더"는 모두 "저장된 환경"으로 읽는다. 구현은 `environment_card_rows`(forge-features), `EnvironmentCard`(forge-widgets), `PickerKind::Environment`다. 근거: `docs/superpowers/roadmap/observations.md` Round 35.

## Goal Capsule

- **Objective:** 새 세션 화면에서 최근에 쓴 폴더로 한 번 눌러 시작할 수 있다. 지금은 폴더 컨트롤의 메뉴를 열어야 최근 폴더가 보인다. Claude 데스크톱 앱은 입력창 위에 최근 환경 카드를 바로 보여 준다.
- **Means:** 입력창 위에 카드 하나를 두고, 이미 있는 최근 폴더 목록과 폴더 선택 동작을 그대로 쓴다(KTD1, KTD2).
- **Product authority:** 이 대화의 사용자 요청("앱의 기능을 모두 실행해보면서 claude desktop app과 비교하며 관찰한 후 ui를 기록하고 개선할 점을 구현"), 관찰 기록 `docs/superpowers/roadmap/observations.md` Round 35 (NEW-26).
- **Target repo:** Forge 저장소, 현재 브랜치(`myhency/develop`)에서 갈라지는 기능 브랜치. 경로는 저장소 기준.
- **Execution profile:** 하나의 PR, 유닛 순서대로 커밋.
- **Finish:** 구현, 검증, PR 마무리는 호출한 파이프라인(`lfg`)이 한다.
- **Open blockers:** 없음.

---

## Product Contract

### Summary

새 세션 화면의 입력창 바로 위에 "최근 폴더" 카드를 둔다. 카드는 둥근 테두리 상자이고, 최근 폴더 최대 3개가 행(폴더 아이콘 + 이름)으로 있고, 맨 아래 행 `폴더 선택 ⌄`이 기존 폴더 메뉴를 연다. 행을 누르면 그 폴더가 새 세션의 시작 폴더가 되고 입력창에 포커스가 돌아온다. 인사말, 입력창, 아래 줄 컨트롤은 그대로다.

### Problem Frame

Forge의 새 세션 화면은 인사말과 입력창뿐이다. 폴더를 바꾸려면 아래 줄의 폴더 컨트롤을 열어야 한다. 데스크톱 앱(Round 35)은 입력창 위에 최근 환경 3개와 `환경 선택 ⌄` 행을 가진 카드를 둔다. 같은 일을 한 번 덜 누르고 할 수 있고, 어떤 폴더로 시작할지 화면에서 바로 보인다.

### Observed facts (Round 35, 두 앱을 직접 본 것)

- 데스크톱 앱: 입력창 위, 입력창과 같은 폭의 열에 카드. 행 셋(노트북 아이콘 + 이름: `forge`, `forge-observe`, `mywork`), 구분선, `환경 선택 ⌄`. 카드 아래에 `Claude Code CLI 세션 2개 가져오기` 카드. 카드 위 모서리에 작은 픽셀 마스코트.
- Forge: 인사말이 위 왼쪽, 입력창이 아래, 카드 없음. 폴더 컨트롤의 메뉴는 최근 폴더 8개, `폴더 열기…`, 워크트리 행을 가진다(`folder_menu_rows`, `MAX_RECENT_FOLDERS`).

### Requirements

- R1. 새 세션 화면(`NewSessionPanel`)에 최근 폴더 카드를 입력창 위, 입력창과 같은 열 폭(`--chat-column`)에 둔다.
- R2. 행은 `workspace.new_session_projects()`의 앞에서 최대 3개(폴더가 실제로 있는 것). 행 = 폴더 아이콘 + 폴더 이름(`folder_label`). 현재 선택된 폴더의 행은 체크 표시.
- R3. 행을 누르면 `workspace.set_new_folder(Some(path))`, 그리고 `refocus_composer()`. 폴더 컨트롤의 선택과 같은 결과여야 한다(아래 줄의 폴더 이름이 바뀐다).
- R4. 마지막 행 `폴더 선택`은 기존 `workspace.open_folder()`(시스템 폴더 대화상자)를 연다. 새 메뉴를 만들지 않는다.
- R5. 카드는 `draft.environment`가 정해진 경우(환경 선택됨)에는 보이지 않는다(`FolderChip`과 같은 조건). 최근 폴더가 하나도 없으면 `폴더 선택` 행만 있는 카드가 아니라 카드를 아예 숨긴다.
- R6. 새 색, 그림자, 토큰을 만들지 않는다. 기존 `.picker-menu` 행 모양과 카드 테두리 토큰을 쓴다. 라이트와 다크에서 확인한다.
- R7. 번역 문장 표(한국어, 영어)에 새 문장을 넣는다. 창이 좁거나 낮을 때(`.chat-narrow`, `.new-session-pane`의 스크롤)도 카드가 입력창을 가리거나 잘리지 않는다.

### Acceptance Examples

- AE1. **Covers R1, R2.** 하네스 fixture(`my-app`, `app (a)`, `app (b)`)에서 새 세션 화면에 카드가 있고 세 행과 `폴더 선택`이 보인다. 현재 폴더 행에 체크.
- AE2. **Covers R3.** `app (a)` 행을 누르면 아래 줄 폴더 컨트롤이 `app (a)`로 바뀌고 입력창에 포커스가 있다. 그 상태에서 보낸 첫 메시지의 세션이 그 폴더에서 시작한다(기존 `set_new_folder` 경로와 같음).
- AE3. **Covers R5.** 프로젝트가 없는 빈 Claude home에서는 카드가 없다.

### Scope Boundaries

- Claude Code CLI 세션 가져오기 카드는 만들지 않는다(Forge가 이미 `~/.claude`를 읽는다).
- 마이크, 마스코트는 만들지 않는다.
- 폴더 메뉴(`FolderChip`)와 환경 칩(`EnvironmentChip`)의 구조와 동작은 바꾸지 않는다.
- 최근 폴더의 정렬 기준과 개수 상한(`MAX_RECENT_FOLDERS`)은 바꾸지 않는다.
- Windows 확인은 하지 않는다(맥에서만, PR에 적는다).

### Assumptions

- 데스크톱 앱의 "환경"은 Forge에서는 최근 폴더에 가장 가깝다(Forge의 환경은 별도 개념 `draft.environment`). 근거: Round 35에서 데스크톱 앱 행의 이름이 폴더 이름과 같다(`forge`, `mywork`). 환경이 정해진 경우는 R5로 숨긴다.
- 3개는 데스크톱 앱 화면에서 센 값이다.

---

## Key Technical Decisions

- **KTD1. 목록은 기존 순수 함수에서.** `crates/forge-features/src/folder_menu.rs`에 `recent_folder_card_rows(projects, current)`를 추가한다. `folder_menu_rows`와 같은 `FolderMenuRow`를 돌려주되 최대 3개(`RECENT_CARD_ROWS`). 현재 폴더가 3개 밖이면 맨 아래 행을 현재 폴더로 바꾸는 규칙은 `folder_menu_rows`와 같다(내부에서 상한만 달리해 공유한다). 입력은 `workspace.new_session_projects()`이고 이 함수가 이미 없는 폴더(`exists` 거짓)와 폴더 없음 임시 폴더를 걸러 준다(`workspace.rs:719`). 이유: 순수 함수라 단위 테스트가 쉽고 메뉴와 카드가 어긋나지 않는다.
- **KTD2. 위젯은 `forge-widgets`에 하나.** `crates/forge-widgets/src/recent_folders_card.rs`(새)의 `RecentFoldersCard` 컴포넌트가 `use_workspace()`와 `use_new_session_draft()`로 읽고, 동작은 `FolderChip`의 `onpick`과 같은 호출(`set_new_folder`, `open_folder`, `refocus_composer`)을 쓴다. `NewSessionPanel`이 `NewSessionComposer` 바로 앞에 놓는다. 이유: 동작을 복제하는 것이 아니라 같은 워크스페이스 메서드를 부른다.
- **KTD3. 위치.** 카드는 `.new-session-pane` 안의 입력창 위, 입력창과 같은 열(`--chat-column`, 가운데 정렬). 인사말과 `UsageCard`가 있는 `.home-top`은 건드리지 않는다.

---

## Implementation Units

### U1. 카드 행을 고르는 순수 함수

- **Goal:** R2.
- **Files:** `crates/forge-features/src/folder_menu.rs`, `crates/forge-features/src/lib.rs`(내보내기).
- **Approach:** KTD1.
- **Test scenarios:** (1) 프로젝트 5개 → 앞 3개, 순서 유지. (2) 프로젝트가 3개 미만이면 있는 만큼만. (3) 현재 폴더가 4번째면 카드는 3개이고 마지막이 현재 폴더이며 `checked`. (4) 프로젝트 없음 → 빈 목록. (5) 현재 폴더가 앞 3개 안이면 그 행만 `checked`.
- **Verification:** `cargo test -p forge-features folder_menu`.

### U2. 카드 컴포넌트와 배치

- **Goal:** R1, R3, R4, R5, R6, R7.
- **Files:** `crates/forge-widgets/src/recent_folders_card.rs`(새), `crates/forge-widgets/src/lib.rs`, `crates/forge-widgets/src/new_session_panel.rs`, `crates/forge-shared/src/theme.css`(`.recent-folders` 규칙, 기존 `.picker-menu`와 카드 토큰 재사용), `crates/forge-shared/src/ko.rs`(`Choose a folder` 등 기존 문장을 재사용하고, 없으면 `Recent folders`, `Choose folder` 추가), 필요하면 영어 표.
- **Approach:** KTD2, KTD3. 행 버튼은 `<button type="button">`, 접근성 이름은 폴더 이름, 체크는 기존 체크 아이콘. 행이 없으면 `rsx! {}`. 마지막 행의 `⌄`은 기존 아이콘 종류를 쓴다.
- **Patterns to follow:** `FolderChip`(`new_session_controls.rs`)의 `onpick`, `Picker` 메뉴 행 CSS.
- **Test scenarios:** 번역 표 테스트(모든 새 문장이 두 언어에 있다)를 통과한다. 하네스: AE1, AE2, AE3, 좁은 창(`resize.py`)과 다크 모드에서 카드가 입력창을 가리지 않는다.
- **Verification:** `cargo test`, 하네스 스크린샷(라이트, 다크, 좁은 창), `scripts/harness/ui_lint.py`가 있으면 통과.

---

## Risks and Mitigations

- **카드와 아래 줄의 폴더 컨트롤이 같은 정보를 두 번 보여 준다.** 데스크톱 앱도 같다(카드 + 아래 줄의 폴더 이름). 완화: 카드는 한 번의 클릭 경로이고 아래 줄은 현재 값 표시. 유지.
- **낮은 창에서 카드가 인사말이나 입력창과 겹친다.** 완화: `.new-session-pane`의 스크롤 규칙(`overflow-y: auto`)을 쓰고 하네스로 낮은 창을 확인한다.
- **`new_session_projects()`가 느리게 채워져** 첫 그림에서 카드가 없다가 나타난다. 완화: R5의 "없으면 숨김"으로 깜빡임 없이 나타나게 하고, 나타날 때 입력창이 밀리지 않도록 카드를 입력창 위 흐름에 둔다.

---

## Sources / Research

- 관찰: `docs/superpowers/roadmap/observations.md` Round 35 (데스크톱 앱 새 세션 화면 스크린샷 두 장, Forge 하네스 스크린샷).
- 코드: `crates/forge-features/src/folder_menu.rs`, `crates/forge-widgets/src/new_session_controls.rs`(`FolderChip`), `new_session_panel.rs`, `crates/forge-features/src/workspace.rs`(`new_session_projects`, `set_new_folder`, `open_folder`).
- 앞선 계획: `docs/plans/2026-10-31-1130-feat-composer-unify-desktop-parity-plan.md`(같은 화면의 입력창 정리, 이 계획은 그 위에 얹는다).

## Verification Contract

`cargo test` 전체 통과, 하네스에서 AE1~AE3 확인(라이트, 다크, 좁은 창), 번역 표 테스트 통과.

## Definition of Done

U1, U2가 끝나고 위 검증이 통과하며, 카드가 데스크톱 앱 화면과 나란히 놓였을 때 같은 자리와 같은 구성(행 셋 + 폴더 선택 행)임이 하네스 캡처로 확인된다.

## Deferred / Open Questions

- 환경(`draft.environment`)이 정해졌을 때 카드 자리에 환경 행을 보일지는 이번 범위가 아니다.
