use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::ChangesViewPrefs;
use forge_features::{filter_files, fold_all, unfold_all, use_workspace, ChangeView};
use forge_ports::{BranchInfo, ChangedFile, CommitInfo};
use forge_shared::tr;

/// The text of the branch button.
pub(crate) fn button_label(view: &ChangeView, info: Option<&BranchInfo>, base: Option<&str>, commits: &[CommitInfo]) -> String {
    match view {
        ChangeView::Uncommitted => tr!("Uncommitted changes").to_string(),
        ChangeView::Commit(sha) => {
            let subject = commits.iter().find(|commit| &commit.sha == sha).map(|commit| commit.subject.as_str()).unwrap_or("");
            format!("{} {}", &sha[..sha.len().min(7)], subject).trim().to_string()
        }
        ChangeView::All => match (base, info) {
            (Some(base), Some(info)) => format!("{base} → {}", info.head_label),
            _ => tr!("Uncommitted changes").to_string(),
        },
    }
}

/// Opens a menu under its button. The tools row scrolls sideways and would clip a menu below the
/// button, so the menu is fixed at the button's measured box: (left, bottom + 6, width).
fn open_below(button_box: Signal<Option<std::rc::Rc<MountedData>>>, mut corner: Signal<(f64, f64, f64)>, mut open: Signal<bool>) {
    spawn(async move {
        let rect = match button_box() {
            Some(mounted) => mounted.get_client_rect().await.ok(),
            None => None,
        };
        if let Some(rect) = rect {
            corner.set((rect.origin.x, rect.origin.y + rect.size.height + 6.0, rect.size.width));
        }
        open.set(true);
    });
}

/// `base → branch ⌄`: a menu of the two modes and the base list. A pick is kept in the session.
#[component]
pub fn BranchButton(id: LiveSessionId, info: Option<BranchInfo>, base: Option<String>, commits: Vec<CommitInfo>, view: ChangeView) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let mut choosing_base = use_signal(|| false);
    let mut button_box = use_signal(|| None::<std::rc::Rc<MountedData>>);
    let corner = use_signal(|| (0.0f64, 0.0f64, 0.0f64));
    let mut query = use_signal(String::new);
    let label = button_label(&view, info.as_ref(), base.as_deref(), &commits);
    let branches: Vec<String> = info
        .as_ref()
        .map(|info| info.branches.iter().filter(|name| query().is_empty() || name.to_lowercase().contains(&query().to_lowercase())).cloned().collect())
        .unwrap_or_default();
    let default_base = info.as_ref().and_then(|info| info.default_base.clone());
    rsx! {
        div { class: "cd-branch",
            button {
                class: "cd-head-button",
                r#type: "button",
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onmounted: move |mounted| button_box.set(Some(mounted.data())),
                onclick: move |_| {
                    choosing_base.set(false);
                    if open() {
                        open.set(false);
                        return;
                    }
                    open_below(button_box, corner, open);
                },
                span { "{label}" }
                span { class: "cd-chevron", "⌄" }
            }
            if open() {
                div { class: "picker-backdrop", onclick: move |_| open.set(false) }
                div {
                    class: "picker-menu cd-menu",
                    role: "menu",
                    style: "position: fixed; left: {corner().0}px; top: {corner().1}px; bottom: auto;",
                    tabindex: "0",
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            open.set(false);
                        }
                    },
                    if choosing_base() {
                        input {
                            class: "cd-menu-search",
                            r#type: "text",
                            placeholder: tr!("Search branches"),
                            value: "{query}",
                            oninput: move |event| query.set(event.value()),
                            onmounted: move |mounted| async move {
                                let _ = mounted.set_focus(true).await;
                            },
                        }
                        for name in branches {
                            button {
                                key: "{name}",
                                class: "picker-item",
                                r#type: "button",
                                role: "menuitem",
                                onclick: {
                                    let name = name.clone();
                                    move |_| {
                                        workspace.update_changes_session(id, |session| {
                                            session.base = Some(name.clone());
                                            session.view = ChangeView::All;
                                        });
                                        open.set(false);
                                    }
                                },
                                span { class: "picker-text", "{name}" }
                                if base.as_deref() == Some(name.as_str()) {
                                    span { class: "picker-hint", "✓" }
                                }
                                if default_base.as_deref() == Some(name.as_str()) {
                                    span { class: "picker-hint", {tr!("Default")} }
                                }
                            }
                        }
                    } else {
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                workspace.update_changes_session(id, |session| session.view = ChangeView::All);
                                open.set(false);
                            },
                            span { class: "picker-text", {tr!("All changes")} }
                            if view == ChangeView::All {
                                span { class: "picker-hint", "✓" }
                            }
                            if let Some(name) = base.clone() {
                                span { class: "picker-hint", {tr!("vs {base}", base = name)} }
                            }
                        }
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                workspace.update_changes_session(id, |session| session.view = ChangeView::Uncommitted);
                                open.set(false);
                            },
                            span { class: "picker-text", {tr!("Uncommitted changes")} }
                            if view == ChangeView::Uncommitted {
                                span { class: "picker-hint", "✓" }
                            }
                        }
                        div { class: "picker-divider" }
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                query.set(String::new());
                                choosing_base.set(true);
                            },
                            span { class: "picker-text", {tr!("Compare to")} }
                            span { class: "picker-hint", "{base.clone().unwrap_or_default()} ›" }
                        }
                    }
                }
            }
        }
    }
}

/// The popover of the search icon: a field that filters the changed files; a pick jumps to the file.
#[component]
pub fn FileSearch(files: Vec<ChangedFile>, on_pick: EventHandler<String>, on_close: EventHandler<()>) -> Element {
    let mut query = use_signal(String::new);
    let shown = filter_files(&files, &query());
    rsx! {
        div { class: "picker-backdrop", onclick: move |_| on_close.call(()) }
        div {
            class: "cd-search",
            onkeydown: move |event: KeyboardEvent| {
                if event.key() == Key::Escape {
                    on_close.call(());
                }
            },
            input {
                r#type: "text",
                placeholder: tr!("Search changed files"),
                value: "{query}",
                oninput: move |event| query.set(event.value()),
                onmounted: move |mounted| async move {
                    let _ = mounted.set_focus(true).await;
                },
            }
            div { class: "cd-search-list",
                for index in shown {
                    {
                        let file = files[index].clone();
                        let (folder, name) = forge_features::split_folder(&file.path);
                        let path = file.path.clone();
                        rsx! {
                            button {
                                key: "{file.path}",
                                class: "cd-row",
                                r#type: "button",
                                onclick: move |_| {
                                    on_pick.call(path.clone());
                                    on_close.call(());
                                },
                                span { class: "cd-name", "{name}" }
                                if !folder.is_empty() {
                                    span { class: "cd-folder", "{folder}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn check_item(label: &'static str, on: bool, onclick: impl FnMut(MouseEvent) + 'static) -> Element {
    rsx! {
        button { class: "picker-item", r#type: "button", role: "menuitemcheckbox", "aria-checked": "{on}", onclick,
            span { class: "picker-text", "{label}" }
            if on {
                span { class: "picker-hint", "✓" }
            }
        }
    }
}

/// The panel's `⋮`: what the diff looks like, fold all, refresh. A click on a check item does not
/// close the menu, as in the desktop app.
#[component]
pub fn ViewMenu(id: LiveSessionId, prefs: ChangesViewPrefs, paths: Vec<String>, list_open: bool, on_toggle_list: EventHandler<()>, on_refresh: EventHandler<()>) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let mut button_box = use_signal(|| None::<std::rc::Rc<MountedData>>);
    let corner = use_signal(|| (0.0f64, 0.0f64, 0.0f64));
    let (fold_paths, unfold_paths) = (paths.clone(), paths);
    rsx! {
        div { class: "cd-branch",
            button {
                class: "icon-button",
                r#type: "button",
                title: tr!("More"),
                "aria-label": "More",
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onmounted: move |mounted| button_box.set(Some(mounted.data())),
                onclick: move |_| {
                    if open() {
                        open.set(false);
                    } else {
                        open_below(button_box, corner, open);
                    }
                },
                "⋮"
            }
            if open() {
                div { class: "picker-backdrop", onclick: move |_| open.set(false) }
                div {
                    class: "picker-menu cd-menu",
                    role: "menu",
                    // Right edges line up with the button's.
                    style: "position: fixed; left: {corner().0 + corner().2}px; top: {corner().1}px; bottom: auto; transform: translateX(-100%);",
                    tabindex: "0",
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            open.set(false);
                        }
                    },
                    {check_item(tr!("Show files"), list_open, move |_| on_toggle_list.call(()))}
                    {check_item(tr!("Group files by folder"), prefs.group_by_folder, move |_| workspace.update_changes_view_prefs(|p| p.group_by_folder = !p.group_by_folder))}
                    div { class: "picker-divider" }
                    button {
                        class: "picker-item",
                        r#type: "button",
                        role: "menuitem",
                        onclick: move |_| {
                            workspace.update_changes_session(id, |session| fold_all(session, &fold_paths));
                            open.set(false);
                        },
                        span { class: "picker-text", {tr!("Collapse all files")} }
                    }
                    button {
                        class: "picker-item",
                        r#type: "button",
                        role: "menuitem",
                        onclick: move |_| {
                            workspace.update_changes_session(id, |session| unfold_all(session, &unfold_paths));
                            open.set(false);
                        },
                        span { class: "picker-text", {tr!("Expand all files")} }
                    }
                    div { class: "picker-divider" }
                    {check_item(tr!("Side by side"), prefs.side_by_side, move |_| workspace.update_changes_view_prefs(|p| p.side_by_side = !p.side_by_side))}
                    {check_item(tr!("Wrap lines"), prefs.wrap, move |_| workspace.update_changes_view_prefs(|p| p.wrap = !p.wrap))}
                    {check_item(tr!("Highlight changed words"), prefs.highlight_words, move |_| workspace.update_changes_view_prefs(|p| p.highlight_words = !p.highlight_words))}
                    {check_item(tr!("Hide whitespace changes"), prefs.hide_whitespace, move |_| workspace.update_changes_view_prefs(|p| p.hide_whitespace = !p.hide_whitespace))}
                    div { class: "picker-divider" }
                    button {
                        class: "picker-item",
                        r#type: "button",
                        role: "menuitem",
                        onclick: move |_| {
                            on_refresh.call(());
                            open.set(false);
                        },
                        span { class: "picker-text", {tr!("Refresh")} }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> BranchInfo {
        BranchInfo { current: Some("feature".to_string()), head_label: "feature".to_string(), default_base: Some("main".to_string()), branches: vec![] }
    }

    fn commit(sha: &str, subject: &str) -> CommitInfo {
        CommitInfo { sha: sha.to_string(), subject: subject.to_string(), author: "a".to_string(), time: 0 }
    }

    #[test]
    fn all_changes_name_the_base_and_the_branch() {
        assert_eq!(button_label(&ChangeView::All, Some(&info()), Some("main"), &[]), "main → feature");
    }

    #[test]
    fn all_changes_with_no_base_read_as_uncommitted() {
        assert_eq!(button_label(&ChangeView::All, Some(&info()), None, &[]), tr!("Uncommitted changes"));
    }

    #[test]
    fn a_commit_is_named_by_its_short_sha_and_subject() {
        let commits = vec![commit("d5bfbad1234", "Update a")];
        assert_eq!(button_label(&ChangeView::Commit("d5bfbad1234".to_string()), None, None, &commits), "d5bfbad Update a");
    }
}
