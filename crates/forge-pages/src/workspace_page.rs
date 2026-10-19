use dioxus::prelude::*;
use forge_domain::{keep_peek, layout_for, pane_class, workspace_class, SidebarMode, SIDEBAR_RAIL_WIDTH};
use forge_features::{refocus_workspace, watch_focus, use_customize_provider, use_routines_provider, use_session_notices, use_settings_provider, use_window_width, use_workspace, use_workspace_provider, Active, PaneIndex, Workspace};
use forge_widgets::{ChatWindow, CustomizePage, RoutinesPage, Lightbox, WebViewHost, NavButtons, NewSessionPanel, PaneClose, SearchPalette, SettingsOverlay, ShortcutsDialog, Sidebar};
use forge_shared::tr;

/// The whole screen: the sidebar on the left and, on the right, either the new-session panel or
/// the active session. The root also catches the session shortcuts.
#[component]
pub fn WorkspacePage() -> Element {
    let settings = use_settings_provider();
    use_customize_provider();
    let workspace = use_workspace_provider();
    use_routines_provider(workspace, settings);
    use_session_notices(workspace, settings);
    let mut dragging = use_signal(|| false);
    let mut pane_dragging = use_signal(|| false);
    // Key events only reach the root while something inside it has focus. Whenever the overlay
    // closes or the main pane switches view, the focused input or button may have been removed
    // and focus falls to <body>, which silences every shortcut until the next click. Give focus
    // back to the root — but only if nothing has it, so a composer the person is typing in keeps it.
    use_hook(watch_focus);
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
    let docked_sidebar = if hidden { SIDEBAR_RAIL_WIDTH } else { prefs.clamped_width() };
    let layout = layout_for(window_width(), docked_sidebar, 0);
    let sidebar_mode = layout.sidebar;
    // The area right of the docked sidebar (the whole window with an overlay sidebar).
    let main_area = match sidebar_mode {
        SidebarMode::Docked => window_width().saturating_sub(docked_sidebar),
        SidebarMode::Overlay => window_width(),
    };
    use_effect(use_reactive!(|main_area| workspace.set_main_width(main_area)));

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
            class: workspace_class(layout, dragging() || pane_dragging()),
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
                if pane_dragging() {
                    let area = main_area.max(1) as f32;
                    let left_edge = window_width().saturating_sub(main_area) as f32;
                    workspace.set_pane_ratio((event.client_coordinates().x as f32 - left_edge) / area);
                }
            },
            onmouseup: move |_| {
                if dragging() {
                    dragging.set(false);
                    workspace.save_prefs();
                }
                if pane_dragging() {
                    pane_dragging.set(false);
                }
            },
            match layout.sidebar {
                SidebarMode::Docked => rsx! {
                    if hidden {
                        div { class: "sidebar-rail",
                            button {
                                class: "icon-button",
                                title: tr!("Show the sidebar"),
                                onclick: move |_| workspace.update_prefs(|prefs| prefs.hidden = false),
                                "›"
                            }
                            NavButtons {}
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
                        title: tr!("Toggle the sidebar"),
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
            {pane_row(workspace, pane_dragging)}
            SettingsOverlay {}
            SearchPalette {}
            ShortcutsDialog {}
            Lightbox {}
            WebViewHost {}
        }
    }
}

// The panes and the divider. A pane is keyed by its screen, so closing the left one does not
// remount the right one.
fn pane_row(workspace: Workspace, mut pane_dragging: Signal<bool>) -> Element {
    let panes = workspace.panes();
    let (left, right) = workspace.pane_widths();
    let split = panes.is_split();
    rsx! {
        div { class: if split { "panes split" } else { "panes" },
            for (index, target) in panes.targets().iter().copied().enumerate() {
                Fragment { key: "{target:?}",
                    if index == 1 {
                        div {
                            class: if pane_dragging() { "pane-divider dragging" } else { "pane-divider" },
                            onmousedown: move |event| {
                                event.prevent_default();
                                pane_dragging.set(true);
                            },
                        }
                    }
                    Pane { index, target, width: if index == 0 { left } else { right } }
                }
            }
        }
    }
}

/// One screen of the main area. It tells its descendants which pane they are in and moves the
/// focus here on any click or focus inside.
#[component]
fn Pane(index: usize, target: Active, width: u32) -> Element {
    let workspace = use_workspace();
    let mut index_signal = use_context_provider(|| PaneIndex(Signal::new(index)));
    use_effect(use_reactive!(|index| {
        if *index_signal.0.peek() != index {
            index_signal.0.set(index);
        }
    }));
    let panes = workspace.panes();
    let split = panes.is_split();
    let dock_width: u32 = match target {
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
    let mut class = pane_class(width, dock_width);
    if split {
        class.push_str(" split");
    }
    if panes.focused() == index {
        class.push_str(" focused");
    }
    // The right pane takes what is left, so the two always fill the area.
    let style = if split && index == 0 { format!("width: {width}px; flex: none;") } else { String::new() };
    rsx! {
        div {
            class,
            style,
            "data-pane": "{index}",
            onmousedown: move |_| workspace.focus_pane(index),
            onfocusin: move |_| workspace.focus_pane(index),
            match target {
                Active::NewSession => rsx! { NewSessionPanel {} },
                Active::Customize => rsx! { CustomizePage {} },
                Active::Routines => rsx! { RoutinesPage {} },
                Active::Live(id) => rsx! { ChatWindow { key: "{id:?}", id } },
            }
            if split && !matches!(target, Active::Live(_)) {
                PaneClose { index }
            }
        }
    }
}
