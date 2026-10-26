use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{copy_text, render_markdown};
use forge_features::{use_workspace, Panel};
use forge_shared::tr;

use crate::panel_frame::PanelFrame;

// The text selected in the rendered plan and where it ends (window coordinates, kept inside the
// window for a box 280 wide and about 190 tall), or nothing. No JS comments in here: the lines are joined.
const READ_SELECTION: &str = "const sel = window.getSelection(); \
    if (!sel || sel.isCollapsed || sel.rangeCount === 0) return null; \
    const body = document.querySelector('.plan-body .markdown'); \
    const range = sel.getRangeAt(0); \
    if (!body || !body.contains(range.commonAncestorContainer)) return null; \
    const text = sel.toString().trim(); \
    if (!text) return null; \
    const rect = range.getBoundingClientRect(); \
    return [text, Math.max(8, Math.min(rect.left, window.innerWidth - 300)), Math.max(8, Math.min(rect.bottom, window.innerHeight - 200))];";

/// The plan Claude proposed, rendered as markdown. Selecting text offers a comment on it; the
/// comments go with a request for changes. The answer itself is given in the card.
#[component]
pub fn PlanPanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let plan = workspace.plan_text(id);
    let copied = plan.clone();
    let mut picked = use_signal(|| None::<(String, f64, f64)>);
    let mut writing = use_signal(|| false);
    let mut draft = use_signal(String::new);
    let comments = workspace.plan_comments(id);
    rsx! {
        PanelFrame {
            id,
            panel: Panel::Plan,
            tools: rsx! {
                button {
                    class: "icon-button",
                    r#type: "button",
                    disabled: plan.is_none(),
                    title: tr!("Copy"),
                    "aria-label": tr!("Copy"),
                    onclick: move |_| {
                        if let Some(text) = copied.as_deref() {
                            copy_text(text);
                        }
                    },
                    "⧉"
                }
            },
            match plan {
                Some(text) => rsx! {
                    div {
                        class: "plan-body",
                        onmouseup: move |_| {
                            if writing() {
                                return;
                            }
                            spawn(async move {
                                let read = document::eval(READ_SELECTION).join::<Option<(String, f64, f64)>>().await.ok().flatten();
                                picked.set(read);
                            });
                        },
                        div { class: "dock-note", {tr!("Read the plan, then answer in the card below.")} }
                        div { class: "markdown", dangerous_inner_html: render_markdown(&text) }
                        if !comments.is_empty() {
                            div { class: "plan-comments",
                                div { class: "dock-note", {tr!("Your comments")} }
                                for (index, comment) in comments.iter().enumerate() {
                                    div { key: "{index}", class: "plan-comment",
                                        div { class: "plan-comment-quote", "{comment.quote}" }
                                        div { class: "plan-comment-row",
                                            span { class: "plan-comment-text", "{comment.text}" }
                                            button {
                                                class: "icon-button",
                                                r#type: "button",
                                                title: tr!("Remove"),
                                                "aria-label": tr!("Remove"),
                                                onclick: move |_| workspace.remove_plan_comment(id, index),
                                                "×"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if let Some((quote, x, y)) = picked() {
                            if writing() {
                                div { class: "plan-comment-box", style: "left: {x}px; top: {y}px;",
                                    textarea {
                                        class: "plan-card-text",
                                        rows: "3",
                                        placeholder: tr!("Add a comment…"),
                                        value: "{draft}",
                                        oninput: move |event| draft.set(event.value()),
                                        onmounted: move |mounted| async move {
                                            let _ = mounted.data().set_focus(true).await;
                                        },
                                    }
                                    div { class: "perm-buttons",
                                        button {
                                            class: "perm-button",
                                            r#type: "button",
                                            onclick: move |_| {
                                                writing.set(false);
                                                picked.set(None);
                                                draft.set(String::new());
                                            },
                                            {tr!("Cancel")}
                                        }
                                        span { class: "spacer" }
                                        button {
                                            class: "perm-button primary",
                                            r#type: "button",
                                            onclick: {
                                                let quote = quote.clone();
                                                move |_| {
                                                    workspace.add_plan_comment(id, quote.clone(), draft());
                                                    writing.set(false);
                                                    picked.set(None);
                                                    draft.set(String::new());
                                                }
                                            },
                                            {tr!("Add")}
                                        }
                                    }
                                }
                            } else {
                                button {
                                    class: "plan-comment-add",
                                    r#type: "button",
                                    style: "left: {x}px; top: {y}px;",
                                    // The selection must survive the press.
                                    onmousedown: move |event| event.prevent_default(),
                                    onclick: move |_| writing.set(true),
                                    {tr!("Add comment")}
                                }
                            }
                        }
                    }
                },
                None => rsx! { div { class: "dock-note", {tr!("No plan yet")} } },
            }
        }
    }
}
