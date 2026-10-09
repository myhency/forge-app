//! Signs a connector in by typing into the CLI's own `/mcp` screen. `PlainText` turns what the terminal prints
//! into text to look at; `SignInDriver` decides what to type.

use std::time::Duration;

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

/// How long one step may wait for its text.
pub const STEP_LIMIT: Duration = Duration::from_secs(15);
/// How long the browser sign-in may take.
pub const WATCH_LIMIT: Duration = Duration::from_secs(300);
/// The shell counts as ready when it has printed something and then been silent this long.
const QUIET: Duration = Duration::from_millis(300);
/// The list is acted on only when it has been still this long: a frame is drawn in several writes, and a half
/// drawn one would make Forge press Down twice.
const SETTLE: Duration = Duration::from_millis(150);
/// Most Down presses before the row is called not found.
const MAX_DOWN: usize = 40;
const DOWN: &[u8] = b"\x1b[B";

/// The texts of the CLI's `/mcp` screens, as seen in `tests/fixtures/sign_in/NOTES.md`. If a Claude Code update
/// changes one, the step times out and the person is handed the terminal; the fix is here and in the captures.
pub(crate) mod mark {
    /// The input box shows one of these under it, by mode (default, auto / plan / accept edits).
    pub const INPUT: &[&str] = &["? for shortcuts", "shift+tab to cycle"];
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
    downs: usize,
    /// A Down was typed and nothing has been drawn since: the pointer seen is the old one.
    awaiting_redraw: bool,
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
            downs: 0,
            awaiting_redraw: false,
        }
    }

    pub fn feed(&mut self, bytes: &[u8], now: Duration) -> Vec<Action> {
        self.plain.push(bytes);
        self.last_output = Some(now);
        self.awaiting_redraw = false;
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
                if mark::INPUT.iter().any(|m| self.plain.text().contains(m)) {
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

    /// The list is drawn once, then only the rows that change are drawn again, so the text is kept between Down
    /// presses and the last line with the pointer is the selection.
    fn on_list(&mut self, now: Duration) -> Vec<Action> {
        let still = self.last_output.is_some_and(|last| now.saturating_sub(last) >= SETTLE);
        if self.awaiting_redraw || !still || !self.plain.text().contains(mark::LIST) {
            return self.limit(now);
        }
        let text = self.plain.text();
        if self.downs == 0 && rows_with_name(text, &self.name).len() > 1 {
            return self.finish(Action::Stop(StopReason::NameTwice));
        }
        match text.lines().rev().find(|line| line.contains(mark::SELECTED)) {
            Some(line) if has_name(line, &self.name) => self.send(b"\r".to_vec(), Step::Item, now),
            Some(_) if self.downs >= MAX_DOWN => self.finish(Action::Stop(StopReason::NameNotFound)),
            Some(_) => {
                self.downs += 1;
                self.awaiting_redraw = true;
                self.step = Step::List;
                self.since = now;
                vec![Action::Send(DOWN.to_vec())]
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

/// The distinct list rows that name the connector. The pointer is not part of a row.
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

    use std::time::Duration;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn sends(actions: &[Action]) -> Vec<Vec<u8>> {
        actions.iter().filter_map(|a| if let Action::Send(b) = a { Some(b.clone()) } else { None }).collect()
    }

    /// Output that is followed by a quiet moment: the driver acts on a screen only when it has stopped changing.
    fn settled(driver: &mut SignInDriver, bytes: &[u8], at: Duration) -> Vec<Action> {
        let mut actions = driver.feed(bytes, at);
        actions.extend(driver.tick(at + ms(200)));
        actions
    }

    /// A driver that has typed the launch line and seen the input box, so it has typed `/mcp` and waits for the list.
    fn at_list(name: &str) -> SignInDriver {
        let mut driver = SignInDriver::new(name, "'/x/claude'", secs(0));
        driver.feed(b"$ ", secs(0));
        assert_eq!(sends(&driver.tick(secs(1))), vec![b"'/x/claude'\r".to_vec()]);
        let actions = driver.feed(format!("{}\r\n", mark::INPUT[0]).as_bytes(), secs(2));
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
        assert!(driver.tick(ms(1100)).is_empty(), "100 ms of quiet is too little");
        assert_eq!(sends(&driver.tick(ms(1400))), vec![b"'/x/claude'\r".to_vec()]);
        assert!(driver.tick(secs(2)).is_empty(), "typed once");
    }

    #[test]
    fn slash_mcp_is_typed_when_the_input_box_shows_in_any_mode() {
        for mark in mark::INPUT {
            let mut driver = SignInDriver::new("web", "'/x/claude'", secs(0));
            driver.feed(b"$ ", secs(0));
            driver.tick(secs(1));
            assert_eq!(sends(&driver.feed(mark.as_bytes(), secs(2))), vec![b"/mcp\r".to_vec()], "{mark}");
        }
    }

    #[test]
    fn it_walks_down_the_list_until_the_row_with_the_name_is_selected_then_presses_enter() {
        let mut driver = at_list("web");
        let first = settled(&mut driver, list("web", 0).as_bytes(), secs(3));
        assert_eq!(sends(&first), vec![b"\x1b[B".to_vec()], "alpha is selected: move down");
        let second = settled(&mut driver, list("web", 1).as_bytes(), secs(4));
        assert_eq!(sends(&second), vec![b"\r".to_vec()], "web is selected: open it");
    }

    #[test]
    fn a_screen_is_acted_on_only_after_it_has_stopped_changing() {
        let mut driver = at_list("web");
        assert!(driver.feed(list("web", 0).as_bytes(), secs(3)).is_empty(), "the frame may be half drawn");
        assert!(driver.tick(secs(3) + ms(100)).is_empty());
        assert_eq!(sends(&driver.tick(secs(3) + ms(200))), vec![b"\x1b[B".to_vec()]);
    }

    #[test]
    fn after_a_down_nothing_more_is_pressed_until_the_screen_has_changed() {
        let mut driver = at_list("web");
        settled(&mut driver, list("web", 0).as_bytes(), secs(3));
        assert!(driver.tick(secs(4)).is_empty(), "the old pointer is still the last one: do not press Down again");
        assert!(driver.tick(secs(8)).is_empty());
    }

    #[test]
    fn a_redraw_of_only_the_changed_rows_is_enough() {
        let mut driver = at_list("web");
        settled(&mut driver, list("web", 0).as_bytes(), secs(3));
        let redraw = format!("  alpha · needs authentication\r\n{} web · needs authentication\r\n", mark::SELECTED);
        assert_eq!(sends(&settled(&mut driver, redraw.as_bytes(), secs(4))), vec![b"\r".to_vec()]);
    }

    #[test]
    fn it_presses_enter_on_the_authenticate_item_and_then_only_watches() {
        let mut driver = at_list("web");
        settled(&mut driver, list("web", 1).as_bytes(), secs(3));
        let item = driver.feed(format!("web MCP Server\r\n{}\r\n", mark::ITEM).as_bytes(), secs(4));
        assert_eq!(sends(&item), vec![b"\r".to_vec()]);
        assert!(driver.feed(b"Opening your browser", secs(5)).is_empty());
        assert!(driver.tick(secs(100)).is_empty());
    }

    #[test]
    fn a_success_text_ends_the_run_as_signed_and_a_failure_text_as_failed() {
        for (text, outcome) in [(mark::SIGNED[0], Outcome::Signed), (mark::FAILED[0], Outcome::Failed)] {
            let mut driver = at_list("web");
            settled(&mut driver, list("web", 1).as_bytes(), secs(3));
            driver.feed(mark::ITEM.as_bytes(), secs(4));
            assert_eq!(driver.feed(text.as_bytes(), secs(30)), vec![Action::Done(outcome)]);
            assert!(driver.feed(text.as_bytes(), secs(31)).is_empty(), "reported once");
        }
    }

    #[test]
    fn five_minutes_of_watching_stop_the_run_quietly() {
        let mut driver = at_list("web");
        settled(&mut driver, list("web", 1).as_bytes(), secs(3));
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
        settled(&mut driver, list("web", 1).as_bytes(), secs(3));
        driver.feed(b"web MCP Server\r\n 1. Re-authenticate\r\n 2. Clear authentication\r\n", secs(4));
        assert_eq!(driver.tick(secs(20)), vec![Action::Stop(StopReason::StepTimedOut("the Authenticate item"))]);
    }

    #[test]
    fn a_name_on_two_rows_stops_the_run() {
        let mut driver = at_list("web");
        let text = format!("{}\r\n❯ web · needs authentication\r\n  web · connected\r\n", mark::LIST);
        assert_eq!(settled(&mut driver, text.as_bytes(), secs(3)), vec![Action::Stop(StopReason::NameTwice)]);
    }

    #[test]
    fn a_name_that_never_shows_stops_the_run_after_the_step_limit() {
        let mut driver = at_list("web");
        settled(&mut driver, format!("{}\r\n❯ alpha\r\n  omega\r\n", mark::LIST).as_bytes(), secs(3));
        assert_eq!(driver.tick(secs(19)), vec![Action::Stop(StopReason::StepTimedOut("the list with the connector"))]);
    }

    #[test]
    fn a_name_that_is_only_part_of_another_name_is_not_a_match() {
        let mut driver = at_list("web");
        let first = settled(&mut driver, format!("{}\r\n❯ web-two\r\n  web\r\n", mark::LIST).as_bytes(), secs(3));
        assert_eq!(sends(&first), vec![b"\x1b[B".to_vec()], "web-two is not web: move down");
    }

    #[test]
    fn a_plugin_server_is_found_by_the_name_after_its_plugin() {
        let mut driver = at_list("context7");
        let text = format!("{}\r\n❯ plugin:context7:context7 needs authentication\r\n", mark::LIST);
        assert_eq!(sends(&settled(&mut driver, text.as_bytes(), secs(3))), vec![b"\r".to_vec()]);
    }

    #[test]
    fn the_walk_down_is_bounded() {
        let mut driver = at_list("web");
        let mut last = Vec::new();
        for i in 0..45 {
            last = settled(&mut driver, format!("{}\r\n❯ alpha\r\n  web\r\n", mark::LIST).as_bytes(), secs(3) + ms(i * 300));
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
        let marked = format!("\x1b[2m{}\x1b[0m", mark::INPUT[0]);
        let (a, b) = marked.as_bytes().split_at(5);
        assert!(driver.feed(a, secs(2)).is_empty());
        assert_eq!(sends(&driver.feed(b, secs(2))), vec![b"/mcp\r".to_vec()]);
    }

    #[test]
    fn the_real_screens_walk_the_driver_to_the_authenticate_item() {
        let mut driver = SignInDriver::new("context7", "'/x/claude'", secs(0));
        driver.feed(b"$ ", secs(0));
        driver.tick(secs(1));
        let input = driver.feed(include_bytes!("../tests/fixtures/sign_in/01_input.bin"), secs(2));
        assert_eq!(sends(&input), vec![b"/mcp\r".to_vec()]);
        let first = settled(&mut driver, include_bytes!("../tests/fixtures/sign_in/02_list.bin"), secs(3));
        assert_eq!(sends(&first), vec![b"\x1b[B".to_vec()], "the first row of the capture is not context7");
        let downs = settled(&mut driver, include_bytes!("../tests/fixtures/sign_in/02c_downs.bin"), secs(4));
        assert_eq!(sends(&downs), vec![b"\r".to_vec()], "the pointer reached context7");
        let item = driver.feed(include_bytes!("../tests/fixtures/sign_in/03_item.bin"), secs(5));
        assert_eq!(sends(&item), vec![b"\r".to_vec()]);
    }
}
