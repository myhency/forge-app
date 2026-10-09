use std::rc::Rc;

/// Listens to a terminal and may type into it. The terminal view calls `output` for every piece of output and
/// `tick` about four times a second; the bytes returned are written to the shell. The dock's terminal has none.
pub trait TerminalTap {
    fn output(&self, bytes: &[u8]) -> Vec<u8>;
    fn tick(&self) -> Vec<u8>;
}

/// A tap as a component prop: equal to itself only.
#[derive(Clone)]
pub struct TapHandle(pub Rc<dyn TerminalTap>);

impl PartialEq for TapHandle {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
