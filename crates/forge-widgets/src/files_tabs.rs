use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use forge_shared::{tr, Icon, IconKind, IconSize};

/// One tab as the strip draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct TabView {
    pub path: String,
    pub preview: bool,
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The tabs above the editor. A preview tab is italic. A press on a tab and a release on another
/// reorders; a press and release on the same tab selects it. A right click opens the tab menu.
/// The pointer is followed with mouse events (not the HTML drag and drop, which a web view may
/// take over for file drops).
#[component]
pub fn FilesTabs(
    tabs: Vec<TabView>,
    selected: Option<String>,
    on_select: EventHandler<String>,
    on_close: EventHandler<String>,
    on_reorder: EventHandler<(String, String)>,
    on_menu: EventHandler<(String, bool, f64, f64)>,
) -> Element {
    // The tab a press started on.
    let mut pressed = use_signal(|| None::<String>);
    rsx! {
        div { class: "files-tabs", role: "tablist", onmouseleave: move |_| pressed.set(None),
            for tab in tabs.iter().cloned() {
                {
                    let path = tab.path.clone();
                    let is_selected = selected.as_deref() == Some(path.as_str());
                    let class = match (is_selected, tab.preview) {
                        (true, true) => "files-tab selected preview",
                        (true, false) => "files-tab selected",
                        (false, true) => "files-tab preview",
                        (false, false) => "files-tab",
                    };
                    rsx! {
                        div {
                            key: "{path}",
                            class: "{class}",
                            role: "tab",
                            title: "{path}",
                            "aria-selected": "{is_selected}",
                            onmousedown: {
                                let path = path.clone();
                                move |event: MouseEvent| {
                                    if event.trigger_button() == Some(MouseButton::Primary) {
                                        pressed.set(Some(path.clone()));
                                    }
                                }
                            },
                            onmouseup: {
                                let path = path.clone();
                                move |_| {
                                    let from = pressed.write().take();
                                    match from {
                                        Some(from) if from != path => on_reorder.call((from, path.clone())),
                                        Some(_) => on_select.call(path.clone()),
                                        None => {}
                                    }
                                }
                            },
                            oncontextmenu: {
                                let (path, preview) = (path.clone(), tab.preview);
                                move |event: MouseEvent| {
                                    event.prevent_default();
                                    let point = event.client_coordinates();
                                    on_menu.call((path.clone(), preview, point.x, point.y));
                                }
                            },
                            span { class: "files-tab-name", "{file_name(&path)}" }
                            button {
                                class: "files-tab-close",
                                r#type: "button",
                                title: tr!("Close"),
                                "aria-label": "Close {file_name(&path)}",
                                onmousedown: move |event: MouseEvent| event.stop_propagation(),
                                onmouseup: move |event: MouseEvent| event.stop_propagation(),
                                onclick: {
                                    let path = path.clone();
                                    move |event: MouseEvent| {
                                        event.stop_propagation();
                                        on_close.call(path.clone());
                                    }
                                },
                                Icon { kind: IconKind::Close, size: IconSize::Small }
                            }
                        }
                    }
                }
            }
        }
    }
}
