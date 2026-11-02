//! The computer's own logs about Forge, collected with the tools every system ships. Each one is best effort: a
//! tool that is missing, fails or runs too long leaves a note in its place instead of stopping the report.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

/// One piece of the computer's logs: a file name inside the zip and its text.
pub struct Part {
    pub name: String,
    pub text: String,
}

const TOOL_TIMEOUT: Duration = Duration::from_secs(15);
/// A crash report older than this is no longer about the problem being reported.
const CRASH_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
const MAX_CRASH_FILES: usize = 3;
const MAX_PART_BYTES: usize = 2 * 1024 * 1024;

pub fn collect(home: Option<&Path>) -> Vec<Part> {
    let mut parts = Vec::new();
    #[cfg(target_os = "macos")]
    {
        parts.push(tool_part(
            "system/macos-unified-log.txt",
            "log",
            &["show", "--last", "30m", "--style", "compact", "--predicate", "process == \"forge-app\" OR process == \"Forge\""],
        ));
        if let Some(home) = home {
            parts.extend(crash_reports(&home.join("Library/Logs/DiagnosticReports")));
        }
    }
    #[cfg(target_os = "windows")]
    {
        let _ = home;
        parts.push(tool_part("system/windows-event-log.txt", "wevtutil", &["qe", "Application", "/q:*[System[(Level=1 or Level=2)]]", "/c:60", "/rd:true", "/f:text"]));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = home;
        parts.push(tool_part("system/journal.txt", "journalctl", &["--since", "30 min ago", "--no-pager", "-p", "warning", "-n", "400"]));
    }
    parts
}

/// The newest crash reports whose file name starts with `forge`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn crash_reports(dir: &Path) -> Vec<Part> {
    let mut found: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().to_lowercase().starts_with("forge"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .filter(|(modified, _)| modified.elapsed().is_ok_and(|age| age <= CRASH_AGE))
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found
        .into_iter()
        .take(MAX_CRASH_FILES)
        .filter_map(|(_, path)| {
            let bytes = std::fs::read(&path).ok()?;
            let text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_PART_BYTES)]).into_owned();
            Some(Part { name: format!("system/crash-{}.txt", path.file_name()?.to_string_lossy()), text })
        })
        .collect()
}

#[cfg_attr(not(any(target_os = "macos", target_os = "windows", unix)), allow(dead_code))]
fn tool_part(name: &str, program: &str, args: &[&str]) -> Part {
    let text = match run(program, args, TOOL_TIMEOUT) {
        Ok(text) if text.trim().is_empty() => format!("`{program}` found nothing to report.\n"),
        Ok(text) => text,
        Err(why) => format!("Could not read this log with `{program}`: {why}\n"),
    };
    Part { name: name.to_string(), text }
}

/// Runs a program and returns its output, cut at 2 MiB, or stops it after `limit`.
pub(crate) fn run(program: &str, args: &[&str], limit: Duration) -> Result<String, String> {
    let mut child = Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|err| err.to_string())?;
    let mut stdout = child.stdout.take().ok_or("no output")?;
    // Reading on a thread keeps a full pipe from blocking the child while this waits for it.
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = (&mut stdout).take(MAX_PART_BYTES as u64).read_to_end(&mut bytes);
        // Keep draining so the child never blocks on a full pipe.
        let _ = std::io::copy(&mut stdout, &mut std::io::sink());
        bytes
    });
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("it took longer than {} seconds", limit.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(err) => return Err(err.to_string()),
        }
    }
    let bytes = reader.join().map_err(|_| "reading failed".to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_tool_that_runs_gives_its_output() {
        assert_eq!(run("echo", &["hi"], Duration::from_secs(5)).unwrap().trim(), "hi");
    }

    #[cfg(unix)]
    #[test]
    fn a_tool_that_runs_too_long_is_stopped() {
        let error = run("sleep", &["5"], Duration::from_millis(200)).unwrap_err();
        assert!(error.contains("longer than"), "{error}");
    }

    #[test]
    fn a_missing_tool_leaves_a_note() {
        let part = tool_part("system/x.txt", "forge-no-such-tool", &[]);
        assert!(part.text.starts_with("Could not read this log"), "{}", part.text);
    }

    #[test]
    fn only_recent_forge_crash_reports_are_taken() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("forge-app-2026.ips"), "crash").unwrap();
        std::fs::write(dir.path().join("Safari-2026.ips"), "other").unwrap();
        let parts = crash_reports(dir.path());
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].name, "system/crash-forge-app-2026.ips.txt");
    }
}
