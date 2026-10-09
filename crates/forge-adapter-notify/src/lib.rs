mod channel;
#[cfg(target_os = "macos")]
mod center;

use forge_ports::NotifyPort;

/// What `osascript` is given: a fixed script that reads its title and body from the arguments.
pub(crate) fn osascript_arguments(title: &str, body: &str) -> Vec<String> {
    vec![
        "-e".to_string(),
        "on run argv".to_string(),
        "-e".to_string(),
        "display notification (item 2 of argv) with title (item 1 of argv)".to_string(),
        "-e".to_string(),
        "end run".to_string(),
        title.to_string(),
        body.to_string(),
    ]
}

/// macOS: a notification through `osascript`. The text goes in as arguments, never inside the script.
pub struct OsascriptNotify {
    attention: AttentionSlot,
}

/// Where the window puts the function that bounces its Dock icon, once it exists.
pub type AttentionSlot = std::sync::Arc<std::sync::OnceLock<Box<dyn Fn() + Send + Sync>>>;

impl OsascriptNotify {
    pub fn new() -> Self {
        Self::with_attention(AttentionSlot::default())
    }

    pub fn with_attention(attention: AttentionSlot) -> Self {
        Self { attention }
    }
}

impl Default for OsascriptNotify {
    fn default() -> Self {
        Self::new()
    }
}

impl NotifyPort for OsascriptNotify {
    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        let output = std::process::Command::new("osascript").args(osascript_arguments(title, body)).output().map_err(|error| error.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }

    fn request_attention(&self) -> Result<(), String> {
        match self.attention.get() {
            Some(bounce) => {
                bounce();
                Ok(())
            }
            None => Err("the window is not ready to bounce the Dock icon".to_string()),
        }
    }
}

/// macOS: the notification center when Forge runs as an app bundle (a click opens the session the notification
/// is about), `osascript` otherwise. The Dock bounce is the same either way.
#[cfg(target_os = "macos")]
pub struct MacNotify {
    fallback: OsascriptNotify,
    center: Option<center::NotificationCenter>,
}

#[cfg(target_os = "macos")]
impl MacNotify {
    pub fn new(attention: AttentionSlot) -> Self {
        let center = match channel::channel_for(center::bundle_identifier().as_deref()) {
            channel::Channel::NotificationCenter => Some(center::NotificationCenter::start()),
            channel::Channel::Osascript => None,
        };
        Self { fallback: OsascriptNotify::with_attention(attention), center }
    }
}

#[cfg(target_os = "macos")]
impl NotifyPort for MacNotify {
    fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        match &self.center {
            Some(center) => center.send(title, body, None),
            None => self.fallback.notify(title, body),
        }
    }

    fn notify_target(&self, title: &str, body: &str, target: &str) -> Result<(), String> {
        match &self.center {
            Some(center) => center.send(title, body, Some(target)),
            None => self.fallback.notify(title, body),
        }
    }

    fn on_click(&self, handler: forge_ports::ClickHandler) {
        if let Some(center) = &self.center {
            center.set_click_handler(handler);
        }
    }

    fn request_attention(&self) -> Result<(), String> {
        self.fallback.request_attention()
    }
}

/// Where there is no adapter (Windows in this plan): says so, so the caller can ignore it.
pub struct NoNotify;

impl NotifyPort for NoNotify {
    fn notify(&self, _title: &str, _body: &str) -> Result<(), String> {
        Err("notifications are not available here".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_for_attention_without_a_window_is_an_error_not_a_panic() {
        assert!(OsascriptNotify::new().request_attention().is_err());
    }

    #[test]
    fn a_request_for_attention_calls_the_hook_the_window_gave() {
        let slot = AttentionSlot::default();
        let notify = OsascriptNotify::with_attention(slot.clone());
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = calls.clone();
        assert!(slot.set(Box::new(move || { counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst); })).is_ok());
        assert_eq!(notify.request_attention(), Ok(()));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn the_text_is_passed_as_arguments_and_never_as_script() {
        let arguments = osascript_arguments("Routine \"x\"; do shell script \"rm -rf ~\"", "it's here");
        // The script lines never contain the text; the last two arguments are the text itself.
        assert!(arguments[..6].iter().all(|part| !part.contains("rm -rf")));
        assert_eq!(arguments[6], "Routine \"x\"; do shell script \"rm -rf ~\"");
        assert_eq!(arguments[7], "it's here");
    }

    #[test]
    fn nothing_is_available_without_an_adapter() {
        assert!(NoNotify.notify("a", "b").is_err());
    }
}
