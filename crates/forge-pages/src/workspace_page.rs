use dioxus::prelude::*;
use forge_domain::{keep_peek, pane_class, sidebar_mode_for, workspace_class, SidebarMode, SIDEBAR_RAIL_WIDTH};
use forge_features::{refocus_workspace, watch_focus, use_customize_provider, use_routines_provider, use_session_notices, use_settings_provider, use_window_height, use_window_width, use_workspace, use_workspace_provider, Active, PaneIndex, Workspace};
use forge_widgets::{ChatWindow, CustomizePage, RoutinesPage, Lightbox, WebViewHost, FileLinkWatcher, NavButtons, NewSessionPanel, PaneClose, SearchPalette, SettingsOverlay, ShortcutsDialog, Sidebar};
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
    let window_height = use_window_height();
    let main_height = window_height();
    let sidebar_mode = sidebar_mode_for(window_width());
    // The area right of the docked sidebar (the whole window with an overlay sidebar).
    let main_area = match sidebar_mode {
        SidebarMode::Docked => window_width().saturating_sub(docked_sidebar),
        SidebarMode::Overlay => window_width(),
    };
    use_effect(use_reactive!(|main_area, main_height| workspace.set_main_size(main_area, main_height)));

    // ⌥⌘R needs the row on screen: bring the sidebar out when it is hidden or an overlay. The mode
    // depends on the window width only, so the dock width does not matter here.
    use_effect(move || {
        if workspace.rename_request().is_none() {
            return;
        }
        match sidebar_mode_for(window_width()) {
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
            class: workspace_class(sidebar_mode, dragging() || pane_dragging()),
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
                    // A release outside the window never reaches `onmouseup`: end the drag as soon as
                    // the move events say the button is up.
                    if !event.held_buttons().contains(dioxus::html::input_data::MouseButton::Primary) {
                        pane_dragging.set(false);
                        return;
                    }
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
            match sidebar_mode {
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
            {pane_row(workspace, main_area, pane_dragging)}
            SettingsOverlay {}
            SearchPalette {}
            ShortcutsDialog {}
            Lightbox {}
            WebViewHost {}
            FileLinkWatcher {}
        }
    }
}

// The panes and the divider. A pane is keyed by its screen, so closing the left one does not
// remount the right one. A main area too narrow for two panes shows only the focused one; the
// other comes back when the window grows.
fn pane_row(workspace: Workspace, main_area: u32, mut pane_dragging: Signal<bool>) -> Element {
    let targets = workspace.pane_targets();
    let focused = workspace.focused_pane();
    let split = targets.len() == 2 && workspace.panes_fit(main_area);
    let (left, right) = workspace.pane_widths(main_area);
    let shown: Vec<(usize, Active)> = if split {
        targets.iter().copied().enumerate().collect()
    } else {
        vec![(focused.min(targets.len() - 1), targets[focused.min(targets.len() - 1)])]
    };
    rsx! {
        div { class: if split { "panes split" } else { "panes" },
            for (index, target) in shown {
                Fragment { key: "{target:?}",
                    if split && index == 1 {
                        div {
                            class: if pane_dragging() { "pane-divider dragging" } else { "pane-divider" },
                            onmousedown: move |event| {
                                event.prevent_default();
                                pane_dragging.set(true);
                            },
                        }
                    }
                    Pane {
                        index,
                        target,
                        width: if !split { main_area } else if index == 0 { left } else { right },
                        split,
                        focused: focused == index,
                    }
                }
            }
        }
    }
}

/// One screen of the main area. It tells its descendants which pane they are in and moves the
/// focus here on any click or focus inside.
#[component]
fn Pane(index: usize, target: Active, width: u32, split: bool, focused: bool) -> Element {
    let workspace = use_workspace();
    let mut index_signal = use_context_provider(|| PaneIndex(Signal::new(index)));
    use_effect(use_reactive!(|index| {
        if *index_signal.0.peek() != index {
            index_signal.0.set(index);
        }
    }));
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
    // A pane that is not a session has no tools row, so the page draws its ×.
    let closable = split && !matches!(target, Active::Live(_));
    if split {
        class.push_str(" split");
    }
    if closable {
        class.push_str(" has-close");
    }
    if focused {
        class.push_str(" focused");
    }
    // The right pane takes what is left, so the two always fill the area. Always spelled out: an
    // empty style would leave the old width on a pane that had been the left one.
    let style = if split && index == 0 { format!("width: {width}px; flex: none;") } else { "width: auto; flex: 1;".to_string() };
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
            if closable {
                PaneClose { index }
            }
        }
    }
}
