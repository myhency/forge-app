use dioxus::prelude::*;
use forge_features::{menu_height, submenu_opens_left, FileAction, MenuEntry};

const MENU_WIDTH: f64 = 230.0;

#[component]
fn Entry(entry: MenuEntry, on_select: EventHandler<FileAction>) -> Element {
    match entry {
        MenuEntry::Divider => rsx! { div { class: "picker-divider" } },
        MenuEntry::Action { label, shortcut, action, enabled } => rsx! {
            button {
                class: if enabled { "picker-item" } else { "picker-item disabled" },
                r#type: "button",
                role: "menuitem",
                onclick: move |_| {
                    if enabled {
                        on_select.call(action);
                    }
                },
                span { class: "picker-text", "{label}" }
                if let Some(key) = shortcut {
                    span { class: "picker-hint", "{key}" }
                }
            }
        },
        MenuEntry::Submenu { .. } => rsx! {},
    }
}

/// A context menu at a point. A backdrop click, a right click on it and Escape close it; a submenu
/// opens to the right of its item, or to the left near the window's right edge.
#[component]
pub fn ContextMenu(entries: Vec<MenuEntry>, x: f64, y: f64, on_select: EventHandler<FileAction>, on_close: EventHandler<()>) -> Element {
    let mut open_sub = use_signal(|| None::<usize>);
    let mut window_width = use_signal(|| 1200.0_f64);
    use_future(move || async move {
        if let Ok(width) = document::eval("dioxus.send(window.innerWidth)").recv::<f64>().await {
            window_width.set(width);
        }
    });
    let flip = submenu_opens_left(x, window_width(), MENU_WIDTH);
    let height = menu_height(&entries);
    rsx! {
        div {
            class: "picker-backdrop",
            onclick: move |_| on_close.call(()),
            oncontextmenu: move |event: MouseEvent| {
                event.prevent_default();
                on_close.call(());
            },
        }
        div {
            class: "picker-menu context-menu",
            role: "menu",
            tabindex: "0",
            style: "left: min({x}px, calc(100vw - {MENU_WIDTH}px)); top: max(0px, min({y}px, calc(100vh - {height}px)));",
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onkeydown: move |event: KeyboardEvent| {
                if event.key() == Key::Escape {
                    event.stop_propagation();
                    on_close.call(());
                }
            },
            for (index, entry) in entries.iter().cloned().enumerate() {
                match entry {
                    MenuEntry::Submenu { label, entries } => rsx! {
                        div {
                            key: "{index}",
                            class: "context-sub",
                            onmouseenter: move |_| open_sub.set(Some(index)),
                            button { class: "picker-item", r#type: "button", role: "menuitem", "aria-haspopup": "menu",
                                span { class: "picker-text", "{label}" }
                                span { class: "picker-hint", "›" }
                            }
                            if open_sub() == Some(index) {
                                div { class: if flip { "picker-menu context-submenu left" } else { "picker-menu context-submenu" }, role: "menu",
                                    for (sub_index, sub) in entries.iter().cloned().enumerate() {
                                        Entry { key: "{sub_index}", entry: sub, on_select }
                                    }
                                }
                            }
                        }
                    },
                    other => rsx! {
                        div { key: "{index}", onmouseenter: move |_| open_sub.set(None),
                            Entry { entry: other, on_select }
                        }
                    },
                }
            }
        }
    }
}
