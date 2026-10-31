use forge_domain::{local_moment, LocalMoment, RateLevel, RateLimit, RateWindow};
use forge_shared::tr;

/// The words of the bar above the message box: what is wrong and when it starts over.
pub fn rate_limit_text(limit: &RateLimit) -> String {
    let what = match (limit.level, limit.window) {
        (RateLevel::Fine, _) => return String::new(),
        (RateLevel::Warning, RateWindow::Weekly) => tr!("Close to the weekly usage limit"),
        (RateLevel::Warning, RateWindow::FiveHour) => tr!("Close to the 5-hour usage limit"),
        (RateLevel::Warning, RateWindow::Other) => tr!("Close to a usage limit"),
        (RateLevel::Reached, RateWindow::Weekly) => tr!("The weekly usage limit is reached"),
        (RateLevel::Reached, RateWindow::FiveHour) => tr!("The 5-hour usage limit is reached"),
        (RateLevel::Reached, RateWindow::Other) => tr!("A usage limit is reached"),
    };
    match limit.resets_at.and_then(local_moment) {
        Some(moment) => format!("{what} · {}", reset_text(moment)),
        None => what.to_string(),
    }
}

fn weekday(index: u32) -> &'static str {
    match index {
        0 => tr!("Mon"),
        1 => tr!("Tue"),
        2 => tr!("Wed"),
        3 => tr!("Thu"),
        4 => tr!("Fri"),
        5 => tr!("Sat"),
        _ => tr!("Sun"),
    }
}

/// `Resets 10/28 (Wed) PM 6:00`.
pub fn reset_text(moment: LocalMoment) -> String {
    let (ampm, hour) = if moment.hour < 12 { (tr!("AM"), moment.hour) } else { (tr!("PM"), moment.hour - 12) };
    let hour = if hour == 0 { 12 } else { hour };
    tr!(
        "Resets {month}/{day} ({weekday}) {ampm} {clock}",
        month = moment.month,
        day = moment.day,
        weekday = weekday(moment.weekday),
        ampm = ampm,
        clock = format!("{hour}:{:02}", moment.minute)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limit(level: RateLevel, window: RateWindow, resets_at: Option<i64>) -> RateLimit {
        RateLimit { level, window, percent: Some(80), resets_at }
    }

    #[test]
    fn each_level_and_window_has_its_own_sentence_and_a_fine_limit_says_nothing() {
        assert_eq!(rate_limit_text(&limit(RateLevel::Warning, RateWindow::Weekly, None)), "Close to the weekly usage limit");
        assert_eq!(rate_limit_text(&limit(RateLevel::Warning, RateWindow::FiveHour, None)), "Close to the 5-hour usage limit");
        assert_eq!(rate_limit_text(&limit(RateLevel::Reached, RateWindow::Other, None)), "A usage limit is reached");
        assert_eq!(rate_limit_text(&limit(RateLevel::Fine, RateWindow::Weekly, Some(1))), "");
    }

    #[test]
    fn the_reset_time_follows_the_sentence_in_local_time() {
        let text = rate_limit_text(&limit(RateLevel::Warning, RateWindow::Weekly, Some(1_793_178_000)));
        assert!(text.starts_with("Close to the weekly usage limit · Resets "), "{text}");
        assert!(text.ends_with("AM") || text.contains(" AM ") || text.contains(" PM "), "{text}");
    }

    #[test]
    fn the_clock_has_twelve_hours() {
        let at = |hour| reset_text(LocalMoment { month: 1, day: 2, weekday: 0, hour, minute: 5 });
        assert_eq!(at(0), "Resets 1/2 (Mon) AM 12:05");
        assert_eq!(at(12), "Resets 1/2 (Mon) PM 12:05");
        assert_eq!(at(18), "Resets 1/2 (Mon) PM 6:05");
    }
}
