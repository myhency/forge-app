use dioxus::prelude::*;
use forge_domain::{keep_peek, layout_for, workspace_class, SidebarMode, SIDEBAR_RAIL_WIDTH};
use forge_features::{refocus_workspace, use_customize_provider, use_routines_provider, use_settings_provider, use_window_width, use_workspace_provider, Active};
use forge_widgets::{ChatWindow, CustomizePage, RoutinesPage, Lightbox, NewSessionPanel, SearchPalette, SettingsOverlay, ShortcutsDialog, Sidebar};

/// The whole screen: the sidebar on the left and, on the right, either the new-session panel or
/// the active session. The root also catches the session shortcuts.
#[component]
pub fn WorkspacePage() -> Element {
    let settings = use_settings_provider();
    use_customize_provider();
    let workspace = use_workspace_provider();
    use_routines_provider(workspace);
    let mut dragging = use_signal(|| false);
    // Key events only reach the root while something inside it has focus. Whenever the overlay
    // closes or the main pane switches view, the focused input or button may have been removed
    // and focus falls to <body>, which silences every shortcut until the next click. Give focus
    // back to the root — but only if nothing has it, so a composer the person is typing in keeps it.
    use_effect(move || {
        let _ = (settings.is_open(), workspace.search_open(), workspace.shortcuts_open(), workspace.active(), workspace.lightbox().is_some());
        refocus_workspace();
    });

    // The tabs list the definitions of the project the person is looking at.
    use_effect(move || settings.set_project(workspace.context_project()));

    let window_width = use_window_width();
    // Whether the overlay sidebar is out. Not saved: `prefs.hidden` and `prefs.width` belong to the
    // docked sidebar and the overlay never writes them.
    let mut peek = use_signal(|| false);
    // Choosing a session, or opening the new-session screen, puts the overlay away.
    use_effect(move || {
        let _ = workspace.active();
        peek.set(false);
    });
    // Growing past the overlay width, or opening Settings or the search palette, puts it away too.
    use_effect(move || {
        let overlay_open = settings.is_open() || workspace.search_open() || workspace.shortcuts_open();
        if *peek.peek() && !keep_peek(true, window_width(), overlay_open) {
            peek.set(false);
        }
    });

    let prefs = workspace.prefs();
    let hidden = prefs.hidden;
    // The room the open dock columns ask for; an expanded column fills the pane and asks for none.
    let dock_width: u32 = match workspace.active() {
        Active::Live(id) => {
            let dock = workspace.dock(id);
            if dock.expanded.is_some() {
                0
            } else {
                dock.columns.iter().map(|column| column.width as u32).sum()
            }
        }
        Active::NewSession | Active::Customize | Active::Routines => 0,
    };
    let docked_sidebar = if hidden { SIDEBAR_RAIL_WIDTH } else { prefs.clamped_width() };
    let layout = layout_for(window_width(), docked_sidebar, dock_width);
    let sidebar_mode = layout.sidebar;

    // ⌥⌘R needs the row on screen: bring the sidebar out when it is hidden or an overlay. The mode
    // depends on the window width only, so the dock width does not matter here.
    use_effect(move || {
        if workspace.rename_request().is_none() {
            return;
        }
        match layout_for(window_width(), SIDEBAR_RAIL_WIDTH, 0).sidebar {
            SidebarMode::Overlay => peek.set(true),
            SidebarMode::Docked => {
                if workspace.prefs().hidden {
                    workspace.update_prefs(|prefs| prefs.hidden = false);
                }
            }
        }
    });

    rsx! {
        div {
            class: workspace_class(layout, dragging()),
            tabindex: "0",
            // Key events only reach the root while something inside it has focus.
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onkeydown: move |event| {
                // Escape puts the overlay sidebar away, unless an overlay of the app is on top of it.
                if peek() && event.key() == Key::Escape && !settings.is_open() && !workspace.search_open() && !workspace.shortcuts_open() {
                    peek.set(false);
                    return;
                }
                // The palette is an overlay too: the session shortcuts wait while it is open.
                if !workspace.on_key(&event, settings.is_open() || workspace.search_open() || workspace.shortcuts_open()) {
                    if forge_features::shortcut_from_event(&event) == Some(forge_features::Shortcut::ToggleSidebar) {
                        // An overlay swallowed the key before this point (`shortcut_allowed`).
                        match sidebar_mode {
                            SidebarMode::Overlay => peek.set(!peek()),
                            SidebarMode::Docked => workspace.update_prefs(|prefs| prefs.hidden = !prefs.hidden),
                        }
                        event.prevent_default();
                    } else {
                        settings.on_key(&event);
                    }
                }
            },
            // The sidebar's width follows the pointer while the handle is held; it is saved once, on release.
            onmousemove: move |event| {
                if dragging() {
                    let x = event.client_coordinates().x.max(0.0) as u32;
                    workspace.change_prefs_unsaved(|prefs| prefs.width = x.clamp(forge_domain::MIN_SIDEBAR_WIDTH, forge_domain::MAX_SIDEBAR_WIDTH));
                }
            },
            onmouseup: move |_| {
                if dragging() {
                    dragging.set(false);
                    workspace.save_prefs();
                }
            },
            match layout.sidebar {
                SidebarMode::Docked => rsx! {
                    if hidden {
                        div { class: "sidebar-rail",
                            button {
                                class: "icon-button",
                                title: "Show the sidebar",
                                onclick: move |_| workspace.update_prefs(|prefs| prefs.hidden = false),
                                "›"
                            }
                        }
                    } else {
                        Sidebar {}
                        div {
                            class: if dragging() { "sidebar-resize dragging" } else { "sidebar-resize" },
                            onmousedown: move |event| {
                                event.prevent_default();
                                dragging.set(true);
                            },
                        }
                    }
                },
                SidebarMode::Overlay => rsx! {
                    button {
                        class: "icon-button sidebar-toggle",
                        r#type: "button",
                        title: "Toggle the sidebar",
                        "aria-label": "Toggle the sidebar",
                        "aria-expanded": "{peek()}",
                        onclick: move |_| peek.set(!peek()),
                        "☰"
                    }
                    if peek() {
                        div { class: "sidebar-backdrop", onclick: move |_| peek.set(false) }
                        Sidebar {}
                    }
                },
            }
            match workspace.active() {
                Active::NewSession => rsx! { NewSessionPanel {} },
                Active::Customize => rsx! { CustomizePage {} },
                Active::Routines => rsx! { RoutinesPage {} },
                Active::Live(id) => rsx! { ChatWindow { key: "{id:?}", id } },
            }
            SettingsOverlay {}
            SearchPalette {}
            ShortcutsDialog {}
            Lightbox {}
        }
    }
}
