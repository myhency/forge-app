use std::sync::Arc;

mod runtime;

use dioxus::desktop::muda::{Menu, PredefinedMenuItem, Submenu};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use forge_adapter_claude_fs::{
    default_claude_home, default_claude_json_path, ClaudeMcpAdapter, ClaudePluginAdapter, ClaudeProjectAdapter,
    ClaudeSessionAdapter, ClaudeSkillAdapter, ClaudeSkillCatalogAdapter, ClaudeUsageStatsAdapter,
};
use forge_adapter_claude_plugins::{claude_binary_for, ClaudePluginCli};
use forge_adapter_dialog::RfdFolderPicker;
use forge_adapter_config::{default_config_path, ConfigFileAdapter};
use forge_adapter_sidecar::SidecarProcessFactory;
use forge_adapter_browser::SystemBrowser;
use forge_adapter_files::LocalFiles;
use forge_adapter_git::GitCli;
#[cfg(not(windows))]
use forge_adapter_sleep::CaffeinateSleep;
#[cfg(windows)]
use forge_adapter_sleep::WindowsSleep;
use forge_adapter_terminal::PtyTerminal;
use forge_application::{ConfigLock, ExtensionsUseCase, PluginCatalogUseCase, SessionRegistry, SettingsUseCase, SkillCatalogUseCase};
use forge_features::WorkspaceServices;
use forge_pages::WorkspacePage;
use forge_shared::THEME_CSS;

// Development default: the sidecar built in this repository (`npm run build` in `sidecar/`).
// Locating a bundled sidecar is packaging's job (spec §9), so `FORGE_SIDECAR_SCRIPT` overrides it.
const DEFAULT_SIDECAR_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sidecar/dist/index.js");

fn main() {
    // The sidebar cuts "Today" and "Yesterday" in the person's own time zone. The offset can only be
    // read soundly while this is still the only thread, so it is read first of all.
    if let Ok(offset) = time::UtcOffset::current_local_offset() {
        forge_features::set_local_offset(offset);
    }
    dioxus::LaunchBuilder::desktop()
        .with_cfg(window_config())
        .with_context(build_services())
        .launch(App);
}

// Amendment A (spec §15.10 (c)): Dioxus's default menu binds the platform's
// standard accelerator (⌘W on macOS) to `close_window`, which closes the OS
// window independently of `onkeydown` — Cmd+N and Ctrl+Tab/Ctrl+Shift+Tab
// reach the page fine, but ⌘W does both, closing the window Workspace::on_key
// was supposed to be the only thing deciding. This is the platform's default
// menu (`dioxus_desktop::menubar::default_menu_bar`) with `close_window`
// left out, so ⌘W is decided solely by `Workspace::on_key` (close the active
// session, never the window); the traffic-light button and ⌘Q are unaffected.
fn app_menu() -> Menu {
    let menu = Menu::new();
    let window_menu = Submenu::new("Window", true);
    window_menu
        .append_items(&[
            &PredefinedMenuItem::fullscreen(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::maximize(None),
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ])
        .expect("static menu items");

    let edit_menu = Submenu::new("Edit", true);
    edit_menu
        .append_items(&[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::select_all(None),
        ])
        .expect("static menu items");

    menu.append_items(&[&window_menu, &edit_menu])
        .expect("static menu items");

    #[cfg(target_os = "macos")]
    window_menu.set_as_windows_menu_for_nsapp();

    menu
}

fn window_config() -> Config {
    Config::new()
        .with_window(
            WindowBuilder::new()
                .with_title("Forge")
                .with_inner_size(LogicalSize::new(1200.0, 800.0))
                // The real Claude window refuses to go below 600 pt wide.
                .with_min_inner_size(LogicalSize::new(600.0, 400.0)),
        )
        .with_menu(app_menu())
        .with_custom_head(format!("{}{}", forge_shared::xterm_head(), forge_shared::editor_head()))
}

// Where Forge reads Claude's files. `FORGE_CLAUDE_HOME` points it at another Claude home (a test
// fixture) so the app can be run against known data without touching the real `~/.claude`. The
// matching `.claude.json` is the sibling of that directory, as it is for the real one. Forge only
// reads from it here; a session started from the app still runs on the real Claude.
fn claude_paths(
    override_home: Option<std::ffi::OsString>,
    default_home: Option<std::path::PathBuf>,
    default_json: Option<std::path::PathBuf>,
) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    match override_home.filter(|home| !home.is_empty()) {
        Some(home) => {
            let home = std::path::PathBuf::from(home);
            let json = home.with_file_name(".claude.json");
            Some((home, json))
        }
        None => Some((default_home?, default_json?)),
    }
}

// Where Forge keeps its own settings (base URL, API key, model, enabled extensions).
// `FORGE_CONFIG_PATH` points it at another file so a test run never reads or overwrites the real one.
fn config_path(
    override_path: Option<std::ffi::OsString>,
    default_path: Option<std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    match override_path.filter(|path| !path.is_empty()) {
        Some(path) => Some(std::path::PathBuf::from(path)),
        None => default_path,
    }
}

// The composition root: the only place that names concrete adapters.
fn build_services() -> WorkspaceServices {
    let (claude_home, claude_json) = claude_paths(
        std::env::var_os("FORGE_CLAUDE_HOME"),
        default_claude_home(),
        default_claude_json_path(),
    )
    .expect("could not resolve the home directory");
    let config_path = config_path(std::env::var_os("FORGE_CONFIG_PATH"), default_config_path())
        .expect("could not resolve the config directory");
    let runtime::Runtime { node, script } = runtime::locate_here(DEFAULT_SIDECAR_SCRIPT);

    let config = Arc::new(ConfigFileAdapter::new(config_path));
    // Every use case that loads, edits and saves the Forge config holds this one lock while it does.
    let config_lock = ConfigLock::default();
    // The Claude program that ships with the SDK sits beside the sidecar script. `CLAUDE_CONFIG_DIR` is
    // set for it only when `FORGE_CLAUDE_HOME` moves the Claude home (the harness); with the real home
    // the CLI would keep `.claude.json` inside the folder instead of beside it.
    let plugin_binary = claude_binary_for(std::path::Path::new(&script)).unwrap_or_else(|| format!("no Claude program for {}-{}", std::env::consts::OS, std::env::consts::ARCH).into());
    let plugin_config_dir = std::env::var_os("FORGE_CLAUDE_HOME").filter(|home| !home.is_empty()).map(std::path::PathBuf::from);
    let plugin_catalog = Arc::new(PluginCatalogUseCase::new(
        Arc::new(ClaudePluginCli::new(plugin_binary, claude_home.clone(), plugin_config_dir)),
        config.clone(),
    ).with_config_lock(config_lock.clone()));
    let skill_catalog = Arc::new(
        SkillCatalogUseCase::new(Arc::new(ClaudeSkillCatalogAdapter::new(claude_home.clone())), config.clone())
            .with_config_lock(config_lock.clone()),
    );
    let factory = Arc::new(SidecarProcessFactory::new(node, script, config.clone()));
    let settings = Arc::new(SettingsUseCase::new(config.clone()).with_config_lock(config_lock.clone()));
    // With no project open the tabs read the home directory as the project, so only the global
    // definitions (`~/.claude/skills`, the global MCP servers) show.
    let home = claude_home.parent().map(std::path::Path::to_path_buf).unwrap_or_else(|| claude_home.clone());
    let extensions = Arc::new(ExtensionsUseCase::new(
        config.clone(),
        Arc::new(ClaudeSkillAdapter::new(claude_home.clone())),
        Arc::new(ClaudePluginAdapter::new(claude_home.clone())),
        Arc::new(ClaudeMcpAdapter::new(claude_json)),
        home,
    ).with_config_lock(config_lock.clone()));
    let sessions = Arc::new(ClaudeSessionAdapter::new(claude_home.clone()));
    let projects = Arc::new(ClaudeProjectAdapter::new(claude_home.clone()));
    let registry = Arc::new(SessionRegistry::new(factory, sessions.clone(), extensions.clone()).with_warm_up());
    let picker = Arc::new(RfdFolderPicker::new());
    let shell = default_shell(std::env::var("SHELL").ok());
    WorkspaceServices {
        registry,
        projects,
        sessions,
        settings,
        extensions,
        plugin_catalog,
        skill_catalog,
        folder_picker: picker.clone(),
        file_picker: picker,
        terminal: Arc::new(PtyTerminal::new(shell)),
        sleep: keep_awake(),
        git: Arc::new(GitCli::new()),
        files: Arc::new(LocalFiles::new()),
        browser: Arc::new(SystemBrowser::new()),
        usage_stats: Arc::new(ClaudeUsageStatsAdapter::new(claude_home.clone())),
    }
}

// The shell of the terminal panel: the person's own on macOS and Linux; PowerShell, which every
// Windows 11 has, on Windows.
fn default_shell(login_shell: Option<String>) -> String {
    if cfg!(windows) {
        return "powershell.exe".to_string();
    }
    login_shell.filter(|shell| !shell.is_empty()).unwrap_or_else(|| "/bin/zsh".to_string())
}

fn keep_awake() -> Arc<dyn forge_ports::SleepPort> {
    #[cfg(windows)]
    return Arc::new(WindowsSleep::new());
    #[cfg(not(windows))]
    return Arc::new(CaffeinateSleep::new());
}

#[component]
fn App() -> Element {
    rsx! {
        style { "{THEME_CSS}" }
        WorkspacePage {}
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn the_terminal_runs_the_login_shell_or_zsh() {
        assert_eq!(default_shell(Some("/bin/bash".into())), "/bin/bash");
        assert_eq!(default_shell(Some(String::new())), "/bin/zsh");
        assert_eq!(default_shell(None), "/bin/zsh");
    }

    #[cfg(windows)]
    #[test]
    fn the_terminal_runs_powershell_whatever_shell_is_set() {
        assert_eq!(default_shell(Some("C:\\Git\\bin\\bash.exe".into())), "powershell.exe");
        assert_eq!(default_shell(None), "powershell.exe");
    }

    fn paths(override_home: Option<&str>) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
        claude_paths(
            override_home.map(std::ffi::OsString::from),
            Some("/home/me/.claude".into()),
            Some("/home/me/.claude.json".into()),
        )
    }

    #[test]
    fn with_no_override_the_real_claude_paths_are_used() {
        assert_eq!(paths(None), Some(("/home/me/.claude".into(), "/home/me/.claude.json".into())));
    }

    #[test]
    fn an_empty_override_counts_as_none() {
        assert_eq!(paths(Some("")), paths(None));
    }

    #[test]
    fn an_override_moves_the_json_file_beside_it() {
        assert_eq!(
            paths(Some("/tmp/fixture/.claude")),
            Some(("/tmp/fixture/.claude".into(), "/tmp/fixture/.claude.json".into()))
        );
    }

    #[test]
    fn no_default_and_no_override_is_none() {
        assert_eq!(claude_paths(None, None, None), None);
    }

    #[test]
    fn with_no_config_override_the_default_config_path_is_used() {
        assert_eq!(
            config_path(None, Some("/home/me/.config/Forge/config.json".into())),
            Some("/home/me/.config/Forge/config.json".into())
        );
    }

    #[test]
    fn an_empty_config_override_counts_as_none() {
        assert_eq!(
            config_path(Some("".into()), Some("/home/me/.config/Forge/config.json".into())),
            Some("/home/me/.config/Forge/config.json".into())
        );
    }

    #[test]
    fn a_config_override_replaces_the_default() {
        assert_eq!(
            config_path(Some("/tmp/fixture/forge-config/config.json".into()), None),
            Some("/tmp/fixture/forge-config/config.json".into())
        );
    }

    #[test]
    fn no_default_and_no_config_override_is_none() {
        assert_eq!(config_path(None, None), None);
    }

    #[test]
    fn the_default_sidecar_script_points_into_this_repositorys_sidecar_directory() {
        let script = Path::new(DEFAULT_SIDECAR_SCRIPT);
        assert!(script.ends_with("sidecar/dist/index.js"), "{DEFAULT_SIDECAR_SCRIPT}");
        assert!(
            script.parent().unwrap().parent().unwrap().join("src/index.ts").exists(),
            "the sidecar sources should sit beside dist/: {DEFAULT_SIDECAR_SCRIPT}"
        );
    }
}
