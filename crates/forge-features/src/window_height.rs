use dioxus::prelude::*;

// The same as the width watcher, for the height.
const WATCH_HEIGHT: &str = "if (window.__forgeHeightHandler) { window.removeEventListener('resize', window.__forgeHeightHandler); } window.__forgeHeightHandler = () => dioxus.send(Math.round(window.innerHeight)); window.addEventListener('resize', window.__forgeHeightHandler); window.__forgeHeightHandler();";

/// The height of the window in CSS pixels. It reads 800 (the size the window opens at) until the
/// page has reported, and changes only when the height does.
pub fn use_window_height() -> Signal<u32> {
    let mut height = use_signal(|| 800u32);
    use_future(move || async move {
        let mut changes = document::eval(WATCH_HEIGHT);
        while let Ok(next) = changes.recv::<u32>().await {
            if height() != next {
                height.set(next);
            }
        }
    });
    height
}

#[cfg(test)]
mod tests {
    use super::WATCH_HEIGHT;

    #[test]
    fn the_listener_replaces_its_old_self_and_reports_at_once_and_on_every_resize() {
        assert!(WATCH_HEIGHT.contains("removeEventListener('resize'"), "{WATCH_HEIGHT}");
        assert!(WATCH_HEIGHT.contains("addEventListener('resize'"));
        assert!(WATCH_HEIGHT.contains("Math.round(window.innerHeight)"));
        assert!(WATCH_HEIGHT.trim_end().ends_with("window.__forgeHeightHandler();"), "reports once at the start");
    }
}
