use std::collections::VecDeque;

use forge_domain::PluginEntry;

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

    /// Whether a job about this plugin is waiting (not yet running).
    pub fn waits_for(&self, id: &str) -> bool {
        self.waiting.iter().any(|job| job.plugin_id() == Some(id))
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
}
