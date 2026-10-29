//! Running a shell code block of an answer in the session's terminal: what is typed, how the output is
//! told from the rest of the terminal, and what the model is told afterwards.

/// The first line of the message that tells the model how a run went. The chat does not draw it.
pub const RUN_MESSAGE_MARK: &str = "[forge-run]";

/// The most lines, and then characters, of output a box and the message to the model hold.
pub const OUTPUT_LINES: usize = 200;
pub const OUTPUT_CHARS: usize = 20_000;

/// Whether a fenced block with this language name is shell code.
pub fn is_shell_language(language: &str) -> bool {
    matches!(language.to_ascii_lowercase().as_str(), "bash" | "sh" | "zsh" | "shell" | "console" | "shellscript" | "terminal")
}

/// What is typed into the shell for run `id`: the block's first line after a `printf` of the start marker
/// (the shell's echo of that line comes before the marker, so what follows it is output), the other lines,
/// and a last line that prints the end marker with the exit code. Every line ends with a newline.
pub fn typed_text(id: u64, block: &str) -> Vec<u8> {
    let block = block.trim_end();
    let mut typed = format!("printf '\\033]777;forge-run-start;{id}\\007'; {block}\n");
    typed.push_str(&format!("printf '\\033]777;forge-run-end;{id};%s\\007' \"$?\"\n"));
    typed.into_bytes()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEvent {
    Started(u64),
    Ended { id: u64, code: i32 },
    /// Plain output between the markers, one finished line at a time (the last piece comes before `Ended`).
    Text(String),
}

/// Reads terminal output and tells the markers of a run from the text between them. Bytes that arrive in any
/// split are fine; text outside a run is dropped.
#[derive(Default)]
pub struct MarkerScanner {
    escape: Escape,
    // Whether a run is open: between its start and end marker.
    open: bool,
    // The line being read. A carriage return is held back: a `\n` right after it is a line end, anything else
    // empties the line first (a progress line keeps its last state).
    line: Vec<u8>,
    return_seen: bool,
}

#[derive(Default)]
enum Escape {
    #[default]
    None,
    // After ESC.
    Seen,
    // In `ESC [ …` until the final byte.
    Csi,
    // In `ESC ] …` until BEL or `ESC \`; the bytes so far.
    Osc(Vec<u8>),
    // In an OSC, after an ESC that may end it.
    OscEnding(Vec<u8>),
}

impl MarkerScanner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Vec<RunEvent> {
        let mut events = Vec::new();
        for &byte in bytes {
            match std::mem::take(&mut self.escape) {
                Escape::None => match byte {
                    0x1b => self.escape = Escape::Seen,
                    b'\n' => {
                        self.return_seen = false;
                        self.line.push(b'\n');
                        self.flush_line(&mut events);
                    }
                    b'\r' => self.return_seen = true,
                    0..=0x08 | 0x0a..=0x1f | 0x7f => {}
                    _ => {
                        if std::mem::take(&mut self.return_seen) {
                            self.line.clear();
                        }
                        self.line.push(byte);
                    }
                },
                Escape::Seen => match byte {
                    b'[' => self.escape = Escape::Csi,
                    b']' => self.escape = Escape::Osc(Vec::new()),
                    // A two-byte sequence (`ESC =`, `ESC 7`…) is dropped whole.
                    _ => {}
                },
                Escape::Csi => {
                    if !(0x40..=0x7e).contains(&byte) {
                        self.escape = Escape::Csi;
                    }
                }
                Escape::Osc(mut body) => match byte {
                    0x07 => self.finish_osc(&body, &mut events),
                    0x1b => self.escape = Escape::OscEnding(body),
                    _ => {
                        body.push(byte);
                        self.escape = Escape::Osc(body);
                    }
                },
                Escape::OscEnding(body) => {
                    // `ESC \` ends the sequence; an ESC that starts something else drops it unread.
                    if byte == b'\\' {
                        self.finish_osc(&body, &mut events);
                    }
                }
            }
        }
        events
    }

    fn finish_osc(&mut self, body: &[u8], events: &mut Vec<RunEvent>) {
        let text = String::from_utf8_lossy(body);
        let Some(rest) = text.strip_prefix("777;forge-run-") else { return };
        if let Some(id) = rest.strip_prefix("start;").and_then(|id| id.parse().ok()) {
            self.line.clear();
            self.return_seen = false;
            self.open = true;
            events.push(RunEvent::Started(id));
        } else if let Some((id, code)) = rest.strip_prefix("end;").and_then(|tail| tail.split_once(';')) {
            let (Ok(id), Ok(code)) = (id.parse(), code.trim().parse()) else { return };
            self.flush_line(events);
            self.open = false;
            events.push(RunEvent::Ended { id, code });
        }
    }

    // Hands out the line read so far as text, unless no run is open or it is the shell's echo of a marker line.
    fn flush_line(&mut self, events: &mut Vec<RunEvent>) {
        let line = std::mem::take(&mut self.line);
        if !self.open || line.is_empty() {
            return;
        }
        let line = String::from_utf8_lossy(&line).into_owned();
        if line.contains("777;forge-run-") {
            return;
        }
        events.push(RunEvent::Text(line));
    }
}

/// The last `OUTPUT_LINES` lines and at most `OUTPUT_CHARS` characters of `text`; true when something was cut.
pub fn cut_output(text: &str) -> (String, bool) {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let start = lines.len().saturating_sub(OUTPUT_LINES);
    let mut kept = lines[start..].concat();
    let mut cut = start > 0;
    let chars = kept.chars().count();
    if chars > OUTPUT_CHARS {
        kept = kept.chars().skip(chars - OUTPUT_CHARS).collect();
        cut = true;
    }
    (kept, cut)
}

/// The text sent to the model when a run is over. `code` is `None` when the shell ended before the command did.
pub fn model_message(block: &str, code: Option<i32>, output: &str, cut: bool) -> String {
    let status = match code {
        Some(code) => format!("exit {code}"),
        None => "shell ended".to_string(),
    };
    let cut_line = if cut { "(cut)\n" } else { "" };
    let output = output.trim_end_matches('\n');
    format!("{RUN_MESSAGE_MARK}\n```bash\n{}\n```\n{status}\n{cut_line}```\n{output}\n```", block.trim_end())
}

/// Whether a user message is a run message (see `RUN_MESSAGE_MARK`).
pub fn is_run_message(text: &str) -> bool {
    text.trim_start().lines().next().is_some_and(|first| first.trim_end() == RUN_MESSAGE_MARK)
}

/// A key for a block: the same text gives the same key, so the box of a run shows under every block with that text.
pub fn text_hash(block: &str) -> String {
    // FNV-1a, 64 bits.
    let hash = block.trim().bytes().fold(0xcbf29ce484222325u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3));
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "\x1b]777;forge-run-start;7\x07";
    const END_0: &str = "\x1b]777;forge-run-end;7;0\x07";

    fn feed_all(input: &[u8], step: usize) -> Vec<RunEvent> {
        let mut scanner = MarkerScanner::new();
        let mut events = Vec::new();
        for chunk in input.chunks(step) {
            events.extend(scanner.feed(chunk));
        }
        events
    }

    // Text events glued together, so a test does not depend on how the lines were split between reads.
    fn merged(events: Vec<RunEvent>) -> Vec<RunEvent> {
        let mut out: Vec<RunEvent> = Vec::new();
        for event in events {
            match (out.last_mut(), event) {
                (Some(RunEvent::Text(last)), RunEvent::Text(more)) => last.push_str(&more),
                (_, event) => out.push(event),
            }
        }
        out
    }

    #[test]
    fn shell_languages_are_the_listed_ones() {
        for name in ["bash", "sh", "zsh", "shell", "console", "shellscript", "terminal", "Bash"] {
            assert!(is_shell_language(name), "{name}");
        }
        for name in ["python", "json", "", "rust"] {
            assert!(!is_shell_language(name), "{name}");
        }
    }

    #[test]
    fn typed_text_wraps_the_block_between_the_two_markers() {
        let typed = String::from_utf8(typed_text(7, "echo hi")).unwrap();
        assert!(typed.starts_with("printf '\\033]777;forge-run-start;7\\007'; echo hi\n"), "{typed}");
        let last = typed.lines().last().unwrap();
        assert!(last.starts_with("printf '\\033]777;forge-run-end;7;%s\\007'") && last.ends_with("\"$?\""), "{typed}");
        assert!(typed.ends_with('\n'));
        assert_eq!(typed_text(7, "echo hi"), typed_text(7, "echo hi\n"));
        assert_eq!(typed_text(7, "echo hi"), typed_text(7, "echo hi\n\n"));
    }

    #[test]
    fn typed_text_keeps_the_later_lines_of_a_block() {
        let typed = String::from_utf8(typed_text(2, "cd /tmp\nls\n")).unwrap();
        let lines: Vec<&str> = typed.lines().collect();
        assert_eq!(lines.len(), 3, "{typed}");
        assert!(lines[0].ends_with("; cd /tmp"));
        assert_eq!(lines[1], "ls");
    }

    #[test]
    fn scanner_returns_started_text_ended() {
        let input = format!("echo hi\r\n{START}hi\r\n{END_0}");
        let events = merged(feed_all(input.as_bytes(), input.len()));
        assert_eq!(events, vec![RunEvent::Started(7), RunEvent::Text("hi\n".into()), RunEvent::Ended { id: 7, code: 0 }]);
    }

    #[test]
    fn text_outside_a_run_is_dropped() {
        let input = format!("prompt> \r\n{START}x\n{END_0}after\r\n");
        let events = merged(feed_all(input.as_bytes(), input.len()));
        assert_eq!(events, vec![RunEvent::Started(7), RunEvent::Text("x\n".into()), RunEvent::Ended { id: 7, code: 0 }]);
    }

    #[test]
    fn scanner_keeps_a_marker_split_between_two_feeds() {
        let input = format!("{START}héllo wörld\r\nline two\r\n{END_0}");
        let whole = merged(feed_all(input.as_bytes(), input.len()));
        for step in 1..input.len() {
            assert_eq!(merged(feed_all(input.as_bytes(), step)), whole, "step {step}");
        }
        assert_eq!(whole[1], RunEvent::Text("héllo wörld\nline two\n".into()));
    }

    #[test]
    fn a_non_zero_exit_code_is_read() {
        let input = "\x1b]777;forge-run-start;3\x07\x1b]777;forge-run-end;3;127\x07";
        let events = merged(feed_all(input.as_bytes(), input.len()));
        assert_eq!(events, vec![RunEvent::Started(3), RunEvent::Ended { id: 3, code: 127 }]);
    }

    #[test]
    fn text_has_colours_and_cursor_codes_removed() {
        let input = format!("{START}\x1b[31mred\x1b[0m \x1b[2K\x1b]0;title\x07done\n{END_0}");
        let events = merged(feed_all(input.as_bytes(), input.len()));
        assert_eq!(events[1], RunEvent::Text("red done\n".into()));
    }

    #[test]
    fn a_progress_line_keeps_the_last_state() {
        let input = format!("{START}10%\r50%\r100%\n{END_0}");
        let events = merged(feed_all(input.as_bytes(), input.len()));
        assert_eq!(events[1], RunEvent::Text("100%\n".into()));
    }

    #[test]
    fn the_echo_of_the_end_line_is_not_output() {
        let input = format!("{START}out\n❯ printf '\\033]777;forge-run-end;7;%s\\007' \"$?\"\r\n{END_0}");
        let events = merged(feed_all(input.as_bytes(), input.len()));
        assert_eq!(events[1], RunEvent::Text("out\n".into()));
    }

    #[test]
    fn a_last_line_without_a_newline_comes_before_the_end() {
        let input = format!("{START}no newline{END_0}");
        let events = merged(feed_all(input.as_bytes(), input.len()));
        assert_eq!(events, vec![RunEvent::Started(7), RunEvent::Text("no newline".into()), RunEvent::Ended { id: 7, code: 0 }]);
    }

    #[test]
    fn output_is_cut_to_the_last_200_lines_and_20000_characters() {
        let many: String = (0..300).map(|n| format!("line {n}\n")).collect();
        let (cut, was_cut) = cut_output(&many);
        assert!(was_cut);
        assert_eq!(cut.lines().count(), 200);
        assert!(cut.starts_with("line 100\n") && cut.ends_with("line 299\n"));

        let long = "x".repeat(30_000);
        let (cut, was_cut) = cut_output(&long);
        assert!(was_cut);
        assert_eq!(cut.chars().count(), 20_000);

        assert_eq!(cut_output("short\n"), ("short\n".to_string(), false));
    }

    #[test]
    fn model_message_has_the_fixed_first_line() {
        let message = model_message("echo hi", Some(2), "hi\n", false);
        assert!(message.starts_with("[forge-run]\n"), "{message}");
        assert!(message.contains("echo hi") && message.contains("exit 2") && message.contains("```\nhi\n```"), "{message}");
        assert!(!message.contains("(cut)"));
        assert!(model_message("echo hi", Some(0), "x\n", true).contains("\n(cut)\n"));
        assert!(model_message("sleep 9", None, "", false).contains("shell ended"));
    }

    #[test]
    fn a_run_message_starts_with_the_mark_on_its_own_line() {
        assert!(is_run_message("[forge-run]\nx"));
        assert!(is_run_message(" \n[forge-run]\nx"));
        assert!(is_run_message("[forge-run]"));
        assert!(!is_run_message("please [forge-run]"));
        assert!(!is_run_message("[forge-run]x"));
        assert!(is_run_message(&model_message("ls", Some(0), "a\n", false)));
    }

    #[test]
    fn the_hash_ignores_surrounding_whitespace_and_differs_for_other_text() {
        assert_eq!(text_hash("echo hi"), text_hash("echo hi\n"));
        assert_eq!(text_hash("echo hi"), text_hash("  echo hi  "));
        assert_ne!(text_hash("echo hi"), text_hash("echo ho"));
        assert_eq!(text_hash("a").len(), 16);
    }
}
