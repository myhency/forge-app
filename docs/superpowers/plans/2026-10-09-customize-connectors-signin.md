# Connectors Sign-in (Plan 30D) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A connector that "needs sign-in" can be signed in from its detail screen: a dialog holds a terminal, Forge types `claude`, `/mcp`, the arrow keys and Enter into it until the CLI opens the browser, and Forge checks the connection when the sign-in ends.

**Architecture:** A pure `SignInDriver` (in `forge-features`) reads the terminal's output (ANSI stripped) and answers with keys to type, a "done" or a "stop". A small `TerminalTap` trait lets the existing xterm bridge (`terminal_panel.rs`) hand output to the driver and write the keys it returns; the dock's terminal passes no tap and is unchanged. `Customize` owns the run and its state; a `Dialog::SignInConnector` shows the terminal, a status line and a Close button. The shell line that starts Claude comes from a new `McpManagerPort::launch_line`, so the bundled `claude` and its config folder are used.

**Tech Stack:** Rust, Dioxus 0.7.10 (desktop), tokio, xterm.js bridge already in `forge-widgets`, the bundled `claude` CLI.

**Spec:** `docs/superpowers/specs/2026-10-09-customize-connectors-signin-design.md` (also read the 30C spec `2026-10-09-customize-connectors-design.md` for the screens this extends).

## Global Constraints

- Keys Forge types are only: the launch line (`claude` as `launch_line` gives it) + Enter, `/mcp` + Enter, arrow Down (`\x1b[B`), Enter (`\r`). A connector name or URL is never put into a shell line.
- Step limit 15 s; limit while watching the browser sign-in 5 minutes. After a stop the driver types nothing more.
- Terminal output is read, never stored or logged. Tokens stay with the CLI (Keychain); Forge never touches them.
- Working directory of the shell: the folder of the project the Customize page was opened for (`Customize.project`), else the home folder.
- The dialog's backdrop does nothing for the sign-in dialog; Close ends the shell. A second "Sign in" press while the dialog is open does nothing.
- The dock terminal behaves exactly as before (no tap).
- Not checked, to be written in "As built": Windows, a proxy, servers that need a client id.
- Repo style: sentence-style test names; comments say why; no new crate dependency.

## Review Focus

- A folder the CLI has not been told to trust: the CLI asks "trust this folder?" before its input box. Forge must type nothing at that prompt and hand over after 15 s. (Task 4 test.)
- A server that is already signed in: its item says "Re-authenticate" / "Clear authentication", not "Authenticate". Forge must not press Enter on another item. (Task 4 test.)
- A name that appears twice on the list (a user and a project server of one name): stop, do not guess. (Task 4 test.)
- Output that is cut in the middle of an escape sequence or a Korean character (the repo's user types Korean): the text must still read right. (Task 3 test.)
- Output after a stop or a verdict: nothing is typed and nothing is reported again (Task 4 tests); a dialog closed while a run is going drops the tap, so the run cannot type into a new shell (Task 7, `close_sign_in`).

## File Structure

- Create `scripts/observe/claude_mcp_screen.py` — captures the real `/mcp` screens as raw bytes.
- Create `crates/forge-features/tests/fixtures/sign_in/{01_input,02_list,03_item}.bin` and `NOTES.md` — what the real screens look like (Task 1).
- Modify `crates/forge-ports/src/mcp_manager_port.rs` — `launch_line`.
- Modify `crates/forge-adapter-claude-plugins/src/mcp.rs` (+ `tests/mcp_cli.rs`) — implement it.
- Modify `crates/forge-application/src/use_cases/connector_catalog.rs` — pass it through.
- Create `crates/forge-features/src/sign_in.rs` — `PlainText` (ANSI strip) and `SignInDriver`.
- Create `crates/forge-features/src/terminal_tap.rs` — `TerminalTap`, `TapHandle`.
- Create `crates/forge-features/src/sign_in_run.rs` — `SignInRun` (driver + clock + report; implements `TerminalTap`).
- Modify `crates/forge-features/src/customize_connectors.rs` and `customize_state.rs` — `SignInState`, texts, `sign_in_connector`, `close_sign_in`, `Dialog::SignInConnector`.
- Modify `crates/forge-features/src/lib.rs` — modules and exports.
- Modify `crates/forge-widgets/src/terminal_panel.rs` — tap in `drive`, `SignInTerminal`.
- Modify `crates/forge-widgets/src/customize_connector_dialogs.rs`, `customize_dialogs.rs`, `customize_connectors.rs` — dialog and the "Sign in" button.
- Modify `crates/forge-shared/src/theme.css` — dialog and terminal size.
- Modify docs: spec "As built", `docs/superpowers/roadmap/README.md` and `backlog.md`.

---

### Task 1: Observe the real `/mcp` screens

This fixes every text the driver waits for. Do it before Task 4; the constants in Task 4 are the expected values and are edited to match what is seen here.

**Files:**
- Create: `scripts/observe/claude_mcp_screen.py`
- Create: `crates/forge-features/tests/fixtures/sign_in/01_input.bin`, `02_list.bin`, `03_item.bin`, `NOTES.md`

**Interfaces:**
- Produces: raw byte captures of three screens of the bundled `claude` and `NOTES.md` listing, for each: the exact visible text that marks it, the key that moves the selection, and the text after Authenticate (success and failure wording, read from a manual run in Task 8).

- [ ] **Step 1: Write the capture script**

```python
#!/usr/bin/env python3
"""Runs the bundled `claude` in a pty against a throwaway config folder that holds one server that needs
sign-in, types `/mcp`, Down and Enter, and saves the raw bytes seen after each step.
Usage: claude_mcp_screen.py <claude-binary> <out-dir>"""
import os, pty, select, subprocess, sys, tempfile, time

claude, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
home = tempfile.mkdtemp(prefix="forge-observe-")
env = dict(os.environ, CLAUDE_CONFIG_DIR=home, TERM="xterm-256color")
subprocess.run([claude, "mcp", "add", "-s", "user", "--transport", "http", "sentry", "https://mcp.sentry.dev/mcp"], env=env, check=True)

pid, fd = pty.fork()
if pid == 0:
    os.chdir(tempfile.mkdtemp(prefix="forge-observe-cwd-"))
    os.execve(claude, [claude], env)

def read_for(seconds):
    data, end = b"", time.time() + seconds
    while time.time() < end:
        ready, _, _ = select.select([fd], [], [], 0.2)
        if ready:
            try:
                data += os.read(fd, 65536)
            except OSError:
                break
    return data

def save(name, data):
    with open(os.path.join(out, name), "wb") as f:
        f.write(data)
    print(name, len(data), "bytes")

save("01_input.bin", read_for(8))
os.write(fd, b"/mcp\r")
save("02_list.bin", read_for(4))
os.write(fd, b"\r")
save("03_item.bin", read_for(4))
os.write(fd, b"\x1b")
time.sleep(0.5)
os.write(fd, b"\x03\x03")
time.sleep(0.5)
os.kill(pid, 9)
print("config folder (throwaway):", home)
```

- [ ] **Step 2: Run it against the bundled binary**

Run (from the repo root):

```bash
python3 scripts/observe/claude_mcp_screen.py "$PWD/sidecar/node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64/claude" crates/forge-features/tests/fixtures/sign_in
```

Expected: three `.bin` files, each larger than 0 bytes. If `01_input.bin` shows a folder trust question or a login question instead of the input box, note it in `NOTES.md` (the throwaway folder is new, so the trust prompt is the likely case): then run the script again after changing `os.chdir(...)` to a folder that is already trusted (for example this repo), and keep the captures of that run; also keep a copy of the trust screen as `00_trust.bin`.

- [ ] **Step 3: Read the captures and write `NOTES.md`**

Run: `python3 - <<'EOF'` with a loop that strips escape codes (`re.sub(rb'\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07]*\x07', b' ', data).decode('utf8','replace')`) and prints each file's visible text. Then write `NOTES.md` with, for each of the three screens: the text that marks it (the constants `INPUT`, `LIST`, `ITEM` of Task 4), which character marks the selected row (expected `❯`), whether one frame holds the whole list (so the last `❯` line is the current one), and the item text of a server that needs sign-in. Check that the Down key `\x1b[B` moves the selection by taking one more capture if the list has more than one row (add a second throwaway server in the script for that).

- [ ] **Step 4: Commit**

```bash
git add scripts/observe/claude_mcp_screen.py crates/forge-features/tests/fixtures/sign_in
git commit -m "test: raw captures of the real /mcp screens for the sign-in driver (Plan 30D)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: `launch_line` on the manager port

**Files:**
- Modify: `crates/forge-ports/src/mcp_manager_port.rs`
- Modify: `crates/forge-adapter-claude-plugins/src/mcp.rs`
- Test: `crates/forge-adapter-claude-plugins/tests/mcp_cli.rs`
- Modify: `crates/forge-application/src/use_cases/connector_catalog.rs`

**Interfaces:**
- Produces: `McpManagerPort::launch_line(&self) -> Result<String, McpManagerError>` (sync); `ConnectorCatalogUseCase::launch_line(&self) -> Result<String, ConnectorsError>`. The string is a shell line (unix): `'/abs/claude'` or `CLAUDE_CONFIG_DIR='<dir>' '/abs/claude'`.

- [ ] **Step 1: Write the failing adapter tests** (append to `tests/mcp_cli.rs`)

```rust
#[test]
fn the_launch_line_quotes_the_program_and_carries_the_config_folder() {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("it's claude");
    std::fs::write(&program, "").unwrap();
    let plain = ClaudeMcpCli::new(program.clone(), None);
    assert_eq!(plain.launch_line().unwrap(), format!("'{}'", program.display().to_string().replace('\'', r"'\''")));
    let with_dir = ClaudeMcpCli::new(program.clone(), Some(PathBuf::from("/tmp/a b")));
    assert!(with_dir.launch_line().unwrap().starts_with("CLAUDE_CONFIG_DIR='/tmp/a b' '"));
}

#[test]
fn the_launch_line_says_so_when_the_program_is_missing() {
    let cli = ClaudeMcpCli::new(PathBuf::from("/no/such/claude"), None);
    assert!(matches!(cli.launch_line(), Err(McpManagerError::BinaryMissing(_))));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-adapter-claude-plugins --test mcp_cli launch_line`
Expected: FAIL to compile, "no method named `launch_line`".

- [ ] **Step 3: Add the port method and the implementation**

In `mcp_manager_port.rs`, inside the trait after `check`:

```rust
    /// The shell line that starts the Claude Code Forge uses, with the config folder it uses. Typed into a
    /// terminal, so the person's `/mcp` sign-in lands in the state the checks read.
    fn launch_line(&self) -> Result<String, McpManagerError>;
```

In `mcp.rs` (inside `impl McpManagerPort for ClaudeMcpCli`, and a free function after the impl):

```rust
    fn launch_line(&self) -> Result<String, McpManagerError> {
        if !self.binary.exists() {
            return Err(McpManagerError::BinaryMissing(self.binary.display().to_string()));
        }
        let program = shell_quote(&self.binary.to_string_lossy());
        Ok(match &self.config_dir {
            Some(dir) => format!("CLAUDE_CONFIG_DIR={} {program}", shell_quote(&dir.to_string_lossy())),
            None => program,
        })
    }
```

```rust
/// Single quotes around the text; a quote inside it closes, escapes and opens again.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}
```

In `connector_catalog.rs`, after `check`:

```rust
    pub fn launch_line(&self) -> Result<String, ConnectorsError> {
        Ok(self.manager.launch_line()?)
    }
```

and a test in its `tests` module:

```rust
    #[test]
    fn the_launch_line_comes_from_the_manager() {
        let mut manager = MockMcpManagerPort::new();
        manager.expect_launch_line().returning(|| Ok("'/x/claude'".into()));
        let (config, _) = memory_config(&[]);
        let line = use_case(MockConnectorCatalogPort::new(), manager, config).launch_line().unwrap();
        assert_eq!(line, "'/x/claude'");
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-adapter-claude-plugins --test mcp_cli && cargo test -p forge-application connector_catalog`
Expected: PASS. If another type implements `McpManagerPort` (search: `grep -rn "McpManagerPort for" crates`), add the method there too.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-ports crates/forge-adapter-claude-plugins crates/forge-application
git commit -m "feat: the manager port gives the shell line that starts Claude with Forge's config folder (Plan 30D)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: `PlainText`, the terminal output as plain text

**Files:**
- Create: `crates/forge-features/src/sign_in.rs`
- Modify: `crates/forge-features/src/lib.rs` (add `mod sign_in;`)

**Interfaces:**
- Produces: `pub struct PlainText` with `new()`, `push(&mut self, bytes: &[u8])`, `text(&self) -> &str`, `clear(&mut self)`. Colour and cursor codes are dropped; cursor-forward (`CSI n C`) becomes a space, cursor-down (`CSI B`/`E`) a newline, `\r` is dropped; a multi-byte character or escape sequence cut between two `push` calls is joined; the text keeps the last 16 KiB.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn plain(chunks: &[&[u8]]) -> String {
        let mut text = PlainText::default();
        for chunk in chunks {
            text.push(chunk);
        }
        text.text().to_string()
    }

    #[test]
    fn colour_and_cursor_codes_are_dropped() {
        assert_eq!(plain(&[b"\x1b[1;32mhello\x1b[0m \x1b[2K\x1b[1Aworld"]), "hello world");
    }

    #[test]
    fn a_cursor_move_forward_reads_as_a_space_and_down_as_a_new_line() {
        assert_eq!(plain(&[b"a\x1b[2Cb\x1b[1Bc"]), "a b\nc");
    }

    #[test]
    fn a_title_sequence_is_dropped_whether_it_ends_with_bel_or_st() {
        assert_eq!(plain(&[b"\x1b]0;title\x07x\x1b]2;t\x1b\\y"]), "xy");
    }

    #[test]
    fn an_escape_sequence_cut_between_two_pieces_is_joined() {
        assert_eq!(plain(&[b"ab\x1b", b"[3", b"1mcd"]), "abcd");
    }

    #[test]
    fn a_korean_character_cut_between_two_pieces_is_joined() {
        let bytes = "한글".as_bytes();
        assert_eq!(plain(&[&bytes[..4], &bytes[4..]]), "한글");
    }

    #[test]
    fn carriage_returns_are_dropped_and_line_feeds_stay() {
        assert_eq!(plain(&[b"one\r\ntwo\r\n"]), "one\ntwo\n");
    }

    #[test]
    fn only_the_last_16_kib_are_kept() {
        let big = vec![b'x'; 40_000];
        let mut text = PlainText::default();
        text.push(&big);
        text.push(b"END");
        assert!(text.text().len() <= 16 * 1024);
        assert!(text.text().ends_with("END"));
    }

    #[test]
    fn clear_forgets_the_text_but_not_a_half_read_escape() {
        let mut text = PlainText::default();
        text.push(b"old\x1b[3");
        text.clear();
        text.push(b"1mnew");
        assert_eq!(text.text(), "new");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-features sign_in`
Expected: FAIL to compile, `PlainText` not found.

- [ ] **Step 3: Implement**

```rust
//! Signs a connector in by typing into the CLI's own `/mcp` screen. `PlainText` turns what the terminal prints
//! into text to look at; `SignInDriver` (below) decides what to type.

const KEEP: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Esc {
    #[default]
    None,
    Escape,
    Csi,
    Osc,
    OscEscape,
}

/// The terminal's output without colours and cursor moves. Output arrives in pieces that may cut a character or
/// an escape sequence in two; both are joined.
#[derive(Debug, Default)]
pub struct PlainText {
    esc: Esc,
    carry: Vec<u8>,
    text: String,
}

impl PlainText {
    pub fn push(&mut self, bytes: &[u8]) {
        self.carry.extend_from_slice(bytes);
        let carry = std::mem::take(&mut self.carry);
        // A character cut at the end waits for its rest; bytes that are not UTF-8 at all become U+FFFD.
        let cut = match std::str::from_utf8(&carry) {
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            _ => carry.len(),
        };
        let (head, tail) = carry.split_at(cut);
        self.carry = tail.to_vec();
        for ch in String::from_utf8_lossy(head).chars() {
            self.take(ch);
        }
        if self.text.len() > KEEP {
            let mut cut = self.text.len() - KEEP;
            while !self.text.is_char_boundary(cut) {
                cut += 1;
            }
            self.text.drain(..cut);
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Forgets the text (a new step looks only at what comes after it). A half-read escape sequence stays half read.
    pub fn clear(&mut self) {
        self.text.clear();
    }

    fn take(&mut self, ch: char) {
        match self.esc {
            Esc::None => match ch {
                '\x1b' => self.esc = Esc::Escape,
                '\r' | '\x07' | '\x08' => {}
                other => self.text.push(other),
            },
            Esc::Escape => {
                self.esc = match ch {
                    '[' => Esc::Csi,
                    ']' => Esc::Osc,
                    _ => Esc::None,
                }
            }
            // Parameters and intermediates are below '@'; the last byte of the sequence is '@'..='~'.
            Esc::Csi => {
                if ('@'..='~').contains(&ch) {
                    match ch {
                        'C' => self.text.push(' '),
                        'B' | 'E' => self.text.push('\n'),
                        _ => {}
                    }
                    self.esc = Esc::None;
                }
            }
            Esc::Osc => {
                self.esc = match ch {
                    '\x07' => Esc::None,
                    '\x1b' => Esc::OscEscape,
                    _ => Esc::Osc,
                }
            }
            Esc::OscEscape => self.esc = Esc::None,
        }
    }
}
```

Add `mod sign_in;` to `lib.rs` (next to `mod shortcut;`).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-features sign_in`
Expected: PASS (8 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: PlainText turns terminal output into text, joining cut characters and escapes (Plan 30D)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: `SignInDriver`

**Files:**
- Modify: `crates/forge-features/src/sign_in.rs` (append above the `tests` module; add tests into it)
- Test: the same file; the fixture test reads `crates/forge-features/tests/fixtures/sign_in/*.bin` through `include_bytes!`

**Interfaces:**
- Consumes: `PlainText` (Task 3); the observed texts in `tests/fixtures/sign_in/NOTES.md` (Task 1).
- Produces:

```rust
pub enum Outcome { Signed, Failed }
pub enum StopReason { StepTimedOut(&'static str), NameNotFound, NameTwice, TimedOut }
pub enum Action { Send(Vec<u8>), Done(Outcome), Stop(StopReason) }
impl SignInDriver {
    pub fn new(name: &str, launch_line: &str, now: Duration) -> Self;
    pub fn feed(&mut self, bytes: &[u8], now: Duration) -> Vec<Action>;
    pub fn tick(&mut self, now: Duration) -> Vec<Action>;
}
```

`now` is the time since the run began. Nothing here touches a clock or a terminal.

- [ ] **Step 1: Write the failing tests** (inside `mod tests` of `sign_in.rs`)

```rust
    use std::time::Duration;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn sends(actions: &[Action]) -> Vec<Vec<u8>> {
        actions.iter().filter_map(|a| if let Action::Send(b) = a { Some(b.clone()) } else { None }).collect()
    }

    /// A driver that has typed the launch line and seen the input box, so it has typed `/mcp` and waits for the list.
    fn at_list(name: &str) -> SignInDriver {
        let mut driver = SignInDriver::new(name, "'/x/claude'", secs(0));
        driver.feed(b"$ ", secs(0));
        assert_eq!(sends(&driver.tick(secs(1))), vec![b"'/x/claude'\r".to_vec()]);
        let actions = driver.feed(format!("{}\r\n", mark::INPUT).as_bytes(), secs(2));
        assert_eq!(sends(&actions), vec![b"/mcp\r".to_vec()]);
        driver
    }

    fn list(name: &str, selected: usize) -> String {
        let rows = ["alpha", name, "omega"];
        let mut text = format!("{}\r\n", mark::LIST);
        for (i, row) in rows.iter().enumerate() {
            let pointer = if i == selected { mark::SELECTED.to_string() } else { " ".to_string() };
            text.push_str(&format!("{pointer} {row} · needs authentication\r\n"));
        }
        text
    }

    #[test]
    fn the_launch_line_is_typed_once_the_shell_has_gone_quiet() {
        let mut driver = SignInDriver::new("web", "'/x/claude'", secs(0));
        assert!(driver.tick(secs(1)).is_empty(), "nothing has been printed yet");
        assert!(driver.feed(b"last login\r\n$ ", secs(1)).is_empty(), "output just arrived");
        assert_eq!(sends(&driver.tick(Duration::from_millis(1400))), vec![b"'/x/claude'\r".to_vec()]);
        assert!(driver.tick(secs(2)).is_empty(), "typed once");
    }

    #[test]
    fn slash_mcp_is_typed_when_the_input_box_shows() {
        let _ = at_list("web");
    }

    #[test]
    fn it_walks_down_the_list_until_the_row_with_the_name_is_selected_then_presses_enter() {
        let mut driver = at_list("web");
        let first = driver.feed(list("web", 0).as_bytes(), secs(3));
        assert_eq!(sends(&first), vec![b"\x1b[B".to_vec()], "alpha is selected: move down");
        let second = driver.feed(list("web", 1).as_bytes(), secs(3));
        assert_eq!(sends(&second), vec![b"\r".to_vec()], "web is selected: open it");
    }

    #[test]
    fn it_presses_enter_on_the_authenticate_item_and_then_only_watches() {
        let mut driver = at_list("web");
        driver.feed(list("web", 1).as_bytes(), secs(3));
        let item = driver.feed(format!("web MCP Server\r\n{}\r\n", mark::ITEM).as_bytes(), secs(4));
        assert_eq!(sends(&item), vec![b"\r".to_vec()]);
        assert!(driver.feed(b"Opening your browser", secs(5)).is_empty());
        assert!(driver.tick(secs(100)).is_empty());
    }

    #[test]
    fn a_success_text_ends_the_run_as_signed_and_a_failure_text_as_failed() {
        for (text, outcome) in [(mark::SIGNED[0], Outcome::Signed), (mark::FAILED[0], Outcome::Failed)] {
            let mut driver = at_list("web");
            driver.feed(list("web", 1).as_bytes(), secs(3));
            driver.feed(mark::ITEM.as_bytes(), secs(4));
            assert_eq!(driver.feed(text.as_bytes(), secs(30)), vec![Action::Done(outcome)]);
            assert!(driver.feed(text.as_bytes(), secs(31)).is_empty(), "reported once");
        }
    }

    #[test]
    fn five_minutes_of_watching_stop_the_run_quietly() {
        let mut driver = at_list("web");
        driver.feed(list("web", 1).as_bytes(), secs(3));
        driver.feed(mark::ITEM.as_bytes(), secs(4));
        assert!(driver.tick(secs(300)).is_empty());
        assert_eq!(driver.tick(secs(305)), vec![Action::Stop(StopReason::TimedOut)]);
    }

    #[test]
    fn a_folder_trust_question_instead_of_the_input_box_hands_over_after_the_step_limit_and_types_nothing() {
        let mut driver = SignInDriver::new("web", "'/x/claude'", secs(0));
        driver.feed(b"$ ", secs(0));
        driver.tick(secs(1));
        driver.feed(b"Do you trust the files in this folder?\r\n 1. Yes, proceed\r\n", secs(2));
        assert!(driver.tick(secs(10)).is_empty());
        assert_eq!(driver.tick(secs(17)), vec![Action::Stop(StopReason::StepTimedOut("the input box"))]);
        assert!(driver.feed(b"more", secs(18)).is_empty(), "nothing is typed after a stop");
        assert!(driver.tick(secs(60)).is_empty());
    }

    #[test]
    fn a_server_that_is_already_signed_in_has_no_authenticate_item_so_it_hands_over() {
        let mut driver = at_list("web");
        driver.feed(list("web", 1).as_bytes(), secs(3));
        driver.feed(b"web MCP Server\r\n 1. Re-authenticate\r\n 2. Clear authentication\r\n", secs(4));
        assert_eq!(driver.tick(secs(20)), vec![Action::Stop(StopReason::StepTimedOut("the Authenticate item"))]);
    }

    #[test]
    fn a_name_on_two_rows_stops_the_run() {
        let mut driver = at_list("web");
        let text = format!("{}\r\n❯ web · needs authentication\r\n  web · connected\r\n", mark::LIST);
        assert_eq!(driver.feed(text.as_bytes(), secs(3)), vec![Action::Stop(StopReason::NameTwice)]);
    }

    #[test]
    fn a_name_that_never_shows_stops_the_run_after_the_step_limit() {
        let mut driver = at_list("web");
        driver.feed(format!("{}\r\n❯ alpha\r\n  omega\r\n", mark::LIST).as_bytes(), secs(3));
        assert_eq!(driver.tick(secs(19)), vec![Action::Stop(StopReason::StepTimedOut("the list with the connector"))]);
    }

    #[test]
    fn a_name_that_is_only_part_of_another_name_is_not_a_match() {
        let mut driver = at_list("web");
        let first = driver.feed(format!("{}\r\n❯ web-two\r\n  web\r\n", mark::LIST).as_bytes(), secs(3));
        assert_eq!(sends(&first), vec![b"\x1b[B".to_vec()], "web-two is not web: move down");
    }

    #[test]
    fn the_walk_down_is_bounded() {
        let mut driver = at_list("web");
        let mut last = Vec::new();
        for i in 0..45 {
            last = driver.feed(format!("{}\r\n❯ alpha\r\n  web\r\n", mark::LIST).as_bytes(), secs(3) + Duration::from_millis(i));
            if matches!(last.first(), Some(Action::Stop(_))) {
                break;
            }
        }
        // alpha stays selected here (a stuck list): the run must stop, not press Down for ever.
        assert_eq!(last, vec![Action::Stop(StopReason::NameNotFound)]);
    }

    #[test]
    fn text_in_pieces_and_with_colours_is_matched_the_same() {
        let mut driver = SignInDriver::new("web", "'/x/claude'", secs(0));
        driver.feed(b"$ ", secs(0));
        driver.tick(secs(1));
        let marked = format!("\x1b[2m{}\x1b[0m", mark::INPUT);
        let (a, b) = marked.as_bytes().split_at(5);
        assert!(driver.feed(a, secs(2)).is_empty());
        assert_eq!(sends(&driver.feed(b, secs(2))), vec![b"/mcp\r".to_vec()]);
    }

    #[test]
    fn the_real_screens_walk_the_driver_to_the_authenticate_item() {
        let mut driver = SignInDriver::new("sentry", "'/x/claude'", secs(0));
        driver.feed(b"$ ", secs(0));
        driver.tick(secs(1));
        let input = driver.feed(include_bytes!("../tests/fixtures/sign_in/01_input.bin"), secs(2));
        assert_eq!(sends(&input), vec![b"/mcp\r".to_vec()]);
        let mut keys = sends(&driver.feed(include_bytes!("../tests/fixtures/sign_in/02_list.bin"), secs(3)));
        assert_eq!(keys.pop(), Some(b"\r".to_vec()), "the list of the capture has sentry selected");
        let item = driver.feed(include_bytes!("../tests/fixtures/sign_in/03_item.bin"), secs(4));
        assert_eq!(sends(&item), vec![b"\r".to_vec()]);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-features sign_in`
Expected: FAIL to compile (`SignInDriver`, `mark`, `Action` not found).

- [ ] **Step 3: Implement** (above `mod tests`, below `PlainText`)

```rust
use std::time::Duration;

/// How long one step may wait for its text.
pub const STEP_LIMIT: Duration = Duration::from_secs(15);
/// How long the browser sign-in may take.
pub const WATCH_LIMIT: Duration = Duration::from_secs(300);
/// The shell counts as ready when it has printed something and then been silent this long.
const QUIET: Duration = Duration::from_millis(300);
/// Most Down presses before the row is called not found.
const MAX_DOWN: usize = 40;
const DOWN: &[u8] = b"\x1b[B";

/// The texts of the CLI's `/mcp` screens, as seen in `tests/fixtures/sign_in/NOTES.md`. If a Claude Code update
/// changes one, the step times out and the person is handed the terminal; the fix is here and in the captures.
pub(crate) mod mark {
    pub const INPUT: &str = "? for shortcuts";
    pub const LIST: &str = "Manage MCP servers";
    pub const SELECTED: char = '❯';
    pub const ITEM: &str = "Authenticate";
    pub const SIGNED: &[&str] = &["Authentication successful"];
    pub const FAILED: &[&str] = &["Authentication failed", "Authentication cancelled"];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Signed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The text a step waits for did not show in time: what it waited for.
    StepTimedOut(&'static str),
    NameNotFound,
    NameTwice,
    /// The browser sign-in took more than five minutes.
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Bytes to type into the terminal.
    Send(Vec<u8>),
    Done(Outcome),
    Stop(StopReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Shell,
    Input,
    List,
    Item,
    Watch,
    Over,
}

impl Step {
    fn waits_for(self) -> &'static str {
        match self {
            Step::Shell => "the shell",
            Step::Input => "the input box",
            Step::List => "the list with the connector",
            Step::Item => "the Authenticate item",
            Step::Watch | Step::Over => "the sign-in",
        }
    }
}

/// Types the keys that open a connector's sign-in in the CLI's `/mcp` screen, one step at a time, each when the
/// text it needs is on the screen. It never reads a clock or a terminal: output and the time come in as
/// arguments, keys and verdicts go out as `Action`s.
#[derive(Debug)]
pub struct SignInDriver {
    name: String,
    launch_line: String,
    step: Step,
    since: Duration,
    last_output: Option<Duration>,
    plain: PlainText,
    rows_counted: bool,
    downs: usize,
}

impl SignInDriver {
    pub fn new(name: &str, launch_line: &str, now: Duration) -> Self {
        Self {
            name: name.to_string(),
            launch_line: launch_line.to_string(),
            step: Step::Shell,
            since: now,
            last_output: None,
            plain: PlainText::default(),
            rows_counted: false,
            downs: 0,
        }
    }

    pub fn feed(&mut self, bytes: &[u8], now: Duration) -> Vec<Action> {
        self.plain.push(bytes);
        self.last_output = Some(now);
        self.advance(now)
    }

    pub fn tick(&mut self, now: Duration) -> Vec<Action> {
        self.advance(now)
    }

    fn advance(&mut self, now: Duration) -> Vec<Action> {
        match self.step {
            Step::Over => Vec::new(),
            Step::Shell => match self.last_output {
                Some(last) if now.saturating_sub(last) >= QUIET => {
                    let line = format!("{}\r", self.launch_line);
                    self.send(line.into_bytes(), Step::Input, now)
                }
                _ => self.limit(now),
            },
            Step::Input => {
                if self.plain.text().contains(mark::INPUT) {
                    self.send(b"/mcp\r".to_vec(), Step::List, now)
                } else {
                    self.limit(now)
                }
            }
            Step::List => self.on_list(now),
            Step::Item => {
                if self.plain.text().contains(mark::ITEM) {
                    self.send(b"\r".to_vec(), Step::Watch, now)
                } else {
                    self.limit(now)
                }
            }
            Step::Watch => {
                let text = self.plain.text();
                if mark::SIGNED.iter().any(|m| text.contains(m)) {
                    self.finish(Action::Done(Outcome::Signed))
                } else if mark::FAILED.iter().any(|m| text.contains(m)) {
                    self.finish(Action::Done(Outcome::Failed))
                } else if now.saturating_sub(self.since) >= WATCH_LIMIT {
                    self.finish(Action::Stop(StopReason::TimedOut))
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn on_list(&mut self, now: Duration) -> Vec<Action> {
        let text = self.plain.text();
        if !text.contains(mark::LIST) {
            return self.limit(now);
        }
        let rows = rows_with_name(text, &self.name);
        if rows.is_empty() {
            return self.limit(now);
        }
        if !self.rows_counted {
            self.rows_counted = true;
            if rows.len() > 1 {
                return self.finish(Action::Stop(StopReason::NameTwice));
            }
        }
        // The last row that carries the pointer is where the selection is now.
        let selected = text.lines().rev().find(|line| line.contains(mark::SELECTED));
        match selected {
            Some(line) if has_name(line, &self.name) => self.send(b"\r".to_vec(), Step::Item, now),
            Some(_) if self.downs >= MAX_DOWN => self.finish(Action::Stop(StopReason::NameNotFound)),
            Some(_) => {
                self.downs += 1;
                self.send(DOWN.to_vec(), Step::List, now)
            }
            None => self.limit(now),
        }
    }

    /// Types `bytes`, moves to `next` and forgets the screen: the next step reads only what comes after.
    fn send(&mut self, bytes: Vec<u8>, next: Step, now: Duration) -> Vec<Action> {
        self.step = next;
        self.since = now;
        self.plain.clear();
        vec![Action::Send(bytes)]
    }

    fn limit(&mut self, now: Duration) -> Vec<Action> {
        if now.saturating_sub(self.since) >= STEP_LIMIT {
            let waited = self.step.waits_for();
            return self.finish(Action::Stop(StopReason::StepTimedOut(waited)));
        }
        Vec::new()
    }

    fn finish(&mut self, action: Action) -> Vec<Action> {
        self.step = Step::Over;
        vec![action]
    }
}

/// The distinct list rows that name the connector. The screen is drawn again for every key, so the same row
/// comes more than once; the pointer is not part of a row.
fn rows_with_name(text: &str, name: &str) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    for line in text.lines().filter(|line| has_name(line, name)) {
        let row = line.replace(mark::SELECTED, "").trim().to_string();
        if !rows.contains(&row) {
            rows.push(row);
        }
    }
    rows
}

/// Whether the line holds the name as a word (`web` is not in `web-two`); a plugin's server shows as
/// `plugin:<plugin>:<name>`, so a word that ends with `:<name>` counts.
fn has_name(line: &str, name: &str) -> bool {
    line.split(|c: char| c.is_whitespace() || c == '·').any(|word| word == name || word.ends_with(&format!(":{name}")))
}
```

Move the existing `const KEEP` block and `use std::time::Duration;` so there is a single `use` at the top of the file (the tests module has its own `use std::time::Duration;`; keep it).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-features sign_in`
Expected: PASS, including `the_real_screens_walk_the_driver_to_the_authenticate_item`. If that test fails, read `NOTES.md`: set `mark::INPUT`, `LIST`, `ITEM`, `SELECTED` to the observed texts, and, if the real list does not keep `name` and the pointer on one line, change `on_list` to match what was seen. Do not loosen the other tests.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: SignInDriver types /mcp, Down and Enter by what the screen shows, and hands over when it does not (Plan 30D)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: `TerminalTap` and `SignInRun`

**Files:**
- Create: `crates/forge-features/src/terminal_tap.rs`
- Create: `crates/forge-features/src/sign_in_run.rs`
- Modify: `crates/forge-features/src/lib.rs`

**Interfaces:**
- Consumes: `SignInDriver`, `Action` (Task 4).
- Produces:

```rust
pub trait TerminalTap { fn output(&self, bytes: &[u8]) -> Vec<u8>; fn tick(&self) -> Vec<u8>; }
#[derive(Clone)] pub struct TapHandle(pub Rc<dyn TerminalTap>);   // PartialEq by pointer
pub struct SignInRun;
impl SignInRun { pub fn new(name: &str, launch_line: &str, report: impl Fn(Action) + 'static) -> Self }
impl TerminalTap for SignInRun
```

`report` is called with `Action::Done(..)` and `Action::Stop(..)` only. Keys come back as the return value of `output`/`tick`.

- [ ] **Step 1: Write the failing tests** (in `sign_in_run.rs`)

```rust
#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::thread::sleep;

    use super::*;
    use crate::sign_in::{mark, Outcome};

    fn run(name: &str) -> (SignInRun, Rc<RefCell<Vec<Action>>>) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = seen.clone();
        (SignInRun::new(name, "'/x/claude'", move |action| sink.borrow_mut().push(action)), seen)
    }

    #[test]
    fn keys_come_back_as_bytes_to_type_and_verdicts_go_to_the_report() {
        let (tap, seen) = run("web");
        assert!(tap.output(b"$ ").is_empty());
        sleep(std::time::Duration::from_millis(400));
        assert_eq!(tap.tick(), b"'/x/claude'\r".to_vec());
        assert_eq!(tap.output(mark::INPUT.as_bytes()), b"/mcp\r".to_vec());
        assert!(seen.borrow().is_empty());
        tap.output(format!("{}\r\n❯ web\r\n", mark::LIST).as_bytes());
        tap.output(mark::ITEM.as_bytes());
        assert!(tap.output(mark::SIGNED[0].as_bytes()).is_empty());
        assert_eq!(*seen.borrow(), vec![Action::Done(Outcome::Signed)]);
    }

    #[test]
    fn a_tap_handle_is_equal_to_itself_only() {
        let (a, _) = run("web");
        let (b, _) = run("web");
        let first = TapHandle(Rc::new(a));
        assert!(first == first.clone());
        assert!(first != TapHandle(Rc::new(b)));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-features sign_in_run`
Expected: FAIL to compile.

- [ ] **Step 3: Implement**

`terminal_tap.rs`:

```rust
use std::rc::Rc;

/// Listens to a terminal and may type into it. The terminal view calls `output` for every piece of output and
/// `tick` about four times a second; the bytes returned are written to the shell. The dock's terminal has none.
pub trait TerminalTap {
    fn output(&self, bytes: &[u8]) -> Vec<u8>;
    fn tick(&self) -> Vec<u8>;
}

/// A tap as a component prop: equal to itself only.
#[derive(Clone)]
pub struct TapHandle(pub Rc<dyn TerminalTap>);

impl PartialEq for TapHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
```

`sign_in_run.rs`:

```rust
use std::cell::RefCell;
use std::time::Instant;

use crate::sign_in::{Action, SignInDriver};
use crate::terminal_tap::TerminalTap;

/// One sign-in: a driver, the clock it is given, and where its verdict goes.
pub struct SignInRun {
    driver: RefCell<SignInDriver>,
    started: Instant,
    report: Box<dyn Fn(Action)>,
}

impl SignInRun {
    pub fn new(name: &str, launch_line: &str, report: impl Fn(Action) + 'static) -> Self {
        let started = Instant::now();
        Self { driver: RefCell::new(SignInDriver::new(name, launch_line, started.elapsed())), started, report: Box::new(report) }
    }

    fn keys_of(&self, actions: Vec<Action>) -> Vec<u8> {
        let mut keys = Vec::new();
        for action in actions {
            match action {
                Action::Send(bytes) => keys.extend(bytes),
                verdict => (self.report)(verdict),
            }
        }
        keys
    }
}

impl TerminalTap for SignInRun {
    fn output(&self, bytes: &[u8]) -> Vec<u8> {
        let actions = self.driver.borrow_mut().feed(bytes, self.started.elapsed());
        self.keys_of(actions)
    }

    fn tick(&self) -> Vec<u8> {
        let actions = self.driver.borrow_mut().tick(self.started.elapsed());
        self.keys_of(actions)
    }
}
```

In `lib.rs` add `mod sign_in_run;`, `mod terminal_tap;` and exports:

```rust
pub use sign_in::{Action as SignInAction, Outcome as SignInOutcome, StopReason as SignInStop};
pub use sign_in_run::SignInRun;
pub use terminal_tap::{TapHandle, TerminalTap};
```

Make `mark` visible to this module's tests: it is `pub(crate)` in `sign_in.rs` already. Simplify the second test if it reads as noise: its point is that a run which has reached no verdict reports nothing and types nothing after the first keys; keep only what the assertions prove.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-features sign_in_run`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: SignInRun gives the driver a clock and a verdict report behind a TerminalTap (Plan 30D)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The terminal view takes a tap

**Files:**
- Modify: `crates/forge-widgets/src/terminal_panel.rs`

**Interfaces:**
- Consumes: `forge_features::{TapHandle, TerminalTap}`.
- Produces: `pub fn SignInTerminal(cwd: PathBuf, tap: TapHandle) -> Element` — one terminal view (no tabs) that starts the shell in `cwd` and feeds the tap. `TerminalTab` gains `#[props(default)] tap: Option<TapHandle>`; the dock passes none.

There is no unit test for a component that talks to xterm; this task is verified by compiling, by the dock still working (Step 3) and by Task 8.

- [ ] **Step 1: Add the tap to `drive` and the tab**

In `drive`, add a `tap: Option<TapHandle>` parameter. After `phase.set(Phase::Running);` and before `loop`, create the ticker, and extend the `select!`:

```rust
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(250));
    loop {
        tokio::select! {
            from_page = page.recv::<FromPage>() => match from_page {
                /* unchanged */
            },
            event = session.recv_batch() => match event {
                Some(TerminalEvent::Output(bytes)) => {
                    if let Some(tap) = &tap {
                        let keys = tap.0.output(&bytes);
                        if !keys.is_empty() {
                            let _ = terminal.write(session.id, &keys);
                        }
                    }
                    let b = base64::engine::general_purpose::STANDARD.encode(bytes);
                    let _ = page.send(serde_json::json!({ "t": "out", "b": b }));
                }
                /* the other arms unchanged */
            },
            _ = ticker.tick(), if tap.is_some() => {
                if let Some(tap) = &tap {
                    let keys = tap.0.tick();
                    if !keys.is_empty() {
                        let _ = terminal.write(session.id, &keys);
                    }
                }
            },
        }
    }
```

Keep the existing arms' bodies exactly as they are; only the `Output` arm gains the first block and the new `ticker` arm is added. Add `TapHandle` to the `use forge_features::{...}` line.

In `TerminalTab`, make the component `pub`-free as before but add the prop and pass it on:

```rust
#[component]
fn TerminalTab(tab: TabId, cwd: PathBuf, active: bool, on_restart: EventHandler<TabId>, #[props(default)] tap: Option<TapHandle>) -> Element {
```

and in its `use_future` closure: `let (host_id, cwd, running, tap) = (host_id.clone(), cwd.clone(), running.clone(), tap.clone());` then `drive(workspace.terminal(), host_id, cwd, phase, running, tap).await`.

- [ ] **Step 2: Add `SignInTerminal`** (at the end of the file)

```rust
/// A tab id no dock terminal uses, so the page element ids do not meet.
const SIGN_IN_TAB: TabId = TabId(u64::MAX);

/// One terminal for the sign-in dialog: the same view as a dock tab, started in `cwd`, with a tap that types.
/// Taking the view away (closing the dialog) ends the shell, as closing a dock tab does.
#[component]
pub fn SignInTerminal(cwd: PathBuf, tap: TapHandle) -> Element {
    rsx! {
        div { class: "sign-in-terminal",
            TerminalTab { tab: SIGN_IN_TAB, cwd, active: true, on_restart: move |_| {}, tap: Some(tap) }
        }
    }
}
```

- [ ] **Step 3: Build and run the existing tests**

Run: `cargo build -p forge-widgets && cargo test -p forge-widgets`
Expected: builds; tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/forge-widgets
git commit -m "feat: the terminal view can feed a tap and type what it returns; SignInTerminal for a dialog (Plan 30D)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: State, dialog and button

**Files:**
- Modify: `crates/forge-features/src/customize_connectors.rs`, `customize_state.rs`, `lib.rs`
- Modify: `crates/forge-widgets/src/customize_connector_dialogs.rs`, `customize_dialogs.rs`, `customize_connectors.rs`
- Modify: `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `SignInRun`, `TapHandle`, `SignInAction`, `SignInOutcome`, `SignInStop` (Task 5), `ConnectorCatalogUseCase::launch_line` (Task 2), `SignInTerminal` (Task 6), the existing `check_connector`, `open_dialog`, `close_dialog`, `connection`.
- Produces:

```rust
pub enum SignInState { Idle, Running, Signed, NeedsHand(String), Failed }
pub fn shows_sign_in(state: &ConnectionState) -> bool;
pub fn stop_text(reason: &SignInStop) -> String;
pub fn sign_in_status(state: &SignInState, connection: &ConnectionState) -> String;
pub fn sign_in_cwd(project: Option<&Path>, home: Option<PathBuf>) -> PathBuf;
Customize::{sign_in_connector(name), close_sign_in(), sign_in_state(), sign_in_tap(), sign_in_cwd()}
Dialog::SignInConnector(String)
```

- [ ] **Step 1: Write the failing tests** (in the `tests` module of `customize_connectors.rs`)

```rust
    #[test]
    fn the_sign_in_button_shows_only_for_a_connector_whose_check_said_needs_sign_in() {
        assert!(shows_sign_in(&ConnectionState::Done(Connection::NeedsSignIn)));
        assert!(!shows_sign_in(&ConnectionState::Done(Connection::Connected)));
        assert!(!shows_sign_in(&ConnectionState::Idle));
        assert!(!shows_sign_in(&ConnectionState::Checking));
        assert!(!shows_sign_in(&ConnectionState::Failed("x".into())));
    }

    #[test]
    fn each_stop_reason_says_what_happened_in_a_few_words() {
        assert_eq!(stop_text(&SignInStop::StepTimedOut("the input box")), "Claude did not show the input box.");
        assert_eq!(stop_text(&SignInStop::NameNotFound), "The connector was not found on Claude's list.");
        assert_eq!(stop_text(&SignInStop::NameTwice), "The name is on Claude's list twice.");
        assert_eq!(stop_text(&SignInStop::TimedOut), "The sign-in took too long.");
    }

    #[test]
    fn the_sign_in_status_line_follows_the_run_and_then_the_check() {
        let idle = ConnectionState::Idle;
        assert_eq!(sign_in_status(&SignInState::Idle, &idle), "");
        assert!(sign_in_status(&SignInState::Running, &idle).contains("browser"));
        assert_eq!(sign_in_status(&SignInState::Signed, &idle), "Signed in. Checking…");
        assert_eq!(sign_in_status(&SignInState::Signed, &ConnectionState::Checking), "Signed in. Checking…");
        assert_eq!(sign_in_status(&SignInState::Signed, &ConnectionState::Done(Connection::Connected)), "Connected");
        assert_eq!(
            sign_in_status(&SignInState::NeedsHand("Claude did not show the input box.".into()), &idle),
            "Claude did not show the input box. Go on in the terminal above."
        );
        assert_eq!(sign_in_status(&SignInState::Failed, &idle), "Sign-in did not finish. See the terminal above.");
    }

    #[test]
    fn the_shell_starts_in_the_project_folder_else_the_home_folder_else_here() {
        use std::path::{Path, PathBuf};
        assert_eq!(sign_in_cwd(Some(Path::new("/p")), Some(PathBuf::from("/h"))), PathBuf::from("/p"));
        assert_eq!(sign_in_cwd(None, Some(PathBuf::from("/h"))), PathBuf::from("/h"));
        assert_eq!(sign_in_cwd(None, None), PathBuf::from("."));
    }

    #[test]
    fn the_sign_in_dialog_is_never_closed_by_its_backdrop() {
        assert!(connector_dialog_busy(&Dialog::SignInConnector("web".into()), false));
        assert!(!connector_dialog_busy(&Dialog::AddConnector, false));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-features customize_connectors`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the pure parts** (in `customize_connectors.rs`; add `use std::path::{Path, PathBuf};` and `use crate::{SignInAction, SignInOutcome, SignInRun, SignInStop, TapHandle};` as the file's imports allow)

```rust
/// How far the sign-in dialog's run is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInState {
    Idle,
    Running,
    /// The CLI says it is signed in; the connection is being checked (see `ConnectionState`).
    Signed,
    /// Forge stopped typing: the reason. The terminal is the person's now.
    NeedsHand(String),
    Failed,
}

/// "Sign in" is offered for a connector whose check said it needs sign-in.
pub fn shows_sign_in(state: &ConnectionState) -> bool {
    matches!(state, ConnectionState::Done(Connection::NeedsSignIn))
}

pub fn stop_text(reason: &SignInStop) -> String {
    match reason {
        SignInStop::StepTimedOut(what) => format!("Claude did not show {what}."),
        SignInStop::NameNotFound => "The connector was not found on Claude's list.".to_string(),
        SignInStop::NameTwice => "The name is on Claude's list twice.".to_string(),
        SignInStop::TimedOut => "The sign-in took too long.".to_string(),
    }
}

pub fn sign_in_status(state: &SignInState, connection: &ConnectionState) -> String {
    match state {
        SignInState::Idle => String::new(),
        SignInState::Running => "Signing in… finish it in your browser when it opens.".to_string(),
        SignInState::Signed => match connection {
            ConnectionState::Idle | ConnectionState::Checking => "Signed in. Checking…".to_string(),
            done => connection_text(done),
        },
        SignInState::NeedsHand(reason) => format!("{reason} Go on in the terminal above."),
        SignInState::Failed => "Sign-in did not finish. See the terminal above.".to_string(),
    }
}

/// Where the sign-in shell starts: project connectors depend on the folder, so the project's, else the home folder.
pub fn sign_in_cwd(project: Option<&Path>, home: Option<PathBuf>) -> PathBuf {
    project.map(Path::to_path_buf).or(home).unwrap_or_else(|| PathBuf::from("."))
}

fn home_folder() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}
```

Change `connector_dialog_busy`:

```rust
pub fn connector_dialog_busy(dialog: &Dialog, connectors_busy: bool) -> bool {
    matches!(dialog, Dialog::SignInConnector(_)) || (connectors_busy && matches!(dialog, Dialog::AddConnector | Dialog::ConfirmRemoveConnector(_)))
}
```

(its doc comment gains: "The sign-in dialog is always held: its terminal is the person's.")

In `customize_state.rs`: add `SignInConnector(String),` to `Dialog`, and make the field `pub(crate) project: Signal<Option<PathBuf>>,`.

In the `Connectors` struct add `sign_in: Signal<SignInState>,` and `sign_in_tap: Signal<Option<TapHandle>>,`, initialised in `new` with `Signal::new_in_scope(SignInState::Idle, ScopeId::ROOT)` and `Signal::new_in_scope(None, ScopeId::ROOT)`.

In `impl Customize`:

```rust
    pub fn sign_in_state(&self) -> SignInState {
        self.connectors.sign_in.read().clone()
    }

    pub fn sign_in_tap(&self) -> Option<TapHandle> {
        self.connectors.sign_in_tap.read().clone()
    }

    pub fn sign_in_cwd(&self) -> PathBuf {
        sign_in_cwd(self.project.peek().as_deref(), home_folder())
    }

    fn set_sign_in(&self, state: SignInState) {
        let mut sign_in = self.connectors.sign_in;
        sign_in.set(state);
    }

    /// The "Sign in" button: opens the dialog whose terminal Forge types into. A press while the dialog is open does nothing.
    pub fn sign_in_connector(&self, name: String) {
        if matches!(self.dialog(), Some(Dialog::SignInConnector(_))) {
            return;
        }
        let launch = match self.connectors.usecase().launch_line() {
            Ok(line) => line,
            Err(error) => {
                self.set_error(Some(error.to_string()));
                return;
            }
        };
        let this = *self;
        let checked_name = name.clone();
        let run = SignInRun::new(&name, &launch, move |verdict| match verdict {
            SignInAction::Done(SignInOutcome::Signed) => {
                this.set_sign_in(SignInState::Signed);
                this.check_connector(checked_name.clone());
            }
            SignInAction::Done(SignInOutcome::Failed) => this.set_sign_in(SignInState::Failed),
            SignInAction::Stop(reason) => this.set_sign_in(SignInState::NeedsHand(stop_text(&reason))),
            SignInAction::Send(_) => {}
        });
        let mut tap = self.connectors.sign_in_tap;
        tap.set(Some(TapHandle(std::rc::Rc::new(run))));
        self.set_sign_in(SignInState::Running);
        self.open_dialog(Dialog::SignInConnector(name));
    }

    /// Close: ends the shell (the dialog's terminal goes away with it) and forgets the run.
    pub fn close_sign_in(&self) {
        let mut tap = self.connectors.sign_in_tap;
        tap.set(None);
        self.set_sign_in(SignInState::Idle);
        self.close_dialog();
    }
```

Check the real names (`set_error`, `dialog()`, `open_dialog`, `close_dialog`) against `customize_state.rs` before use and adapt only the call, not the behaviour.

In `lib.rs` export the new items: add `shows_sign_in, sign_in_cwd, sign_in_status, stop_text, SignInState` to the `customize_connectors` `pub use` line.

- [ ] **Step 4: Run to verify the pure parts pass**

Run: `cargo test -p forge-features`
Expected: PASS.

- [ ] **Step 5: The dialog, the button, the CSS**

`customize_connector_dialogs.rs` (add imports `forge_features::sign_in_status`, `crate::terminal_panel::SignInTerminal`):

```rust
#[component]
pub fn SignInConnector(name: String) -> Element {
    let customize = use_customize();
    let Some(tap) = customize.sign_in_tap() else {
        return rsx! {};
    };
    let status = sign_in_status(&customize.sign_in_state(), &customize.connection());
    rsx! {
        h2 { "Sign in to {name}" }
        p { class: "hint", "Forge types into Claude Code's /mcp screen for you. Finish the sign-in in your browser. If it stops, you can type here." }
        SignInTerminal { cwd: customize.sign_in_cwd(), tap }
        div { class: "status-line customize-wrap", "{status}" }
        div { class: "customize-dialog-buttons",
            Button { label: "Close", kind: ButtonKind::Primary, onclick: move |_| customize.close_sign_in() }
        }
    }
}
```

`customize_dialogs.rs`: import `SignInConnector` next to the other two; add the arm `Dialog::SignInConnector(name) => rsx! { SignInConnector { name } },`; make the dialog box wider for it:

```rust
let wide = matches!(dialog, Dialog::SignInConnector(_));
...
div { class: if wide { "customize-dialog sign-in" } else { "customize-dialog" }, onclick: ... }
```

(`dialog` is moved by `match dialog`; compute `wide` before the `rsx!`.)

`customize_connectors.rs` (detail screen), inside the `customize-check` div after the `Check connection` button, add:

```rust
                        if shows_sign_in(&state) {
                            Button {
                                label: "Sign in",
                                onclick: {
                                    let name = name.clone();
                                    move |_| customize.sign_in_connector(name.clone())
                                },
                            }
                        }
```

and change the `NeedsSignIn` text in `connection_text` to `"Needs sign-in. Use the Sign in button, or run /mcp in the terminal's Claude Code."` (and update the existing test `the_connection_line`, which asserts it contains `/mcp`: it still does).

`theme.css`: read the existing `.terminal-view`, `.terminal-host` and `.customize-dialog` rules, then add:

```css
.customize-dialog.sign-in { width: min(760px, 92vw); }
.sign-in-terminal { height: 320px; margin: 8px 0; display: flex; border-radius: 6px; overflow: hidden; }
.sign-in-terminal .terminal-view { flex: 1; }
```

and, if `.terminal-host` takes its size from a dock rule that does not apply here, copy the sizing it needs (`width: 100%; height: 100%`) under `.sign-in-terminal .terminal-host`.

- [ ] **Step 6: Build and test everything**

Run: `cargo build -p forge-app && cargo test --workspace`
Expected: builds; all tests PASS.

- [ ] **Step 7: Commit**

```bash
git add crates docs
git commit -m "feat: Sign in on a connector that needs it, in a dialog with a terminal that Forge types into (Plan 30D)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Check in the real window, then write it down

**Files:**
- Modify: `docs/superpowers/specs/2026-10-09-customize-connectors-signin-design.md` (add "As built")
- Modify: `docs/superpowers/roadmap/README.md` (row 30D), `docs/superpowers/roadmap/backlog.md` (PG-02 entry)

**Interfaces:**
- Consumes: everything above; the harness in `scripts/harness/README.md` (launch, capture, click, the `FORGE_CLAUDE_HOME` fixture).

- [ ] **Step 1: Launch the app with the harness home** as `scripts/harness/README.md` says. Add a connector that needs sign-in in the app (Customize → Connectors → Add → Remote http, `sentry`, `https://mcp.sentry.dev/mcp`). Open it, press "Check connection": it says it needs sign-in and the "Sign in" button appears.

- [ ] **Step 2: Sign in to the browser step.** Press "Sign in". Expected, with no key typed by hand: the dialog opens, the terminal starts `claude`, shows the input box, `/mcp`, the list with `sentry` selected, the server's menu, and the CLI opens the browser. Capture the dialog at each stage (`scripts/harness/capture.py`) and put what the status line says at each in the notes. If any step stops early, the status line must say "Go on in the terminal above." and the terminal must accept typing; record which step and its text, fix the `mark` constant or the screen model in `sign_in.rs`, add the capture to `tests/fixtures/sign_in` and a test, and repeat.

- [ ] **Step 3: Finish the sign-in with the person** (the browser part needs their account; for Figma use `https://mcp.figma.com/mcp`). Expected: the status line goes "Signed in. Checking…" then "Connected"; the detail screen's line says Connected; the "needs sign-in" tag is gone from Mine. Read what the CLI prints on success and on cancel in the terminal and put the exact success and failure texts into `mark::SIGNED` and `mark::FAILED`; if the CLI prints no text Forge can use, say so in "As built" and rely on the manual "Check connection" (the status line then stays on "Signing in…" until the person presses it or closes the dialog; make the dialog's hint say so).

- [ ] **Step 4: Edge checks.** Close the dialog while it is running: the shell ends (no `claude` process left: `pgrep -fl "claude"` shows none of the dialog's), and the detail screen is usable. Press "Sign in" twice quickly: one dialog. Open a connector under a folder that Claude has not trusted: the status line hands over after 15 s and typing works. Sign in a long name and a project-scope connector.

- [ ] **Step 5: Write "As built"** at the end of the spec: what was checked (list the checks above with pass or found-and-fixed), what was not (Windows, a proxy, servers that need a client id), and the real texts of the screens. Update `roadmap/README.md` (row 30D, "built, checked in the real window") and the PG-02 entry in `backlog.md` (30D built).

- [ ] **Step 6: Final run and commit**

Run: `cargo test --workspace`
Expected: PASS.

```bash
git add docs crates
git commit -m "docs: Plan 30D as built, roadmap rows (Plan 30D)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
