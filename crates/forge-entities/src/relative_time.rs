use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// "just now", "5m ago", "3h ago", "2d ago", or the date for anything older than a week. An
/// unparseable timestamp gives an empty string rather than an error: it is only a label.
pub fn relative_time(updated_at: &str, now: OffsetDateTime) -> String {
    let Ok(then) = OffsetDateTime::parse(updated_at, &Rfc3339) else {
        return String::new();
    };
    let seconds = (now - then).whole_seconds();
    match seconds {
        s if s < 60 => "just now".to_string(),
        s if s < 3_600 => format!("{}m ago", s / 60),
        s if s < 86_400 => format!("{}h ago", s / 3_600),
        s if s < 7 * 86_400 => format!("{}d ago", s / 86_400),
        _ => format!("{:04}-{:02}-{:02}", then.year(), u8::from(then.month()), then.day()),
    }
}

#[cfg(test)]
mod tests {
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    use super::*;

    fn now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-09-29T12:00:00Z", &Rfc3339).unwrap()
    }

    #[test]
    fn under_a_minute_is_just_now() {
        assert_eq!(relative_time("2026-09-29T11:59:30Z", now()), "just now");
    }

    #[test]
    fn a_timestamp_in_the_future_is_just_now() {
        assert_eq!(relative_time("2026-09-29T12:05:00Z", now()), "just now");
    }

    #[test]
    fn minutes_hours_and_days_are_relative() {
        assert_eq!(relative_time("2026-09-29T11:55:00Z", now()), "5m ago");
        assert_eq!(relative_time("2026-09-29T09:00:00Z", now()), "3h ago");
        assert_eq!(relative_time("2026-09-27T12:00:00Z", now()), "2d ago");
    }

    #[test]
    fn a_week_or_older_shows_the_date() {
        assert_eq!(relative_time("2026-09-22T12:00:00Z", now()), "2026-09-22");
        assert_eq!(relative_time("2025-01-05T08:00:00Z", now()), "2025-01-05");
    }

    #[test]
    fn fractional_seconds_are_understood() {
        assert_eq!(relative_time("2026-09-29T11:55:00.960415885Z", now()), "4m ago");
    }

    #[test]
    fn an_unparseable_timestamp_is_an_empty_label() {
        assert_eq!(relative_time("not a time", now()), "");
        assert_eq!(relative_time("", now()), "");
    }
}
