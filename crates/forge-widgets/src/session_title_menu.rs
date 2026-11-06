use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::TranscriptView;
use forge_features::{menu_key_action, refocus_workspace, use_workspace, ArchiveAction, DeleteAction, MenuKeyAction, RowActions};
use forge_shared::{tr, Icon, IconKind, IconSize};

/// What the title menu shows: its rows, the transcript-view choices, or the delete question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuPage {
    Main,
    Views,
    Styles,
    ConfirmDelete,
}

/// The chevron after the title of the open session and its menu, laid out like the desktop app's
/// (round 37): `이름 변경 R`, `포크 F`, then `트랜스크립트 보기 ›` (normal, thinking, detailed), then
/// `보관 A` and `삭제 D`. Delete asks first, then closes the session and moves it to the trash.
/// `출력 스타일 ›` changes this session's output style. `다음에서 열기` (new window, cloud) is left out.
#[component]
pub fn SessionTitleMenu(id: LiveSessionId, actions: RowActions, mode: TranscriptView, #[props(default)] style: Option<forge_domain::OutputStyle>) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let mut page = use_signal(|| MenuPage::Main);
    if actions.archive == ArchiveAction::Unavailable && !actions.can_rename && !actions.can_fork {
        return rsx! {};
    }
    let archive_label = if actions.archive == ArchiveAction::Archive { tr!("Archive") } else { tr!("Unarchive") };
    let can_delete = actions.delete == DeleteAction::Allowed;
    let mut close = move || {
        open.set(false);
        page.set(MenuPage::Main);
        refocus_workspace();
    };
    rsx! {
        div { class: "title-menu",
            button {
                class: if open() { "title-chevron open" } else { "title-chevron" },
                title: tr!("Session menu"),
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onclick: move |_| {
                    page.set(MenuPage::Main);
                    open.set(!open());
                },
                Icon { kind: IconKind::Chevron, size: IconSize::Small, open: true }
            }
            if open() {
                div { class: "menu-backdrop", onclick: move |_| close() }
                div {
                    class: "row-menu title-menu-list",
                    tabindex: "0",
                    role: "menu",
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            if page() == MenuPage::Main {
                                close();
                            } else {
                                page.set(MenuPage::Main);
                            }
                            return;
                        }
                        if page() != MenuPage::Main {
                            return;
                        }
                        let modified = event.modifiers().intersects(Modifiers::CONTROL | Modifiers::META | Modifiers::ALT);
                        let Some(action) = menu_key_action(&event.code().to_string(), modified, actions) else {
                            return;
                        };
                        event.stop_propagation();
                        event.prevent_default();
                        match action {
                            MenuKeyAction::Rename => {
                                open.set(false);
                                workspace.rename_active();
                            }
                            MenuKeyAction::Fork => {
                                close();
                                workspace.fork_active();
                            }
                            MenuKeyAction::ToggleArchive => {
                                close();
                                workspace.archive_active();
                            }
                            MenuKeyAction::Delete => page.set(MenuPage::ConfirmDelete),
                            // Pinning is a sidebar row's; the title menu has no 고정.
                            MenuKeyAction::TogglePin | MenuKeyAction::ToggleUnread => {}
                        }
                    },
                    match page() {
                        MenuPage::Main => rsx! {
                            if actions.can_rename {
                                button {
                                    class: "row-menu-item",
                                    role: "menuitem",
                                    onclick: move |_| {
                                        open.set(false);
                                        workspace.rename_active();
                                    },
                                    span { {tr!("Rename")} }
                                    span { class: "menu-key", "R" }
                                }
                            }
                            if actions.can_fork {
                                button {
                                    class: "row-menu-item",
                                    role: "menuitem",
                                    onclick: move |_| {
                                        close();
                                        workspace.fork_active();
                                    },
                                    span { {tr!("Fork")} }
                                    span { class: "menu-key", "F" }
                                }
                            }
                            div { class: "row-menu-separator" }
                            button {
                                class: "row-menu-item",
                                role: "menuitem",
                                "aria-haspopup": "menu",
                                onclick: move |_| page.set(MenuPage::Views),
                                span { {tr!("Transcript view")} }
                                span { class: "menu-key", Icon { kind: IconKind::Chevron, size: IconSize::Small } }
                            }
                            // The desktop app's `출력 스타일 ›` (observed 2026-11-06): 기본값 and Claude Code's styles.
                            button {
                                class: "row-menu-item",
                                role: "menuitem",
                                "aria-haspopup": "menu",
                                onclick: move |_| page.set(MenuPage::Styles),
                                span { {tr!("Output style")} }
                                span { class: "menu-key", Icon { kind: IconKind::Chevron, size: IconSize::Small } }
                            }
                            button {
                                class: "row-menu-item",
                                role: "menuitem",
                                onclick: move |_| {
                                    close();
                                    workspace.export_active_session(id);
                                },
                                span { {tr!("Export")} }
                            }
                            if actions.archive != ArchiveAction::Unavailable || can_delete {
                                div { class: "row-menu-separator" }
                            }
                            if actions.archive != ArchiveAction::Unavailable {
                                button {
                                    class: "row-menu-item",
                                    role: "menuitem",
                                    onclick: move |_| {
                                        close();
                                        workspace.archive_active();
                                    },
                                    span { {archive_label} }
                                    span { class: "menu-key", "A" }
                                }
                            }
                            if can_delete {
                                button {
                                    class: "row-menu-item danger",
                                    role: "menuitem",
                                    onclick: move |_| page.set(MenuPage::ConfirmDelete),
                                    span { {tr!("Delete")} }
                                    span { class: "menu-key", "D" }
                                }
                            }
                        },
                        MenuPage::Views => rsx! {
                            button {
                                class: "row-menu-item row-menu-back",
                                role: "menuitem",
                                onclick: move |_| page.set(MenuPage::Main),
                                Icon { kind: IconKind::ChevronLeft, size: IconSize::Small }
                                span { {tr!("Transcript view")} }
                            }
                            for option in TranscriptView::ALL {
                                button {
                                    key: "{option.label()}",
                                    class: "row-menu-item",
                                    role: "menuitemradio",
                                    "aria-checked": "{option == mode}",
                                    onclick: move |_| {
                                        workspace.set_transcript_view(id, option);
                                        close();
                                    },
                                    span { {forge_shared::i18n::text(option.label())} }
                                    if option == mode {
                                        span { class: "menu-key", Icon { kind: IconKind::Check, size: IconSize::Small } }
                                    }
                                }
                            }
                        },
                        MenuPage::Styles => rsx! {
                            button {
                                class: "row-menu-item row-menu-back",
                                role: "menuitem",
                                onclick: move |_| page.set(MenuPage::Main),
                                Icon { kind: IconKind::ChevronLeft, size: IconSize::Small }
                                span { {tr!("Output style")} }
                            }
                            for option in std::iter::once(None).chain(forge_domain::OutputStyle::ALL.into_iter().map(Some)) {
                                button {
                                    key: "{option.map(|style| style.name()).unwrap_or(\"default\")}",
                                    class: "row-menu-item",
                                    role: "menuitemradio",
                                    "aria-checked": "{option == style}",
                                    onclick: move |_| {
                                        workspace.set_output_style(id, option);
                                        close();
                                    },
                                    span {
                                        match option {
                                            Some(choice) => forge_shared::i18n::text(choice.label()).to_string(),
                                            None => tr!("Default").to_string(),
                                        }
                                    }
                                    if option == style {
                                        span { class: "menu-key", Icon { kind: IconKind::Check, size: IconSize::Small } }
                                    }
                                }
                            }
                        },
                        MenuPage::ConfirmDelete => rsx! {
                            div { class: "row-menu-note", {tr!("Delete this session? It is closed and moved to the trash.")} }
                            div { class: "row-menu-actions",
                                button {
                                    class: "row-menu-item",
                                    onclick: move |_| page.set(MenuPage::Main),
                                    span { {tr!("Cancel")} }
                                }
                                button {
                                    class: "row-menu-item danger",
                                    onclick: move |_| {
                                        close();
                                        workspace.delete_active();
                                    },
                                    span { {tr!("Delete")} }
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}
