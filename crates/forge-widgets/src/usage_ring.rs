use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{ring_dash, UsageView};
use forge_features::use_workspace;
use forge_shared::tr;

/// The context ring beside the message box. Clicking it opens a popover with the session's token
/// totals, an estimated cost and a line per model.
#[component]
pub fn UsageRing(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let Some(view_signal) = workspace.live_view(id) else {
        return rsx! {};
    };
    let usage = {
        let view = view_signal.read();
        UsageView::for_session(&view.session.usage, view.context)
    };

    let context_text = usage
        .context
        .as_ref()
        .map(|context| context.label.clone())
        .unwrap_or_else(|| tr!("Not measured yet — it appears after the first reply.").to_string());
    let cost_text = usage.cost.clone().unwrap_or_else(|| "—".to_string());
    let (level, dash, tooltip) = match &usage.context {
        Some(context) => (context.level.class(), ring_dash(context.percent), context.label.clone()),
        None => ("empty", ring_dash(0), tr!("Context window not measured yet").to_string()),
    };

    rsx! {
        div { class: "usage",
            button {
                class: "usage-ring {level}",
                title: "{tooltip}",
                aria_label: tr!("Session usage"),
                onclick: move |_| open.set(!open()),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        open.set(false);
                    }
                },
                svg { view_box: "0 0 20 20",
                    circle { class: "track", cx: "10", cy: "10", r: "8" }
                    circle { class: "arc", cx: "10", cy: "10", r: "8", stroke_dasharray: "{dash}" }
                }
            }
            if open() {
                div { class: "usage-backdrop", onclick: move |_| open.set(false) }
                div { class: "usage-popover",
                    div { class: "usage-section",
                        div { class: "usage-heading", {tr!("Context window")} }
                        div { class: "usage-row", span { "{context_text}" } }
                    }
                    div { class: "usage-section",
                        div { class: "usage-heading", {tr!("This session")} }
                        div { class: "usage-row", span { {tr!("Input")} } span { class: "value", "{usage.input}" } }
                        div { class: "usage-row", span { {tr!("Output")} } span { class: "value", "{usage.output}" } }
                        div { class: "usage-row", span { {tr!("Cache read")} } span { class: "value", "{usage.cache_read}" } }
                        div { class: "usage-row", span { {tr!("Cache write")} } span { class: "value", "{usage.cache_write}" } }
                        if let Some(searches) = usage.web_searches.as_ref() {
                            div { class: "usage-row", span { {tr!("Web searches")} } span { class: "value", "{searches}" } }
                        }
                    }
                    div { class: "usage-section",
                        div { class: "usage-heading", {tr!("Estimated cost")} }
                        div { class: "usage-row", span { {tr!("Total")} } span { class: "value", "{cost_text}" } }
                        if let Some(note) = usage.cost_note.as_ref() {
                            div { class: "usage-note", "{note}" }
                        }
                        div { class: "usage-note", {tr!("List-price estimate; it may differ from your bill.")} }
                    }
                    if !usage.models.is_empty() {
                        div { class: "usage-section",
                            div { class: "usage-heading", {tr!("By model")} }
                            for line in usage.models.iter() {
                                div { class: "usage-row", key: "{line.model}",
                                    span { "{line.model}" }
                                    span { class: "value", "{line.tokens} · {line.cost}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
