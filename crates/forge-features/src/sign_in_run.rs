use std::cell::RefCell;
use std::time::Instant;

use crate::sign_in::{Action, SignInDriver};
use crate::terminal_tap::TerminalTap;

/// One sign-in: a driver, the clock it is given, and where its verdict goes.
pub struct SignInRun {
    driver: RefCell<SignInDriver>,
    started: Instant,
    report: Box<dyn Fn(Action)>,
}

impl SignInRun {
    pub fn new(name: &str, launch_line: &str, report: impl Fn(Action) + 'static) -> Self {
        let started = Instant::now();
        Self { driver: RefCell::new(SignInDriver::new(name, launch_line, started.elapsed())), started, report: Box::new(report) }
    }

    fn keys_of(&self, actions: Vec<Action>) -> Vec<u8> {
        let mut keys = Vec::new();
        for action in actions {
            match action {
                Action::Send(bytes) => keys.extend(bytes),
                verdict => (self.report)(verdict),
            }
        }
        keys
    }
}

impl TerminalTap for SignInRun {
    fn output(&self, bytes: &[u8]) -> Vec<u8> {
        let actions = self.driver.borrow_mut().feed(bytes, self.started.elapsed());
        self.keys_of(actions)
    }

    fn tick(&self) -> Vec<u8> {
        let actions = self.driver.borrow_mut().tick(self.started.elapsed());
        self.keys_of(actions)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::thread::sleep;
    use std::time::Duration;

    use super::*;
    use crate::sign_in::{mark, Outcome};
    use crate::terminal_tap::TapHandle;

    fn run(name: &str) -> (SignInRun, Rc<RefCell<Vec<Action>>>) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = seen.clone();
        (SignInRun::new(name, "'/x/claude'", move |action| sink.borrow_mut().push(action)), seen)
    }

    #[test]
    fn keys_come_back_as_bytes_to_type_and_verdicts_go_to_the_report() {
        let (tap, seen) = run("web");
        assert!(tap.output(b"$ ").is_empty());
        sleep(Duration::from_millis(400));
        assert_eq!(tap.tick(), b"'/x/claude'\r".to_vec());
        assert_eq!(tap.output(mark::INPUT[0].as_bytes()), b"/mcp\r".to_vec());
        assert!(seen.borrow().is_empty());
        tap.output(format!("{}\r\n❯ web\r\n", mark::LIST).as_bytes());
        sleep(Duration::from_millis(200));
        assert_eq!(tap.tick(), b"\r".to_vec(), "web is selected: open it");
        tap.output(mark::ITEM.as_bytes());
        assert!(tap.output(mark::SIGNED[0].as_bytes()).is_empty());
        assert_eq!(*seen.borrow(), vec![Action::Done(Outcome::Signed)]);
    }

    #[test]
    fn a_tap_handle_is_equal_to_itself_only() {
        let (a, _) = run("web");
        let (b, _) = run("web");
        let first = TapHandle(Rc::new(a));
        assert!(first == first.clone());
        assert!(first != TapHandle(Rc::new(b)));
    }
}
