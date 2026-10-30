use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use forge_entities::code_run::{cut_output, model_message, text_hash, typed_text, MarkerScanner, RunEvent};

use crate::terminal_tap::TerminalTap;

/// How long a run waits for a terminal to take it before it ends.
pub const START_WAIT: Duration = Duration::from_secs(10);
const KEPT_RUNS: usize = 50;
// More raw output than this is trimmed from the front; the box and the message show far less.
const RAW_LIMIT: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// Not typed yet, or typed and the start marker has not come back.
    Waiting,
    Running,
    Done { code: i32 },
    /// The shell ended, the tab was closed, or no terminal took the run in time.
    Ended,
}

impl RunState {
    fn is_open(self) -> bool {
        matches!(self, RunState::Waiting | RunState::Running)
    }
}

/// A run as the chat draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct RunView {
    pub id: u64,
    pub hash: String,
    pub state: RunState,
    pub output: String,
    pub cut: bool,
    /// When the run ended, in milliseconds since the Unix epoch.
    pub ended_ms: Option<u64>,
}

/// A run that is over, handed out once so the model can be told.
#[derive(Debug, Clone, PartialEq)]
pub struct Finished {
    pub block: String,
    pub code: Option<i32>,
    pub output: String,
    pub cut: bool,
}

struct Run {
    id: u64,
    block: String,
    state: RunState,
    typed: bool,
    raw: String,
    created: Instant,
    reported: bool,
    ended_ms: Option<u64>,
}

impl Run {
    fn close(&mut self, state: RunState) {
        self.state = state;
        self.ended_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|since| since.as_millis() as u64);
    }
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    runs: Vec<Run>,
    // The last time `expire` saw a run in flight: a run queued behind it waits from then, not from when it was made.
    last_busy: Option<Instant>,
}

impl Inner {
    fn run_mut(&mut self, id: u64) -> Option<&mut Run> {
        self.runs.iter_mut().find(|run| run.id == id)
    }
}

/// The runs of one session. A handle is cheap to clone; every clone sees the same runs.
#[derive(Clone, Default)]
pub struct CodeRuns(Rc<RefCell<Inner>>);

/// A handle as a component prop: equal to itself only.
impl PartialEq for CodeRuns {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The messages to send the model for runs that ended, in order.
pub fn finished_messages(finished: Vec<Finished>) -> Vec<String> {
    finished.into_iter().map(|run| model_message(&run.block, run.code, &run.output, run.cut)).collect()
}

impl CodeRuns {
    /// Adds a run for `block`. A block that is already waiting or running gives that run's id and adds nothing.
    pub fn start(&self, block: &str, now: Instant) -> u64 {
        let mut inner = self.0.borrow_mut();
        if let Some(open) = inner.runs.iter().find(|run| run.state.is_open() && run.block.trim() == block.trim()) {
            return open.id;
        }
        let id = inner.next_id;
        inner.next_id += 1;
        inner.runs.push(Run { id, block: block.to_string(), state: RunState::Waiting, typed: false, raw: String::new(), created: now, reported: false, ended_ms: None });
        while inner.runs.len() > KEPT_RUNS {
            let Some(position) = inner.runs.iter().position(|run| !run.state.is_open() && run.reported) else { break };
            inner.runs.remove(position);
        }
        id
    }

    pub fn snapshot(&self) -> Vec<RunView> {
        self.0
            .borrow()
            .runs
            .iter()
            .map(|run| {
                let (output, cut) = cut_output(&run.raw);
                RunView { id: run.id, hash: text_hash(&run.block), state: run.state, output, cut, ended_ms: run.ended_ms }
            })
            .collect()
    }

    /// The runs that ended since the last call, each once.
    pub fn take_finished(&self) -> Vec<Finished> {
        let mut inner = self.0.borrow_mut();
        let mut finished = Vec::new();
        for run in inner.runs.iter_mut().filter(|run| !run.state.is_open() && !run.reported) {
            run.reported = true;
            let code = match run.state {
                RunState::Done { code } => Some(code),
                _ => None,
            };
            let (output, cut) = cut_output(&run.raw);
            finished.push(Finished { block: run.block.clone(), code, output, cut });
        }
        finished
    }

    // True while a run has been typed into a shell and has not ended.
    fn in_flight(&self) -> bool {
        self.0.borrow().runs.iter().any(|run| run.typed && run.state.is_open())
    }

    // The text to type for the oldest run nobody has typed yet; marks it typed.
    fn take_next_to_type(&self) -> Option<(u64, Vec<u8>)> {
        let mut inner = self.0.borrow_mut();
        let run = inner.runs.iter_mut().find(|run| !run.typed && run.state == RunState::Waiting)?;
        run.typed = true;
        Some((run.id, typed_text(run.id, &run.block)))
    }

    fn start_seen(&self, id: u64) -> bool {
        match self.0.borrow_mut().run_mut(id) {
            Some(run) if run.state.is_open() => {
                run.state = RunState::Running;
                true
            }
            _ => false,
        }
    }

    fn add_output(&self, id: u64, text: &str) {
        if let Some(run) = self.0.borrow_mut().run_mut(id).filter(|run| run.state == RunState::Running) {
            run.raw.push_str(text);
            if run.raw.len() > RAW_LIMIT {
                let mut from = run.raw.len() - RAW_LIMIT;
                while !run.raw.is_char_boundary(from) {
                    from += 1;
                }
                run.raw.drain(..from);
            }
        }
    }

    fn end(&self, id: u64, state: RunState) {
        if let Some(run) = self.0.borrow_mut().run_mut(id).filter(|run| run.state.is_open()) {
            run.close(state);
        }
    }

    /// Ends the runs that no terminal took in `START_WAIT`, unless a run is in flight (they are only queued behind it).
    pub fn expire(&self, now: Instant) {
        let busy = self.in_flight();
        let mut inner = self.0.borrow_mut();
        if busy {
            inner.last_busy = Some(now);
            return;
        }
        let idle_since = inner.last_busy;
        for run in inner.runs.iter_mut() {
            let since = idle_since.map_or(run.created, |busy| busy.max(run.created));
            if !run.typed && run.state == RunState::Waiting && now.saturating_duration_since(since) >= START_WAIT {
                run.close(RunState::Ended);
            }
        }
    }
}

/// Types the runs of a session into one terminal tab and reads their output back. Acts only while `active` is true.
pub struct RunTap {
    runs: CodeRuns,
    active: Rc<Cell<bool>>,
    scanner: RefCell<MarkerScanner>,
    // The run whose text this tap typed and that has not ended.
    typed: Cell<Option<u64>>,
    // The run the scanner is inside.
    current: Cell<Option<u64>>,
    // Whether the shell has printed anything yet, and how many ticks have passed since it last did. A run is typed
    // only once the shell has started and settled: input that comes earlier is echoed raw by the terminal.
    seen_output: Cell<bool>,
    quiet_ticks: Cell<u32>,
}

// Ticks (a quarter of a second each) of silence after the shell's output before a run is typed.
const SETTLE_TICKS: u32 = 2;

impl RunTap {
    pub fn new(runs: CodeRuns, active: Rc<Cell<bool>>) -> Self {
        Self { runs, active, scanner: RefCell::new(MarkerScanner::new()), typed: Cell::new(None), current: Cell::new(None), seen_output: Cell::new(false), quiet_ticks: Cell::new(0) }
    }
}

impl TerminalTap for RunTap {
    fn output(&self, bytes: &[u8]) -> Vec<u8> {
        self.seen_output.set(true);
        self.quiet_ticks.set(0);
        let events = self.scanner.borrow_mut().feed(bytes);
        for event in events {
            match event {
                RunEvent::Started(id) => self.current.set(self.runs.start_seen(id).then_some(id)),
                RunEvent::Text(text) => {
                    if let Some(id) = self.current.get() {
                        self.runs.add_output(id, &text);
                    }
                }
                RunEvent::Ended { id, code } => {
                    self.runs.end(id, RunState::Done { code });
                    if self.current.get() == Some(id) {
                        self.current.set(None);
                    }
                    if self.typed.get() == Some(id) {
                        self.typed.set(None);
                    }
                }
            }
        }
        Vec::new()
    }

    fn tick(&self) -> Vec<u8> {
        self.quiet_ticks.set(self.quiet_ticks.get().saturating_add(1));
        if !self.seen_output.get() || self.quiet_ticks.get() < SETTLE_TICKS || !self.active.get() || self.runs.in_flight() {
            return Vec::new();
        }
        match self.runs.take_next_to_type() {
            Some((id, keys)) => {
                self.typed.set(Some(id));
                keys
            }
            None => Vec::new(),
        }
    }

    fn shell_ended(&self) {
        if let Some(id) = self.typed.take() {
            self.runs.end(id, RunState::Ended);
        }
        self.current.set(None);
    }
}

/// Whether the run button can work with this shell: zsh and a bash with bracketed paste (not Apple's 3.2).
pub fn shell_supported(shell: Option<&str>, windows: bool) -> bool {
    // Apple's `/bin/bash` is version 3.2: no bracketed paste, so the block would be typed as raw text and a Ctrl-C
    // would leave the run open. A newer bash from elsewhere is fine.
    !windows && shell != Some("/bin/bash") && shell.and_then(|shell| shell.rsplit('/').next()).is_some_and(|name| matches!(name, "zsh" | "bash"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: &str = "\x1b]777;forge-run-start;";
    const END: &str = "\x1b]777;forge-run-end;";

    fn marked(id: u64, text: &str, code: i32) -> Vec<u8> {
        format!("{START}{id}\x07{text}{END}{id};{code}\x07").into_bytes()
    }

    // A tap whose shell has printed its prompt and been quiet for one tick: the next tick may type.
    fn tap_for(runs: &CodeRuns, active: bool) -> (RunTap, Rc<Cell<bool>>) {
        let flag = Rc::new(Cell::new(active));
        let tap = RunTap::new(runs.clone(), flag.clone());
        tap.output(b"% ");
        assert!(tap.tick().is_empty());
        (tap, flag)
    }

    #[test]
    fn nothing_is_typed_until_the_shell_has_printed_and_gone_quiet() {
        let runs = CodeRuns::default();
        let tap = RunTap::new(runs.clone(), Rc::new(Cell::new(true)));
        let id = runs.start("echo hi", Instant::now());
        assert!(tap.tick().is_empty() && tap.tick().is_empty() && tap.tick().is_empty(), "the shell has not printed yet");
        tap.output(b"banner\r\n");
        assert!(tap.tick().is_empty(), "one quiet tick is not enough");
        tap.output(b"more\r\n");
        assert!(tap.tick().is_empty(), "output starts the quiet again");
        assert_eq!(tap.tick(), typed_text(id, "echo hi"));
    }

    #[test]
    fn a_started_run_is_typed_on_the_next_tick_of_an_active_tab_only() {
        let runs = CodeRuns::default();
        let (tap, active) = tap_for(&runs, false);
        let id = runs.start("echo hi", Instant::now());
        assert!(tap.tick().is_empty(), "an inactive tab types nothing");
        active.set(true);
        assert_eq!(tap.tick(), typed_text(id, "echo hi"));
        assert!(tap.tick().is_empty(), "typed once");
    }

    #[test]
    fn output_between_markers_fills_the_run_and_the_end_marker_finishes_it() {
        let runs = CodeRuns::default();
        let (tap, _) = tap_for(&runs, true);
        let id = runs.start("echo hi", Instant::now());
        tap.tick();
        assert!(tap.output(format!("{START}{id}\x07hi\r\n").as_bytes()).is_empty());
        let view = &runs.snapshot()[0];
        assert_eq!((view.state, view.output.as_str()), (RunState::Running, "hi\n"));
        tap.output(format!("{END}{id};3\x07").as_bytes());
        let view = &runs.snapshot()[0];
        assert_eq!(view.state, RunState::Done { code: 3 });
        assert_eq!(runs.take_finished(), vec![Finished { block: "echo hi".into(), code: Some(3), output: "hi\n".into(), cut: false }]);
        assert!(runs.take_finished().is_empty());
    }

    #[test]
    fn a_second_run_waits_until_the_first_ended() {
        let runs = CodeRuns::default();
        let (tap, _) = tap_for(&runs, true);
        let first = runs.start("sleep 1", Instant::now());
        let second = runs.start("echo two", Instant::now());
        assert_eq!(tap.tick(), typed_text(first, "sleep 1"));
        assert!(tap.tick().is_empty(), "the first is still in flight");
        tap.output(&marked(first, "", 0));
        assert!(tap.tick().is_empty(), "the shell is let to settle after the first");
        assert_eq!(tap.tick(), typed_text(second, "echo two"));
    }

    #[test]
    fn pressing_the_same_block_again_while_it_is_waiting_or_running_starts_nothing() {
        let runs = CodeRuns::default();
        let now = Instant::now();
        let first = runs.start("echo hi", now);
        assert_eq!(runs.start("echo hi\n", now), first);
        assert_eq!(runs.snapshot().len(), 1);
        let (tap, _) = tap_for(&runs, true);
        tap.tick();
        tap.output(&marked(first, "x\n", 0));
        let again = runs.start("echo hi", now);
        assert_ne!(again, first, "a finished run can be run again");
        assert_eq!(runs.snapshot().len(), 2);
    }

    #[test]
    fn a_run_not_started_in_ten_seconds_ends() {
        let runs = CodeRuns::default();
        let t0 = Instant::now();
        runs.start("echo hi", t0);
        runs.expire(t0 + Duration::from_secs(9));
        assert_eq!(runs.snapshot()[0].state, RunState::Waiting);
        runs.expire(t0 + Duration::from_secs(11));
        assert_eq!(runs.snapshot()[0].state, RunState::Ended);
        let finished = runs.take_finished();
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].code, None);
    }

    #[test]
    fn a_run_queued_behind_a_long_one_does_not_expire() {
        let runs = CodeRuns::default();
        let (tap, _) = tap_for(&runs, true);
        let t0 = Instant::now();
        let first = runs.start("sleep 99", t0);
        runs.start("echo two", t0);
        tap.tick();
        tap.output(format!("{START}{first}\x07").as_bytes());
        runs.expire(t0 + Duration::from_secs(60));
        assert_eq!(runs.snapshot()[1].state, RunState::Waiting);
    }

    #[test]
    fn a_run_queued_behind_a_long_one_is_given_its_own_ten_seconds_after_the_first_ends() {
        let runs = CodeRuns::default();
        let (tap, _) = tap_for(&runs, true);
        let t0 = Instant::now();
        let first = runs.start("sleep 30", t0);
        runs.start("echo two", t0);
        tap.tick();
        tap.output(format!("{START}{first}\x07").as_bytes());
        runs.expire(t0 + Duration::from_secs(30));
        tap.output(format!("{END}{first};0\x07").as_bytes());
        runs.expire(t0 + Duration::from_millis(30_100));
        assert_eq!(runs.snapshot()[1].state, RunState::Waiting, "the first has only just ended");
        runs.expire(t0 + Duration::from_secs(41));
        assert_eq!(runs.snapshot()[1].state, RunState::Ended, "nobody took it in ten seconds");
    }

    #[test]
    fn a_shell_that_ends_ends_the_running_run_with_no_code() {
        let runs = CodeRuns::default();
        let (tap, _) = tap_for(&runs, true);
        let id = runs.start("sleep 99", Instant::now());
        tap.tick();
        tap.output(format!("{START}{id}\x07partial\n").as_bytes());
        tap.shell_ended();
        assert_eq!(runs.snapshot()[0].state, RunState::Ended);
        let finished = runs.take_finished();
        assert_eq!((finished[0].code, finished[0].output.as_str()), (None, "partial\n"));
        tap.shell_ended();
        assert!(runs.take_finished().is_empty(), "told once");
    }

    #[test]
    fn output_of_one_run_never_lands_in_another() {
        let runs = CodeRuns::default();
        let (tap, _) = tap_for(&runs, true);
        let a = runs.start("echo same", Instant::now());
        tap.tick();
        tap.output(&marked(a, "from a\n", 0));
        let b = runs.start("echo same", Instant::now());
        tap.tick();
        tap.output(&marked(b, "from b\n", 0));
        let views = runs.snapshot();
        assert_eq!(views[0].output, "from a\n");
        assert_eq!(views[1].output, "from b\n");
        assert_eq!(views[0].hash, views[1].hash);
        tap.output(&marked(99, "stray\n", 0));
        assert_eq!(runs.snapshot().len(), 2, "markers of an unknown run are ignored");
    }

    #[test]
    fn snapshot_cuts_output_the_way_the_message_does() {
        let runs = CodeRuns::default();
        let (tap, _) = tap_for(&runs, true);
        let id = runs.start("seq 300", Instant::now());
        tap.tick();
        let text: String = (0..300).map(|n| format!("{n}\r\n")).collect();
        tap.output(&marked(id, &text, 0));
        let view = &runs.snapshot()[0];
        assert!(view.cut);
        assert_eq!(view.output.lines().count(), 200);
    }

    #[test]
    fn only_the_last_runs_are_kept() {
        let runs = CodeRuns::default();
        for n in 0..60 {
            runs.start(&format!("echo {n}"), Instant::now());
        }
        runs.expire(Instant::now() + Duration::from_secs(60));
        // Only runs the model has been told about are let go.
        assert_eq!(runs.take_finished().len(), 60);
        runs.start("echo last", Instant::now());
        assert!(runs.snapshot().len() <= 50);
    }

    #[test]
    fn pruning_keeps_a_run_the_model_has_not_been_told_about() {
        let runs = CodeRuns::default();
        let t0 = Instant::now();
        runs.start("echo first", t0);
        runs.expire(t0 + Duration::from_secs(60));
        // The first has ended but nobody has taken it yet; fifty more must not push it out.
        for n in 0..60 {
            runs.start(&format!("echo {n}"), t0 + Duration::from_secs(60));
        }
        let told = runs.take_finished();
        assert!(told.iter().any(|run| run.block == "echo first"), "the ended run was dropped before it was reported");
    }

    #[test]
    fn a_run_that_ended_has_an_end_time_and_one_that_runs_has_none() {
        let runs = CodeRuns::default();
        let (tap, _) = tap_for(&runs, true);
        let id = runs.start("echo hi", Instant::now());
        tap.tick();
        tap.output(format!("{START}{id}\x07").as_bytes());
        assert_eq!(runs.snapshot()[0].ended_ms, None);
        tap.output(format!("{END}{id};0\x07").as_bytes());
        assert!(runs.snapshot()[0].ended_ms.is_some_and(|ms| ms > 1_700_000_000_000));
    }

    #[test]
    fn handles_of_one_session_are_equal_and_those_of_two_are_not() {
        let runs = CodeRuns::default();
        assert!(runs == runs.clone());
        assert!(runs != CodeRuns::default());
    }

    #[test]
    fn finished_runs_become_model_messages_in_order() {
        let messages = finished_messages(vec![
            Finished { block: "ls".into(), code: Some(0), output: "a\n".into(), cut: false },
            Finished { block: "sleep 9".into(), code: None, output: String::new(), cut: false },
        ]);
        assert_eq!(messages.len(), 2);
        assert!(messages[0].starts_with("[forge-run]\n") && messages[0].contains("exit 0"));
        assert!(messages[1].contains("shell ended"));
    }

    #[test]
    fn shell_support() {
        assert!(shell_supported(Some("/bin/zsh"), false));
        assert!(shell_supported(Some("/usr/bin/bash"), false));
        assert!(shell_supported(Some("/opt/homebrew/bin/bash"), false));
        assert!(!shell_supported(Some("/bin/bash"), false), "macOS's own bash 3.2 has no bracketed paste");
        assert!(!shell_supported(Some("/usr/bin/fish"), false));
        assert!(!shell_supported(None, false));
        assert!(!shell_supported(Some("/bin/zsh"), true));
    }

    #[test]
    fn a_hash_is_the_text_hash() {
        let runs = CodeRuns::default();
        runs.start("echo hi", Instant::now());
        assert_eq!(runs.snapshot()[0].hash, text_hash("echo hi"));
    }
}
