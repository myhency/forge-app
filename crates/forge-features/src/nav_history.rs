/// The screens the person has been to, for the back and forward buttons. It is generic so it can be
/// tested without the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavHistory<T> {
    stops: Vec<T>,
    /// Index of the screen being shown in `stops`.
    at: usize,
}

/// Screens older than this many steps back are forgotten.
const MAX_STOPS: usize = 50;

impl<T: Copy + PartialEq> NavHistory<T> {
    pub fn new(first: T) -> Self {
        Self { stops: vec![first], at: 0 }
    }

    /// The person went to `to`. Screens ahead of the current one are dropped (a new branch), and going
    /// to the screen already shown records nothing.
    pub fn visit(&mut self, to: T) {
        if self.stops[self.at] == to {
            return;
        }
        self.stops.truncate(self.at + 1);
        self.stops.push(to);
        if self.stops.len() > MAX_STOPS {
            self.stops.remove(0);
        }
        self.at = self.stops.len() - 1;
    }

    /// Steps back to the nearest earlier screen `usable` accepts and returns it. Screens it rejects
    /// (a session closed since) are skipped; with none left nothing moves.
    pub fn back(&mut self, usable: impl Fn(T) -> bool) -> Option<T> {
        let target = (0..self.at).rev().find(|&index| usable(self.stops[index]))?;
        self.at = target;
        Some(self.stops[target])
    }

    /// The same, forward.
    pub fn forward(&mut self, usable: impl Fn(T) -> bool) -> Option<T> {
        let target = (self.at + 1..self.stops.len()).find(|&index| usable(self.stops[index]))?;
        self.at = target;
        Some(self.stops[target])
    }

    pub fn can_back(&self, usable: impl Fn(T) -> bool) -> bool {
        self.stops[..self.at].iter().any(|&stop| usable(stop))
    }

    pub fn can_forward(&self, usable: impl Fn(T) -> bool) -> bool {
        self.stops[self.at + 1..].iter().any(|&stop| usable(stop))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any(_: i32) -> bool {
        true
    }

    #[test]
    fn a_new_history_has_nowhere_to_go() {
        let history = NavHistory::new(0);
        assert!(!history.can_back(any));
        assert!(!history.can_forward(any));
    }

    #[test]
    fn back_and_forward_walk_the_visited_screens() {
        let mut history = NavHistory::new(0);
        history.visit(1);
        history.visit(2);
        assert_eq!(history.back(any), Some(1));
        assert_eq!(history.back(any), Some(0));
        assert_eq!(history.back(any), None);
        assert_eq!(history.forward(any), Some(1));
        assert_eq!(history.forward(any), Some(2));
        assert_eq!(history.forward(any), None);
    }

    #[test]
    fn a_visit_after_going_back_drops_what_was_ahead() {
        let mut history = NavHistory::new(0);
        history.visit(1);
        history.visit(2);
        history.back(any);
        history.visit(3);
        assert!(!history.can_forward(any));
        assert_eq!(history.back(any), Some(1));
    }

    #[test]
    fn visiting_the_screen_already_shown_records_nothing() {
        let mut history = NavHistory::new(0);
        history.visit(0);
        history.visit(1);
        history.visit(1);
        assert_eq!(history.back(any), Some(0));
        assert_eq!(history.back(any), None);
    }

    #[test]
    fn screens_that_are_gone_are_skipped_and_do_not_light_the_button() {
        let mut history = NavHistory::new(0);
        history.visit(1);
        history.visit(2);
        let gone_one = |stop: i32| stop != 1;
        assert!(history.can_back(gone_one));
        assert_eq!(history.back(gone_one), Some(0));
        assert!(!history.can_back(gone_one));
        assert_eq!(history.forward(gone_one), Some(2));
    }

    #[test]
    fn the_oldest_screens_are_forgotten_past_the_limit() {
        let mut history = NavHistory::new(0);
        for n in 1..=60 {
            history.visit(n);
        }
        let mut steps = 0;
        while history.back(any).is_some() {
            steps += 1;
        }
        assert_eq!(steps, MAX_STOPS - 1);
    }
}
