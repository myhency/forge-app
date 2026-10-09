use dioxus::prelude::*;
use forge_features::{account_rows, use_settings, use_workspace, user_initial, user_name, AccountAction, AccountRow};
use forge_shared::{i18n, tr};

#[component]
fn Row(row: AccountRow, on_select: EventHandler<AccountAction>) -> Element {
    match row {
        AccountRow::Divider => rsx! { div { class: "picker-divider" } },
        AccountRow::Heading(text) => rsx! { div { class: "account-heading", "{text}" } },
        AccountRow::Item { label, keys, action, check } => rsx! {
            button {
                class: "picker-item",
                r#type: "button",
                role: "menuitem",
                onclick: move |_| on_select.call(action),
                if let Some(checked) = check {
                    span { class: "picker-check", if checked { "✓" } }
                }
                span { class: "picker-text", "{i18n::text(label)}" }
                if let Some(keys) = keys {
                    span { class: "picker-hint", "{keys}" }
                }
            }
        },
        AccountRow::Submenu { .. } => rsx! {},
    }
}

/// The account area at the bottom of the sidebar: the OS user's name, and the menu it opens.
#[component]
pub fn AccountMenu() -> Element {
    let settings = use_settings();
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let mut open_sub = use_signal(|| None::<usize>);
    // Read here so the whole menu is drawn again when the language changes.
    let language = i18n::current();
    let name = user_name(std::env::var("USER").ok().as_deref(), std::env::var("USERNAME").ok().as_deref());
    let initial = user_initial(&name);
    let rows = account_rows(&name, language);
    let account_title = tr!("Account menu");

    let on_select = move |action: AccountAction| {
        open.set(false);
        open_sub.set(None);
        match action {
            AccountAction::OpenSettings => settings.open(),
            AccountAction::SetLanguage(language) => settings.set_language(language),
            AccountAction::ShowShortcuts => workspace.open_shortcuts(),
            AccountAction::Close => {}
        }
    };

    rsx! {
        div { class: "account",
            button {
                class: "sidebar-footer-button account-button",
                r#type: "button",
                title: "{account_title}",
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onclick: move |_| {
                    open_sub.set(None);
                    open.toggle();
                },
                span { class: "account-avatar", "{initial}" }
                span { class: "account-name", "{name}" }
                span { class: "account-chevron", "⌃" }
            }
            if open() {
                div {
                    class: "picker-backdrop",
                    onclick: move |_| {
                        open.set(false);
                        open_sub.set(None);
                    },
                }
                div {
                    class: "picker-menu account-menu",
                    role: "menu",
                    tabindex: "0",
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            open.set(false);
                            open_sub.set(None);
                        }
                    },
                    for (index, row) in rows.into_iter().enumerate() {
                        match row {
                            AccountRow::Submenu { label, rows: sub_rows } => rsx! {
                                div {
                                    key: "{index}",
                                    class: "context-sub",
                                    onmouseenter: move |_| open_sub.set(Some(index)),
                                    button {
                                        class: "picker-item",
                                        r#type: "button",
                                        role: "menuitem",
                                        "aria-haspopup": "menu",
                                        onclick: move |_| open_sub.set(Some(index)),
                                        span { class: "picker-text", "{i18n::text(label)}" }
                                        span { class: "picker-hint", "›" }
                                    }
                                    if open_sub() == Some(index) {
                                        div { class: "picker-menu context-submenu", role: "menu",
                                            for (sub_index, sub) in sub_rows.into_iter().enumerate() {
                                                Row { key: "{sub_index}", row: sub, on_select }
                                            }
                                        }
                                    }
                                }
                            },
                            other => rsx! {
                                div { key: "{index}", onmouseenter: move |_| open_sub.set(None),
                                    Row { row: other, on_select }
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}
