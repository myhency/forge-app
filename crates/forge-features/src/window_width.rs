use dioxus::prelude::*;

// The page tells Rust its width. The handler is replaced each time (`dioxus.send` belongs to the
// eval that created it) and reports once at the start.
const WATCH_WIDTH: &str = "if (window.__forgeWidthHandler) { window.removeEventListener('resize', window.__forgeWidthHandler); } window.__forgeWidthHandler = () => dioxus.send(Math.round(window.innerWidth)); window.addEventListener('resize', window.__forgeWidthHandler); window.__forgeWidthHandler();";

/// The width of the window in CSS pixels, which are the points of the screen. It reads 1200 (the
/// size the window opens at) until the page has reported, and changes only when the width does.
pub fn use_window_width() -> Signal<u32> {
    let mut width = use_signal(|| 1200u32);
    use_future(move || async move {
        let mut changes = document::eval(WATCH_WIDTH);
        while let Ok(next) = changes.recv::<u32>().await {
            if width() != next {
                width.set(next);
            }
        }
    });
    width
}

#[cfg(test)]
mod tests {
    use super::WATCH_WIDTH;

    #[test]
    fn the_listener_replaces_its_old_self_and_reports_at_once_and_on_every_resize() {
        // `dioxus.send` belongs to the eval that installed the listener; a new page scope must take
        // over, so the old handler is removed first.
        assert!(WATCH_WIDTH.contains("removeEventListener('resize'"), "{WATCH_WIDTH}");
        assert!(WATCH_WIDTH.contains("addEventListener('resize'"));
        assert!(WATCH_WIDTH.contains("dioxus.send"));
        assert!(WATCH_WIDTH.contains("Math.round(window.innerWidth)"));
        assert!(WATCH_WIDTH.trim_end().ends_with("window.__forgeWidthHandler();"), "reports once at the start");
    }
}
