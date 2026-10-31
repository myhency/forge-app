//! The hook that turns an English sentence into the person's language. The domain cannot know the
//! tables or the screen, so `forge-shared` registers the real translator at start (`set_translator`);
//! until then, and in the domain's own unit tests, the English sentence is returned as it is.

use std::sync::OnceLock;

static TRANSLATOR: OnceLock<fn(&'static str) -> &'static str> = OnceLock::new();

/// Registers the function `text` uses. Only the first call counts.
pub fn set_translator(translator: fn(&'static str) -> &'static str) {
    let _ = TRANSLATOR.set(translator);
}

/// The sentence a key may carry after a bar (`"Created|tool"`) tells two meanings of one English
/// word apart in the tables; it is never shown.
pub fn without_context(english: &'static str) -> &'static str {
    english.split('|').next().unwrap_or(english)
}

/// `english` in the language on screen.
pub fn text(english: &'static str) -> &'static str {
    match TRANSLATOR.get() {
        Some(translator) => translator(english),
        None => without_context(english),
    }
}

/// Replaces each `{name}` in `template` with its value. A placeholder with no value stays as
/// written, and the text of a value is never searched again.
pub fn fill(template: &str, args: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let value = after
            .find('}')
            .and_then(|end| args.iter().find(|(name, _)| *name == &after[1..end]).map(|(_, value)| (end, value)));
        match value {
            Some((end, value)) => {
                out.push_str(value);
                rest = &after[end + 1..];
            }
            None => {
                out.push('{');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The text in the current language: `tr!("Settings")`, or with values `tr!("{n} sessions", n = count)`.
/// Takes a string literal only, so a test can list every sentence.
#[macro_export]
macro_rules! tr {
    ($text:literal) => {
        $crate::i18n::text($text)
    };
    ($text:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::fill($crate::i18n::text($text), &[$((stringify!($name), ($value).to_string())),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_translator_the_english_sentence_is_returned_without_its_context() {
        assert_eq!(text("Settings"), "Settings");
        assert_eq!(text("Created|tool"), "Created");
        assert_eq!(fill(text("{n} sessions"), &[("n", 3.to_string())]), "3 sessions");
    }

    #[test]
    fn values_are_filled_by_name_and_a_missing_one_stays_as_written() {
        let args = [("n", "3".to_string())];
        assert_eq!(fill("{n} of {total}", &args), "3 of {total}");
        let tricky = [("a", "{b}".to_string()), ("b", "x".to_string())];
        assert_eq!(fill("{a}", &tricky), "{b}");
    }
}
