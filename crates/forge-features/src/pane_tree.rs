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

// `part` of `whole` as a CSS percentage; a zero area reads as 0.
fn percent(part: u32, whole: u32) -> String {
    if whole == 0 {
        return "0%".to_string();
    }
    format!("{}%", part as f64 * 100.0 / whole as f64)
}

impl Rect {
    /// The CSS that positions an absolutely placed element at this rectangle of an area `w` by `h`.
    /// Percentages, so the pane follows the real size of its box even if the page's measure of the
    /// area is a frame behind.
    pub fn percent_style(&self, w: u32, h: u32) -> String {
        format!(
            "left: {}; top: {}; width: {}; height: {};",
            percent(self.x, w),
            percent(self.y, h),
            percent(self.w, w),
            percent(self.h, h)
        )
    }
}

impl Divider {
    pub fn percent_style(&self, w: u32, h: u32) -> String {
        match self.dir {
            Dir::Beside => format!("left: {}; top: {}; height: {};", percent(self.x, w), percent(self.y, h), percent(self.length, h)),
            Dir::Below => format!("left: {}; top: {}; width: {};", percent(self.x, w), percent(self.y, h), percent(self.length, w)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Leaf(Active),
    Split { dir: Dir, ratio: f32, first: Box<Node>, second: Box<Node> },
}

impl Node {
    // The tree without the leaf that shows `target`; its sibling takes the parent's place. None when
    // the whole tree was that leaf.
    fn without(self, target: Active) -> Option<Node> {
        match self {
            Node::Leaf(shown) => (shown != target).then_some(Node::Leaf(shown)),
            Node::Split { dir, ratio, first, second } => match (first.without(target), second.without(target)) {
                (Some(a), Some(b)) => Some(Node::Split { dir, ratio, first: Box::new(a), second: Box::new(b) }),
                (Some(only), None) | (None, Some(only)) => Some(only),
                (None, None) => None,
            },
        }
    }

    fn rename(&mut self, from: Active, to: Active) {
        match self {
            Node::Leaf(shown) => {
                if *shown == from {
                    *shown = to;
                }
            }
            Node::Split { first, second, .. } => {
                first.rename(from, to);
                second.rename(from, to);
            }
        }
    }

    // The steps from this node to the leaf that shows `target` (false = first child).
    fn path_to(&self, target: Active) -> Option<Vec<bool>> {
        match self {
            Node::Leaf(shown) => (*shown == target).then(Vec::new),
            Node::Split { first, second, .. } => {
                if let Some(mut path) = first.path_to(target) {
                    path.insert(0, false);
                    return Some(path);
                }
                let mut path = second.path_to(target)?;
                path.insert(0, true);
                Some(path)
            }
        }
    }

    fn node_at(&self, path: &[bool]) -> Option<&Node> {
        let mut node = self;
        for &step in path {
            let Node::Split { first, second, .. } = node else { return None };
            node = if step { &**second } else { &**first };
        }
        Some(node)
    }

    fn node_at_mut(&mut self, path: &[bool]) -> Option<&mut Node> {
        let mut node = self;
        for &step in path {
            let Node::Split { first, second, .. } = node else { return None };
            node = if step { &mut **second } else { &mut **first };
        }
        Some(node)
    }

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
    // The same screen under a new name, in the tree and in the opened order, keeping its place.
    fn rename(&mut self, from: Active, to: Active) {
        self.root.rename(from, to);
        for opened in &mut self.opened {
            if *opened == from {
                *opened = to;
            }
        }
        if self.focus == from {
            self.focus = to;
        }
    }

    /// Puts `target` in the focused pane. A screen another pane already shows takes the focus
    /// instead, so two panes never show the same screen (two New-session panes would share one
    /// unsent text).
    pub fn show(&mut self, target: Active) {
        if self.pane_of(target).is_some() {
            self.focus = target;
        } else {
            self.rename(self.focus, target);
        }
    }

    /// ⌥+click, and a plain click while the area is split: `target` gets a new pane, which takes the
    /// focus. The new pane splits the widest pane (the first on a tie) along its longer side; when
    /// that would leave a pane under its minimum size, the next widest is tried. At the limit, or
    /// when nothing has room, the pane that was opened last shows `target` instead. A screen that is
    /// already shown only takes the focus.
    pub fn add(&mut self, target: Active, w: u32, h: u32) {
        if self.pane_of(target).is_some() {
            self.focus = target;
            return;
        }
        if self.len() < MAX_PANES {
            let (shown, rects) = (self.targets(), self.rects(w, h));
            let mut order: Vec<usize> = (0..shown.len()).collect();
            // A stable sort: panes of equal area stay in reading order.
            order.sort_by_key(|&i| std::cmp::Reverse(u64::from(rects[i].w) * u64::from(rects[i].h)));
            for i in order {
                let dir = if rects[i].w >= rects[i].h { Dir::Beside } else { Dir::Below };
                let candidate = self.root.clone().split_leaf(shown[i], dir, target);
                if candidate.min_width() <= w && candidate.min_height() <= h {
                    self.root = candidate;
                    self.focus = target;
                    self.opened.push(target);
                    return;
                }
            }
        }
        let last = *self.opened.last().expect("a tree always has a pane");
        self.rename(last, target);
        self.focus = target;
    }

    /// A New-session pane whose first message started a session becomes that session. The focus
    /// stays on the same pane. False when `from` is not shown.
    pub fn replace(&mut self, from: Active, to: Active) -> bool {
        if self.pane_of(from).is_none() {
            return false;
        }
        self.rename(from, to);
        true
    }

    /// A session that was closed. When split, the pane that shows it closes. True when a pane was
    /// closed; with one pane the caller picks what to show.
    pub fn remove_target(&mut self, target: Active) -> bool {
        match self.pane_of(target) {
            Some(index) if self.is_split() => {
                self.close(index);
                true
            }
            _ => false,
        }
    }

    /// Closes pane `index`; its sibling takes the parent's place. A closed pane that had the focus
    /// hands it to the pane now at its place in the reading order.
    pub fn close(&mut self, index: usize) {
        let Some(target) = self.target(index) else { return };
        if !self.is_split() {
            return;
        }
        let had_focus = self.focus == target;
        let root = std::mem::replace(&mut self.root, Node::Leaf(target)).without(target);
        self.root = root.expect("a split keeps at least one pane");
        self.opened.retain(|&opened| opened != target);
        if had_focus {
            let remaining = self.targets();
            self.focus = remaining[index.min(remaining.len() - 1)];
        }
    }

    /// True when the focus moved.
    pub fn focus(&mut self, index: usize) -> bool {
        match self.target(index) {
            Some(target) if target != self.focus => {
                self.focus = target;
                true
            }
            _ => false,
        }
    }

    /// The `⋮` menu's move items. `dir` is the direction of the pane's parent split: moving along it
    /// ("move left", "move up") swaps a second child with its sibling; moving across it ("move
    /// down", "move to the right") turns the split. Focus stays on the same content. A move that
    /// would leave a layout that fits in an area `w` by `h` without room is refused (false).
    pub fn move_pane(&mut self, index: usize, dir: Dir, w: u32, h: u32) -> bool {
        if !self.can_move(index, dir, w, h) {
            return false;
        }
        self.apply_move(index, dir);
        true
    }

    /// Whether `move_pane` would do the move: not when the layout fits now and would not after it. A
    /// layout that is already too big for the area may be turned (that is a way back to a fit).
    pub fn can_move(&self, index: usize, dir: Dir, w: u32, h: u32) -> bool {
        let mut moved = self.clone();
        moved.apply_move(index, dir);
        !self.fits(w, h) || moved.fits(w, h)
    }

    fn apply_move(&mut self, index: usize, dir: Dir) {
        let Some(target) = self.target(index) else { return };
        let Some(path) = self.root.path_to(target) else { return };
        let Some((&second, parent_path)) = path.split_last() else { return };
        let Some(Node::Split { dir: parent_dir, ratio, first, second: other, .. }) = self.root.node_at_mut(parent_path) else { return };
        if *parent_dir == dir {
            if second {
                std::mem::swap(first, other);
                *ratio = 1.0 - *ratio;
            }
        } else {
            *parent_dir = dir;
        }
    }

    /// The direction of the split the pane sits in and whether it is that split's second child;
    /// none for the only pane. The `⋮` menu's labels follow it.
    pub fn pane_slot(&self, index: usize) -> Option<(Dir, bool)> {
        let path = self.root.path_to(self.target(index)?)?;
        let (&second, parent_path) = path.split_last()?;
        match self.root.node_at(parent_path)? {
            Node::Split { dir, .. } => Some((*dir, second)),
            Node::Leaf(_) => None,
        }
    }

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

    // 1 over 3 on the left, 2 over 4 on the right: the reading order is [1, 3, 2, 4].
    fn grid() -> PaneTree {
        let mut tree = PaneTree::new(live(1));
        for n in 2..=4 {
            tree.add(live(n), W, H);
        }
        tree
    }

    #[test]
    fn add_builds_a_grid_in_the_order_the_desktop_app_does() {
        let mut tree = PaneTree::new(live(1));
        tree.add(live(2), W, H);
        assert_eq!(tree.targets(), vec![live(1), live(2)]);
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 600, 800), rect(600, 0, 600, 800)]);
        // The third pane goes under the left one (the widest pane, the first on a tie, split along its longer side).
        tree.add(live(3), W, H);
        assert_eq!(tree.targets(), vec![live(1), live(3), live(2)]);
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 600, 400), rect(0, 400, 600, 400), rect(600, 0, 600, 800)]);
        // The fourth under the right one: a 2 x 2 grid.
        tree.add(live(4), W, H);
        assert_eq!(tree.targets(), vec![live(1), live(3), live(2), live(4)]);
        assert_eq!(
            tree.rects(W, H),
            vec![rect(0, 0, 600, 400), rect(0, 400, 600, 400), rect(600, 0, 600, 400), rect(600, 400, 600, 400)]
        );
        assert_eq!(tree.focused_target(), live(4));
    }

    #[test]
    fn add_to_a_stacked_pair_splits_the_first_pane_beside() {
        let mut tree = PaneTree::new(live(1));
        assert!(tree.split_focused(Dir::Below, live(2), W, H));
        tree.add(live(3), W, H);
        assert_eq!(tree.targets(), vec![live(1), live(3), live(2)]);
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 600, 400), rect(600, 0, 600, 400), rect(0, 400, 1200, 400)]);
    }

    #[test]
    fn add_at_the_limit_replaces_the_pane_opened_last_and_focuses_it() {
        let mut tree = grid();
        tree.focus = live(1);
        tree.add(live(5), W, H);
        assert_eq!(tree.targets(), vec![live(1), live(3), live(2), live(5)], "the bottom right pane, opened last, changed");
        assert_eq!(tree.focused_target(), live(5));
        // It stays the last one: the next add replaces it again.
        tree.add(live(6), W, H);
        assert_eq!(tree.targets(), vec![live(1), live(3), live(2), live(6)]);
    }

    #[test]
    fn add_a_shown_screen_only_focuses_it() {
        let mut tree = grid();
        tree.add(live(2), W, H);
        assert_eq!(tree.len(), 4);
        assert_eq!(tree.focused_target(), live(2));
    }

    #[test]
    fn add_without_room_replaces_the_pane_opened_last_instead_of_adding() {
        let mut tree = PaneTree::new(live(1));
        tree.add(live(2), 2 * MIN_PANE_WIDTH - 1, H);
        assert_eq!(tree.targets(), vec![live(2)]);
        assert_eq!(tree.focused_target(), live(2));
    }

    #[test]
    fn add_replaces_when_no_pane_can_be_split_within_the_minimums() {
        // 1 | 2 in an area 840 wide and 519 high: each pane is 420 x 519, so a split would be
        // below, and 2 x 260 high does not fit. The pane opened last shows the screen instead.
        let mut tree = pair();
        tree.add(live(3), 2 * MIN_PANE_WIDTH, 2 * MIN_PANE_HEIGHT - 1);
        assert_eq!(tree.targets(), vec![live(1), live(3)]);
        // One pixel more height and the third pane fits: 1 over 3 on the left.
        let mut roomy = pair();
        roomy.add(live(3), 2 * MIN_PANE_WIDTH, 2 * MIN_PANE_HEIGHT);
        assert_eq!(roomy.targets(), vec![live(1), live(3), live(2)]);
    }

    #[test]
    fn show_replaces_the_focused_pane_and_a_shown_screen_only_moves_focus() {
        let mut tree = pair();
        tree.show(live(3));
        assert_eq!(tree.targets(), vec![live(1), live(3)]);
        tree.show(live(1));
        assert_eq!(tree.targets(), vec![live(1), live(3)]);
        assert_eq!(tree.focused_target(), live(1));
        tree.show(live(4));
        assert_eq!(tree.targets(), vec![live(4), live(3)]);
    }

    #[test]
    fn closing_a_pane_gives_its_space_to_its_sibling() {
        let mut tree = grid();
        tree.close(1);
        assert_eq!(tree.targets(), vec![live(1), live(2), live(4)]);
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 600, 800), rect(600, 0, 600, 400), rect(600, 400, 600, 400)]);
        assert_eq!(tree.focused_target(), live(4), "the focus stays on a pane that was not closed");
        // The focused pane closes: the focus goes to the pane now at its place in the reading order.
        tree.close(2);
        assert_eq!(tree.targets(), vec![live(1), live(2)]);
        assert_eq!(tree.focused_target(), live(2));
        // Out of range and the only pane: nothing happens.
        tree.close(7);
        assert_eq!(tree.len(), 2);
        tree.close(0);
        tree.close(0);
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn a_closed_session_closes_its_pane_even_when_it_is_nested_and_not_when_it_is_alone() {
        let mut tree = grid();
        assert!(tree.remove_target(live(2)));
        assert_eq!(tree.targets(), vec![live(1), live(3), live(4)]);
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 600, 400), rect(0, 400, 600, 400), rect(600, 0, 600, 800)]);
        assert!(!tree.remove_target(live(9)), "not shown");
        let mut single = PaneTree::new(live(1));
        assert!(!single.remove_target(live(1)), "the caller picks what to show");
    }

    #[test]
    fn the_new_session_pane_becomes_the_started_session_wherever_it_is_and_focus_stays() {
        let mut tree = pair();
        tree.focus = live(1);
        assert!(tree.split_focused(Dir::Below, Active::NewSession, W, H));
        tree.focus = live(1);
        assert!(tree.replace(Active::NewSession, live(9)));
        assert_eq!(tree.targets(), vec![live(1), live(9), live(2)]);
        assert_eq!(tree.focused_target(), live(1), "focus stays where it was");
        // When the New-session pane had the focus it keeps it as the session.
        let mut focused = PaneTree::new(live(1));
        assert!(focused.split_focused(Dir::Beside, Active::NewSession, W, H));
        assert!(focused.replace(Active::NewSession, live(9)));
        assert_eq!(focused.focused_target(), live(9));
        assert!(!focused.replace(Active::NewSession, live(10)), "no New session pane is left");
    }

    #[test]
    fn focus_reports_whether_it_changed_and_ignores_an_index_that_is_not_there() {
        let mut tree = PaneTree::new(live(1));
        assert!(!tree.focus(1));
        assert!(tree.split_focused(Dir::Beside, live(2), W, H));
        assert!(!tree.focus(1));
        assert!(tree.focus(0));
        assert_eq!(tree.focused(), 0);
    }

    #[test]
    fn move_pane_swaps_with_the_sibling_or_turns_the_split() {
        let mut tree = pair();
        assert_eq!(tree.pane_slot(0), Some((Dir::Beside, false)));
        assert_eq!(tree.pane_slot(1), Some((Dir::Beside, true)));
        // "Move split view left" on the right pane: the two trade places, focus follows the content.
        tree.move_pane(1, Dir::Beside, W, H);
        assert_eq!(tree.targets(), vec![live(2), live(1)]);
        assert_eq!(tree.focused_target(), live(2));
        // The left pane has nowhere further left to go.
        tree.move_pane(0, Dir::Beside, W, H);
        assert_eq!(tree.targets(), vec![live(2), live(1)]);
        // "Move split view down": the side by side split turns into a stacked one.
        tree.move_pane(1, Dir::Below, W, H);
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 1200, 400), rect(0, 400, 1200, 400)]);
        assert_eq!(tree.pane_slot(1), Some((Dir::Below, true)));
        // "Move split view up" on the bottom pane swaps; "to the right" turns it back.
        tree.move_pane(1, Dir::Below, W, H);
        assert_eq!(tree.targets(), vec![live(1), live(2)]);
        tree.move_pane(1, Dir::Beside, W, H);
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 600, 800), rect(600, 0, 600, 800)]);
        // One pane: nothing to move.
        let mut single = PaneTree::new(live(1));
        assert_eq!(single.pane_slot(0), None);
        single.move_pane(0, Dir::Below, W, H);
        assert_eq!(single.targets(), vec![live(1)]);
    }

    #[test]
    fn move_pane_swapping_keeps_the_divider_where_the_content_was() {
        let mut tree = pair();
        tree.set_ratio(&[], 0.6, W, H);
        tree.move_pane(1, Dir::Beside, W, H);
        // The wide pane (1, 60 %) is now on the right.
        assert_eq!(tree.rects(W, H), vec![rect(0, 0, 480, 800), rect(480, 0, 720, 800)]);
    }

    #[test]
    fn the_opened_order_follows_adds_closes_and_replacements() {
        let mut tree = PaneTree::new(live(1));
        tree.add(live(2), W, H);
        tree.add(live(3), W, H);
        tree.close(0); // closes 1
        assert_eq!(tree.opened, vec![live(2), live(3)]);
        tree.show(live(7)); // the focused pane (3) now shows 7, in 3's place
        assert_eq!(tree.opened, vec![live(2), live(7)]);
        assert!(tree.replace(live(2), live(8)));
        assert_eq!(tree.opened, vec![live(8), live(7)]);
    }

    #[test]
    fn a_rect_is_styled_in_percent_of_the_area_so_it_follows_the_real_size() {
        assert_eq!(rect(0, 0, 1200, 800).percent_style(1200, 800), "left: 0%; top: 0%; width: 100%; height: 100%;");
        assert_eq!(rect(600, 400, 600, 400).percent_style(1200, 800), "left: 50%; top: 50%; width: 50%; height: 50%;");
        // An area of 0 does not divide by zero.
        assert!(!rect(0, 0, 0, 0).percent_style(0, 0).contains("NaN"));
    }

    #[test]
    fn a_divider_is_styled_by_its_direction() {
        let vertical = Divider { path: vec![], dir: Dir::Beside, x: 600, y: 0, length: 800 };
        assert_eq!(vertical.percent_style(1200, 800), "left: 50%; top: 0%; height: 100%;");
        let horizontal = Divider { path: vec![], dir: Dir::Below, x: 0, y: 400, length: 1200 };
        assert_eq!(horizontal.percent_style(1200, 800), "left: 0%; top: 50%; width: 100%;");
    }

    #[test]
    fn a_move_that_would_not_fit_is_refused_but_one_that_helps_a_folded_layout_is_allowed() {
        // 2 x 2 in 1200 x 800: turning the right column side by side needs 420 + 840 = 1260 wide.
        let mut full = grid();
        assert!(!full.can_move(2, Dir::Beside, W, H));
        assert!(!full.move_pane(2, Dir::Beside, W, H));
        assert_eq!(full.targets(), vec![live(1), live(3), live(2), live(4)]);
        assert_eq!(full.rects(W, H).len(), 4);
        // A swap keeps the minimum sizes, so it is always allowed.
        assert!(full.can_move(3, Dir::Below, W, H));
        assert!(full.move_pane(3, Dir::Below, W, H));
        assert_eq!(full.targets(), vec![live(1), live(3), live(4), live(2)]);
        // Under 840 px nothing fits anyway: a turn that makes the layout smaller is allowed.
        let mut folded = grid();
        assert!(!folded.fits(600, H));
        assert!(folded.can_move(2, Dir::Beside, 600, H));
        // A layout that fits may not be turned into one that does not (here: too short for 3 stacked).
        let mut pair = pair();
        assert!(!pair.can_move(1, Dir::Below, W, 2 * MIN_PANE_HEIGHT - 1));
    }
}
