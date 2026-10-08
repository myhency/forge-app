use std::collections::VecDeque;

use forge_domain::{split_plugin_id, PluginEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomizeTab {
    Skills,
    Connectors,
    Plugins,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomizeMode {
    Mine,
    Explore,
}

/// One plugin in a list: the entry and whether Forge has it switched on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRow {
    pub entry: PluginEntry,
    pub enabled: bool,
}

/// The plugins of one marketplace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginGroup {
    pub marketplace: String,
    pub rows: Vec<PluginRow>,
}

fn matches(entry: &PluginEntry, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || [&entry.name, &entry.description, &entry.marketplace].iter().any(|text| text.to_lowercase().contains(&query))
}

// `plugins` arrives sorted by marketplace then name, so equal marketplaces are next to each other.
fn groups(plugins: &[PluginEntry], enabled: &[String], query: &str, keep: impl Fn(&PluginEntry) -> bool) -> Vec<PluginGroup> {
    let mut out: Vec<PluginGroup> = Vec::new();
    for entry in plugins.iter().filter(|entry| keep(entry) && matches(entry, query)) {
        let row = PluginRow { enabled: enabled.contains(&entry.id), entry: entry.clone() };
        match out.last_mut() {
            Some(group) if group.marketplace == entry.marketplace => group.rows.push(row),
            _ => out.push(PluginGroup { marketplace: entry.marketplace.clone(), rows: vec![row] }),
        }
    }
    out
}

/// The Mine list: installed plugins only, one group per marketplace.
pub fn mine_groups(plugins: &[PluginEntry], enabled: &[String], query: &str) -> Vec<PluginGroup> {
    groups(plugins, enabled, query, |entry| entry.installed)
}

/// The Explore list: every plugin the marketplaces offer, installed ones included.
pub fn explore_groups(plugins: &[PluginEntry], enabled: &[String], query: &str) -> Vec<PluginGroup> {
    groups(plugins, enabled, query, |_| true)
}

pub fn group_title(marketplace: &str) -> String {
    if marketplace.is_empty() {
        "(no marketplace)".to_string()
    } else {
        marketplace.to_string()
    }
}

/// One thing the page asks the use case to do. Jobs run one at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    Install(String),
    Uninstall(String),
    SetEnabled(String, bool),
    AddMarketplace(String),
    RemoveMarketplace(String),
    UpdateMarketplace(String),
}

impl Job {
    /// The plugin id, for the jobs about one plugin.
    pub fn plugin_id(&self) -> Option<&str> {
        match self {
            Job::Install(id) | Job::Uninstall(id) | Job::SetEnabled(id, _) => Some(id),
            _ => None,
        }
    }

    /// The marketplace name, for the jobs about an existing marketplace (an add has no name yet).
    pub fn marketplace_name(&self) -> Option<&str> {
        match self {
            Job::UpdateMarketplace(name) | Job::RemoveMarketplace(name) => Some(name),
            _ => None,
        }
    }
}

/// The jobs waiting and the one running. The caller starts what `push` and `finish` return.
#[derive(Debug, Default)]
pub struct JobQueue {
    running: Option<Job>,
    waiting: VecDeque<Job>,
}

impl JobQueue {
    /// Adds a job. Returns it back when nothing was running, so the caller starts it now.
    pub fn push(&mut self, job: Job) -> Option<Job> {
        if self.running.is_none() {
            self.running = Some(job.clone());
            Some(job)
        } else {
            self.waiting.push_back(job);
            None
        }
    }

    /// The running job is done (or failed). Returns the next job to start, if any.
    pub fn finish(&mut self) -> Option<Job> {
        self.running = self.waiting.pop_front();
        self.running.clone()
    }

    pub fn running(&self) -> Option<&Job> {
        self.running.as_ref()
    }

    /// Whether an Add marketplace job is running or waiting (the dialog must not take a second one).
    pub fn adding_marketplace(&self) -> bool {
        let adding = |job: &Job| matches!(job, Job::AddMarketplace(_));
        self.running.as_ref().is_some_and(adding) || self.waiting.iter().any(adding)
    }

    /// Whether an update or remove of this marketplace is running.
    pub fn running_marketplace(&self, name: &str) -> bool {
        self.running.as_ref().and_then(Job::marketplace_name) == Some(name)
    }

    /// Whether an update or remove of this marketplace is waiting.
    pub fn waits_for_marketplace(&self, name: &str) -> bool {
        self.waiting.iter().any(|job| job.marketplace_name() == Some(name))
    }

    /// Whether a job about this plugin is waiting (not yet running).
    pub fn waits_for(&self, id: &str) -> bool {
        self.waiting.iter().any(|job| job.plugin_id() == Some(id))
    }
}

/// Whether the error of a finished job belongs inside the dialog that is open now, or on the page's bar.
/// An Add marketplace error goes to the Add dialog's field if that dialog is still open; an update or a
/// remove goes to the Manage dialog if that is open. Any other case (the dialog was closed, another
/// one is open, a plugin job) is the page bar's, so no message is lost.
pub fn error_in_dialog(job: &Job, add_dialog_open: bool, manage_dialog_open: bool) -> bool {
    match job {
        Job::AddMarketplace(_) => add_dialog_open,
        Job::UpdateMarketplace(_) | Job::RemoveMarketplace(_) => manage_dialog_open,
        _ => false,
    }
}

/// Whether a finished job closes the plugin detail that is open: an uninstall that worked, of that very
/// plugin, or a removed marketplace that the plugin belongs to. A failed job leaves the detail where it is.
pub fn closes_detail(job: &Job, succeeded: bool, detail: Option<&str>) -> bool {
    let (Some(open), true) = (detail, succeeded) else {
        return false;
    };
    match job {
        Job::Uninstall(id) => id == open,
        Job::RemoveMarketplace(name) => split_plugin_id(open).1 == Some(name.as_str()),
        _ => false,
    }
}

/// Numbers the reads of the catalogue so a slow older one cannot overwrite a newer one.
#[derive(Debug, Default)]
pub struct Latest {
    issued: u64,
}

impl Latest {
    /// Starts a read and returns its number.
    pub fn issue(&mut self) -> u64 {
        self.issued += 1;
        self.issued
    }

    /// Whether no later read has been started since `token` was issued.
    pub fn is_latest(&self, token: u64) -> bool {
        self.issued == token
    }
}

/// How a plugin's version reads in a list: `v1.0.0` for a version number, and the first seven characters for a
/// commit hash (a plugin from a Git marketplace is versioned by the commit it was taken from).
pub fn version_label(version: &str) -> String {
    let hex = version.len() >= 8 && version.bytes().all(|b| b.is_ascii_hexdigit());
    let has_letter = version.bytes().any(|b| b.is_ascii_alphabetic());
    if hex && has_letter {
        version[..7].to_string()
    } else {
        format!("v{version}")
    }
}

/// `2026-10-08T13:52:10.756Z` as `2026-10-08 13:52 UTC`; any other text is returned as it is.
pub fn short_time(iso: &str) -> String {
    let b = iso.as_bytes();
    let shaped = b.len() >= 16 && b[4] == b'-' && b[7] == b'-' && b[10] == b'T' && b[13] == b':' && iso.is_char_boundary(16);
    if shaped && iso.ends_with('Z') {
        format!("{} {} UTC", &iso[..10], &iso[11..16])
    } else {
        iso.to_string()
    }
}

/// The text of the Add marketplace field, trimmed. One non-empty line that is not an option.
pub fn check_source(input: &str) -> Result<String, &'static str> {
    let source = input.trim();
    if source.is_empty() {
        Err("Type a GitHub repository (owner/repo), a Git URL or a folder.")
    } else if source.contains('\n') || source.contains('\r') {
        Err("Use one line.")
    } else if source.starts_with('-') {
        Err("A source cannot start with a dash.")
    } else {
        Ok(source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::PluginEntry;

    fn entry(name: &str, marketplace: &str, installed: bool, description: &str) -> PluginEntry {
        PluginEntry {
            id: format!("{name}@{marketplace}"),
            name: name.into(),
            marketplace: marketplace.into(),
            version: None,
            description: description.into(),
            installed,
            scope: None,
            install_path: None,
        }
    }

    fn sample() -> Vec<PluginEntry> {
        vec![
            entry("alpha", "one", true, "Finds bugs"),
            entry("beta", "one", false, "Writes docs"),
            entry("alpha", "two", true, "Another alpha"),
        ]
    }

    #[test]
    fn mine_lists_installed_plugins_grouped_by_marketplace_in_order() {
        let groups = mine_groups(&sample(), &[], "");
        let names: Vec<(&str, Vec<&str>)> =
            groups.iter().map(|g| (g.marketplace.as_str(), g.rows.iter().map(|r| r.entry.name.as_str()).collect())).collect();
        assert_eq!(names, [("one", vec!["alpha"]), ("two", vec!["alpha"])]);
    }

    #[test]
    fn explore_lists_everything_and_keeps_the_installed_flag() {
        let groups = explore_groups(&sample(), &[], "");
        assert_eq!(groups[0].rows.len(), 2);
        assert!(groups[0].rows[0].entry.installed && !groups[0].rows[1].entry.installed);
    }

    #[test]
    fn a_row_is_on_when_its_full_id_is_in_the_enabled_list() {
        let groups = mine_groups(&sample(), &["alpha@two".to_string()], "");
        assert!(!groups[0].rows[0].enabled, "alpha@one is off");
        assert!(groups[1].rows[0].enabled, "alpha@two is on");
    }

    #[test]
    fn the_search_matches_name_description_and_marketplace_ignoring_case() {
        assert_eq!(mine_groups(&sample(), &[], "ALPHA").len(), 2);
        assert_eq!(explore_groups(&sample(), &[], "docs").len(), 1);
        assert_eq!(explore_groups(&sample(), &[], "TWO").len(), 1, "the marketplace name counts");
        assert!(explore_groups(&sample(), &[], "zzz").is_empty());
        assert_eq!(explore_groups(&sample(), &[], "  ").len(), 2, "blank means everything");
    }

    #[test]
    fn a_plugin_without_a_marketplace_gets_a_readable_group_title() {
        assert_eq!(group_title(""), "(no marketplace)");
        assert_eq!(group_title("one"), "one");
    }

    #[test]
    fn the_first_job_starts_at_once_and_later_ones_wait() {
        let mut queue = JobQueue::default();
        assert_eq!(queue.push(Job::Install("a@m".into())), Some(Job::Install("a@m".into())));
        assert_eq!(queue.push(Job::Install("b@m".into())), None);
        assert!(queue.waits_for("b@m") && !queue.waits_for("a@m"));
        assert_eq!(queue.running(), Some(&Job::Install("a@m".into())));
    }

    #[test]
    fn finishing_a_job_hands_out_the_next_one_in_order_whether_or_not_it_failed() {
        let mut queue = JobQueue::default();
        queue.push(Job::Install("a@m".into()));
        queue.push(Job::Install("b@m".into()));
        queue.push(Job::Uninstall("c@m".into()));
        assert_eq!(queue.finish(), Some(Job::Install("b@m".into())));
        assert_eq!(queue.finish(), Some(Job::Uninstall("c@m".into())));
        assert_eq!(queue.finish(), None);
        assert_eq!(queue.running(), None);
        // Idle again: the next push starts at once.
        assert!(queue.push(Job::Install("d@m".into())).is_some());
    }

    #[test]
    fn a_job_knows_which_plugin_it_is_about() {
        assert_eq!(Job::Install("a@m".into()).plugin_id(), Some("a@m"));
        assert_eq!(Job::SetEnabled("a@m".into(), true).plugin_id(), Some("a@m"));
        assert_eq!(Job::AddMarketplace("o/r".into()).plugin_id(), None);
    }

    #[test]
    fn a_source_is_trimmed_and_must_be_one_non_empty_line() {
        assert_eq!(check_source("  owner/repo \n"), Ok("owner/repo".to_string()));
        assert_eq!(check_source("https://git.example.com/team/plugins.git"), Ok("https://git.example.com/team/plugins.git".to_string()));
        assert!(check_source("   ").is_err());
        assert!(check_source("a\nb").is_err());
        assert!(check_source("--help").is_err(), "an option is not a source");
    }

    #[test]
    fn an_add_marketplace_job_counts_while_it_runs_and_while_it_waits() {
        let mut queue = JobQueue::default();
        assert!(!queue.adding_marketplace());
        queue.push(Job::Install("a@m".into()));
        queue.push(Job::AddMarketplace("o/r".into()));
        assert!(queue.adding_marketplace(), "waiting behind an install still counts");
        queue.finish();
        assert!(queue.adding_marketplace(), "running counts");
        queue.finish();
        assert!(!queue.adding_marketplace());
    }

    #[test]
    fn a_marketplace_job_knows_its_marketplace_and_the_queue_sees_it_running_or_waiting() {
        assert_eq!(Job::UpdateMarketplace("m".into()).marketplace_name(), Some("m"));
        assert_eq!(Job::RemoveMarketplace("m".into()).marketplace_name(), Some("m"));
        assert_eq!(Job::AddMarketplace("o/r".into()).marketplace_name(), None, "an add has no name yet");
        assert_eq!(Job::Install("a@m".into()).marketplace_name(), None);
        let mut queue = JobQueue::default();
        queue.push(Job::UpdateMarketplace("one".into()));
        queue.push(Job::RemoveMarketplace("two".into()));
        assert!(queue.running_marketplace("one") && !queue.running_marketplace("two"));
        assert!(queue.waits_for_marketplace("two") && !queue.waits_for_marketplace("one"));
    }

    #[test]
    fn an_error_stays_in_a_dialog_only_while_that_dialog_is_open() {
        let add = Job::AddMarketplace("o/r".into());
        assert!(error_in_dialog(&add, true, false));
        assert!(!error_in_dialog(&add, false, false), "the dialog was closed: the page bar shows it");
        assert!(!error_in_dialog(&add, false, true), "another dialog is open: not the add dialog's field");
        let update = Job::UpdateMarketplace("m".into());
        assert!(error_in_dialog(&update, false, true));
        assert!(!error_in_dialog(&update, true, false));
        assert!(!error_in_dialog(&Job::Install("a@m".into()), true, true), "an install error is always the page bar's");
    }

    #[test]
    fn the_detail_closes_only_when_its_own_plugin_or_marketplace_was_removed() {
        let uninstall = Job::Uninstall("a@m".into());
        assert!(closes_detail(&uninstall, true, Some("a@m")));
        assert!(!closes_detail(&uninstall, false, Some("a@m")), "a failed uninstall keeps the detail");
        assert!(!closes_detail(&uninstall, true, Some("b@m")), "another plugin's detail stays");
        assert!(!closes_detail(&uninstall, true, None));
        let remove = Job::RemoveMarketplace("m".into());
        assert!(closes_detail(&remove, true, Some("a@m")));
        assert!(!closes_detail(&remove, true, Some("a@other")));
        assert!(!closes_detail(&remove, false, Some("a@m")));
        assert!(!closes_detail(&Job::Install("a@m".into()), true, Some("a@m")));
    }

    #[test]
    fn only_the_newest_read_may_show_its_result() {
        let mut latest = Latest::default();
        let first = latest.issue();
        let second = latest.issue();
        assert!(!latest.is_latest(first), "an older read that lands late must be dropped");
        assert!(latest.is_latest(second));
    }

    #[test]
    fn a_time_from_the_known_marketplaces_file_reads_as_date_and_minutes_in_utc() {
        assert_eq!(short_time("2026-10-08T13:52:10.756Z"), "2026-10-08 13:52 UTC");
        assert_eq!(short_time("2026-02-17T04:26:16.034Z"), "2026-02-17 04:26 UTC");
        assert_eq!(short_time("yesterday"), "yesterday", "anything else is shown as it is");
        assert_eq!(short_time(""), "");
    }

    #[test]
    fn a_version_number_gets_a_v_and_a_commit_hash_is_cut_short() {
        assert_eq!(version_label("1.0.0"), "v1.0.0");
        assert_eq!(version_label("0.2.0-beta.1"), "v0.2.0-beta.1");
        assert_eq!(version_label("b78ac49cdc6b"), "b78ac49", "plugins of a Git marketplace are versioned by commit");
        assert_eq!(version_label("b78ac49cdc6b4d1e8f3a9b0c1d2e3f4a5b6c7d8e"), "b78ac49");
        assert_eq!(version_label("20240101"), "v20240101", "a short run of digits is a version, not a hash");
        assert_eq!(version_label("deadbeef"), "deadbee");
    }
}
