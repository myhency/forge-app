//! Which screens the main area shows: one pane, or two side by side. Pure rules, no Dioxus, so the
//! rules for opening, closing and focusing are tested without a window.
use crate::workspace::Active;

/// A pane never gets narrower than this when the main area has room for two.
pub const MIN_PANE_WIDTH: u32 = 420;
pub const DEFAULT_RATIO: f32 = 0.5;

#[derive(Debug, Clone, PartialEq)]
pub struct Panes {
    /// One or two screens, left to right.
    targets: Vec<Active>,
    focused: usize,
    /// The left pane's share of the main area, when split.
    ratio: f32,
}

impl Panes {
    pub fn new(first: Active) -> Self {
        Self { targets: vec![first], focused: 0, ratio: DEFAULT_RATIO }
    }

    pub fn len(&self) -> usize {
        self.targets.len()
    }

    pub fn is_split(&self) -> bool {
        self.targets.len() == 2
    }

    pub fn focused(&self) -> usize {
        self.focused
    }

    pub fn focused_target(&self) -> Active {
        self.targets[self.focused]
    }

    pub fn target(&self, index: usize) -> Option<Active> {
        self.targets.get(index).copied()
    }

    pub fn targets(&self) -> &[Active] {
        &self.targets
    }

    pub fn pane_of(&self, target: Active) -> Option<usize> {
        self.targets.iter().position(|&shown| shown == target)
    }

    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    pub fn can_split(area: u32) -> bool {
        area >= 2 * MIN_PANE_WIDTH
    }

    /// Puts `target` in the focused pane. A target the other pane already shows takes the focus
    /// instead, so two panes never show the same screen (two New-session panes would share one
    /// unsent text).
    pub fn show(&mut self, target: Active) {
        match self.pane_of(target) {
            Some(index) => self.focused = index,
            None => self.targets[self.focused] = target,
        }
    }

    /// ⌥+click: `target` goes in the right pane, which is added when there is only one and `room`
    /// says the main area can hold two. Without room it is a plain `show`.
    pub fn open_beside(&mut self, target: Active, room: bool) {
        if let Some(index) = self.pane_of(target) {
            self.focused = index;
            return;
        }
        if self.is_split() {
            self.targets[1] = target;
            self.focused = 1;
        } else if room {
            self.targets.push(target);
            self.focused = 1;
        } else {
            self.show(target);
        }
    }

    /// A New-session pane whose first message started a session becomes that session. Focus stays
    /// where it was. False when `from` is not shown.
    pub fn replace(&mut self, from: Active, to: Active) -> bool {
        match self.pane_of(from) {
            Some(index) => {
                self.targets[index] = to;
                true
            }
            None => false,
        }
    }

    /// A session that was closed. With two panes the one that shows it closes; with one the caller
    /// picks what to show. True when a pane was closed.
    pub fn remove_target(&mut self, target: Active) -> bool {
        match self.pane_of(target) {
            Some(index) if self.is_split() => {
                self.close(index);
                true
            }
            _ => false,
        }
    }

    pub fn close(&mut self, index: usize) {
        if !self.is_split() || index >= self.targets.len() {
            return;
        }
        self.targets.remove(index);
        self.focused = 0;
        self.ratio = DEFAULT_RATIO;
    }

    /// "Move split view left": the two panes trade places and focus stays on the same content.
    pub fn swap(&mut self) {
        if self.is_split() {
            self.targets.swap(0, 1);
            self.focused = 1 - self.focused;
            self.ratio = 1.0 - self.ratio;
        }
    }

    /// True when the focus moved.
    pub fn focus(&mut self, index: usize) -> bool {
        if index < self.targets.len() && index != self.focused {
            self.focused = index;
            true
        } else {
            false
        }
    }

    pub fn set_ratio(&mut self, ratio: f32, area: u32) {
        self.ratio = clamp_ratio(ratio, area);
    }

    /// The left and right widths in px for a main area `area` wide; `(area, 0)` with one pane.
    pub fn widths(&self, area: u32) -> (u32, u32) {
        if !self.is_split() {
            return (area, 0);
        }
        let left = ((area as f32) * clamp_ratio(self.ratio, area)).round() as u32;
        let left = left.min(area);
        (left, area - left)
    }
}

// Both panes keep `MIN_PANE_WIDTH`; an area too small for that splits in half.
fn clamp_ratio(ratio: f32, area: u32) -> f32 {
    if !Panes::can_split(area) {
        return DEFAULT_RATIO;
    }
    let low = MIN_PANE_WIDTH as f32 / area as f32;
    ratio.clamp(low, 1.0 - low)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Active;
    use forge_application::LiveSessionId;

    fn live(n: u64) -> Active {
        Active::Live(LiveSessionId(n))
    }

    #[test]
    fn one_pane_shows_its_target_and_is_not_split() {
        let panes = Panes::new(Active::NewSession);
        assert_eq!(panes.len(), 1);
        assert!(!panes.is_split());
        assert_eq!(panes.focused_target(), Active::NewSession);
        assert_eq!(panes.widths(1000), (1000, 0));
    }

    #[test]
    fn show_replaces_the_focused_pane_and_never_makes_a_third() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        assert_eq!((panes.len(), panes.focused()), (2, 1));
        panes.show(live(3));
        assert_eq!(panes.targets(), &[live(1), live(3)]);
        panes.focus(0);
        panes.show(live(4));
        assert_eq!(panes.targets(), &[live(4), live(3)]);
    }

    #[test]
    fn showing_a_target_already_in_the_other_pane_only_moves_focus() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.show(live(1));
        assert_eq!(panes.targets(), &[live(1), live(2)]);
        assert_eq!(panes.focused(), 0);
    }

    #[test]
    fn open_beside_adds_a_second_pane_on_the_right_and_focuses_it() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        assert_eq!(panes.targets(), &[live(1), live(2)]);
        assert_eq!(panes.focused(), 1);
    }

    #[test]
    fn open_beside_while_split_replaces_the_right_pane_even_when_the_left_is_focused() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.focus(0);
        panes.open_beside(live(3), true);
        assert_eq!(panes.targets(), &[live(1), live(3)]);
        assert_eq!(panes.focused(), 1);
    }

    #[test]
    fn open_beside_a_target_that_is_shown_only_focuses_it() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.open_beside(live(1), true);
        assert_eq!(panes.targets(), &[live(1), live(2)]);
        assert_eq!(panes.focused(), 0);
        // One pane, and the target is the one shown: still one pane.
        let mut single = Panes::new(live(1));
        single.open_beside(live(1), true);
        assert_eq!(single.len(), 1);
    }

    #[test]
    fn open_beside_without_room_does_a_plain_show() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), false);
        assert_eq!(panes.targets(), &[live(2)]);
        assert!(!panes.is_split());
    }

    #[test]
    fn closing_a_pane_gives_its_space_to_the_other_and_resets_the_ratio() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.set_ratio(0.7, 2000);
        panes.close(0);
        assert_eq!(panes.targets(), &[live(2)]);
        assert_eq!(panes.focused(), 0);
        assert_eq!(panes.ratio(), DEFAULT_RATIO);
        // Closing the only pane does nothing.
        panes.close(0);
        assert_eq!(panes.len(), 1);
    }

    #[test]
    fn swap_exchanges_the_panes_and_focus_follows_the_content() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.set_ratio(0.4, 2000);
        panes.swap();
        assert_eq!(panes.targets(), &[live(2), live(1)]);
        assert_eq!(panes.focused(), 0);
        assert!((panes.ratio() - 0.6).abs() < 1e-6);
        // Not split: nothing happens.
        let mut single = Panes::new(live(1));
        single.swap();
        assert_eq!(single.targets(), &[live(1)]);
    }

    #[test]
    fn the_ratio_keeps_both_panes_at_least_the_minimum_width() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.set_ratio(0.01, 1000);
        assert_eq!(panes.widths(1000), (MIN_PANE_WIDTH, 1000 - MIN_PANE_WIDTH));
        panes.set_ratio(0.99, 1000);
        assert_eq!(panes.widths(1000), (1000 - MIN_PANE_WIDTH, MIN_PANE_WIDTH));
        panes.set_ratio(0.5, 1000);
        assert_eq!(panes.widths(1000), (500, 500));
    }

    #[test]
    fn a_tiny_area_splits_in_half_and_never_underflows() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        assert_eq!(panes.widths(600), (300, 300));
        assert_eq!(panes.widths(0), (0, 0));
        panes.set_ratio(0.9, 600);
        assert_eq!(panes.widths(600), (300, 300));
    }

    #[test]
    fn can_split_needs_room_for_two_minimum_panes() {
        assert!(!Panes::can_split(2 * MIN_PANE_WIDTH - 1));
        assert!(Panes::can_split(2 * MIN_PANE_WIDTH));
    }

    #[test]
    fn replace_turns_the_new_session_pane_into_the_started_session_wherever_focus_is() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(Active::NewSession, true);
        panes.focus(0);
        assert!(panes.replace(Active::NewSession, live(9)));
        assert_eq!(panes.targets(), &[live(1), live(9)]);
        assert_eq!(panes.focused(), 0, "focus stays where it was");
        assert!(!panes.replace(Active::NewSession, live(10)), "no New session pane is left");
    }

    #[test]
    fn remove_target_closes_the_pane_that_shows_it_when_split_and_does_nothing_otherwise() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.focus(1);
        assert!(panes.remove_target(live(1)));
        assert_eq!(panes.targets(), &[live(2)]);
        assert_eq!(panes.focused(), 0);
        // Single pane: the caller decides what to show instead.
        assert!(!panes.remove_target(live(2)));
        assert_eq!(panes.targets(), &[live(2)]);
        // A target that is not shown.
        assert!(!panes.remove_target(live(7)));
    }

    #[test]
    fn focus_reports_whether_it_changed_and_ignores_an_index_that_is_not_there() {
        let mut panes = Panes::new(live(1));
        assert!(!panes.focus(1));
        panes.open_beside(live(2), true);
        assert!(!panes.focus(1));
        assert!(panes.focus(0));
    }
}
