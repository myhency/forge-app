//! How the window width decides the layout. Round 10 of the roadmap observations (the real Claude
//! app at 1500 pt down to 600 pt) is where the numbers come from.

/// The window cannot be made narrower than this (the real app refuses too).
pub const MIN_WINDOW_WIDTH: u32 = 600;
/// At this width and below the sidebar is an overlay that the toggle shows.
pub const SIDEBAR_OVERLAY_UP_TO: u32 = 700;
/// What the folded sidebar's rail takes in a docked layout.
pub const SIDEBAR_RAIL_WIDTH: u32 = 36;
/// An open dock panel pushes the chat only while the chat keeps this much.
pub const MIN_CHAT_WIDTH: u32 = 300;
/// A chat narrower than this gets the compact header.
pub const NARROW_CHAT_BELOW: u32 = 560;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarMode {
    Docked,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockMode {
    Push,
    Float,
}

/// Whether the sidebar is docked or an overlay at this window width.
pub fn sidebar_mode_for(window_width: u32) -> SidebarMode {
    if window_width.max(MIN_WINDOW_WIDTH) <= SIDEBAR_OVERLAY_UP_TO {
        SidebarMode::Overlay
    } else {
        SidebarMode::Docked
    }
}

/// How one pane lays out its dock. `dock_width` is the sum of the open dock columns, 0 when there is
/// none or when one is expanded over the pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneLayout {
    pub dock: DockMode,
    /// The width the chat column gets.
    pub chat_width: u32,
}

pub fn pane_layout(pane_width: u32, dock_width: u32) -> PaneLayout {
    let pushed = pane_width.saturating_sub(dock_width);
    if dock_width == 0 || pushed >= MIN_CHAT_WIDTH {
        PaneLayout { dock: DockMode::Push, chat_width: pushed }
    } else {
        PaneLayout { dock: DockMode::Float, chat_width: pane_width }
    }
}

/// Whether an open overlay sidebar stays open. It goes away when the window grows past the overlay
/// width (so it does not come back by itself on the way down) and when Settings or the search
/// palette opens over it (they would be half hidden under the sidebar).
pub fn keep_peek(peek: bool, window_width: u32, overlay_open: bool) -> bool {
    peek && window_width <= SIDEBAR_OVERLAY_UP_TO && !overlay_open
}

/// The class list of `.workspace`: the sidebar mode and the sidebar drag.
pub fn workspace_class(sidebar: SidebarMode, resizing: bool) -> String {
    let mut class = String::from("workspace");
    if resizing {
        class.push_str(" resizing");
    }
    if sidebar == SidebarMode::Overlay {
        class.push_str(" sidebar-overlay");
    }
    class
}

/// A pane lower than this tightens its vertical spacing.
pub const SHORT_PANE_BELOW: u32 = 360;

/// The class list of one pane: the responsive rules that depend on the pane's own size. With one
/// pane the size is the whole main area.
pub fn pane_class(pane_width: u32, pane_height: u32, dock_width: u32) -> String {
    let layout = pane_layout(pane_width, dock_width);
    let mut class = String::from("pane");
    if layout.dock == DockMode::Float {
        class.push_str(" dock-float");
    }
    if layout.chat_width < NARROW_CHAT_BELOW {
        class.push_str(" chat-narrow");
    }
    if pane_height < SHORT_PANE_BELOW {
        class.push_str(" pane-short");
    }
    class
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sidebar_is_an_overlay_up_to_700_and_docked_from_701() {
        assert_eq!(sidebar_mode_for(600), SidebarMode::Overlay);
        assert_eq!(sidebar_mode_for(700), SidebarMode::Overlay);
        assert_eq!(sidebar_mode_for(701), SidebarMode::Docked);
        assert_eq!(sidebar_mode_for(1200), SidebarMode::Docked);
    }

    #[test]
    fn a_window_below_the_minimum_is_treated_like_the_smallest() {
        assert_eq!(sidebar_mode_for(0), SidebarMode::Overlay);
    }

    #[test]
    fn the_dock_pushes_the_chat_until_the_chat_would_be_under_300() {
        // 780 - 480 = 300: exactly enough.
        assert_eq!(pane_layout(780, 480), PaneLayout { dock: DockMode::Push, chat_width: 300 });
        // One point less and it floats; the chat keeps all the room of the pane.
        assert_eq!(pane_layout(779, 480), PaneLayout { dock: DockMode::Float, chat_width: 779 });
    }

    #[test]
    fn no_dock_is_always_a_push_and_the_chat_gets_what_is_left() {
        assert_eq!(pane_layout(221, 0), PaneLayout { dock: DockMode::Push, chat_width: 221 });
        assert_eq!(pane_layout(936, 0).chat_width, 936);
    }

    #[test]
    fn a_dock_wider_than_the_pane_does_not_underflow() {
        assert_eq!(pane_layout(600, 4000), PaneLayout { dock: DockMode::Float, chat_width: 600 });
    }

    #[test]
    fn the_workspace_class_names_the_sidebar_mode_and_the_drag() {
        assert_eq!(workspace_class(SidebarMode::Docked, false), "workspace");
        assert_eq!(workspace_class(SidebarMode::Docked, true), "workspace resizing");
        assert_eq!(workspace_class(SidebarMode::Overlay, false), "workspace sidebar-overlay");
    }

    #[test]
    fn a_pane_names_each_mode_that_is_on() {
        assert_eq!(pane_class(1236, 800, 0), "pane");
        // 650 wide with a 350 dock still pushes (the chat keeps exactly 300) and is narrow.
        assert_eq!(pane_class(650, 800, 350), "pane chat-narrow");
        // A 480 dock would leave 240, so it floats and the chat keeps the whole 720.
        assert_eq!(pane_class(720, 800, 480), "pane dock-float");
        // 1000 wide: the dock pushes, the chat keeps 520, which is narrow.
        assert_eq!(pane_class(1000, 800, 480), "pane chat-narrow");
        // Floating over a 601 wide pane: the chat has all 601, which is not narrow.
        assert_eq!(pane_class(601, 800, 480), "pane dock-float");
    }

    #[test]
    fn the_header_is_narrow_under_560() {
        assert_eq!(pane_class(559, 800, 0), "pane chat-narrow");
        assert_eq!(pane_class(560, 800, 0), "pane");
    }

    #[test]
    fn the_overlay_sidebar_stays_out_only_in_an_overlay_window_with_nothing_on_top_of_it() {
        assert!(keep_peek(true, 650, false));
        assert!(!keep_peek(true, 650, true), "Settings or the search palette opened from the sidebar puts it away");
        assert!(!keep_peek(true, 701, false), "a window grown past the overlay width forgets it");
        assert!(!keep_peek(false, 650, false));
    }

    #[test]
    fn a_pane_under_360_high_is_short() {
        assert_eq!(pane_class(1000, 360, 0), "pane");
        assert_eq!(pane_class(1000, 359, 0), "pane pane-short");
        // Width and height classes add up.
        assert_eq!(pane_class(500, 300, 0), "pane chat-narrow pane-short");
    }
}
