# Run button on shell code blocks (NEW-22)

Spec for Plan 71. Source: observation round 34 in `docs/superpowers/roadmap/observations.md`, backlog row NEW-22. The goal is the same behaviour as the Claude Code desktop app. Statements marked **(my call)** were not seen in the desktop app or were not decided by the user; they are the author's choice and may be changed.

## Why

In the desktop app a shell code block in an answer has a `▷` next to the copy icon. One press runs the block in the session's terminal and the result comes back into the chat, and the model is told what happened. Forge shows only the copy icon, so a person has to copy the command, open the terminal and paste it. The user asked for Forge to behave the same as the desktop app.

## What the desktop app does (seen in round 34)

- The `▷` sits in the block's header row, left of the copy icon.
- Pressing it runs the text **at once**, with no confirmation card, also in the `수동` permission mode. The model is not asked.
- The command runs in the session's own terminal: the dock terminal tab (`터미널 <session folder>`) shows `❯ echo …` and the output. The working directory is the session folder.
- A result box appears directly under the code block: same width, thin border, grey monospace. It holds the output, then the working directory line and a faint time at the right, and a small `>_` icon at the top right. The box stays while the conversation goes on.
- `>_` opens the dock terminal.
- When the run is over, a new model turn starts by itself with no user bubble and nothing typed. The model answers about the result (``echo … 를 실행했고 … 가 출력됐습니다. 오류는 없었습니다``).

## Goals

1. Shell code blocks get `▷` left of the copy icon. A click runs the block in the terminal of that chat's session.
2. A result box under the block shows the output, the working directory, the time, and a `>_` icon that opens the dock terminal.
3. When the command ends, the model receives the command, its output and its exit code as a new turn, shown to the person as no message of their own.
4. Same behaviour with several panes: the click runs in the session of the pane that was clicked, as file links do.

## Non-goals

- No confirmation question, no permission card, no setting to turn the button off. This is the desktop behaviour and the user chose to copy it. **Work PCs:** the memory rule for strict-policy Windows PCs says to skip OS-level features; this is not one (it types into Forge's own terminal), so it is built.
- No `▷` on blocks that are not shell code (python, json, text, no language). **(my call)** The desktop app was seen with a bash block only.
- No run history, no re-run button on the box, no kill button beyond what the terminal already has (Ctrl-C in the dock terminal).
- Windows (cmd, PowerShell) is not built or checked. The marker described below is written for POSIX shells. The button is hidden on Windows until that is done. **(my call)**

## Behaviour

**Which blocks.** A fenced block whose language is one of `bash`, `sh`, `zsh`, `shell`, `console`, `shellscript`, `terminal`. **(my call)** The block text is what is typed; a leading `$ ` prompt on every line is not removed.

**Where it runs.** The session's dock terminal. If the Terminal panel is closed it is opened first; if it has no tab a tab is opened; if it has tabs the active tab is used. The working directory is the session's folder, as the panel already does. Typing happens only after the shell has started and reported its size.

**What is typed.** The block text, then a line that prints an end marker that the person never sees: an OSC escape sequence that carries the exit code of the block (for zsh and bash, `printf '\033]777;forge-run;%s;<run id>\007' "$?"`). **(my call)** A multi-line block is typed as it is, in one go, so that it runs as the person wrote it. The run id ties the marker to one box, so two blocks run in a row do not mix.

**Capturing the result.** The terminal view already reads every byte of output (`TerminalTap`). From the moment the block is typed until the marker is seen, the bytes are collected; terminal control codes and the typed echo of the command are removed so the box holds only what the command printed. **(my call)** The box shows at most the last 200 lines; the full output stays in the terminal. The box scrolls inside itself above a fixed height.

**The box.** While the command runs: the output so far, updated about four times a second, and a running mark in the top right corner in place of `>_`. When it ends: the output, a line with the working directory (blue, the same blue as links) and the time of the end at the right, and `>_`. A non-zero exit code shows as a red `exit N` in that line. **(my call)** The desktop app also printed the user's shell prompt (`(base)`); Forge does not, because it depends on the person's shell setup. The box belongs to the block that was run and is not saved: after a restart, the block shows without it. **(my call, not observed)**

**Telling the model.** When the marker arrives, the session gets one new turn whose text states the command, the exit code and the output (the last 200 lines, cut at 20 000 characters), in a fixed plain format. **(my call)** If the session is running another turn at that moment, the message is queued the way the composer queue does it (Plan 58B) and sent when that turn ends. The turn is not drawn as a user message. If the model answers, the answer shows as usual.

**Stopped or lost runs.** If the terminal tab is closed or the shell exits before the marker, the box ends with `종료됨` (the shell ended) and the model is told the same, with the output so far. **(my call)** A command that never ends leaves the box in its running state; closing the terminal tab ends it as above.

**A second press** on the same block while it runs does nothing. A press on another block while one runs queues the typed text behind it in the same shell; it runs when the first command returns to the prompt. **(my call)**

## Constraints from the code (read on 2026-10-29)

- Code blocks are plain HTML made by `crates/forge-entities/src/markdown.rs` (`code_block_html`), with the copy button as a fixed `onclick` script. Clicks on file links reach Rust through one delegated handler in `crates/forge-widgets/src/file_link_watcher.rs` that sends `"<pane index>\n<data>"`. The run button should reach Rust the same way, with a separate handler; the block text is read from the page, never put in an attribute that a message could forge. The same HTML is used by the side panel, the plan panel and the Customize pages; `▷` must show in chat messages only.
- The terminal lives only while its panel tab is mounted (`terminal_panel.rs`, `drive`, closed on drop). The panel owns the tabs and the stream of output. The feature therefore needs a per-session list of pending runs that the mounted terminal tab picks up and answers, not a direct write from the chat.
- `TerminalTap` (`terminal_tap.rs`) already lets code listen to a terminal and type into it, and `sign_in_run.rs` uses it. The new feature may reuse it or extend it.
- The model turn goes through the sidecar; how to send a turn without a visible user message must be found by the builder (see risks).

## Risks and open points left to the builder

- **A turn with no bubble.** After a restart, the transcript file will hold that turn as a user message and Forge would show it as one. The builder decides how to hide it (a recognised prefix in the text, for example) or accepts a visible grey line. A visible line is allowed if hiding is not cheap. **(my call)**
- **Echo removal.** The shell echoes the typed command; removing exactly that echo from the collected bytes must also work when the line wraps in a narrow terminal. If it cannot be done cleanly, the builder may hide the echo by typing the command with echo off (`stty -echo` around it) instead.
- **zsh and bash only for the marker.** `fish` and other shells print a different `$?`. The button still types the command; if no marker comes, the run ends only by the closing of the tab. The builder may detect the shell from `$SHELL` and skip the button for shells it cannot mark.
- **Interactive programs** (`vim`, `less`, `ssh`) make the output meaningless in a box. Not handled; the person sees them in the terminal.
- **Tests.** Pure parts (marker parser, control code stripping, output cut, the shell block language test, the model message format) get unit tests. The click path and the terminal pick-up are checked in the real window with the harness; the harness needs a sample answer with a bash block (`scripts/harness/add_*_samples.py` pattern).
- **Not checked in the desktop app** (see round 34): failing commands, endless commands, multi-line blocks, non-shell blocks, long output, whether the box survives a restart. Every one of these is **(my call)** above.
- **Translation.** New visible words (`종료됨`, `exit N`, the running mark's title, button titles) go through `tr!` and `ko.rs` like every other string.

## Calls made after the builder check (all **my call**)

1. **The turn without a bubble.** The sidecar has no hidden-turn path (checked: no such flag in `sidecar/src`), and none is added. The result goes through the normal send path as a user message whose text starts with the fixed line `[forge-run]` (English, so the matcher does not depend on the language setting), then the command, `exit <code>` and the output in a fenced block. The chat does not draw a user message that starts with `[forge-run]`, live or after a reload of the transcript. If the sidecar or the session reader makes this hard, the visible grey line is the accepted fallback; nothing larger is built for it.
2. **Terminal not mounted, or another session's dock.** A run is stored with the session id of the pane that was clicked. Pressing `▷` opens the Terminal panel in that session's dock if it is closed (the dock of a narrow window is the floating one of Plan 26; it opens as the dock button does). The terminal tab of that session picks the run up when it mounts. A run that has not started after 10 seconds ends as `종료됨` and the model is told.
3. **Echo.** The dock terminal keeps showing the typed command, as in the desktop app. The box is cut by two markers instead of by removing the echo: the first typed line starts with a `printf` of a start marker, so what follows the start marker is output; the end marker comes last. For a multi-line block the echo of the later lines can appear in the box; that is accepted. No `stty`.
4. **Message format.** As in call 1; the output is the last 200 lines, cut at 20 000 characters, with `(cut)` on a line of its own when something was cut.
5. **Where `▷` shows.** `render_markdown` gets no new switch for panels: the run button is in the HTML for every user of it, and a CSS rule hides `.code-run` outside the chat column (`.chat-window .code-run` is the only place that shows it). The click handler is installed once, like the file-link handler, and only acts inside a chat pane (`[data-pane]`).
6. **Other shells.** The button is shown only when `$SHELL` is zsh or bash (read once when the app starts). With any other shell the button is not drawn, so there is no run that cannot end. On Windows it is not drawn either.
