use std::path::PathBuf;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// What the Archive inactive sessions menu offers, in order (`None` is off).
pub const AUTO_ARCHIVE_CHOICES: [Option<u32>; 6] = [None, Some(1), Some(2), Some(7), Some(14), Some(30)];

/// A session as the auto archive sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveCandidate {
    pub project: PathBuf,
    pub session_id: String,
    /// RFC 3339, the time of the session's last activity.
    pub updated_at: String,
    /// Open and running, waiting for an approval, or with background work.
    pub busy: bool,
    pub archived: bool,
}

/// The sessions to archive now: not archived, not busy, and idle for at least `days` days.
/// `days == 0` (a hand-edited config) means off: nothing is due.
pub fn sessions_to_archive(now_ms: i64, days: u32, rows: &[ArchiveCandidate]) -> Vec<&ArchiveCandidate> {
    if days == 0 {
        return Vec::new();
    }
    let limit = i64::from(days) * DAY_MS;
    rows.iter()
        .filter(|row| !row.archived && !row.busy)
        .filter(|row| {
            chrono::DateTime::parse_from_rfc3339(&row.updated_at)
                .map(|at| now_ms - at.timestamp_millis() >= limit)
                .unwrap_or(false)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 24 * 60 * 60 * 1000;
    // 2026-10-17T00:00:00Z
    const NOW: i64 = 1_792_195_200_000;

    fn row(updated_at: &str) -> ArchiveCandidate {
        ArchiveCandidate { project: "/p".into(), session_id: "s".to_string(), updated_at: updated_at.to_string(), busy: false, archived: false }
    }

    #[test]
    fn a_session_idle_for_exactly_the_days_is_due() {
        let rows = vec![row("2026-10-16T00:00:00Z")];
        assert_eq!(sessions_to_archive(NOW, 1, &rows).len(), 1);
        assert_eq!(NOW - DAY, 1_792_108_800_000);
    }

    #[test]
    fn a_session_idle_for_less_than_the_days_is_not_due() {
        let rows = vec![row("2026-10-16T00:00:01Z")];
        assert!(sessions_to_archive(NOW, 1, &rows).is_empty());
    }

    #[test]
    fn a_busy_or_archived_session_is_never_due() {
        let busy = ArchiveCandidate { busy: true, ..row("2020-01-01T00:00:00Z") };
        let archived = ArchiveCandidate { archived: true, ..row("2020-01-01T00:00:00Z") };
        assert!(sessions_to_archive(NOW, 30, &[busy, archived]).is_empty());
    }

    #[test]
    fn zero_days_means_off() {
        assert!(sessions_to_archive(NOW, 0, &[row("2020-01-01T00:00:00Z")]).is_empty());
    }

    #[test]
    fn a_time_that_cannot_be_read_is_not_due() {
        assert!(sessions_to_archive(NOW, 1, &[row(""), row("yesterday")]).is_empty());
    }

    #[test]
    fn a_time_with_an_offset_is_read_correctly() {
        // 2026-10-16T09:00:00+09:00 is 2026-10-16T00:00:00Z.
        assert_eq!(sessions_to_archive(NOW, 1, &[row("2026-10-16T09:00:00+09:00")]).len(), 1);
    }

    #[test]
    fn the_menu_choices_are_none_and_the_five_periods() {
        assert_eq!(AUTO_ARCHIVE_CHOICES, [None, Some(1), Some(2), Some(7), Some(14), Some(30)]);
    }
}
