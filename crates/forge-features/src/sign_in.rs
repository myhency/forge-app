//! Signs a connector in by typing into the CLI's own `/mcp` screen. `PlainText` turns what the terminal prints
//! into text to look at; `SignInDriver` decides what to type.

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
