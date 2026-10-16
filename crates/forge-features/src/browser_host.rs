use forge_ports::Surface;

/// What decides whether a tab's web view may be seen.
#[derive(Debug, Clone, PartialEq)]
pub struct ShowInput {
    pub session_active: bool,
    pub panel_open: bool,
    pub tab_active: bool,
    /// Any menu, dialog, palette or lightbox of the app is open. A native view sits above the page, so it must step aside.
    pub overlay_open: bool,
    pub surface: Option<Surface>,
    /// The panel shows the "cannot connect" notice instead of the page.
    pub failed: bool,
}

pub fn should_show(input: &ShowInput) -> bool {
    let sized = input.surface.is_some_and(|surface| surface.width >= 1.0 && surface.height >= 1.0);
    input.session_active && input.panel_open && input.tab_active && !input.overlay_open && !input.failed && sized
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_good() -> ShowInput {
        ShowInput {
            session_active: true,
            panel_open: true,
            tab_active: true,
            overlay_open: false,
            surface: Some(Surface { x: 10.0, y: 10.0, width: 400.0, height: 300.0 }),
            failed: false,
        }
    }

    #[test]
    fn it_shows_when_nothing_is_in_the_way() {
        assert!(should_show(&all_good()));
    }

    #[test]
    fn each_reason_alone_hides_it() {
        let cases: [(&str, fn(&mut ShowInput)); 6] = [
            ("another session", |i| i.session_active = false),
            ("closed panel", |i| i.panel_open = false),
            ("another tab", |i| i.tab_active = false),
            ("an overlay", |i| i.overlay_open = true),
            ("no surface yet", |i| i.surface = None),
            ("the failed notice", |i| i.failed = true),
        ];
        for (name, change) in cases {
            let mut input = all_good();
            change(&mut input);
            assert!(!should_show(&input), "{name}");
        }
    }

    #[test]
    fn an_empty_surface_hides_it() {
        for (w, h) in [(0.0, 300.0), (400.0, 0.0), (0.5, 300.0)] {
            let mut input = all_good();
            input.surface = Some(Surface { x: 0.0, y: 0.0, width: w, height: h });
            assert!(!should_show(&input), "{w}x{h}");
        }
    }

    #[test]
    fn it_shows_again_once_the_reason_is_gone() {
        let mut input = all_good();
        input.overlay_open = true;
        assert!(!should_show(&input));
        input.overlay_open = false;
        assert!(should_show(&input));
    }
}
