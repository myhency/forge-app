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

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
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
    pub archived: Vec<ArchivedRef>,
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
}
