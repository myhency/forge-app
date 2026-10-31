/// Where the account stands against one of its usage limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimit {
    pub level: RateLevel,
    pub window: RateWindow,
    /// How much of the limit is used, in percent, when known.
    pub percent: Option<u32>,
    /// When the limit starts over (seconds since 1970), when known.
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLevel {
    Fine,
    /// Close to the limit.
    Warning,
    /// At the limit: nothing more can be sent until it starts over.
    Reached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateWindow {
    FiveHour,
    Weekly,
    Other,
}

impl RateLimit {
    /// What the SDK's words mean; `None` for a status that says nothing about a limit.
    pub fn from_report(status: &str, kind: Option<&str>, percent: Option<u32>, resets_at: Option<i64>) -> Option<Self> {
        let level = match status {
            "allowed" => RateLevel::Fine,
            "allowed_warning" => RateLevel::Warning,
            "rejected" => RateLevel::Reached,
            _ => return None,
        };
        let window = match kind {
            Some("five_hour") => RateWindow::FiveHour,
            Some("seven_day") | Some("seven_day_opus") | Some("seven_day_sonnet") => RateWindow::Weekly,
            _ => RateWindow::Other,
        };
        Some(Self { level, window, percent, resets_at })
    }

    /// Whether the chat shows a bar about it.
    pub fn needs_a_bar(&self) -> bool {
        self.level != RateLevel::Fine
    }
}

/// A moment in the person's own time zone, in the parts a sentence needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalMoment {
    pub month: u32,
    pub day: u32,
    /// 0 is Monday.
    pub weekday: u32,
    pub hour: u32,
    pub minute: u32,
}

/// `seconds` since 1970 as local calendar parts (`None` for a value out of range).
pub fn local_moment(seconds: i64) -> Option<LocalMoment> {
    use chrono::{Datelike, TimeZone, Timelike};
    let moment = chrono::Local.timestamp_opt(seconds, 0).single()?;
    Some(LocalMoment { month: moment.month(), day: moment.day(), weekday: moment.weekday().num_days_from_monday(), hour: moment.hour(), minute: moment.minute() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moment_has_calendar_parts() {
        let moment = local_moment(1_793_178_000).unwrap();
        assert!((1..=12).contains(&moment.month) && (1..=31).contains(&moment.day) && moment.weekday < 7 && moment.hour < 24 && moment.minute < 60);
        assert!(local_moment(i64::MAX).is_none());
    }

    #[test]
    fn the_sdks_status_words_become_levels_and_windows() {
        let warn = RateLimit::from_report("allowed_warning", Some("seven_day"), Some(75), Some(10)).unwrap();
        assert_eq!((warn.level, warn.window, warn.percent, warn.resets_at), (RateLevel::Warning, RateWindow::Weekly, Some(75), Some(10)));
        assert!(warn.needs_a_bar());
        let hit = RateLimit::from_report("rejected", Some("five_hour"), None, None).unwrap();
        assert_eq!((hit.level, hit.window), (RateLevel::Reached, RateWindow::FiveHour));
        assert!(!RateLimit::from_report("allowed", None, None, None).unwrap().needs_a_bar());
        assert_eq!(RateLimit::from_report("allowed", Some("overage"), None, None).unwrap().window, RateWindow::Other);
        assert!(RateLimit::from_report("something new", None, None, None).is_none());
    }
}
