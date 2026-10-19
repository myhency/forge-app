//! Which screens the main area shows: up to four panes in a tree of splits (side by side and
//! stacked). Pure rules and geometry, no Dioxus, so opening, closing, moving and sizing are tested
//! without a window.
use crate::workspace::Active;

pub const MAX_PANES: usize = 4;
/// A pane never gets narrower than this when the area has room for the layout.
pub const MIN_PANE_WIDTH: u32 = 420;
/// A pane never gets lower than this: the compact header and a composer.
pub const MIN_PANE_HEIGHT: u32 = 260;
pub const DEFAULT_RATIO: f32 = 0.5;

/// How a split puts its two children: `Beside` first left and second right, `Below` first on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Beside,
    Below,
}

impl Dir {
    pub fn other(self) -> Dir {
        match self {
            Dir::Beside => Dir::Below,
            Dir::Below => Dir::Beside,
        }
    }
}

/// A rectangle in px, relative to the main area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// The line between the two children of a split. `path` leads from the root to the split (false =
/// first child, true = second). `dir` is the split's direction: a `Beside` split has a vertical line
/// at `x` that is `length` tall, a `Below` split a horizontal line at `y` that is `length` wide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divider {
    pub path: Vec<bool>,
    pub dir: Dir,
    pub x: u32,
    pub y: u32,
    pub length: u32,
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Leaf(Active),
    Split { dir: Dir, ratio: f32, first: Box<Node>, second: Box<Node> },
}

impl Node {
    fn leaves(&self, out: &mut Vec<Active>) {
        match self {
            Node::Leaf(target) => out.push(*target),
            Node::Split { first, second, .. } => {
                first.leaves(out);
                second.leaves(out);
            }
        }
    }

    fn min_width(&self) -> u32 {
        match self {
            Node::Leaf(_) => MIN_PANE_WIDTH,
            Node::Split { dir: Dir::Beside, first, second, .. } => first.min_width() + second.min_width(),
            Node::Split { first, second, .. } => first.min_width().max(second.min_width()),
        }
    }

    fn min_height(&self) -> u32 {
        match self {
            Node::Leaf(_) => MIN_PANE_HEIGHT,
            Node::Split { dir: Dir::Below, first, second, .. } => first.min_height() + second.min_height(),
            Node::Split { first, second, .. } => first.min_height().max(second.min_height()),
        }
    }

    // Puts a new leaf after the leaf showing `target`, in a split of direction `dir`.
    fn split_leaf(self, target: Active, dir: Dir, new: Active) -> Node {
        match self {
            Node::Leaf(shown) if shown == target => Node::Split {
                dir,
                ratio: DEFAULT_RATIO,
                first: Box::new(Node::Leaf(shown)),
                second: Box::new(Node::Leaf(new)),
            },
            Node::Leaf(_) => self,
            Node::Split { dir: parent, ratio, first, second } => Node::Split {
                dir: parent,
                ratio,
                first: Box::new(first.split_leaf(target, dir, new)),
                second: Box::new(second.split_leaf(target, dir, new)),
            },
        }
    }

    fn lay_out(&self, rect: Rect, path: &mut Vec<bool>, leaves: &mut Vec<Rect>, dividers: &mut Vec<Divider>) {
        match self {
            Node::Leaf(_) => leaves.push(rect),
            Node::Split { dir, ratio, first, second } => {
                let (a, b) = split_rects(rect, *dir, *ratio, first, second);
                dividers.push(match dir {
                    Dir::Beside => Divider { path: path.clone(), dir: *dir, x: b.x, y: rect.y, length: rect.h },
                    Dir::Below => Divider { path: path.clone(), dir: *dir, x: rect.x, y: b.y, length: rect.w },
                });
                path.push(false);
                first.lay_out(a, path, leaves, dividers);
                path.pop();
                path.push(true);
                second.lay_out(b, path, leaves, dividers);
                path.pop();
            }
        }
    }
}

// The extent of the first child: the ratio, kept so that both sides keep their minimum. An area too
// small for the two minimums splits in half.
fn cut_at(ratio: f32, total: u32, first_min: u32, second_min: u32) -> u32 {
    if total < first_min + second_min {
        return total / 2;
    }
    let cut = (total as f32 * ratio).round() as u32;
    cut.clamp(first_min, total - second_min)
}

fn split_rects(rect: Rect, dir: Dir, ratio: f32, first: &Node, second: &Node) -> (Rect, Rect) {
    match dir {
        Dir::Beside => {
            let cut = cut_at(ratio, rect.w, first.min_width(), second.min_width());
            (Rect { w: cut, ..rect }, Rect { x: rect.x + cut, w: rect.w - cut, ..rect })
        }
        Dir::Below => {
            let cut = cut_at(ratio, rect.h, first.min_height(), second.min_height());
            (Rect { h: cut, ..rect }, Rect { y: rect.y + cut, h: rect.h - cut, ..rect })
        }
    }
}

/// The screens of the main area. Screens are unique, so the focus is kept as the focused screen.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneTree {
    root: Node,
    focus: Active,
    /// The screens in the order their panes were opened, oldest first; the last one is replaced when
    /// a pane is added at the limit.
    opened: Vec<Active>,
}

impl PaneTree {
    pub fn new(first: Active) -> Self {
        Self { root: Node::Leaf(first), focus: first, opened: vec![first] }
    }

    /// The screens in reading order: left to right, top to bottom within a column of the tree.
    pub fn targets(&self) -> Vec<Active> {
        let mut out = Vec::new();
        self.root.leaves(&mut out);
        out
    }

    pub fn len(&self) -> usize {
        self.targets().len()
    }

    pub fn is_split(&self) -> bool {
        self.len() > 1
    }

    pub fn focused_target(&self) -> Active {
        self.focus
    }

    pub fn focused(&self) -> usize {
        self.pane_of(self.focus).unwrap_or(0)
    }

    pub fn target(&self, index: usize) -> Option<Active> {
        self.targets().get(index).copied()
    }

    pub fn pane_of(&self, target: Active) -> Option<usize> {
        self.targets().iter().position(|&shown| shown == target)
    }

    /// One rectangle per pane, in reading order, for a main area `w` by `h`.
    pub fn rects(&self, w: u32, h: u32) -> Vec<Rect> {
        self.layout(w, h).0
    }

    pub fn dividers(&self, w: u32, h: u32) -> Vec<Divider> {
        self.layout(w, h).1
    }

    fn layout(&self, w: u32, h: u32) -> (Vec<Rect>, Vec<Divider>) {
        let (mut leaves, mut dividers) = (Vec::new(), Vec::new());
        self.root.lay_out(Rect { x: 0, y: 0, w, h }, &mut Vec::new(), &mut leaves, &mut dividers);
        (leaves, dividers)
    }

    /// Whether the area holds the layout with every pane at its minimum size.
    pub fn fits(&self, w: u32, h: u32) -> bool {
        self.root.min_width() <= w && self.root.min_height() <= h
    }

    /// The rectangle the split at `path` cuts (what a divider drag measures its pointer in); none
    /// when `path` does not lead to a split.
    pub fn split_rect(&self, path: &[bool], w: u32, h: u32) -> Option<Rect> {
        let mut rect = Rect { x: 0, y: 0, w, h };
        let mut node = &self.root;
        for &step in path {
            let Node::Split { dir, ratio, first, second } = node else { return None };
            let (a, b) = split_rects(rect, *dir, *ratio, first, second);
            (rect, node) = if step { (b, &**second) } else { (a, &**first) };
        }
        matches!(node, Node::Split { .. }).then_some(rect)
    }

    /// Moves the divider of the split at `path`: `ratio` is the first child's share of the split's
    /// rectangle, kept so that both sides keep their minimum.
    pub fn set_ratio(&mut self, path: &[bool], ratio: f32, w: u32, h: u32) {
        let mut rect = Rect { x: 0, y: 0, w, h };
        let mut node = &mut self.root;
        for &step in path {
            let Node::Split { dir, ratio, first, second } = node else { return };
            let (a, b) = split_rects(rect, *dir, *ratio, first, second);
            if step {
                rect = b;
                node = &mut **second;
            } else {
                rect = a;
                node = &mut **first;
            }
        }
        let Node::Split { dir, ratio: stored, first, second } = node else { return };
        let (total, first_min, second_min) = match dir {
            Dir::Beside => (rect.w, first.min_width(), second.min_width()),
            Dir::Below => (rect.h, first.min_height(), second.min_height()),
        };
        if total < first_min + second_min {
            return;
        }
        // Clamped in whole px: clamping the two ratios themselves can invert by a rounding error.
        let cut = ((total as f32 * ratio).round() as u32).clamp(first_min, total - second_min);
        *stored = cut as f32 / total as f32;
    }

    /// `⌃⌘\` and `⌃⌘-`: `target` opens in a new pane beside or below the focused one and takes the
    /// focus. A screen that is already shown only takes the focus. Nothing happens at the limit or
    /// without room. True when the focus or the layout changed.
    pub fn split_focused(&mut self, dir: Dir, target: Active, w: u32, h: u32) -> bool {
        if self.pane_of(target).is_some() {
            let moved = self.focus != target;
            self.focus = target;
            return moved;
        }
        if self.len() >= MAX_PANES {
            return false;
        }
        let candidate = self.root.clone().split_leaf(self.focus, dir, target);
        if candidate.min_width() > w || candidate.min_height() > h {
            return false;
        }
        self.root = candidate;
        self.focus = target;
        self.opened.push(target);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Active;
    use forge_application::LiveSessionId;

    const W: u32 = 1200;
    const H: u32 = 800;

    fn live(n: u64) -> Active {
        Active::Live(LiveSessionId(n))
    }

    fn rect(x: u32, y: u32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    // 1 | 2
    fn pair() -> PaneTree {
        let mut tree = PaneTree::new(live(1));
        assert!(tree.split_focused(Dir::Beside, live(2), W, H));
        tree
    }

    // 1 | (2 over 3)
    fn three() -> PaneTree {
        let mut tree = pair();
        assert!(tree.split_focused(Dir::Below, live(3), W, H));
        tree
    }

    #[test]
    fn one_pane_fills_the_area_and_has_no_divider() {
        let tree = PaneTree::new(Active::NewSession);
        assert_eq!(tree.len(), 1);
        assert!(!tree.is_split());
        assert_eq!(tree.focused_target(), Active::NewSession);
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, W, H)]);
        assert!(tree.dividers(W, H).is_empty());
        assert!(tree.fits(MIN_PANE_WIDTH, MIN_PANE_HEIGHT));
        assert!(!tree.fits(MIN_PANE_WIDTH - 1, MIN_PANE_HEIGHT));
    }

    #[test]
    fn two_side_by_side_share_the_width_and_have_one_vertical_divider() {
        let tree = pair();
        assert_eq!(tree.targets(), vec![live(1), live(2)]);
        assert_eq!(tree.focused(), 1, "the new pane takes the focus");
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 600, 800), rect(600, 0, 600, 800)]);
        assert_eq!(tree.dividers(W, H), vec![Divider { path: vec![], dir: Dir::Beside, x: 600, y: 0, length: 800 }]);
    }

    #[test]
    fn two_stacked_share_the_height_and_have_one_horizontal_divider() {
        let mut tree = PaneTree::new(live(1));
        assert!(tree.split_focused(Dir::Below, live(2), W, H));
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 1200, 400), rect(0, 400, 1200, 400)]);
        assert_eq!(tree.dividers(W, H), vec![Divider { path: vec![], dir: Dir::Below, x: 0, y: 400, length: 1200 }]);
    }

    #[test]
    fn a_nested_split_lays_out_inside_its_parent_and_lists_its_own_divider() {
        let tree = three();
        assert_eq!(tree.targets(), vec![live(1), live(2), live(3)]);
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 600, 800), rect(600, 0, 600, 400), rect(600, 400, 600, 400)]);
        assert_eq!(
            tree.dividers(W, H),
            vec![
                Divider { path: vec![], dir: Dir::Beside, x: 600, y: 0, length: 800 },
                Divider { path: vec![true], dir: Dir::Below, x: 600, y: 400, length: 600 },
            ]
        );
    }

    #[test]
    fn the_ratio_keeps_both_panes_at_least_the_minimum_width() {
        let mut tree = pair();
        tree.set_ratio(&[], 0.01, 1000, H);
        assert_eq!(tree.rects(1000, H), vec![rect(0, 0, 420, 800), rect(420, 0, 580, 800)]);
        tree.set_ratio(&[], 0.99, 1000, H);
        assert_eq!(tree.rects(1000, H), vec![rect(0, 0, 580, 800), rect(580, 0, 420, 800)]);
        tree.set_ratio(&[], 0.5, 1000, H);
        assert_eq!(tree.rects(1000, H), vec![rect(0, 0, 500, 800), rect(500, 0, 500, 800)]);
    }

    #[test]
    fn a_nested_divider_keeps_the_minimum_height_of_both_sides() {
        let mut tree = three();
        // The split at path [true] covers (600, 0, 600, 800): the top may grow to 800 - 260.
        tree.set_ratio(&[true], 0.99, W, H);
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 600, 800), rect(600, 0, 600, 540), rect(600, 540, 600, 260)]);
        tree.set_ratio(&[true], 0.0, W, H);
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 600, 800), rect(600, 0, 600, 260), rect(600, 260, 600, 540)]);
    }

    #[test]
    fn a_side_that_holds_two_panes_keeps_the_room_for_both() {
        // (1 | 3) | 2 at 1260 wide: the left side needs 2 x 420, so the divider cannot go under 840.
        let mut tree = PaneTree::new(live(1));
        assert!(tree.split_focused(Dir::Beside, live(2), 1260, H));
        tree.focus = live(1);
        assert!(tree.split_focused(Dir::Beside, live(3), 1260, H));
        assert_eq!(tree.targets(), vec![live(1), live(3), live(2)]);
        tree.set_ratio(&[], 0.1, 1260, H);
        let rects = tree.rects(1260, H);
        assert_eq!(rects[2], rect(840, 0, 420, 800));
    }

    #[test]
    fn an_area_too_small_for_the_minimums_splits_in_half_and_never_underflows() {
        let tree = pair();
        assert_eq!(tree.rects(600, H), vec![rect(0, 0, 300, 800), rect(300, 0, 300, 800)]);
        let empty = tree.rects(0, 0);
        assert_eq!(empty, vec![rect(0, 0, 0, 0), rect(0, 0, 0, 0)]);
    }

    #[test]
    fn fits_counts_the_minimum_sizes_of_the_whole_layout() {
        let side_by_side = pair();
        assert!(side_by_side.fits(2 * MIN_PANE_WIDTH, MIN_PANE_HEIGHT));
        assert!(!side_by_side.fits(2 * MIN_PANE_WIDTH - 1, H));
        let mut stacked = PaneTree::new(live(1));
        assert!(stacked.split_focused(Dir::Below, live(2), W, H));
        assert!(stacked.fits(W, 2 * MIN_PANE_HEIGHT));
        assert!(!stacked.fits(W, 2 * MIN_PANE_HEIGHT - 1));
        let nested = three();
        assert!(nested.fits(2 * MIN_PANE_WIDTH, 2 * MIN_PANE_HEIGHT));
        assert!(!nested.fits(2 * MIN_PANE_WIDTH, 2 * MIN_PANE_HEIGHT - 1));
    }

    #[test]
    fn split_rect_is_the_rectangle_a_divider_cuts() {
        let tree = three();
        assert_eq!(tree.split_rect(&[], W, H), Some(rect(0, 0, 1200, 800)));
        assert_eq!(tree.split_rect(&[true], W, H), Some(rect(600, 0, 600, 800)));
        assert_eq!(tree.split_rect(&[false], W, H), None, "a leaf is not a split");
        assert_eq!(tree.split_rect(&[true, true], W, H), None);
    }

    #[test]
    fn split_focused_splits_the_focused_pane_and_focuses_the_new_one() {
        let mut tree = pair();
        tree.focus = live(1);
        assert!(tree.split_focused(Dir::Below, Active::NewSession, W, H));
        assert_eq!(tree.targets(), vec![live(1), Active::NewSession, live(2)]);
        assert_eq!(tree.focused_target(), Active::NewSession);
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 600, 400), rect(0, 400, 600, 400), rect(600, 0, 600, 800)]);
    }

    #[test]
    fn split_focused_with_a_shown_target_only_moves_focus() {
        let mut tree = pair();
        assert!(tree.split_focused(Dir::Beside, live(1), W, H));
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.focused_target(), live(1));
        // The focus already there: nothing changed.
        assert!(!tree.split_focused(Dir::Beside, live(1), W, H));
    }

    #[test]
    fn split_focused_refuses_a_fifth_pane_and_a_split_without_room() {
        let mut tree = PaneTree::new(live(1));
        for n in 2..=4 {
            assert!(tree.split_focused(Dir::Beside, live(n), 2000, H));
        }
        assert_eq!(tree.len(), MAX_PANES);
        assert!(!tree.split_focused(Dir::Beside, live(5), 2000, H));
        assert_eq!(tree.len(), MAX_PANES);
        // One pane in an area too narrow, or too short, for two.
        let mut narrow = PaneTree::new(live(1));
        assert!(!narrow.split_focused(Dir::Beside, live(2), 2 * MIN_PANE_WIDTH - 1, H));
        assert!(!narrow.split_focused(Dir::Below, live(2), W, 2 * MIN_PANE_HEIGHT - 1));
        assert_eq!(narrow.targets(), vec![live(1)]);
    }
}
