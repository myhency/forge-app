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
pub struct OsascriptNotify;

impl OsascriptNotify {
    pub fn new() -> Self {
        Self
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
