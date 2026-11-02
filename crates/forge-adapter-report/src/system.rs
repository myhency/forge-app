//! What a report says about the computer: system, processor, memory, disk, language and the Forge version.
//! The computer's name and the person's name are left out on purpose.

use std::fmt::Write;
use std::time::Duration;

use sysinfo::{Disks, ProcessesToUpdate, System};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

pub fn describe() -> String {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();
    // The processor load is the change between two readings.
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.max(Duration::from_millis(200)));
    sys.refresh_cpu_all();
    let pid = sysinfo::get_current_pid().ok();
    if let Some(pid) = pid {
        sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    }

    let mut out = String::new();
    let _ = writeln!(out, "Forge {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(out, "Built for {} {}", std::env::consts::OS, std::env::consts::ARCH);
    let _ = writeln!(out);
    let _ = writeln!(out, "[System]");
    let _ = writeln!(out, "OS: {} {}", System::name().unwrap_or_default(), System::long_os_version().or_else(System::os_version).unwrap_or_default());
    let _ = writeln!(out, "Kernel: {}", System::kernel_version().unwrap_or_default());
    let _ = writeln!(out, "Uptime: {}", hours(System::uptime()));
    if let Some(tag) = sys_locale_tag() {
        let _ = writeln!(out, "Locale: {tag}");
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "[Processor]");
    let cpus = sys.cpus();
    let _ = writeln!(out, "Model: {}", cpus.first().map(|cpu| cpu.brand().trim().to_string()).unwrap_or_default());
    let _ = writeln!(out, "Cores: {} physical, {} logical", System::physical_core_count().map(|n| n.to_string()).unwrap_or_else(|| "?".into()), cpus.len());
    let _ = writeln!(out, "Load now: {:.0}%", sys.global_cpu_usage());
    let load = System::load_average();
    if cfg!(unix) {
        let _ = writeln!(out, "Load average (1/5/15 min): {:.2} / {:.2} / {:.2}", load.one, load.five, load.fifteen);
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "[Memory]");
    let _ = writeln!(out, "Total: {:.1} GiB", sys.total_memory() as f64 / GIB);
    let _ = writeln!(out, "Used: {:.1} GiB", sys.used_memory() as f64 / GIB);
    let _ = writeln!(out, "Available: {:.1} GiB", sys.available_memory() as f64 / GIB);
    let _ = writeln!(out, "Swap used: {:.1} of {:.1} GiB", sys.used_swap() as f64 / GIB, sys.total_swap() as f64 / GIB);
    if let Some(process) = pid.and_then(|pid| sys.process(pid)) {
        let _ = writeln!(out, "Forge uses: {:.0} MiB", process.memory() as f64 / (1024.0 * 1024.0));
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "[Disks]");
    for disk in Disks::new_with_refreshed_list().list() {
        // Only the disks a person's files live on: a read-only system or a recovery volume only adds noise.
        if disk.total_space() == 0 || disk.is_read_only() {
            continue;
        }
        let _ = writeln!(out, "{}: {:.0} GiB free of {:.0} GiB", disk.mount_point().display(), disk.available_space() as f64 / GIB, disk.total_space() as f64 / GIB);
    }
    out
}

fn hours(seconds: u64) -> String {
    format!("{}h {}m", seconds / 3600, seconds % 3600 / 60)
}

fn sys_locale_tag() -> Option<String> {
    ["LC_ALL", "LANG"].iter().find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_description_names_the_processor_and_the_memory() {
        let text = describe();
        for heading in ["[System]", "[Processor]", "[Memory]", "[Disks]"] {
            assert!(text.contains(heading), "{text}");
        }
        assert!(text.contains("Total:") && text.contains("Cores:"));
    }

    #[test]
    fn uptime_reads_as_hours_and_minutes() {
        assert_eq!(hours(3 * 3600 + 125), "3h 2m");
    }
}
