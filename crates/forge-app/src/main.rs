use std::sync::Arc;

use dioxus::desktop::muda::{Menu, PredefinedMenuItem, Submenu};
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use forge_adapter_claude_fs::{
    default_claude_home, default_claude_json_path, ClaudeMcpAdapter, ClaudePluginAdapter, ClaudeProjectAdapter,
    ClaudeSessionAdapter, ClaudeSkillAdapter,
};
use forge_adapter_dialog::RfdFolderPicker;
use forge_adapter_config::{default_config_path, ConfigFileAdapter};
use forge_adapter_sidecar::SidecarProcessFactory;
use forge_application::{ExtensionsUseCase, SessionRegistry, SettingsUseCase};
use forge_features::WorkspaceServices;
use forge_pages::WorkspacePage;
use forge_shared::THEME_CSS;

// Development default: the sidecar built in this repository (`npm run build` in `sidecar/`).
// Locating a bundled sidecar is packaging's job (spec §9), so `FORGE_SIDECAR_SCRIPT` overrides it.
const DEFAULT_SIDECAR_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sidecar/dist/index.js");

fn main() {
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
                .with_inner_size(LogicalSize::new(1200.0, 800.0)),
        )
        .with_menu(app_menu())
}

// The composition root: the only place that names concrete adapters.
fn build_services() -> WorkspaceServices {
    let claude_home = default_claude_home().expect("could not resolve the home directory");
    let config_path = default_config_path().expect("could not resolve the config directory");
    let node = std::env::var("FORGE_NODE").unwrap_or_else(|_| "node".to_string());
    let script = std::env::var("FORGE_SIDECAR_SCRIPT").unwrap_or_else(|_| DEFAULT_SIDECAR_SCRIPT.to_string());

    let config = Arc::new(ConfigFileAdapter::new(config_path));
    let factory = Arc::new(SidecarProcessFactory::new(node, script, config.clone()));
    let settings = Arc::new(SettingsUseCase::new(config.clone()));
    let claude_json = default_claude_json_path().expect("could not resolve the home directory");
    // With no project open the tabs read the home directory as the project, so only the global
    // definitions (`~/.claude/skills`, the global MCP servers) show.
    let home = claude_home.parent().map(std::path::Path::to_path_buf).unwrap_or_else(|| claude_home.clone());
    let extensions = Arc::new(ExtensionsUseCase::new(
        config.clone(),
        Arc::new(ClaudeSkillAdapter::new(claude_home.clone())),
        Arc::new(ClaudePluginAdapter::new(claude_home.clone())),
        Arc::new(ClaudeMcpAdapter::new(claude_json)),
        home,
    ));
    let sessions = Arc::new(ClaudeSessionAdapter::new(claude_home.clone()));
    let projects = Arc::new(ClaudeProjectAdapter::new(claude_home));
    let registry = Arc::new(SessionRegistry::new(factory, sessions.clone(), extensions.clone()));
    let folder_picker = Arc::new(RfdFolderPicker::new());
    WorkspaceServices { registry, projects, sessions, settings, extensions, folder_picker }
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
