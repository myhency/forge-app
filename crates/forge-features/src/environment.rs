//! What the environment menu, the environment dialog and a send from an environment decide, apart
//! from any signal, so each rule is tested on its own.

use std::path::PathBuf;

use forge_domain::{new_environment_id, parse_env, EnvParseError, EnvVars, Environment};

use crate::worktree_view::NewWorktree;

/// One row of the environment menu. `id` is `None` for this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvRow {
    pub id: Option<String>,
    pub name: String,
    /// The key that picks it (⌥⌘`number`), when it has one.
    pub number: Option<usize>,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentMenu {
    pub visible: Vec<EnvRow>,
    pub more: Vec<EnvRow>,
}

/// ⌥⌘1 is this computer, so at most eight saved environments are numbered.
const MAX_SAVED_NUMBERED: usize = 8;

// Pinned first, then the one used last; a name breaks a tie.
fn ordered(environments: &[Environment]) -> Vec<&Environment> {
    let mut list: Vec<&Environment> = environments.iter().collect();
    list.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.last_used_at.cmp(&a.last_used_at)).then_with(|| a.name.cmp(&b.name)));
    list
}

// The saved environments shown before `More`: every pinned one and the newest other one, at most
// eight; the rest stay under `More`.
fn split(environments: &[Environment]) -> (Vec<&Environment>, Vec<&Environment>) {
    let mut visible: Vec<&Environment> = Vec::new();
    let mut more: Vec<&Environment> = Vec::new();
    let mut unpinned_shown = false;
    for environment in ordered(environments) {
        let shown = if environment.pinned {
            true
        } else if !unpinned_shown {
            unpinned_shown = true;
            true
        } else {
            false
        };
        if shown && visible.len() < MAX_SAVED_NUMBERED {
            visible.push(environment);
        } else {
            more.push(environment);
        }
    }
    (visible, more)
}

fn row(environment: &Environment, number: Option<usize>) -> EnvRow {
    EnvRow { id: Some(environment.id.clone()), name: environment.name.clone(), number, pinned: environment.pinned }
}

/// The menu for `query`: with an empty query this computer, the visible saved environments (numbered
/// 1, 2, …) and the rest under `more`; with a query every match, flat and unnumbered, newest first.
pub fn environment_menu(environments: &[Environment], query: &str, this_computer: &str) -> EnvironmentMenu {
    let query = query.trim().to_lowercase();
    if !query.is_empty() {
        let matches = ordered(environments).into_iter().filter(|environment| environment.name.to_lowercase().contains(&query));
        let mut visible: Vec<&Environment> = matches.collect();
        visible.sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at).then_with(|| a.name.cmp(&b.name)));
        return EnvironmentMenu { visible: visible.into_iter().map(|environment| row(environment, None)).collect(), more: Vec::new() };
    }
    let (shown, more) = split(environments);
    let mut visible = vec![EnvRow { id: None, name: this_computer.to_string(), number: Some(1), pinned: false }];
    visible.extend(shown.into_iter().enumerate().map(|(index, environment)| row(environment, Some(index + 2))));
    EnvironmentMenu { visible, more: more.into_iter().map(|environment| row(environment, None)).collect() }
}

/// The ids the keys ⌥⌘1…⌥⌘9 pick, in order; index 0 is this computer (`None`).
pub fn numbered_ids(environments: &[Environment]) -> Vec<Option<String>> {
    let (shown, _) = split(environments);
    std::iter::once(None).chain(shown.into_iter().map(|environment| Some(environment.id.clone()))).collect()
}

/// What the dialog edits. `id` is `None` while a new environment is being made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentDraft {
    pub id: Option<String>,
    pub name: String,
    pub folders: Vec<PathBuf>,
    pub branch: Option<String>,
    pub worktree: bool,
    pub env_vars: String,
}

impl EnvironmentDraft {
    /// A new environment: the name and folder suggested, a worktree on (as in the folder menu).
    pub fn blank(name: String, folder: Option<PathBuf>) -> Self {
        Self { id: None, name, folders: folder.into_iter().collect(), branch: None, worktree: true, env_vars: String::new() }
    }

    pub fn of(environment: &Environment) -> Self {
        Self {
            id: Some(environment.id.clone()),
            name: environment.name.clone(),
            folders: environment.folders.clone(),
            branch: environment.branch.clone(),
            worktree: environment.worktree,
            env_vars: environment.env_vars.clone(),
        }
    }

    pub fn changed_from(&self, initial: &EnvironmentDraft) -> bool {
        self != initial
    }

    pub fn variables_error(&self) -> Option<EnvParseError> {
        parse_env(&self.env_vars).err()
    }

    pub fn can_save(&self) -> bool {
        !self.name.trim().is_empty() && self.variables_error().is_none()
    }

    /// The environment to store. An edit keeps the id, the pin and the times of `previous`; a new one
    /// gets an id from `id_seed` and `now` as both times.
    pub fn build(&self, previous: Option<&Environment>, id_seed: u64, now: i64) -> Environment {
        Environment {
            id: previous.map_or_else(|| new_environment_id(id_seed), |environment| environment.id.clone()),
            name: self.name.clone(),
            folders: self.folders.clone(),
            branch: self.branch.clone(),
            worktree: self.worktree,
            env_vars: self.env_vars.clone(),
            pinned: previous.is_some_and(|environment| environment.pinned),
            created_at: previous.map_or(now, |environment| environment.created_at),
            last_used_at: previous.map_or(now, |environment| environment.last_used_at),
        }
        .normalized()
    }
}

/// What a send from an environment starts with.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentStart {
    pub folder: PathBuf,
    /// Folders besides the first, none equal to it, each once.
    pub extra: Vec<PathBuf>,
    pub env: EnvVars,
    pub worktree: Option<NewWorktree>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartError {
    NoFolder,
    Variables(EnvParseError),
}

pub fn start_plan(environment: &Environment) -> Result<EnvironmentStart, StartError> {
    let normalized = environment.clone().normalized();
    let mut folders = normalized.folders.into_iter();
    let folder = folders.next().ok_or(StartError::NoFolder)?;
    let env = parse_env(&normalized.env_vars).map_err(StartError::Variables)?;
    Ok(EnvironmentStart {
        folder,
        extra: folders.collect(),
        env: EnvVars(env),
        worktree: normalized.worktree.then(|| NewWorktree { base: normalized.branch }),
    })
}

/// Whether the first folder of the plan is gone, so nothing may be started in it.
pub fn folder_missing(plan: &EnvironmentStart) -> bool {
    !plan.folder.is_dir()
}

/// A key the new-session screen's environment chip answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentKey {
    /// ⌥⌘1…⌥⌘9, counted from 0 (0 is this computer).
    Pick(usize),
    /// ⌥⌘0.
    New,
}

/// What opens the environment dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentRequest {
    New { folder: Option<PathBuf> },
    Edit(String),
}

/// The worktree switch and branch a picked environment puts in the draft; `None` for this computer,
/// which leaves the draft alone.
pub fn draft_choice(environment: Option<&Environment>) -> Option<(bool, Option<String>)> {
    environment.map(|environment| (environment.worktree, environment.branch.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::EnvParseReason;

    fn env(id: &str, name: &str, pinned: bool, last_used: i64) -> Environment {
        Environment {
            id: id.to_string(),
            name: name.to_string(),
            folders: vec![format!("/work/{name}").into()],
            branch: None,
            worktree: false,
            env_vars: String::new(),
            pinned,
            created_at: 0,
            last_used_at: last_used,
        }
    }

    fn names(rows: &[EnvRow]) -> Vec<String> {
        rows.iter().map(|row| row.name.clone()).collect()
    }

    #[test]
    fn the_menu_lists_this_computer_pinned_ones_and_the_newest_one_with_the_rest_under_more() {
        let list = vec![env("a", "old", false, 1), env("b", "new", false, 9), env("c", "mid", false, 5), env("d", "pin", true, 0)];
        let menu = environment_menu(&list, "", "This computer");
        assert_eq!(names(&menu.visible), vec!["This computer", "pin", "new"]);
        assert_eq!(names(&menu.more), vec!["mid", "old"]);
        assert_eq!(menu.visible.iter().map(|row| row.number).collect::<Vec<_>>(), vec![Some(1), Some(2), Some(3)]);
        assert_eq!(menu.more.iter().map(|row| row.number).collect::<Vec<_>>(), vec![None, None]);
        assert_eq!(menu.visible[0].id, None);
    }

    #[test]
    fn picking_an_environment_takes_its_worktree_choice_and_this_computer_leaves_the_draft_as_it_is() {
        let mut environment = env("a", "app", false, 0);
        environment.worktree = true;
        environment.branch = Some("dev".to_string());
        assert_eq!(draft_choice(Some(&environment)), Some((true, Some("dev".to_string()))));
        assert_eq!(draft_choice(None), None);
    }

    #[test]
    fn with_no_saved_environment_only_this_computer_shows() {
        let menu = environment_menu(&[], "", "This computer");
        assert_eq!(names(&menu.visible), vec!["This computer"]);
        assert!(menu.more.is_empty());
    }

    #[test]
    fn at_most_nine_rows_are_numbered_and_extra_pinned_ones_go_under_more() {
        let list: Vec<Environment> = (0..12).map(|n| env(&format!("e{n}"), &format!("p{n:02}"), true, n)).collect();
        let menu = environment_menu(&list, "", "This computer");
        assert_eq!(menu.visible.len(), 9);
        assert_eq!(menu.more.len(), 4);
        assert_eq!(numbered_ids(&list).len(), 9);
        assert_eq!(numbered_ids(&list)[0], None);
    }

    #[test]
    fn a_query_lists_every_match_flat_without_numbers_and_ignores_case() {
        let list = vec![env("a", "Work App", false, 1), env("b", "play", false, 2), env("c", "work docs", false, 3)];
        let menu = environment_menu(&list, "WORK", "This computer");
        assert_eq!(names(&menu.visible), vec!["work docs", "Work App"]);
        assert!(menu.visible.iter().all(|row| row.number.is_none()) && menu.more.is_empty());
    }

    #[test]
    fn numbered_ids_follow_the_menu_so_a_key_picks_the_row_it_shows() {
        let list = vec![env("a", "old", false, 1), env("b", "new", false, 9)];
        assert_eq!(numbered_ids(&list), vec![None, Some("b".to_string())]);
    }

    #[test]
    fn a_blank_draft_takes_the_folder_and_is_not_changed_until_edited() {
        let draft = EnvironmentDraft::blank("app".to_string(), Some("/work/app".into()));
        assert_eq!(draft.folders, vec![std::path::PathBuf::from("/work/app")]);
        assert!(draft.worktree);
        assert!(!draft.changed_from(&draft.clone()));
        let mut edited = draft.clone();
        edited.name = "other".to_string();
        assert!(edited.changed_from(&draft));
    }

    #[test]
    fn a_draft_can_be_saved_with_a_name_and_valid_variables_and_reports_the_bad_line() {
        let mut draft = EnvironmentDraft::blank("app".to_string(), None);
        assert!(draft.can_save());
        draft.name = "   ".to_string();
        assert!(!draft.can_save());
        draft.name = "app".to_string();
        draft.env_vars = "A=1\nnope".to_string();
        assert!(!draft.can_save());
        assert_eq!(draft.variables_error().map(|error| error.line), Some(2));
    }

    #[test]
    fn building_a_new_environment_sets_the_ids_and_times_and_an_edit_keeps_what_the_dialog_does_not_show() {
        let mut draft = EnvironmentDraft::blank(" app ".to_string(), Some("/a".into()));
        draft.folders.push("/a".into());
        let made = draft.build(None, 255, 1000);
        assert_eq!((made.id.as_str(), made.name.as_str(), made.created_at, made.last_used_at), ("env-00000000000000ff", "app", 1000, 1000));
        assert_eq!(made.folders, vec![std::path::PathBuf::from("/a")]);

        let mut stored = made.clone();
        stored.pinned = true;
        stored.last_used_at = 5000;
        let mut edit = EnvironmentDraft::of(&stored);
        edit.name = "renamed".to_string();
        let changed = edit.build(Some(&stored), 1, 9999);
        assert_eq!((changed.id.as_str(), changed.name.as_str(), changed.pinned, changed.created_at, changed.last_used_at), (stored.id.as_str(), "renamed", true, 1000, 5000));
    }

    #[test]
    fn the_start_plan_uses_the_first_folder_the_others_as_extras_and_the_variables() {
        let mut environment = env("a", "app", false, 0);
        environment.folders = vec!["/a".into(), "/b".into(), "/a".into(), "/c".into()];
        environment.env_vars = "X=1\nY=\"two words\"".to_string();
        environment.worktree = true;
        environment.branch = Some("dev".to_string());
        let plan = start_plan(&environment).unwrap();
        assert_eq!(plan.folder, std::path::PathBuf::from("/a"));
        assert_eq!(plan.extra, vec![std::path::PathBuf::from("/b"), "/c".into()]);
        assert_eq!(plan.env, EnvVars(vec![("X".to_string(), "1".to_string()), ("Y".to_string(), "two words".to_string())]));
        assert_eq!(plan.worktree, Some(NewWorktree { base: Some("dev".to_string()) }));
    }

    #[test]
    fn the_start_plan_refuses_an_environment_with_no_folder_or_with_bad_variables() {
        let mut environment = env("a", "app", false, 0);
        environment.folders.clear();
        assert_eq!(start_plan(&environment).unwrap_err(), StartError::NoFolder);
        let mut bad = env("b", "app", false, 0);
        bad.env_vars = "oops".to_string();
        assert_eq!(start_plan(&bad).unwrap_err(), StartError::Variables(EnvParseError { line: 1, reason: EnvParseReason::NotAPair }));
        let mut plain = env("c", "app", false, 0);
        plain.worktree = false;
        assert_eq!(start_plan(&plain).unwrap().worktree, None);
    }

    #[test]
    fn a_plan_whose_first_folder_is_gone_is_reported_missing() {
        let plan = |folder: PathBuf| EnvironmentStart { folder, extra: Vec::new(), env: EnvVars(Vec::new()), worktree: None };
        assert!(folder_missing(&plan(std::env::temp_dir().join("forge-no-such-folder-8f3a1c"))));
        assert!(!folder_missing(&plan(std::env::temp_dir())));
    }
}
