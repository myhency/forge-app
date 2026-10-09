/// How a notification is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Channel {
    /// The system's notification center, which can tell Forge about a click. Needs an app bundle.
    NotificationCenter,
    /// `osascript`, for a binary that is not an app bundle (a development run). A click opens Script Editor.
    Osascript,
}

/// The notification center only works for a program that has a bundle identifier.
pub(crate) fn channel_for(bundle_identifier: Option<&str>) -> Channel {
    match bundle_identifier {
        Some(identifier) if !identifier.trim().is_empty() => Channel::NotificationCenter,
        _ => Channel::Osascript,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_app_bundle_uses_the_notification_center() {
        assert_eq!(channel_for(Some("com.example.Forge")), Channel::NotificationCenter);
    }

    #[test]
    fn a_bare_binary_falls_back_to_osascript() {
        assert_eq!(channel_for(None), Channel::Osascript);
        assert_eq!(channel_for(Some("  ")), Channel::Osascript);
    }
}
