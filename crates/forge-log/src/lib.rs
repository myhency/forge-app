//! Forge's own log file: what the app did, one line each, kept on disk so the person can send it with a report.
//!
//! Lines are written raw; hiding personal details is the report's job (`forge-adapter-report`), so the person's
//! own copy stays readable. Before `init` runs (and in tests) every call is a no-op.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// The live file is `forge.log`; when it passes this size it becomes `forge.log.1`, and the older ones move up.
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// `forge.log.1` to `forge.log.4` are kept beside the live file: about 10 MB at most.
const KEPT_FILES: u32 = 4;
/// One line never grows past this, so a runaway message cannot fill the disk.
const MAX_LINE_CHARS: usize = 4000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

struct Sink {
    dir: PathBuf,
    file: File,
    written: u64,
}

static SINK: OnceLock<Mutex<Sink>> = OnceLock::new();

/// Opens `forge.log` in `dir` (made if missing), notes the start, and makes a panic leave a line. Safe to call
/// once; later calls are ignored. A folder that cannot be written leaves logging off without stopping the app.
pub fn init(dir: &Path) {
    if SINK.get().is_some() || fs::create_dir_all(dir).is_err() {
        return;
    }
    let path = dir.join("forge.log");
    let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else { return };
    let written = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    if SINK.set(Mutex::new(Sink { dir: dir.to_path_buf(), file, written })).is_err() {
        return;
    }
    install_panic_hook();
}

/// The folder the log files live in, once logging is on.
pub fn log_dir() -> Option<PathBuf> {
    SINK.get().and_then(|sink| sink.lock().ok()).map(|sink| sink.dir.clone())
}

pub fn info(target: &str, message: &str) {
    write_line(Level::Info, target, message);
}

pub fn warn(target: &str, message: &str) {
    write_line(Level::Warn, target, message);
}

pub fn error(target: &str, message: &str) {
    write_line(Level::Error, target, message);
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let place = info.location().map(|at| format!("{}:{}", at.file(), at.line())).unwrap_or_default();
        let text = info.payload().downcast_ref::<&str>().map(|text| text.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
        let trace = std::backtrace::Backtrace::force_capture();
        write_line(Level::Error, "panic", &format!("{text} at {place}\n{trace}"));
        previous(info);
    }));
}

fn write_line(level: Level, target: &str, message: &str) {
    let Some(sink) = SINK.get() else { return };
    let Ok(mut sink) = sink.lock() else { return };
    let line = format_line(OffsetDateTime::now_utc(), level, target, message);
    if sink.written + line.len() as u64 > MAX_FILE_BYTES {
        rotate(&mut sink);
    }
    if sink.file.write_all(line.as_bytes()).is_ok() {
        sink.written += line.len() as u64;
    }
}

fn format_line(at: OffsetDateTime, level: Level, target: &str, message: &str) -> String {
    let stamp = at.format(&Rfc3339).unwrap_or_default();
    let message: String = message.chars().take(MAX_LINE_CHARS).collect();
    // A multi-line message (a backtrace) keeps its lines, indented, so every record still starts with a timestamp.
    let mut lines = message.lines();
    let mut text = format!("{stamp} {:<5} {target}: {}\n", level.label(), lines.next().unwrap_or(""));
    for rest in lines {
        text.push_str("    ");
        text.push_str(rest);
        text.push('\n');
    }
    text
}

fn rotate(sink: &mut Sink) {
    let name = |n: u32| sink.dir.join(if n == 0 { "forge.log".to_string() } else { format!("forge.log.{n}") });
    let _ = fs::remove_file(name(KEPT_FILES));
    for n in (0..KEPT_FILES).rev() {
        let _ = fs::rename(name(n), name(n + 1));
    }
    if let Ok(file) = OpenOptions::new().create(true).append(true).open(name(0)) {
        sink.file = file;
        sink.written = 0;
    }
}

/// The log files in `dir`, oldest first, so a reader gets them in the order they were written.
pub fn log_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = (1..=KEPT_FILES).rev().map(|n| dir.join(format!("forge.log.{n}"))).chain([dir.join("forge.log")]).filter(|path| path.is_file()).collect();
    files.shrink_to_fit();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_starts_with_a_timestamp_the_level_and_the_target() {
        let line = format_line(OffsetDateTime::UNIX_EPOCH, Level::Warn, "sidecar", "exited");
        assert_eq!(line, "1970-01-01T00:00:00Z WARN  sidecar: exited\n");
    }

    #[test]
    fn a_multi_line_message_is_indented_under_one_timestamp() {
        let line = format_line(OffsetDateTime::UNIX_EPOCH, Level::Error, "panic", "boom\nframe 1\nframe 2");
        assert_eq!(line.lines().count(), 3);
        assert!(line.lines().skip(1).all(|row| row.starts_with("    ")));
    }

    #[test]
    fn a_huge_message_is_cut() {
        let line = format_line(OffsetDateTime::UNIX_EPOCH, Level::Info, "t", &"x".repeat(100_000));
        assert!(line.len() < MAX_LINE_CHARS + 100);
    }

    #[test]
    fn rotation_moves_files_up_and_drops_the_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("forge.log");
        fs::write(&path, "live").unwrap();
        for n in 1..=KEPT_FILES {
            fs::write(dir.path().join(format!("forge.log.{n}")), format!("old{n}")).unwrap();
        }
        let mut sink = Sink { dir: dir.path().to_path_buf(), file: File::open(&path).unwrap(), written: MAX_FILE_BYTES };
        rotate(&mut sink);
        assert_eq!(fs::read_to_string(dir.path().join("forge.log.1")).unwrap(), "live");
        assert_eq!(fs::read_to_string(dir.path().join("forge.log.4")).unwrap(), "old3");
        assert_eq!(fs::read_to_string(&path).unwrap(), "");
        assert_eq!(sink.written, 0);
    }

    #[test]
    fn log_files_come_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["forge.log", "forge.log.2", "forge.log.1", "other.txt"] {
            fs::write(dir.path().join(name), "x").unwrap();
        }
        let names: Vec<String> = log_files(dir.path()).iter().map(|path| path.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["forge.log.2", "forge.log.1", "forge.log"]);
    }

    #[test]
    fn calls_before_init_do_nothing() {
        info("t", "no sink yet");
    }
}
