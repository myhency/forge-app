---
title: "feat: UI kit, auto height, reviewed examples and an evaluation loop for interactive pages"
type: feat
date: 2026-10-31
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-brainstorm
execution: code
---

# feat: raise the quality of interactive pages (items 1-4 of the quality note)

## Goal Capsule
- **Objective:** An interactive page Claude draws looks designed and consistent, fits its content, and the quality can be measured before and after a change.
- **Means:** a Forge UI kit injected into every frame (KTD1), a frame-reported height (KTD2), a `ui_guide` tool that hands Claude the kit reference and reviewed example pages (KTD3), and an evaluation loop of fixed prompts, a page linter and the harness (KTD4).
- **Source:** `docs/superpowers/specs/2026-10-30-generative-ui-quality.md`, items 1 to 4, chosen by the user ("1,2,3,4 진행해").
- **Execution profile:** one change on the open PR branch.

## Product Contract
- R1. Every frame carries the kit: layout, card, title, field, input, range, button, stat tile, table, callout, steps, tabs, checklist, progress bar, badge, six chart colours, in both themes. Tabs work with no page script.
- R2. A card follows its page's height (80 to 1400 px) with no page code; `initialHeight` is only the starting height.
- R3. Claude can call `ui_guide` (topic `kit` or the name of a reviewed example) and get the kit's class list, design rules and a complete example call to adapt. The description of `generate_sandboxed_ui` tells it to do so before its first page.
- R4. At least seven reviewed examples exist as data files, are shown by the harness, and each passes the linter.
- R5. A linter scores any page (hosts, kit use, blocked map libraries, height, images without `onerror`, size); a fixed list of prompts can be run through the real sidecar and model, linted, and rendered in the real window for review.

## Planning Contract
- KTD1: one CSS file (`crates/forge-entities/src/ui_kit.css`, `include_str!`) is the source of the kit; a Rust test pins that every `fx-` class in it is documented in `sidecar/src/ui_examples/kit.md` and the reverse; a sidecar test pins that every `fx-` class used by an example exists in the CSS. Tabs are wired by the bridge script (`.fx-tabset`, `.fx-tab[data-tab]`, `.fx-panel[data-panel]`).
- KTD2: the bridge script observes the body and posts `{n, resize: px}`; the parent listener (identified by `event.source`, no focus needed because it can only change a height) clamps and sets the iframe height. Pages must not size by `vh`.
- KTD3: examples are JSON files under `sidecar/src/ui_examples/` (compiled into `dist` so packaging, which copies only `dist`, carries them; imported with JSON import attributes). The harness and the evaluation read the same files, so the examples are also the quality baseline.
- KTD4: `sidecar/scripts/eval_ui.mjs` drives the real SDK with the `forge_ui` server and writes each prompt's tool calls to a run folder; `scripts/harness/ui_lint.py` scores them (its host list is parsed from `generated_ui.rs`, one source); `scripts/harness/add_eval_samples.py` turns a run into harness sessions for screenshots. Real model runs cost tokens, so they are on request, with `--limit`.

## Implementation Units
- U1. UI kit: `ui_kit.css`, theme tokens, tab wiring, injection in `page_document`, tests (Rust).
- U2. Auto height: bridge script + parent listener, tests (Rust).
- U3. `ui_guide` tool, `kit.md`, seven examples, description, allowed tool, tests (sidecar); harness samples from the examples.
- U4. `ui_lint.py` with tests, `ui_eval_prompts.json`, `eval_ui.mjs`, `add_eval_samples.py`, docs.

## Verification Contract
`cargo test -p forge-entities`, `cd sidecar && npx tsc --noEmit && npx vitest run`, `python3 -m unittest discover -s scripts/harness`, real window (each example renders, tabs switch, card heights follow content), and one small real-model eval run (`--limit 3`) if the login works.

## Definition of Done
R1-R5 met; examples render in the real window in light mode; the eval run (or why it could not run) is reported.
