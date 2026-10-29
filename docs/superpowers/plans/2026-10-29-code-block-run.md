# Run Button on Shell Code Blocks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `▷` on shell code blocks in the chat runs the block in the session's dock terminal, shows the result in a box under the block, and tells the model, as the desktop app does.

**Architecture:** Pure parts (shell block test, typed line, marker scanner, output cleaning, model message, hide test) live in one new entity module. A per-session run store in the workspace holds runs; a `TerminalTap` on the dock's terminal tabs types the next run into the active tab and feeds the output back into the store. A delegated click handler (like the file-link one) starts runs; the result boxes are drawn into the chat HTML by one idempotent script fed from the store; a finished run is sent to the model through `LiveSessionHandle::queue` as a `[forge-run]` message that the chat does not draw.

**Tech Stack:** Rust (Dioxus 0.6, `pulldown_cmark` already used by `markdown.rs`), `document::eval` bridges, the harness in `scripts/harness/`. No sidecar change.

**Spec:** `docs/superpowers/specs/2026-10-29-code-block-run-design.md` (read it first, including its last section "Calls made after the builder check").

## Global Constraints

- Shell languages: `bash`, `sh`, `zsh`, `shell`, `console`, `shellscript`, `terminal`. The button shows only when `$SHELL` ends in `zsh` or `bash` and the OS is not Windows; otherwise no `▷` is drawn.
- Markers (OSC 777, invisible): start `\x1b]777;forge-run-start;<id>\x07`, end `\x1b]777;forge-run-end;<id>;<exit code>\x07`. The typed first line starts with a `printf` of the start marker; the last typed line is a `printf` of the end marker with `$?`.
- Box and message: output = last 200 lines, cut at 20 000 characters, `(cut)` on its own line when something was cut. A run not started after 10 seconds ends as `종료됨`.
- Model message starts with the line `[forge-run]` (English, not translated), then the command, `exit <code>`, and the output in a fenced block. A user message starting with `[forge-run]` is not drawn (chat, queued card, rewind list).
- `render_markdown` gets no new parameter; `.code-run` is in the HTML for every user of it and shows only under `html[data-code-run="on"] .messages`.
- Every new sentence goes through `tr!` with a Korean entry in `crates/forge-shared/src/ko.rs` (sorted; `every_sentence_in_the_sources_has_a_korean_text` checks it).
- Windows is not built or checked. Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

- A block whose text has no trailing newline, or ends with a blank line: one run, no double Enter. Pinned in Task 1.
- Output that holds a half marker split between two terminal reads: no marker is lost and no garbage reaches the box. Pinned in Task 1.
- Output with colours, cursor moves and `\r` progress lines: the box shows plain text only. Pinned in Task 1.
- Two identical blocks in one answer: both show the box of the run (same text, same hash); a run of one never shows output of another run. Pinned in Task 2 and Task 3.
- The terminal tab closes or the shell exits before the end marker: the box ends as `종료됨` and the model is told once. Pinned in Task 3.
- The session ends or the chat is closed while a run is open: nothing is sent to a closed session. Pinned in Task 5.
- A press while another run is open in the same session: it waits and starts after the first one ends. Pinned in Task 3.
- A `[forge-run]` message in an old transcript: not drawn, and the next assistant text still shows. Pinned in Task 4.

## File Structure

- Create `crates/forge-entities/src/code_run.rs` (pure parts), `crates/forge-features/src/code_runs.rs` (store and tap), `crates/forge-widgets/src/code_run_watcher.rs` (click bridge, box drawing, shell check).
- Modify `crates/forge-entities/src/lib.rs`, `crates/forge-entities/src/markdown.rs`, `crates/forge-entities/src/turn_view.rs`, `crates/forge-entities/src/rewind_view.rs`, `crates/forge-features/src/lib.rs`, `crates/forge-features/src/workspace.rs`, `crates/forge-features/src/queued_card.rs`, `crates/forge-widgets/src/lib.rs`, `crates/forge-widgets/src/terminal_panel.rs`, `crates/forge-widgets/src/chat_window.rs` (mount the watcher where `FileLinkWatcher` is mounted), `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/ko.rs`.
- Create `scripts/harness/add_code_run_samples.py`; modify `docs/superpowers/roadmap/backlog.md`, `docs/superpowers/roadmap/README.md`.

### Task 1: Pure parts in `forge-entities`

**Files:**
- Create: `crates/forge-entities/src/code_run.rs`; Modify: `crates/forge-entities/src/lib.rs` (`pub mod code_run;` and re-exports like its neighbours)

**Interfaces:**
- Produces:
  - `pub fn is_shell_language(language: &str) -> bool`
  - `pub fn typed_text(id: u64, block: &str) -> Vec<u8>` (start-marker line, the block with one trailing newline, end-marker line; every line ends `\n`)
  - `pub enum RunEvent { Started(u64), Ended { id: u64, code: i32 }, Text(String) }`
  - `pub struct MarkerScanner` with `new()` and `feed(&mut self, bytes: &[u8]) -> Vec<RunEvent>`; `Text` is plain output between markers with control codes removed (CSI and OSC sequences dropped, `\r\n` to `\n`, a lone `\r` drops the line text before it)
  - `pub fn cut_output(text: &str) -> (String, bool)` (last 200 lines, then at most 20 000 characters from the end; bool = something was cut)
  - `pub fn model_message(block: &str, code: Option<i32>, output: &str, cut: bool) -> String` (`code: None` = the shell ended; line reads `shell ended`)
  - `pub fn is_run_message(text: &str) -> bool` (first line is exactly `[forge-run]` after trimming the start)
  - `pub fn text_hash(block: &str) -> String` (hex of FNV-1a 64 over the trimmed text; the key of the box)

- [ ] **Step 1: Write the failing tests** in the module's `tests`: `shell_languages_are_the_listed_ones` (`bash` yes, `python` no, `Bash` yes: case-insensitive); `typed_text_wraps_the_block_between_two_marker_lines` (assert starts with `printf '\033]777;forge-run-start;7\007'`, ends with the end `printf` containing `"$?"`, and a block with and without a trailing newline give the same bytes); `scanner_returns_started_text_ended` (feed `ESC]777;forge-run-start;7 BEL hi\r\n ESC]777;forge-run-end;7;0 BEL`, expect `[Started(7), Text("hi\n"), Ended{id:7,code:0}]`); `scanner_keeps_a_marker_split_between_two_feeds` (split at every byte offset of the same input, same events concatenating text); `text_has_colours_and_cursor_codes_removed` (`ESC[31mred ESC[0m ESC[2K` gives `red `); `a_progress_line_keeps_the_last_state` (`10%\r50%\r100%\n` gives `100%\n`); `output_is_cut_to_the_last_200_lines_and_20000_characters` (300 lines gives 200 and `true`; 30 000 characters gives at most 20 000 and `true`; short text gives `false`); `model_message_has_the_fixed_first_line` (starts `[forge-run]\n`, contains the block, `exit 2`, a fenced output, `(cut)` only when cut; `None` code gives `shell ended`); `run_message_test` (`[forge-run]\nx` yes, ` [forge-run]\nx` yes, `please [forge-run]` no, `[forge-run]x` no); `hash_ignores_surrounding_whitespace_and_differs_for_other_text`.
- [ ] **Step 2: Run to see them fail:** `cargo test -p forge-entities code_run` → FAIL (module missing).
- [ ] **Step 3: Implement the functions above.** `MarkerScanner` keeps a byte buffer for an unfinished escape sequence and for a split UTF-8 character; `Text` is emitted only while a run is open (between `Started` and `Ended`), bytes outside a run are dropped.
- [ ] **Step 4: Run:** `cargo test -p forge-entities code_run` → PASS.
- [ ] **Step 5: Commit** `feat: pure parts of the code block run (markers, cleaning, model message)`.

### Task 2: The `▷` button in the HTML and its CSS

**Files:**
- Modify: `crates/forge-entities/src/markdown.rs` (`code_block_html`), `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `is_shell_language`, `text_hash` (Task 1)
- Produces: for a shell block, one `<button class="code-run" type="button" title="Run" data-hash="<hash>">▷</button>` placed **after** the copy button and before `<pre>`; the existing copy button and its `starts_with` test stay as they are. A block with another language or none has no run button.

- [ ] **Step 1: Write the failing tests** in `markdown.rs`: `a_bash_block_has_a_run_button_after_the_copy_button` (html contains `class="code-run"` after `class="code-copy"` and `data-hash="`), `a_python_block_has_no_run_button`, `a_block_without_a_language_has_no_run_button`, `the_run_button_has_no_script_and_no_message_text` (no `onclick` on `.code-run`, and a block holding `"><script>` does not put it in the attribute).
- [ ] **Step 2: Run:** `cargo test -p forge-entities markdown` → the new tests FAIL.
- [ ] **Step 3: Implement** in `code_block_html`; title through plain text `Run` (set at runtime by the watcher with `tr!`, so `markdown.rs` stays free of `tr!`). In `theme.css`: `.code-run { display: none; }` and `html[data-code-run="on"] .messages .code-run { display: inline-block; … }` styled and placed like `.code-copy` but `right: 36px`; shown with the same hover opacity rule as `.code-copy`.
- [ ] **Step 4: Run** the same command → PASS.
- [ ] **Step 5: Commit** `feat: run button on shell code blocks`.

### Task 3: Run store and terminal tap in `forge-features`

**Files:**
- Create: `crates/forge-features/src/code_runs.rs`; Modify: `crates/forge-features/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 items; `TerminalTap` (`crates/forge-features/src/terminal_tap.rs`)
- Produces:
  - `pub struct CodeRuns` (cloneable handle over `Rc<RefCell<…>>`, one per session) with: `start(&self, block: &str, now: Instant) -> u64` (returns the run id, state Waiting), `snapshot(&self) -> Vec<RunView>`, `take_finished(&self) -> Vec<Finished>` (each finished run is returned once)
  - `pub struct RunView { pub id: u64, pub hash: String, pub state: RunState, pub output: String, pub cut: bool }`; `pub enum RunState { Waiting, Running, Done { code: i32 }, Ended }` (`Ended` = shell ended or timed out)
  - `pub struct Finished { pub block: String, pub code: Option<i32>, pub output: String, pub cut: bool }`
  - `pub struct RunTap` implementing `TerminalTap`, created with `RunTap::new(runs: CodeRuns, active: Rc<Cell<bool>>, now: impl Fn() -> Instant)`; `output` feeds a `MarkerScanner` and routes events to the open run; `tick` returns `typed_text` of the next Waiting run when `active.get()` and no run is Running, and ends Waiting runs older than 10 seconds as `Ended`; `shell_ended(&self)` ends the Running run as `Ended`
  - `pub fn shell_supported(shell: Option<&str>, windows: bool) -> bool`

- [ ] **Step 1: Write the failing tests**: `a_started_run_is_typed_on_the_next_tick_of_an_active_tab_only`; `output_between_markers_fills_the_run_and_the_end_marker_finishes_it` (code 3 gives `Done{code:3}` and one `Finished`, a second `take_finished` is empty); `pressing_the_same_block_again_while_it_is_waiting_or_running_starts_nothing` (`start` with the same text returns the open run's id and the store holds one run); `a_second_run_waits_until_the_first_ended` (tick returns nothing while the first runs, then its text after the end marker); `a_run_not_started_in_ten_seconds_ends` (clock injected; at 10 s `Ended`); `a_shell_that_ends_ends_the_running_run_with_no_code` (`Finished.code == None`); `output_of_one_run_never_lands_in_another` (two runs with the same text, markers carry different ids); `snapshot_cuts_output_the_way_the_message_does` (300 lines gives 200 and `cut`); `shell_support` (`/bin/zsh` yes, `/usr/bin/bash` yes, `/usr/bin/fish` no, `None` no, any shell with `windows = true` no).
- [ ] **Step 2: Run:** `cargo test -p forge-features code_runs` → FAIL.
- [ ] **Step 3: Implement** the types above in `code_runs.rs`; run ids come from a counter in the store; the store keeps at most the last 50 runs per session (older ones drop).
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat: run store and terminal tap for code block runs`.

### Task 4: Hide the run messages

**Files:**
- Modify: `crates/forge-entities/src/turn_view.rs` (`build_turns`, user branch), `crates/forge-entities/src/rewind_view.rs:29`, `crates/forge-features/src/queued_card.rs`

**Interfaces:**
- Consumes: `is_run_message` (Task 1)

- [ ] **Step 1: Write the failing tests**: in `turn_view.rs` `a_run_message_is_not_drawn_and_the_answer_after_it_still_shows` (messages: user "hi", assistant "a", user `[forge-run]\n…`, assistant "b" → no `Item::User` for the run message, a text block "b" present); `the_end_card_of_an_earlier_turn_is_not_confused_by_a_run_message` (a `wrap_up` card turn followed by a run message keeps the card's Live/Spent rule: the run message does not count as a reply); in `rewind_view.rs` `a_run_message_is_not_a_rewind_point`; in `queued_card.rs` `a_queued_run_message_has_no_card` (find the function that lists queued rows; read it first).
- [ ] **Step 2: Run:** `cargo test -p forge-entities turn_view rewind_view && cargo test -p forge-features queued_card` → FAIL.
- [ ] **Step 3: Implement.** In `build_turns` still call `finish_turn` for a run message but push no `Item::User`. Filter the other two places the same way.
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat: forge-run messages are not drawn`.

### Task 5: Workspace wiring, the click bridge, the boxes, the model message

**Files:**
- Create: `crates/forge-widgets/src/code_run_watcher.rs`; Modify: `crates/forge-features/src/workspace.rs`, `crates/forge-widgets/src/terminal_panel.rs`, `crates/forge-widgets/src/chat_window.rs`, `crates/forge-widgets/src/lib.rs`, `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: Task 3 (`CodeRuns`, `RunTap`, `RunState`, `Finished`, `shell_supported`), Task 1 (`model_message`, `text_hash`)
- Produces:
  - `Workspace::code_runs(&self, id: LiveSessionId) -> CodeRuns` (a map in a root signal like `end_card_marks`, created on first use, removed when the session is removed)
  - `Workspace::run_code(&self, id: LiveSessionId, block: &str)` (starts the run, opens the Terminal panel with `open_panel_over_auto_details(id, Panel::Terminal)`, and returns; the panel mounts one tab when it has none)
  - `Workspace::code_run_available(&self) -> bool` (`shell_supported(std::env::var("SHELL").ok().as_deref(), cfg!(windows))`)
  - component `CodeRunWatcher` (mounted next to `FileLinkWatcher`)
  - `TerminalPanel` gives every dock `TerminalTab` a `RunTap` of its session whose `active` cell follows the tab's `active` prop; `TerminalTab`'s `drive` calls `tap.shell_ended()` (new method on `TerminalTap` with a default empty body) when the shell exits or the tab is dropped.

- [ ] **Step 1: Write the failing tests**: in `code_run_watcher.rs` `the_handler_names_the_pane_and_sends_the_block_text` (JS constant contains `removeEventListener('click'`, `.code-run`, `closest('[data-pane]')`, `dioxus.send`, and reads text from the `pre` of the same `.code-wrap`, not from an attribute); `the_box_script_is_idempotent` (the JS constant looks the box up by `data-run` before creating one); in `workspace.rs` tests `run_code_opens_the_terminal_panel_and_stores_a_waiting_run` and `a_finished_run_is_sent_once_to_the_session_as_a_queued_forge_run_message` (use the existing test helpers for a live session with a fake sidecar; read how the Plan 70 end-card tests do it).
- [ ] **Step 2: Run:** `cargo test -p forge-widgets code_run_watcher && cargo test -p forge-features workspace::tests::run_code` → FAIL.
- [ ] **Step 3: Implement.** `CodeRunWatcher` (a) sets `document.documentElement.dataset.codeRun = 'on'` when `code_run_available()`, and sets each `.code-run` title to `tr!("Run")` through the same script, (b) installs the click handler (replace-on-install, as `WATCH_FILE_LINKS`) that sends `"<pane>\n<block text>"` and calls `workspace.run_code` for the pane's live session (`pane_target(index)`), (c) every 250 ms sends the boxes of every shown pane's session to one script `applyRunBoxes`, which for each `.code-run[data-hash=H]` makes sure a `.run-box[data-run=ID]` follows its `.code-wrap` and sets its content: output (scrolling above a fixed height), a line with the session folder in link blue and the time `HH:MM:SS`, `>_` button (sends `"terminal"` to open the panel), a running mark while `Running` or `Waiting`, red `exit N` for a non-zero code, `종료됨` for `Ended`. (d) after each tick, for each `take_finished()` item it calls `handle.queue(model_message(...), Vec::new())` on the session's `LiveSessionHandle`, only when the session is still open. Add the Korean entries to `ko.rs` in sorted place.
- [ ] **Step 4: Run** the Step 2 commands and `cargo test -p forge-shared every_sentence_in_the_sources_has_a_korean_text` → PASS.
- [ ] **Step 5: Run the whole suite:** `cargo test --workspace` (the plugin test `round_trip` is known to be flaky; rerun it alone before calling it a failure) and `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] **Step 6: Commit** `feat: run button runs shell blocks in the dock terminal and tells the model`.

### Task 6: Look at it in the real window, then close the roadmap rows

**Files:**
- Create: `scripts/harness/add_code_run_samples.py` (a session whose last answer has a bash block `echo forge-run-hello`, a two-line block, a block that exits 3, and a python block; pattern of `add_turn_cards_samples.py`); Modify: `docs/superpowers/roadmap/backlog.md` (NEW-22 row: `Plan 71 (built, window checked)`), `docs/superpowers/roadmap/README.md` (row 71)

- [ ] **Step 1:** Launch with `FORGE_HARNESS_EXTRA=scripts/harness/add_code_run_samples.py scripts/harness/launch.sh` (symlink `sidecar/node_modules` from `/Users/james/Developer/forge/sidecar/node_modules` first if the sidecar dies). Press `▷` on each block and capture. Check: `▷` is left of the copy icon and only on shell blocks; the Terminal panel opens by itself; the box shows output, the folder line and time, `>_`; exit 3 shows red `exit 3`; the two-line block runs once; the python block has no `▷`; `>_` opens the terminal; a second press while the first runs waits; no `[forge-run]` bubble appears; the model's answer shows (a real session, throwaway folder).
- [ ] **Step 2:** Fix what the window shows, with a test for each real defect, commit each fix.
- [ ] **Step 3:** Update the two roadmap files and commit `docs: Plan 71 built, NEW-22`. Leave dark mode and Windows as listed in the notes of the row.
