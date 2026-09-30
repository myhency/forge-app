use crate::settings::{SettingsSection, SettingsStatus};

/// The status of the last save, if it belongs to the section being shown. The slot remembers which
/// section it came from, so a save that finishes after the person switched tab does not appear on
/// the new one (spec §17.5).
pub(crate) fn status_for(
    slot: &Option<(SettingsSection, SettingsStatus)>,
    current: SettingsSection,
) -> Option<SettingsStatus> {
    match slot {
        Some((section, status)) if *section == current => Some(status.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_shows_only_on_the_section_it_belongs_to() {
        let slot = Some((SettingsSection::Model, SettingsStatus::Saved));
        assert_eq!(status_for(&slot, SettingsSection::Model), Some(SettingsStatus::Saved));
        assert_eq!(status_for(&slot, SettingsSection::Connection), None);
        assert_eq!(status_for(&slot, SettingsSection::Skills), None);
    }

    #[test]
    fn no_status_shows_nothing() {
        assert_eq!(status_for(&None, SettingsSection::Model), None);
    }

    #[test]
    fn an_error_is_kept_with_its_message() {
        let slot = Some((SettingsSection::Plugins, SettingsStatus::Error("boom".to_string())));
        assert_eq!(status_for(&slot, SettingsSection::Plugins), Some(SettingsStatus::Error("boom".to_string())));
    }
}
