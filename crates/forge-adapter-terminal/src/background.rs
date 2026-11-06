//! Programs started in the background with their output kept (see `BackgroundPort`).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use forge_ports::BackgroundPort;

const KEPT: usize = 100_000;

struct Running {
    child: Child,
    output: Arc<Mutex<Vec<u8>>>,
    ended: Option<i32>,
}

#[derive(Default)]
pub struct BackgroundPrograms {
    next: Mutex<u64>,
    programs: Mutex<HashMap<u64, Running>>,
}

impl BackgroundPrograms {
    pub fn new() -> Self {
        Self::default()
    }
}

fn shell(command: &str) -> Command {
    if cfg!(windows) {
        let mut shell = Command::new("cmd");
        shell.arg("/C").arg(command);
        shell
    } else {
        let mut shell = Command::new("sh");
        shell.arg("-c").arg(command);
        // Its own process group, so stopping it (and Forge quitting) ends everything it started.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut shell, 0);
        shell
    }
}

// The process groups still running, ended when Forge exits (an exit skips destructors).
#[cfg(unix)]
static GROUPS: Mutex<Vec<i32>> = Mutex::new(Vec::new());

#[cfg(unix)]
extern "C" fn end_groups() {
    if let Ok(groups) = GROUPS.lock() {
        for group in groups.iter() {
            // SAFETY: signals a process group this process started.
            unsafe { libc::kill(-group, libc::SIGTERM) };
        }
    }
}

#[cfg(unix)]
fn track(child: &Child) {
    static HOOK: std::sync::Once = std::sync::Once::new();
    // SAFETY: registers a plain function to run at exit.
    HOOK.call_once(|| unsafe {
        libc::atexit(end_groups);
    });
    if let Ok(mut groups) = GROUPS.lock() {
        groups.push(child.id() as i32);
    }
}

#[cfg(unix)]
fn untrack(pid: u32) {
    if let Ok(mut groups) = GROUPS.lock() {
        groups.retain(|group| *group != pid as i32);
    }
}

// On Windows every program goes into one job that ends its processes when Forge's handle to it
// closes, which the system does however Forge ends.
#[cfg(windows)]
fn track(child: &Child) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE};
    static JOB: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let job = *JOB.get_or_init(|| {
        // SAFETY: a nameless job with default security; the handle is kept for the life of the process.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(job, JobObjectExtendedLimitInformation, std::ptr::addr_of!(limits).cast(), std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32);
            job as usize
        }
    });
    if job != 0 {
        // SAFETY: both handles are valid; a failure only leaves the program outside the job.
        unsafe { AssignProcessToJobObject(job as _, child.as_raw_handle() as _) };
    }
}

#[cfg(windows)]
fn untrack(_pid: u32) {}

#[cfg(not(any(unix, windows)))]
fn track(_child: &Child) {}
#[cfg(not(any(unix, windows)))]
fn untrack(_pid: u32) {}

fn keep(mut from: impl Read + Send + 'static, into: Arc<Mutex<Vec<u8>>>) {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        while let Ok(read) = from.read(&mut buffer) {
            if read == 0 {
                break;
            }
            if let Ok(mut kept) = into.lock() {
                kept.extend_from_slice(&buffer[..read]);
                if kept.len() > KEPT {
                    let cut = kept.len() - KEPT;
                    kept.drain(..cut);
                }
            }
        }
    });
}

impl BackgroundPort for BackgroundPrograms {
    fn start(&self, cwd: &Path, command: &str) -> Result<u64, String> {
        let mut child = shell(command).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|error| error.to_string())?;
        track(&child);
        let output = Arc::new(Mutex::new(Vec::new()));
        if let Some(out) = child.stdout.take() {
            keep(out, output.clone());
        }
        if let Some(err) = child.stderr.take() {
            keep(err, output.clone());
        }
        let mut next = self.next.lock().map_err(|_| "busy".to_string())?;
        *next += 1;
        let id = *next;
        self.programs.lock().map_err(|_| "busy".to_string())?.insert(id, Running { child, output, ended: None });
        Ok(id)
    }

    fn output(&self, id: u64) -> Option<String> {
        let programs = self.programs.lock().ok()?;
        let kept = programs.get(&id)?.output.lock().ok()?;
        Some(String::from_utf8_lossy(&kept).into_owned())
    }

    fn exit_code(&self, id: u64) -> Option<Option<i32>> {
        let mut programs = self.programs.lock().ok()?;
        let running = programs.get_mut(&id)?;
        if running.ended.is_none() {
            if let Ok(Some(status)) = running.child.try_wait() {
                running.ended = Some(status.code().unwrap_or(-1));
            }
        }
        Some(running.ended)
    }

    fn stop(&self, id: u64) -> bool {
        let Ok(mut programs) = self.programs.lock() else { return false };
        let Some(running) = programs.get_mut(&id) else { return false };
        if running.ended.is_some() || matches!(running.child.try_wait(), Ok(Some(_))) {
            return false;
        }
        // `npm run dev` starts node under npm under the shell: the whole tree goes.
        let pid = running.child.id();
        let killed = if cfg!(windows) {
            Command::new("taskkill").args(["/T", "/F", "/PID", &pid.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|status| status.success()) || running.child.kill().is_ok()
        } else {
            #[cfg(unix)]
            // SAFETY: signals the process group this adapter started.
            let group = unsafe { libc::kill(-(pid as i32), libc::SIGTERM) } == 0;
            #[cfg(not(unix))]
            let group = false;
            group || running.child.kill().is_ok()
        };
        untrack(pid);
        let _ = running.child.wait();
        running.ended = Some(-1);
        killed
    }
}

impl Drop for BackgroundPrograms {
    fn drop(&mut self) {
        if let Ok(mut programs) = self.programs.lock() {
            for running in programs.values_mut() {
                let _ = running.child.kill();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_runs_its_output_is_kept_and_it_can_be_stopped() {
        let programs = BackgroundPrograms::new();
        let dir = tempfile::tempdir().unwrap();
        let id = programs.start(dir.path(), "echo ready; sleep 30").unwrap();
        for _ in 0..50 {
            if programs.output(id).is_some_and(|out| out.contains("ready")) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(programs.output(id).unwrap().contains("ready"));
        assert_eq!(programs.exit_code(id), Some(None), "still running");
        assert!(programs.stop(id));
        assert_eq!(programs.exit_code(id), Some(Some(-1)));
        assert!(!programs.stop(id), "already ended");
        assert_eq!(programs.output(999), None);
    }
}
