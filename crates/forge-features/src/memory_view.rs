use chrono::{Local, TimeZone};

/// The day a note last changed as `2026-11-02` (local time), or blank when the time is unknown.
pub fn updated_day(seconds: i64) -> String {
    Local.timestamp_opt(seconds, 0).single().map(|time| time.format("%Y-%m-%d").to_string()).unwrap_or_default()
}

/// A kind from a note's header (`user`, `feedback`, `project`, `reference`) in words, or the kind itself when it
/// is one this app does not know.
pub fn kind_label(kind: &str) -> String {
    match kind {
        "user" => forge_shared::tr!("About you").to_string(),
        "feedback" => forge_shared::tr!("How to work").to_string(),
        "project" => forge_shared::tr!("Project").to_string(),
        "reference" => forge_shared::tr!("Reference").to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_becomes_a_day_and_an_impossible_time_becomes_blank() {
        assert_eq!(updated_day(0).len(), 10);
        assert_eq!(updated_day(i64::MAX), "");
    }

    #[test]
    fn an_unknown_kind_is_shown_as_it_is() {
        assert_eq!(kind_label("diary"), "diary");
        assert_eq!(kind_label(""), "");
    }
}
