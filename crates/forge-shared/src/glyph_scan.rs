//! Finds pictographs (emoji and symbol glyphs used as pictures) in Rust source, so a glyph does not come back in place of an icon.
//!
//! What is not looked at: comments, the body of a `#[cfg(test)]` module, and a line that carries the `icon-ok` marker. What is allowed
//! on a line that holds a translated sentence (a call of the two translation macros): the arrows and the modifier keys, which are ordinary text there.

use std::path::{Path, PathBuf};

/// A glyph found in a file: the 1-based line and the character.
pub type Found = (usize, char);

// The two translation macros, spelled in pieces so the i18n scan of the sources does not read this file's code as a call.
const TRANSLATE: &str = concat!("tr", "!(");
const MESSAGE: &str = concat!("msg", "!(");

// The modifier keys of a shortcut, which are letters of a key name rather than pictures.
const KEY_NAMES: [char; 4] = ['\u{2318}', '\u{21E7}', '\u{2325}', '\u{2303}'];
const ARROWS: [char; 4] = ['\u{2190}', '\u{2191}', '\u{2192}', '\u{2193}'];

fn is_pictograph(c: char) -> bool {
    matches!(c as u32,
        0x1F300..=0x1FAFF   // emoji
        | 0x2600..=0x27BF   // symbols and dingbats
        | 0x2B00..=0x2BFF   // arrows and symbols
        | 0x2190..=0x21FF   // arrows
        | 0x2300..=0x23FF   // technical
        | 0x25A0..=0x25FF   // geometric shapes
        | 0x2261            // identical to
        | 0x22EE            // vertical ellipsis
        | 0xFE0F            // emoji presentation selector
    )
}

/// The line with a trailing `//` comment cut off, keeping a `//` that sits inside a string.
fn without_comment(line: &str) -> &str {
    let (mut in_string, mut escaped) = (false, false);
    let bytes = line.as_bytes();
    for (at, byte) in bytes.iter().enumerate() {
        match (in_string, escaped, *byte) {
            (true, true, _) => escaped = false,
            (true, false, b'\\') => escaped = true,
            (_, _, b'"') => in_string = !in_string,
            (false, _, b'/') if bytes.get(at + 1) == Some(&b'/') => return &line[..at],
            _ => {}
        }
    }
    line
}

/// The source with every `#[cfg(test)]` module body blanked (the lines stay, so line numbers still match).
fn without_test_modules(source: &str) -> Vec<String> {
    let mut lines: Vec<String> = source.lines().map(str::to_string).collect();
    let mut at = 0;
    while at < lines.len() {
        if lines[at].trim_start().starts_with("#[cfg(test)]") {
            let mut depth = 0i32;
            let mut opened = false;
            let mut end = at;
            for (index, line) in lines.iter().enumerate().skip(at) {
                for c in without_comment(line).chars() {
                    match c {
                        '{' => {
                            depth += 1;
                            opened = true;
                        }
                        '}' => depth -= 1,
                        _ => {}
                    }
                }
                end = index;
                if opened && depth <= 0 {
                    break;
                }
            }
            for line in &mut lines[at..=end] {
                line.clear();
            }
            at = end;
        }
        at += 1;
    }
    lines
}

/// Every pictograph in `source` that is not a comment, a test, a marked line or text of a translated sentence.
pub fn pictographs(source: &str) -> Vec<Found> {
    let mut found = Vec::new();
    for (index, line) in without_test_modules(source).iter().enumerate() {
        if line.contains("icon-ok") {
            continue;
        }
        let code = without_comment(line);
        let sentence = code.contains(TRANSLATE) || code.contains(MESSAGE);
        for c in code.chars().filter(|c| is_pictograph(*c)) {
            let text = sentence && (ARROWS.contains(&c) || KEY_NAMES.contains(&c));
            if !text && !KEY_NAMES.contains(&c) {
                found.push((index + 1, c));
            }
        }
    }
    found
}

/// The workspace's `crates/` directory.
fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// `crates/<crate>/src/<file>` as read from disk.
pub fn read(relative: &str) -> String {
    let path = crates_dir().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// Pictographs in each named file (paths relative to `crates/`), as `file:line: glyph`.
pub fn in_files(files: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    for file in files {
        for (line, glyph) in pictographs(&read(file)) {
            found.push(format!("{file}:{line}: {glyph}"));
        }
    }
    found
}

/// Every Rust source file under `crates/*/src`, relative to `crates/`.
pub fn all_sources() -> Vec<String> {
    fn walk(directory: &Path, root: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(directory).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push(path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let root = crates_dir();
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap().flatten() {
        let src = entry.path().join("src");
        if src.is_dir() {
            walk(&src, &root, &mut out);
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_emoji_or_a_symbol_in_code_is_found() {
        assert_eq!(pictographs("let a = \"\u{1F4C1}\";"), vec![(1, '\u{1F4C1}')]);
        assert_eq!(pictographs("a\nb \"\u{2715}\""), vec![(2, '\u{2715}')]);
        assert_eq!(pictographs("\"\u{2713}\""), vec![(1, '\u{2713}')]);
    }

    #[test]
    fn what_is_not_a_picture_is_let_through() {
        assert!(pictographs("// \u{1F4C1} in a comment").is_empty());
        assert!(pictographs("code // \u{2715}").is_empty());
        assert!(pictographs("let a = \"\u{2318}W\";").is_empty(), "modifier keys are key names");
        assert!(pictographs(&format!("{TRANSLATE}\"\u{2190} Skills\")")).is_empty(), "an arrow in a translated sentence is text");
        assert!(pictographs("\"\u{2715}\" // icon-ok").is_empty(), "a marked line is allowed");
        assert!(pictographs("let url = \"https://x\"; \"\u{2713}\"").len() == 1, "a // inside a string does not start a comment");
    }

    #[test]
    fn a_test_module_is_not_looked_at() {
        let source = "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn t() { let x = \"\u{1F600}\"; }\n}\nfn b() { \"\u{2715}\" }\n";
        assert_eq!(pictographs(source), vec![(6, '\u{2715}')]);
    }

    #[test]
    fn an_emoji_in_a_translated_sentence_is_still_found() {
        assert_eq!(pictographs(&format!("{TRANSLATE}\"\u{1F4C1} Folder\")")), vec![(1, '\u{1F4C1}')]);
    }
}
