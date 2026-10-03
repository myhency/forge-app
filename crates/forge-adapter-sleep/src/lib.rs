use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use forge_ports::SleepPort;

/// Keeps the Mac awake with one `caffeinate` child per holder. `-w <pid>` ties each child to
/// Forge's own process, so even a crash cannot leave one running.
pub struct CaffeinateSleep {
    program: String,
    args: Vec<String>,
    held: Mutex<HashMap<u64, Child>>,
}

impl CaffeinateSleep {
    pub fn new() -> Self {
        Self::with_command("caffeinate", vec!["-i".to_string(), "-w".to_string(), std::process::id().to_string()])
    }

    pub fn with_command(program: impl Into<String>, args: Vec<String>) -> Self {
        Self { program: program.into(), args, held: Mutex::new(HashMap::new()) }
    }

    #[cfg(test)]
    fn is_running(&self, key: u64) -> bool {
        match self.held.lock().unwrap().get_mut(&key) {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        }
    }

    #[cfg(test)]
    fn held_count(&self) -> usize {
        self.held.lock().unwrap().len()
    }
}

impl Default for CaffeinateSleep {
    fn default() -> Self {
        Self::new()
    }
}

impl SleepPort for CaffeinateSleep {
    fn hold(&self, key: u64) -> Result<(), String> {
        let mut held = self.held.lock().unwrap();
        if held.contains_key(&key) {
            return Ok(());
        }
        let child = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not start {}: {error}", self.program))?;
        held.insert(key, child);
        Ok(())
    }

    fn release(&self, key: u64) {
        let removed = self.held.lock().unwrap().remove(&key);
        if let Some(mut child) = removed {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for CaffeinateSleep {
    fn drop(&mut self) {
        if let Ok(held) = self.held.get_mut() {
            for (_, mut child) in held.drain() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holder() -> CaffeinateSleep {
        // `sleep 30` stands in for caffeinate: a child that stays alive until killed.
        CaffeinateSleep::with_command("sleep", vec!["30".to_string()])
    }

    #[test]
    fn holding_starts_a_child_and_releasing_ends_it() {
        let sleep = holder();
        sleep.hold(1).unwrap();
        assert!(sleep.is_running(1));
        sleep.release(1);
        assert!(!sleep.is_running(1));
    }

    #[test]
    fn holding_twice_for_one_key_is_one_child() {
        let sleep = holder();
        sleep.hold(1).unwrap();
        sleep.hold(1).unwrap();
        assert_eq!(sleep.held_count(), 1);
        sleep.release(1);
    }

    #[test]
    fn two_keys_hold_and_release_independently() {
        let sleep = holder();
        sleep.hold(1).unwrap();
        sleep.hold(2).unwrap();
        sleep.release(1);
        assert!(!sleep.is_running(1));
        assert!(sleep.is_running(2));
        sleep.release(2);
    }

    #[test]
    fn releasing_a_key_that_is_not_held_is_ignored() {
        holder().release(9);
    }

    #[test]
    fn a_program_that_cannot_start_is_reported_as_a_message() {
        let sleep = CaffeinateSleep::with_command("forge-no-such-program", Vec::new());
        let error = sleep.hold(1).unwrap_err();
        assert!(error.contains("forge-no-such-program"), "{error}");
        assert_eq!(sleep.held_count(), 0);
    }

    #[test]
    fn dropping_the_holder_ends_its_children() {
        let sleep = holder();
        sleep.hold(1).unwrap();
        let pid = sleep.held.lock().unwrap().get(&1).unwrap().id();
        drop(sleep);
        // `kill -0` fails once the process is gone and reaped.
        let alive = std::process::Command::new("kill").args(["-0", &pid.to_string()]).status().unwrap().success();
        assert!(!alive);
    }
}
