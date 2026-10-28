use serde::{Deserialize, Serialize};

pub const DEFAULT_SIDEBAR_WIDTH: u32 = 264;
pub const MIN_SIDEBAR_WIDTH: u32 = 200;
pub const MAX_SIDEBAR_WIDTH: u32 = 480;

/// What the sidebar orders a group's sessions by. An unknown stored value reads as `Activity`, so a
/// config written by another version still loads (`other` must sit on the last variant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    Name,
    Created,
    #[default]
    #[serde(other)]
    Activity,
}

/// How the sidebar groups sessions. An unknown stored value reads as `Folder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    Date,
    Flat,
    #[default]
    #[serde(other)]
    Folder,
}

/// Which sessions the sidebar lists. An unknown stored value reads as `Active`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusFilter {
    Archived,
    All,
    #[default]
    #[serde(other)]
    Active,
}

/// What a session's Changes panel compared, kept so it is the same after a restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedChangeView {
    All,
    Uncommitted,
    Commit(String),
}

/// The Changes panel's view and base of one session, found by project folder and Claude session
/// id like `ArchivedRef`. A session on the default (all changes, default base) has no entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeModeRef {
    pub project: std::path::PathBuf,
    pub session_id: String,
    pub view: SavedChangeView,
    pub base: Option<String>,
}

/// A session the person archived. The Claude transcript is not changed; only Forge's config
/// remembers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchivedRef {
    pub project: std::path::PathBuf,
    pub session_id: String,
}

/// How the sidebar looks, kept in the config so it is the same after a restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SidebarPrefs {
    pub sort_by: SortBy,
    pub group_by: GroupBy,
    pub status: StatusFilter,
    /// Whether a project with no sessions still gets a heading.
    pub show_empty: bool,
    /// Ids of the collapsed groups (`arrange` makes them: `folder:<path>`, `date:<bucket>`).
    pub collapsed: Vec<String>,
    pub width: u32,
    pub hidden: bool,
}

impl Default for SidebarPrefs {
    fn default() -> Self {
        Self {
            sort_by: SortBy::default(),
            group_by: GroupBy::default(),
            status: StatusFilter::default(),
            show_empty: false,
            collapsed: Vec::new(),
            width: DEFAULT_SIDEBAR_WIDTH,
            hidden: false,
        }
    }
}

impl SidebarPrefs {
    /// The width to draw: a hand-edited or corrupt value never makes the sidebar unusable.
    pub fn clamped_width(&self) -> u32 {
        self.width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH)
    }

    pub fn is_collapsed(&self, id: &str) -> bool {
        self.collapsed.iter().any(|collapsed| collapsed == id)
    }

    pub fn toggle_collapsed(&mut self, id: &str) {
        match self.collapsed.iter().position(|collapsed| collapsed == id) {
            Some(index) => {
                self.collapsed.remove(index);
            }
            None => self.collapsed.push(id.to_string()),
        }
    }
}

pub const DEFAULT_TAB_SIZE: u8 = 4;

/// How the Files panel's editor behaves: the settings of its `⋮` menu. They are kept for the whole
/// of Forge, not per session. A field missing from the stored config reads as its default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorPrefs {
    /// Whether the file tree is shown.
    pub tree: bool,
    /// Whether a file opens in an italic preview tab that the next file replaces.
    pub preview_tabs: bool,
    pub auto_save: bool,
    /// Whether the Tab key indents (else it moves focus).
    pub tab_indent: bool,
    pub wrap: bool,
    pub indent_guides: bool,
    pub whitespace: bool,
    pub scroll_past_end: bool,
    pub tab_size: u8,
}

impl Default for EditorPrefs {
    fn default() -> Self {
        Self {
            tree: true,
            preview_tabs: true,
            auto_save: true,
            tab_indent: true,
            wrap: true,
            indent_guides: true,
            whitespace: false,
            scroll_past_end: false,
            tab_size: DEFAULT_TAB_SIZE,
        }
    }
}

impl EditorPrefs {
    /// The tab size to use: a hand-edited value other than 2, 4 or 8 reads as 4.
    pub fn clamped_tab_size(&self) -> u8 {
        match self.tab_size {
            2 | 4 | 8 => self.tab_size,
            _ => DEFAULT_TAB_SIZE,
        }
    }
}

/// Where the Changes panel puts its file list. `Auto` is the right column, under the diffs when
/// the panel is narrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileListPosition {
    #[default]
    Auto,
    Left,
    Right,
}

/// How the Changes panel draws a diff (its `⋮` menu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChangesViewPrefs {
    /// Whether the right column shows folders (else a flat list).
    pub group_by_folder: bool,
    /// Whether a diff draws the old and the new lines in two columns (else one column).
    pub side_by_side: bool,
    /// Whether long lines wrap (else the diff scrolls sideways).
    pub wrap: bool,
    /// Whether the changed words of a changed line are marked.
    pub highlight_words: bool,
    /// Whether git leaves out whitespace-only changes (`-w`).
    pub hide_whitespace: bool,
    pub file_list_position: FileListPosition,
    /// Whether test files and generated files are listed apart from the others.
    pub separate_special: bool,
}

impl Default for ChangesViewPrefs {
    fn default() -> Self {
        Self {
            group_by_folder: true,
            side_by_side: false,
            wrap: true,
            highlight_words: true,
            hide_whitespace: false,
            file_list_position: FileListPosition::Auto,
            separate_special: false,
        }
    }
}

macro_rules! choice_enum {
    ($name:ident, default $default:ident, { $($variant:ident => ($value:literal, $label:expr)),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }
        impl Default for $name {
            fn default() -> Self {
                $name::$default
            }
        }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];
            /// The text used in the page's `data-*` attribute and in the config file.
            pub fn value(self) -> &'static str {
                match self { $($name::$variant => $value),+ }
            }
            pub fn label(self) -> &'static str {
                match self { $($name::$variant => $label),+ }
            }
            pub fn from_value(value: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|choice| choice.value() == value)
            }
        }
    };
}

// Light, dark, or whatever the system says.
choice_enum!(Theme, default System, { System => ("system", crate::msg!("System")), Light => ("light", crate::msg!("Light")), Dark => ("dark", crate::msg!("Dark")) });
// One font for the interface, the chat and code.
choice_enum!(FontChoice, default System, { System => ("system", crate::msg!("System")), OpenDyslexic => ("opendyslexic", crate::msg!("OpenDyslexic")) });
choice_enum!(TextSize, default Normal, { Small => ("small", crate::msg!("Small")), Normal => ("normal", crate::msg!("Normal|size")), Large => ("large", crate::msg!("Large")) });
choice_enum!(ChatWidth, default Normal, { Narrow => ("narrow", crate::msg!("Narrow")), Normal => ("normal", crate::msg!("Normal|width")), Wide => ("wide", crate::msg!("Wide")) });
choice_enum!(MotionPref, default System, { System => ("system", crate::msg!("System")), Reduced => ("reduced", crate::msg!("Reduced")) });
// The language of the interface. Each language is named in its own language and never translated.
choice_enum!(Language, default English, { English => ("en", "English"), Korean => ("ko", "한국어") });

impl Language {
    /// The language for a system locale tag such as `ko-KR` or `ko_KR.UTF-8`: Korean when the
    /// language code is `ko`, English for everything else (and for text that is not a tag).
    pub fn from_locale(tag: &str) -> Language {
        let code = tag.trim().split(|c: char| matches!(c, '-' | '_' | '.' | '@')).next().unwrap_or("");
        if code.eq_ignore_ascii_case("ko") {
            Language::Korean
        } else {
            Language::English
        }
    }
}

/// How Forge looks. Every field falls back to its default for a missing or unknown value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Appearance {
    #[serde(default, deserialize_with = "lenient")]
    pub theme: Theme,
    #[serde(default, deserialize_with = "lenient")]
    pub font: FontChoice,
    #[serde(default, deserialize_with = "lenient")]
    pub text_size: TextSize,
    #[serde(default, deserialize_with = "lenient")]
    pub chat_width: ChatWidth,
    #[serde(default, deserialize_with = "lenient")]
    pub motion: MotionPref,
}

/// Which notifications Forge sends. All on until the person turns one off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationPrefs {
    #[serde(default = "default_true")]
    pub reply_done: bool,
    #[serde(default = "default_true")]
    pub scheduled_task: bool,
    #[serde(default = "default_true")]
    pub code_notice: bool,
    #[serde(default = "default_true")]
    pub permission_request: bool,
}

impl Default for NotificationPrefs {
    fn default() -> Self {
        Self { reply_done: true, scheduled_task: true, code_notice: true, permission_request: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    /// The model new sessions start with; `None` lets the SDK choose.
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub enabled_skills: Vec<String>,
    #[serde(default)]
    pub enabled_plugins: Vec<String>,
    #[serde(default)]
    pub enabled_mcp_servers: Vec<String>,
    #[serde(default)]
    pub sidebar: SidebarPrefs,
    #[serde(default)]
    pub editor: EditorPrefs,
    #[serde(default)]
    pub changes_view: ChangesViewPrefs,
    #[serde(default)]
    pub archived: Vec<ArchivedRef>,
    #[serde(default)]
    pub changes_modes: Vec<ChangeModeRef>,
    #[serde(default)]
    pub routines: Vec<crate::Routine>,
    #[serde(default)]
    pub routine_runs: Vec<crate::RoutineRun>,
    /// The transcript view a session opens in until the person picks one for it.
    #[serde(default, deserialize_with = "lenient")]
    pub default_transcript_view: crate::TranscriptView,
    /// The output style new sessions start with; `None` is the SDK's own default.
    #[serde(default, deserialize_with = "lenient")]
    pub output_style: Option<crate::OutputStyle>,
    /// Whether Claude predicts the next prompt after each answer.
    #[serde(default = "default_true")]
    pub prompt_suggestions: bool,
    /// Whether the bypass-permissions mode may be chosen at all.
    #[serde(default)]
    pub allow_bypass_mode: bool,
    /// Whether Claude may run several agents in parallel for a complex task (the SDK's Workflow tool).
    #[serde(default)]
    pub dynamic_workflows: bool,
    /// Whether Claude gets the `update_plan` and `wrap_up` tools: a task list while it works and a
    /// next-step card at the end of a turn.
    #[serde(default = "default_true")]
    pub turn_tools: bool,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub notifications: NotificationPrefs,
    /// Bounce the Dock icon when a notification says Claude needs the person.
    #[serde(default)]
    pub attention_bounce: bool,
    /// The language the person chose; `None` follows the system language.
    #[serde(default, deserialize_with = "lenient")]
    pub language: Option<Language>,
    /// Folders the person trusts: a folder on the list, and every folder below it, is added to a
    /// session without a question. Absolute paths, as `forge_domain::add_trusted` keeps them.
    #[serde(default, deserialize_with = "lenient")]
    pub trusted_folders: Vec<std::path::PathBuf>,
    /// The worktrees Forge made for sessions.
    #[serde(default, deserialize_with = "lenient")]
    pub worktrees: Vec<crate::WorktreeRecord>,
    /// Where new worktrees are kept.
    #[serde(default, deserialize_with = "lenient")]
    pub worktree_location: crate::WorktreeLocation,
    /// Sessions with no activity for this many days are archived; `None` is off.
    #[serde(default, deserialize_with = "lenient")]
    pub auto_archive_days: Option<u32>,
    /// Saved environments of the new-session screen.
    #[serde(default, deserialize_with = "lenient_entries")]
    pub environments: Vec<crate::Environment>,
    /// Sessions moved to another environment by `폴더 변경…`.
    #[serde(default, deserialize_with = "lenient_entries")]
    pub session_environments: Vec<crate::SessionEnvironmentRef>,
}

fn default_true() -> bool {
    true
}

// A value this Forge does not know (a hand edit, or a newer Forge) reads as the default, so the
// rest of the config still loads.
fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

// Like `lenient`, but one entry that does not parse is skipped and the others stay, so a hand edit
// cannot empty the list (the next save would write the empty list back).
fn lenient_entries<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let serde_json::Value::Array(entries) = value else { return Ok(Vec::new()) };
    Ok(entries.into_iter().filter_map(|entry| serde_json::from_value(entry).ok()).collect())
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            api_key: None,
            model: None,
            enabled_skills: Vec::new(),
            enabled_plugins: Vec::new(),
            enabled_mcp_servers: Vec::new(),
            sidebar: SidebarPrefs::default(),
            editor: EditorPrefs::default(),
            changes_view: ChangesViewPrefs::default(),
            archived: Vec::new(),
            changes_modes: Vec::new(),
            routines: Vec::new(),
            routine_runs: Vec::new(),
            default_transcript_view: crate::TranscriptView::default(),
            output_style: None,
            prompt_suggestions: true,
            allow_bypass_mode: false,
            dynamic_workflows: false,
            turn_tools: true,
            appearance: Appearance::default(),
            notifications: NotificationPrefs::default(),
            attention_bounce: false,
            language: None,
            trusted_folders: Vec::new(),
            worktrees: Vec::new(),
            worktree_location: crate::WorktreeLocation::Project,
            auto_archive_days: None,
            environments: Vec::new(),
            session_environments: Vec::new(),
        }
    }
}

/// What a settings text field is stored as: trimmed, and `None` when nothing is left.
pub fn normalize_setting(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_config_has_no_saved_modes() {
        let config: AppConfig = serde_json::from_str("{}").unwrap();
        assert!(config.changes_modes.is_empty());
    }

    #[test]
    fn saved_modes_survive_a_round_trip() {
        let config = AppConfig {
            changes_modes: vec![
                ChangeModeRef { project: "/work/a".into(), session_id: "s1".to_string(), view: SavedChangeView::Commit("abc".to_string()), base: Some("dev".to_string()) },
                ChangeModeRef { project: "/work/a".into(), session_id: "s2".to_string(), view: SavedChangeView::Uncommitted, base: None },
            ],
            ..AppConfig::default()
        };
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.changes_modes, config.changes_modes);
    }

    #[test]
    fn a_config_without_trusted_folders_has_an_empty_list() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":"https://x","api_key":null}"#).unwrap();
        assert!(config.trusted_folders.is_empty());
    }

    #[test]
    fn trusted_folders_are_stored_and_read_back() {
        let config = AppConfig { trusted_folders: vec!["/work/app".into(), "/other".into()], ..AppConfig::default() };
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.trusted_folders, config.trusted_folders);
    }

    #[test]
    fn a_bad_trusted_folders_value_reads_as_empty_and_the_rest_still_loads() {
        for text in [r#""trusted_folders": null"#, r#""trusted_folders": "/work""#, r#""trusted_folders": ["/work", 7]"#, r#""trusted_folders": 7"#] {
            let config: AppConfig = serde_json::from_str(&format!(r#"{{"base_url":"https://x",{text}}}"#)).unwrap();
            assert!(config.trusted_folders.is_empty(), "{text}");
            assert_eq!(config.base_url.as_deref(), Some("https://x"), "{text}");
        }
    }

    #[test]
    fn a_config_without_a_language_follows_the_system() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":"https://x","api_key":null}"#).unwrap();
        assert_eq!(config.language, None);
        assert_eq!(config.base_url.as_deref(), Some("https://x"));
    }

    #[test]
    fn a_null_or_unknown_language_reads_as_none_and_the_rest_still_loads() {
        for text in [r#""language": null"#, r#""language": "klingon""#, r#""language": 7"#] {
            let config: AppConfig = serde_json::from_str(&format!(r#"{{"base_url":"https://x",{text}}}"#)).unwrap();
            assert_eq!(config.language, None, "{text}");
            assert_eq!(config.base_url.as_deref(), Some("https://x"), "{text}");
        }
    }

    #[test]
    fn a_chosen_language_is_stored_and_read_back() {
        let config = AppConfig { language: Some(Language::Korean), ..AppConfig::default() };
        let text = serde_json::to_string(&config).unwrap();
        assert!(text.contains(r#""language":"korean""#), "{text}");
        let back: AppConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(back.language, Some(Language::Korean));
    }

    #[test]
    fn the_system_language_is_korean_only_for_the_language_code_ko() {
        for tag in ["ko", "ko-KR", "ko_KR", "ko_KR.UTF-8", "KO", "ko-Kore-KR", " ko-KR "] {
            assert_eq!(Language::from_locale(tag), Language::Korean, "{tag}");
        }
        for tag in ["en-US", "en", "ja-JP", "kok-IN", "", "   ", "😀", "korean"] {
            assert_eq!(Language::from_locale(tag), Language::English, "{tag}");
        }
    }

    #[test]
    fn languages_are_named_in_their_own_language() {
        assert_eq!(Language::English.label(), "English");
        assert_eq!(Language::Korean.label(), "한국어");
        assert_eq!(Language::from_value("ko"), Some(Language::Korean));
    }

    #[test]
    fn an_old_config_loads_with_todays_look_and_all_notifications_on() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();
        assert_eq!(config.appearance, Appearance::default());
        assert_eq!(config.appearance.theme, Theme::System);
        assert_eq!(config.appearance.font, FontChoice::System);
        assert_eq!(config.appearance.text_size, TextSize::Normal);
        assert_eq!(config.appearance.chat_width, ChatWidth::Normal);
        assert_eq!(config.appearance.motion, MotionPref::System);
        assert!(config.notifications.reply_done && config.notifications.scheduled_task);
        assert!(config.notifications.code_notice && config.notifications.permission_request);
        assert!(!config.attention_bounce);
    }

    #[test]
    fn an_unknown_appearance_value_reads_as_the_default_and_the_rest_still_loads() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":"https://x","appearance":{"theme":"purple","text_size":"large"}}"#).unwrap();
        assert_eq!(config.base_url.as_deref(), Some("https://x"));
        assert_eq!(config.appearance.theme, Theme::System);
        assert_eq!(config.appearance.text_size, TextSize::Large);
    }

    #[test]
    fn appearance_and_notifications_survive_a_round_trip() {
        let mut config = AppConfig::default();
        config.appearance = Appearance { theme: Theme::Dark, font: FontChoice::OpenDyslexic, text_size: TextSize::Small, chat_width: ChatWidth::Wide, motion: MotionPref::Reduced };
        config.notifications.reply_done = false;
        config.attention_bounce = true;
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.appearance, config.appearance);
        assert!(!back.notifications.reply_done);
        assert!(back.attention_bounce);
    }

    #[test]
    fn a_choice_is_found_again_by_its_value() {
        for theme in Theme::ALL {
            assert_eq!(Theme::from_value(theme.value()), Some(*theme));
        }
        assert_eq!(Theme::from_value("nope"), None);
    }

    #[test]
    fn an_old_config_without_sidebar_preferences_loads_with_the_defaults() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();

        assert_eq!(config.sidebar, SidebarPrefs::default());
        assert_eq!(config.sidebar.sort_by, SortBy::Activity);
        assert_eq!(config.sidebar.group_by, GroupBy::Folder);
        assert!(!config.sidebar.show_empty);
        assert!(!config.sidebar.hidden);
        assert_eq!(config.sidebar.width, DEFAULT_SIDEBAR_WIDTH);
    }

    #[test]
    fn an_old_config_without_routines_loads_with_none() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();
        assert!(config.routines.is_empty());
        assert!(config.routine_runs.is_empty());
    }

    #[test]
    fn sidebar_preferences_survive_a_round_trip() {
        let prefs = SidebarPrefs {
            sort_by: SortBy::Name,
            group_by: GroupBy::Date,
            status: StatusFilter::All,
            show_empty: true,
            collapsed: vec!["folder:/work/a".to_string()],
            width: 300,
            hidden: true,
        };
        let config = AppConfig { sidebar: prefs.clone(), ..AppConfig::default() };

        let json = serde_json::to_string(&config).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(back.sidebar, prefs);
    }

    #[test]
    fn an_unknown_sort_or_group_value_falls_back_instead_of_breaking_the_config() {
        let config: AppConfig =
            serde_json::from_str(r#"{"sidebar":{"sort_by":"relevance","group_by":"project"}}"#).unwrap();

        assert_eq!(config.sidebar.sort_by, SortBy::Activity);
        assert_eq!(config.sidebar.group_by, GroupBy::Folder);
    }

    #[test]
    fn a_stored_width_outside_the_range_is_clamped_for_use() {
        let narrow = SidebarPrefs { width: 0, ..SidebarPrefs::default() };
        let wide = SidebarPrefs { width: 9999, ..SidebarPrefs::default() };

        assert_eq!(narrow.clamped_width(), MIN_SIDEBAR_WIDTH);
        assert_eq!(wide.clamped_width(), MAX_SIDEBAR_WIDTH);
        assert_eq!(SidebarPrefs::default().clamped_width(), DEFAULT_SIDEBAR_WIDTH);
    }

    #[test]
    fn toggling_a_group_collapses_and_expands_it() {
        let mut prefs = SidebarPrefs::default();

        prefs.toggle_collapsed("folder:/a");
        assert!(prefs.is_collapsed("folder:/a"));
        assert!(!prefs.is_collapsed("folder:/b"));

        prefs.toggle_collapsed("folder:/a");
        assert!(!prefs.is_collapsed("folder:/a"));
        assert!(prefs.collapsed.is_empty());
    }

    #[test]
    fn app_config_round_trips_through_json() {
        let config = AppConfig {
            base_url: Some("https://api.anthropic.com".to_string()),
            api_key: Some("sk-ant-test".to_string()),
            model: None,
            enabled_skills: vec!["brainstorming".to_string()],
            enabled_plugins: vec![],
            enabled_mcp_servers: vec!["playwright".to_string()],
            ..AppConfig::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, back);
    }

    #[test]
    fn app_config_defaults_to_empty_lists_and_no_credentials() {
        let config = AppConfig::default();
        assert_eq!(config.base_url, None);
        assert_eq!(config.api_key, None);
        assert!(config.enabled_skills.is_empty());
        assert!(config.enabled_plugins.is_empty());
        assert!(config.enabled_mcp_servers.is_empty());
    }

    #[test]
    fn app_config_deserializes_from_json_missing_optional_lists() {
        let json = r#"{"base_url": null, "api_key": null}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled_skills.is_empty());
    }

    #[test]
    fn a_config_file_without_a_model_loads_with_none() {
        let json = r#"{"base_url": null, "api_key": null, "enabled_skills": ["a"]}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.model, None);
        assert_eq!(config.enabled_skills, vec!["a".to_string()]);
    }

    #[test]
    fn the_model_round_trips_through_json() {
        let config = AppConfig { model: Some("claude-sonnet-5-5".to_string()), ..AppConfig::default() };
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.model.as_deref(), Some("claude-sonnet-5-5"));
    }

    #[test]
    fn normalize_setting_trims_and_turns_blank_into_none() {
        assert_eq!(normalize_setting(""), None);
        assert_eq!(normalize_setting("   \n"), None);
        assert_eq!(normalize_setting(" sk-ant-1 \n"), Some("sk-ant-1".to_string()));
        assert_eq!(normalize_setting("https://x.test"), Some("https://x.test".to_string()));
    }

    #[test]
    fn an_old_config_loads_with_nothing_archived_and_the_active_filter() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();

        assert!(config.archived.is_empty());
        assert_eq!(config.sidebar.status, StatusFilter::Active);
    }

    #[test]
    fn a_sidebar_saved_before_the_status_filter_existed_still_loads() {
        let config: AppConfig =
            serde_json::from_str(r#"{"sidebar":{"sort_by":"name","group_by":"date"}}"#).unwrap();

        assert_eq!(config.sidebar.sort_by, SortBy::Name);
        assert_eq!(config.sidebar.status, StatusFilter::Active);
    }

    #[test]
    fn archived_sessions_and_the_status_filter_survive_a_round_trip() {
        let config = AppConfig {
            archived: vec![ArchivedRef { project: "/work/a".into(), session_id: "s1".to_string() }],
            sidebar: SidebarPrefs { status: StatusFilter::Archived, ..SidebarPrefs::default() },
            ..AppConfig::default()
        };

        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();

        assert_eq!(back, config);
    }

    #[test]
    fn an_unknown_status_value_reads_as_active() {
        let config: AppConfig = serde_json::from_str(r#"{"sidebar":{"status":"deleted"}}"#).unwrap();

        assert_eq!(config.sidebar.status, StatusFilter::Active);
    }

    #[test]
    fn an_old_config_without_editor_preferences_loads_with_the_defaults() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();

        assert_eq!(config.editor, EditorPrefs::default());
        let editor = &config.editor;
        assert!(editor.tree && editor.preview_tabs && editor.auto_save && editor.tab_indent && editor.wrap && editor.indent_guides);
        assert!(!editor.whitespace && !editor.scroll_past_end);
        assert_eq!(editor.tab_size, DEFAULT_TAB_SIZE);
    }

    #[test]
    fn editor_preferences_survive_a_round_trip() {
        let prefs = EditorPrefs { tree: false, auto_save: false, wrap: false, whitespace: true, tab_size: 8, ..EditorPrefs::default() };
        let config = AppConfig { editor: prefs.clone(), ..AppConfig::default() };

        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();

        assert_eq!(back.editor, prefs);
    }

    #[test]
    fn a_config_that_names_only_some_editor_fields_keeps_the_defaults_for_the_rest() {
        let config: AppConfig = serde_json::from_str(r#"{"editor":{"wrap":false}}"#).unwrap();

        assert!(!config.editor.wrap);
        assert!(config.editor.tree);
        assert_eq!(config.editor.tab_size, 4);
    }

    #[test]
    fn a_tab_size_other_than_2_4_or_8_is_used_as_4() {
        for stored in [0u8, 1, 3, 5, 16, 255] {
            assert_eq!(EditorPrefs { tab_size: stored, ..EditorPrefs::default() }.clamped_tab_size(), 4, "stored {stored}");
        }
        for stored in [2u8, 4, 8] {
            assert_eq!(EditorPrefs { tab_size: stored, ..EditorPrefs::default() }.clamped_tab_size(), stored);
        }
    }

    #[test]
    fn changes_view_defaults_and_round_trips() {
        let config: AppConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(
            config.changes_view,
            ChangesViewPrefs {
                group_by_folder: true,
                side_by_side: false,
                wrap: true,
                highlight_words: true,
                hide_whitespace: false,
                file_list_position: FileListPosition::Auto,
                separate_special: false,
            }
        );
        let prefs = ChangesViewPrefs {
            side_by_side: true,
            hide_whitespace: true,
            file_list_position: FileListPosition::Left,
            separate_special: true,
            ..ChangesViewPrefs::default()
        };
        let config = AppConfig { changes_view: prefs.clone(), ..AppConfig::default() };
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.changes_view, prefs);
    }

    #[test]
    fn the_file_list_position_is_written_in_snake_case() {
        let config: AppConfig = serde_json::from_str(r#"{"changes_view":{"file_list_position":"right"}}"#).unwrap();
        assert_eq!(config.changes_view.file_list_position, FileListPosition::Right);
        assert!(!config.changes_view.separate_special);
    }

    #[test]
    fn a_partial_changes_view_keeps_the_defaults_for_the_rest() {
        let config: AppConfig = serde_json::from_str(r#"{"changes_view":{"wrap":false}}"#).unwrap();
        assert!(!config.changes_view.wrap);
        assert!(config.changes_view.group_by_folder);
    }

    #[test]
    fn an_old_config_loads_with_the_code_tab_defaults() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();
        assert_eq!(config.default_transcript_view, crate::TranscriptView::Normal);
        assert_eq!(config.output_style, None);
        assert!(config.prompt_suggestions, "suggestions stay on for a config written before the switch existed");
        assert!(!config.allow_bypass_mode);
        assert!(!config.dynamic_workflows, "workflows stay off for a config written before the switch existed");
        assert!(AppConfig::default().prompt_suggestions, "the manual Default agrees with the file default");
    }

    #[test]
    fn turn_tools_are_on_for_a_config_written_before_the_switch_existed() {
        let config: AppConfig = serde_json::from_str("{}").unwrap();
        assert!(config.turn_tools);
        assert!(AppConfig::default().turn_tools);
        let off: AppConfig = serde_json::from_str(r#"{"turn_tools": false}"#).unwrap();
        assert!(!off.turn_tools);
    }

    #[test]
    fn the_code_tab_values_round_trip() {
        let config = AppConfig {
            default_transcript_view: crate::TranscriptView::Detailed,
            output_style: Some(crate::OutputStyle::Concise),
            prompt_suggestions: false,
            allow_bypass_mode: true,
            dynamic_workflows: true,
            turn_tools: true,
            ..AppConfig::default()
        };
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back, config);
    }

    #[test]
    fn an_unknown_style_or_view_in_the_file_does_not_stop_the_config_loading() {
        let config: AppConfig =
            serde_json::from_str(r#"{"base_url":null,"api_key":null,"output_style":"poetic","default_transcript_view":"loud"}"#).unwrap();
        assert_eq!(config.output_style, None);
        assert_eq!(config.default_transcript_view, crate::TranscriptView::Normal);
    }

    #[test]
    fn a_config_from_an_older_forge_loads_with_no_worktrees_and_no_auto_archive() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();
        assert!(config.worktrees.is_empty());
        assert_eq!(config.worktree_location, crate::WorktreeLocation::Project);
        assert_eq!(config.auto_archive_days, None);
    }

    #[test]
    fn a_broken_worktree_setting_reads_as_the_default_and_keeps_the_rest() {
        let text = r#"{"base_url":"https://x.test","api_key":null,"worktree_location":{"kind":"nonsense"},"worktrees":"oops","auto_archive_days":"soon"}"#;
        let config: AppConfig = serde_json::from_str(text).unwrap();
        assert_eq!(config.base_url.as_deref(), Some("https://x.test"));
        assert_eq!(config.worktree_location, crate::WorktreeLocation::Project);
        assert!(config.worktrees.is_empty());
        assert_eq!(config.auto_archive_days, None);
    }

    #[test]
    fn worktree_records_and_the_settings_are_saved_and_read_back() {
        let config = AppConfig {
            worktrees: vec![crate::WorktreeRecord {
                name: "x-1".to_string(),
                path: "/w/.claude/worktrees/x-1".into(),
                repo: "/w".into(),
                branch: "claude/x-1".to_string(),
                base_branch: "master".to_string(),
                created_at: 5,
            }],
            worktree_location: crate::WorktreeLocation::Folder("/trees".into()),
            auto_archive_days: Some(7),
            ..AppConfig::default()
        };
        let text = serde_json::to_string(&config).unwrap();
        assert_eq!(serde_json::from_str::<AppConfig>(&text).unwrap(), config);
    }

    #[test]
    fn a_config_from_an_older_forge_has_no_environments_and_a_broken_value_reads_as_none() {
        let old: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();
        assert!(old.environments.is_empty());
        let broken: AppConfig = serde_json::from_str(r#"{"base_url":"https://x.test","api_key":null,"environments":"oops"}"#).unwrap();
        assert_eq!(broken.base_url.as_deref(), Some("https://x.test"));
        assert!(broken.environments.is_empty());
    }

    #[test]
    fn the_config_debug_output_hides_environment_variable_values() {
        let config = AppConfig {
            environments: vec![crate::Environment {
                id: "env-1".into(),
                name: "work".into(),
                folders: Vec::new(),
                branch: None,
                worktree: false,
                env_vars: "TOKEN=s3cret".into(),
                pinned: false,
                created_at: 1,
                last_used_at: 2,
                model: None,
                effort: None,
                extra_choices: Vec::new(),
            }],
            ..AppConfig::default()
        };
        assert!(!format!("{config:?}").contains("s3cret"));
    }

    #[test]
    fn old_configs_without_session_environments_load_with_none() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":"https://x.test","api_key":null}"#).unwrap();
        assert!(config.session_environments.is_empty());
    }

    #[test]
    fn a_bad_session_environments_entry_is_dropped_and_the_rest_kept() {
        let text = r#"{"base_url":null,"api_key":null,"session_environments":[{"project":"/a","session_id":"s","environment_id":"B"},{"project":"/a"},"junk"]}"#;
        let config: AppConfig = serde_json::from_str(text).unwrap();
        assert_eq!(config.session_environments.len(), 1);
        assert_eq!(config.session_environments[0].environment_id, "B");
    }

    #[test]
    fn one_bad_environment_entry_is_skipped_and_the_good_one_stays() {
        let text = r#"{"base_url":null,"api_key":null,"environments":[{"id":"a","name":"app"},{"name":"no id"},"junk"]}"#;
        let config: AppConfig = serde_json::from_str(text).unwrap();
        assert_eq!(config.environments.len(), 1);
        assert_eq!(config.environments[0].id, "a");
    }

    #[test]
    fn environments_that_are_not_a_list_read_as_empty_and_the_rest_loads() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":"https://x.test","api_key":null,"environments":{"id":"a"}}"#).unwrap();
        assert!(config.environments.is_empty());
        assert_eq!(config.base_url.as_deref(), Some("https://x.test"));
    }

    #[test]
    fn environments_are_saved_and_read_back() {
        let config = AppConfig {
            environments: vec![crate::Environment {
                id: "env-1".into(),
                name: "work".into(),
                folders: vec!["/a".into(), "/b".into()],
                branch: Some("dev".into()),
                worktree: true,
                env_vars: "A=1".into(),
                pinned: true,
                created_at: 5,
                last_used_at: 9,
                model: None,
                effort: None,
                extra_choices: vec![crate::FolderChoice { branch: Some("x".into()), worktree: true }],
            }],
            ..AppConfig::default()
        };
        let text = serde_json::to_string(&config).unwrap();
        assert_eq!(serde_json::from_str::<AppConfig>(&text).unwrap(), config);
    }
}
