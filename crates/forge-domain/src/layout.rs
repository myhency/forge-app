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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub sidebar: SidebarMode,
    pub dock: DockMode,
    /// The width the chat column gets.
    pub chat_width: u32,
}

/// `sidebar_width` is what a docked sidebar (or its rail) takes; `dock_width` is the sum of the open
/// dock columns, 0 when there is none or when one is expanded over the pane.
pub fn layout_for(window_width: u32, sidebar_width: u32, dock_width: u32) -> Layout {
    let window_width = window_width.max(MIN_WINDOW_WIDTH);
    let sidebar = if window_width <= SIDEBAR_OVERLAY_UP_TO { SidebarMode::Overlay } else { SidebarMode::Docked };
    let taken = match sidebar {
        SidebarMode::Docked => sidebar_width,
        SidebarMode::Overlay => 0,
    };
    let beside = window_width.saturating_sub(taken);
    let pushed = beside.saturating_sub(dock_width);
    if dock_width == 0 || pushed >= MIN_CHAT_WIDTH {
        Layout { sidebar, dock: DockMode::Push, chat_width: pushed }
    } else {
        Layout { sidebar, dock: DockMode::Float, chat_width: beside }
    }
}

/// Whether an open overlay sidebar stays open. It goes away when the window grows past the overlay
/// width (so it does not come back by itself on the way down) and when Settings or the search
/// palette opens over it (they would be half hidden under the sidebar).
pub fn keep_peek(peek: bool, window_width: u32, overlay_open: bool) -> bool {
    peek && window_width <= SIDEBAR_OVERLAY_UP_TO && !overlay_open
}

/// The class list of `.workspace`: every visual rule of the responsive layout hangs on one of these.
pub fn workspace_class(layout: Layout, resizing: bool) -> String {
    let mut class = String::from("workspace");
    if resizing {
        class.push_str(" resizing");
    }
    if layout.sidebar == SidebarMode::Overlay {
        class.push_str(" sidebar-overlay");
    }
    if layout.dock == DockMode::Float {
        class.push_str(" dock-float");
    }
    if layout.chat_width < NARROW_CHAT_BELOW {
        class.push_str(" chat-narrow");
    }
    class
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sidebar_is_an_overlay_up_to_700_and_docked_from_701() {
        assert_eq!(layout_for(600, 264, 0).sidebar, SidebarMode::Overlay);
        assert_eq!(layout_for(700, 264, 0).sidebar, SidebarMode::Overlay);
        assert_eq!(layout_for(701, 264, 0).sidebar, SidebarMode::Docked);
        assert_eq!(layout_for(1200, 264, 0).sidebar, SidebarMode::Docked);
    }

    #[test]
    fn a_window_below_the_minimum_is_treated_like_the_smallest() {
        assert_eq!(layout_for(0, 264, 0).sidebar, SidebarMode::Overlay);
        assert_eq!(layout_for(599, 264, 480).dock, DockMode::Float);
    }

    #[test]
    fn the_dock_pushes_the_chat_until_the_chat_would_be_under_300() {
        // 1060 - 280 - 480 = 300: exactly enough.
        assert_eq!(layout_for(1060, 280, 480).dock, DockMode::Push);
        assert_eq!(layout_for(1060, 280, 480).chat_width, 300);
        // One point less and it floats; the chat keeps all the room beside the sidebar.
        assert_eq!(layout_for(1059, 280, 480).dock, DockMode::Float);
        assert_eq!(layout_for(1059, 280, 480).chat_width, 779);
    }

    #[test]
    fn an_overlay_sidebar_takes_no_room_from_the_dock() {
        // 650 - 0 - 350 = 300 even though the saved sidebar width is 480.
        let layout = layout_for(650, 480, 350);
        assert_eq!(layout.sidebar, SidebarMode::Overlay);
        assert_eq!(layout.dock, DockMode::Push);
        assert_eq!(layout.chat_width, 300);
    }

    #[test]
    fn a_wide_saved_sidebar_in_a_small_docked_window_floats_the_dock() {
        let layout = layout_for(701, 480, 260);
        assert_eq!(layout.sidebar, SidebarMode::Docked);
        assert_eq!(layout.dock, DockMode::Float);
        assert_eq!(layout.chat_width, 221);
    }

    #[test]
    fn no_dock_is_always_a_push_and_the_chat_gets_what_is_left() {
        let layout = layout_for(701, 480, 0);
        assert_eq!(layout.dock, DockMode::Push);
        assert_eq!(layout.chat_width, 221);
        assert_eq!(layout_for(1200, 264, 0).chat_width, 936);
    }

    #[test]
    fn a_dock_wider_than_the_window_does_not_underflow() {
        let layout = layout_for(600, 264, 4000);
        assert_eq!(layout.dock, DockMode::Float);
        assert_eq!(layout.chat_width, 600);
    }

    #[test]
    fn the_class_list_names_each_mode_that_is_on() {
        assert_eq!(workspace_class(layout_for(1500, 264, 0), false), "workspace");
        assert_eq!(workspace_class(layout_for(1500, 264, 0), true), "workspace resizing");
        // 650 pt: overlay sidebar, a 350 pt dock still pushes (chat exactly 300), the chat is narrow.
        assert_eq!(workspace_class(layout_for(650, 264, 350), false), "workspace sidebar-overlay chat-narrow");
        // 1000 pt: the 480 pt dock would leave 240 pt, so it floats and the chat keeps 720 pt.
        assert_eq!(workspace_class(layout_for(1000, 280, 480), false), "workspace dock-float");
        // 601 pt: overlay sidebar, floating dock, the chat has all 601 pt so it is not narrow.
        assert_eq!(workspace_class(layout_for(601, 264, 480), false), "workspace sidebar-overlay dock-float");
    }

    #[test]
    fn the_header_is_narrow_under_560() {
        assert_eq!(workspace_class(layout_for(823, 264, 0), false), "workspace chat-narrow"); // chat 559
        assert_eq!(workspace_class(layout_for(824, 264, 0), false), "workspace"); // chat 560
    }

    #[test]
    fn the_overlay_sidebar_stays_out_only_in_an_overlay_window_with_nothing_on_top_of_it() {
        assert!(keep_peek(true, 650, false));
        assert!(!keep_peek(true, 650, true), "Settings or the search palette opened from the sidebar puts it away");
        assert!(!keep_peek(true, 701, false), "a window grown past the overlay width forgets it");
        assert!(!keep_peek(false, 650, false));
    }
}
