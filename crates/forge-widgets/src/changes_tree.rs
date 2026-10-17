use std::collections::HashSet;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::ChangesViewPrefs;
use forge_entities::icon_of;
use forge_features::{tree_rows, use_workspace, ChangeTreeRow, ChangeView, ROW_LIMIT};
use forge_ports::{ChangeStatus, ChangedFile, CommitInfo};
use forge_shared::tr;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::files_icons::FileTypeIcon;

/// The right column: the changed files as a folder tree (or flat), then whatever `commits` draws.
#[component]
pub fn ChangesTree(
    id: LiveSessionId,
    files: Vec<ChangedFile>,
    prefs: ChangesViewPrefs,
    closed: HashSet<String>,
    open_sections: HashSet<forge_entities::FileClass>,
    on_pick: EventHandler<String>,
    on_menu: EventHandler<(String, bool, f64, f64)>,
    commits: Element,
) -> Element {
    let workspace = use_workspace();
    let rows = tree_rows(&files, prefs.group_by_folder, &closed, prefs.separate_special, &open_sections);
    let hidden = rows.len().saturating_sub(ROW_LIMIT);
    rsx! {
        div { class: "cd-side",
            ul { class: "cd-tree",
                for row in rows.into_iter().take(ROW_LIMIT) {
                    match row {
                        ChangeTreeRow::Folder { path, name, depth, open } => rsx! {
                            li { key: "dir:{path}",
                                button {
                                    class: "cd-row",
                                    r#type: "button",
                                    style: "padding-left: {depth * 14 + 6}px;",
                                    onclick: {
                                        let path = path.clone();
                                        move |_| {
                                            workspace.update_changes_session(id, |session| {
                                                if !session.closed_folders.remove(&path) {
                                                    session.closed_folders.insert(path.clone());
                                                }
                                            })
                                        }
                                    },
                                    span { class: "cd-chevron", if open { "▾" } else { "▸" } }
                                    span { class: "cd-name", "{name}" }
                                }
                            }
                        },
                        ChangeTreeRow::Section { .. } => rsx! {},
                        ChangeTreeRow::File { path, name, depth, added, removed, status, .. } => {
                            let deleted = status == ChangeStatus::Deleted;
                            let counts = match (added, removed) {
                                (Some(added), Some(removed)) => rsx! {
                                    span { class: "cd-counts",
                                        span { class: "cd-add", "+{added}" }
                                        " "
                                        span { class: "cd-del", "−{removed}" }
                                    }
                                },
                                _ => rsx! {},
                            };
                            rsx! {
                                li { key: "file:{path}",
                                    button {
                                        class: "cd-row",
                                        r#type: "button",
                                        title: "{path}",
                                        style: "padding-left: {depth * 14 + 6}px;",
                                        onclick: {
                                            let path = path.clone();
                                            move |_| on_pick.call(path.clone())
                                        },
                                        oncontextmenu: {
                                            let path = path.clone();
                                            move |event: MouseEvent| {
                                                event.prevent_default();
                                                let point = event.client_coordinates();
                                                on_menu.call((path.clone(), deleted, point.x, point.y));
                                            }
                                        },
                                        FileTypeIcon { icon: icon_of(&path) }
                                        span { class: "cd-name", "{name}" }
                                        {counts}
                                    }
                                }
                            }
                        }
                    }
                }
                if hidden > 0 {
                    li { class: "cd-note", {tr!("… {count} more files", count = hidden)} }
                }
            }
            {commits}
        }
    }
}

/// `d5bfbad · author · 4m ago`.
pub(crate) fn commit_meta(commit: &CommitInfo, now: OffsetDateTime) -> String {
    let ago = OffsetDateTime::from_unix_timestamp(commit.time)
        .ok()
        .and_then(|then| then.format(&Rfc3339).ok())
        .map(|text| forge_entities::relative_time(&text, now))
        .unwrap_or_default();
    format!("{} · {} · {}", &commit.sha[..commit.sha.len().min(7)], commit.author, ago)
}

/// `Commits N ⌄`: the two modes and one row per commit of the branch.
#[component]
pub fn CommitsSection(id: LiveSessionId, commits: Vec<CommitInfo>, view: ChangeView) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| true);
    let now = OffsetDateTime::now_utc();
    rsx! {
        div { class: "cd-commits",
            div { class: "cd-commits-title", onclick: move |_| open.set(!open()),
                span { class: "cd-chevron", if open() { "▾" } else { "▸" } }
                span { {tr!("Commits")} }
                span { style: "margin-left: auto;", "{commits.len()}" }
            }
            if open() {
                button {
                    class: if view == ChangeView::All { "cd-commit selected" } else { "cd-commit" },
                    r#type: "button",
                    onclick: move |_| workspace.update_changes_session(id, |session| session.view = ChangeView::All),
                    span { class: "subject", {tr!("All changes")} }
                }
                button {
                    class: if view == ChangeView::Uncommitted { "cd-commit selected" } else { "cd-commit" },
                    r#type: "button",
                    onclick: move |_| workspace.update_changes_session(id, |session| session.view = ChangeView::Uncommitted),
                    span { class: "subject", {tr!("Uncommitted changes")} }
                }
                for commit in commits.iter() {
                    button {
                        key: "{commit.sha}",
                        class: if view == ChangeView::Commit(commit.sha.clone()) { "cd-commit selected" } else { "cd-commit" },
                        r#type: "button",
                        title: "{commit.subject}",
                        onclick: {
                            let sha = commit.sha.clone();
                            move |_| workspace.update_changes_session(id, |session| session.view = ChangeView::Commit(sha.clone()))
                        },
                        span { class: "subject", "{commit.subject}" }
                        span { class: "meta", "{commit_meta(commit, now)}" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_ports::CommitInfo;
    use time::OffsetDateTime;

    #[test]
    fn the_meta_line_has_the_short_sha_the_author_and_the_age() {
        let commit = CommitInfo { sha: "d5bfbad1234567".to_string(), subject: "s".to_string(), author: "myhency".to_string(), time: 1_000_000 };
        let now = OffsetDateTime::from_unix_timestamp(1_000_000 + 4 * 60).unwrap();
        assert_eq!(commit_meta(&commit, now), format!("d5bfbad · myhency · {}", tr!("{n}m ago", n = 4)));
    }
}
