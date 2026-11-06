use dioxus::prelude::*;
use forge_application::{LiveSessionId, StyleDraft};
use forge_domain::TranscriptView;
use forge_features::{menu_key_action, refocus_workspace, use_workspace, ArchiveAction, DeleteAction, MenuKeyAction, RowActions};
use forge_shared::{tr, Button, ButtonKind, Icon, IconKind, IconSize};

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
/// `출력 스타일 ›` changes this session's output style, and its `새 스타일...` has Claude write a new
/// one. `다음에서 열기` (new window, cloud) is left out.
#[component]
pub fn SessionTitleMenu(
    id: LiveSessionId,
    actions: RowActions,
    mode: TranscriptView,
    #[props(default)] style: Option<forge_domain::OutputStyle>,
    /// The styles the SDK lists for the session (`default` first); empty until it reports them.
    #[props(default)] styles: Vec<String>,
    #[props(default)] draft: StyleDraft,
) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let mut new_style = use_signal(|| false);
    let mut page = use_signal(|| MenuPage::Main);
    if actions.archive == ArchiveAction::Unavailable && !actions.can_rename && !actions.can_fork {
        return rsx! {};
    }
    // Claude Code's four styles, then the person's own in the order the SDK lists them.
    let mut choices: Vec<Option<forge_domain::OutputStyle>> = vec![None];
    choices.extend(forge_domain::OutputStyle::ALL.into_iter().map(Some));
    choices.extend(styles.iter().filter_map(|name| forge_domain::OutputStyle::from_name(name)).filter(|choice| choice.is_custom()).map(Some));
    if let Some(current) = style.as_ref().filter(|current| current.is_custom() && !choices.contains(&Some((*current).clone()))) {
        choices.push(Some(current.clone()));
    }
    let archive_label = if actions.archive == ArchiveAction::Archive { tr!("Archive") } else { tr!("Unarchive") };
    let can_delete = actions.delete == DeleteAction::Allowed;
    let mut close = move || {
        open.set(false);
        page.set(MenuPage::Main);
        refocus_workspace();
    };
    rsx! {
        if new_style() {
            NewStyleDialog { id, draft: draft.clone(), on_close: move |_| new_style.set(false) }
        }
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
                            for option in choices.iter().cloned() {
                                button {
                                    key: "{option.as_ref().map(|style| style.name().to_string()).unwrap_or_else(|| \"default\".to_string())}",
                                    class: "row-menu-item",
                                    role: "menuitemradio",
                                    "aria-checked": "{option == style}",
                                    onclick: {
                                        let option = option.clone();
                                        move |_| {
                                            workspace.set_output_style(id, option.clone());
                                            close();
                                        }
                                    },
                                    span {
                                        match option.as_ref() {
                                            Some(choice) => choice.display(),
                                            None => tr!("Default").to_string(),
                                        }
                                    }
                                    if option == style {
                                        span { class: "menu-key", Icon { kind: IconKind::Check, size: IconSize::Small } }
                                    }
                                }
                            }
                            // The desktop app's `새 스타일...` (observed 2026-11-06): describe a style and Claude drafts it.
                            div { class: "row-menu-separator" }
                            button {
                                class: "row-menu-item",
                                role: "menuitem",
                                onclick: move |_| {
                                    close();
                                    workspace.clear_style_draft(id);
                                    new_style.set(true);
                                },
                                span { {tr!("New style…")} }
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

/// The desktop app's `새 출력 스타일` dialog (observed 2026-11-06): what the person wants and an
/// optional name. Claude drafts the style; it is saved in their `output-styles` folder and chosen for
/// this session, and the dialog closes.
#[component]
fn NewStyleDialog(id: LiveSessionId, draft: StyleDraft, on_close: EventHandler<()>) -> Element {
    let workspace = use_workspace();
    let mut description = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut asked = use_signal(|| false);
    let drafting = draft == StyleDraft::Drafting;
    let name_ok = style_name_ok(&name());
    let can_create = !drafting && !description().trim().is_empty() && name_ok;
    let created = asked() && matches!(draft, StyleDraft::Created(_));
    use_effect(use_reactive!(|created| {
        if created {
            on_close.call(());
        }
    }));
    let failure = match (&draft, asked()) {
        (StyleDraft::Failed(reason), true) => Some(reason.clone()),
        _ => None,
    };
    let mut create = move || {
        if !can_create {
            return;
        }
        asked.set(true);
        let typed = name().trim().to_string();
        workspace.create_output_style(id, description().trim().to_string(), (!typed.is_empty()).then_some(typed));
    };
    rsx! {
        div {
            class: "settings-backdrop",
            onclick: move |_| on_close.call(()),
            div {
                class: "env-dialog style-dialog",
                role: "dialog",
                "aria-label": tr!("New output style"),
                onclick: move |event| event.stop_propagation(),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        event.stop_propagation();
                        on_close.call(());
                    }
                },
                div { class: "env-head",
                    h2 { {tr!("New output style")} }
                    button { class: "settings-close", r#type: "button", title: tr!("Close (Esc)"), onclick: move |_| on_close.call(()), Icon { kind: IconKind::Close } }
                }
                p { class: "hint", {tr!("Describe the style you want and Claude will draft it.")} }
                label { class: "env-label", {tr!("What style do you want?")} }
                textarea {
                    class: "style-description",
                    placeholder: tr!("Short answer first, then the details. No filler."),
                    maxlength: "2000",
                    disabled: drafting,
                    value: "{description}",
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    oninput: move |event| description.set(event.value()),
                }
                label { class: "env-label", {tr!("Name (optional)")} }
                input {
                    class: "customize-field style-name",
                    maxlength: "64",
                    disabled: drafting,
                    value: "{name}",
                    oninput: move |event| name.set(event.value()),
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Enter {
                            event.prevent_default();
                            create();
                        }
                    },
                }
                if !name_ok {
                    p { class: "hint style-name-error", {tr!("Use letters, numbers, spaces and hyphens only.")} }
                }
                if let Some(reason) = failure {
                    div { class: "banner error", {style_failure_text(&reason)} }
                }
                div { class: "env-buttons",
                    span { class: "spacer" }
                    Button { label: tr!("Cancel"), onclick: move |_| on_close.call(()) }
                    Button {
                        label: if drafting { tr!("Creating…") } else { tr!("Create style") },
                        kind: ButtonKind::Primary,
                        disabled: !can_create,
                        onclick: move |_| create(),
                    }
                }
            }
        }
    }
}

/// The sidecar's and the session's reasons in the person's language; anything else (an SDK error) as it came.
fn style_failure_text(reason: &str) -> String {
    let known = [
        ("Describe the style in up to 2,000 characters.", tr!("Describe the style in up to 2,000 characters.")),
        ("Use letters, numbers, spaces and hyphens only.", tr!("Use letters, numbers, spaces and hyphens only.")),
        ("Styles cannot be saved here.", tr!("Styles cannot be saved here.")),
        ("A style is already being drafted.", tr!("A style is already being drafted.")),
        ("Claude took too long to draft the style.", tr!("Claude took too long to draft the style.")),
        ("Claude's draft could not be read. Try again.", tr!("Claude's draft could not be read. Try again.")),
        ("The style could not be saved under a free name.", tr!("The style could not be saved under a free name.")),
        ("The session stopped before the style was saved. Try again.", tr!("The session stopped before the style was saved. Try again.")),
    ];
    if let Some((_, shown)) = known.iter().find(|(english, _)| *english == reason) {
        return shown.to_string();
    }
    if let Some(name) = reason.strip_prefix("A style called ").and_then(|rest| rest.strip_suffix(" already exists.")) {
        return tr!("A style called {name} already exists.", name = name);
    }
    reason.to_string()
}

/// A name the person may give a style (it becomes a file name): letters, digits, spaces and hyphens,
/// at most 64 characters. Empty is fine (Claude suggests one).
fn style_name_ok(name: &str) -> bool {
    let name = name.trim();
    name.chars().count() <= 64 && name.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::style_name_ok;

    #[test]
    fn a_style_name_is_letters_digits_spaces_and_hyphens() {
        assert!(style_name_ok(""));
        assert!(style_name_ok("Pirate talk-2"));
        assert!(style_name_ok("간결한 리뷰"));
        assert!(!style_name_ok("../escape"));
        assert!(!style_name_ok("a/b"));
        assert!(!style_name_ok(&"x".repeat(65)));
    }
}
