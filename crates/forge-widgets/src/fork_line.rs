use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::relative_time;
use forge_features::use_workspace;
use forge_shared::tr;
use time::OffsetDateTime;

/// The grey line at the top of a fork's chat: the session it came from (a link to it) and how long ago the fork began.
#[component]
pub fn ForkLine(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(origin) = workspace.fork_origin(id) else { return rsx! {} };
    let time = relative_time(&origin.forked_at, OffsetDateTime::now_utc());
    let label = tr!("Forked from");
    let project = origin.project.clone();
    let session_id = origin.session_id.clone();
    rsx! {
        div { class: "start-line fork-line",
            span { "{label} " }
            button {
                class: "start-line-link",
                r#type: "button",
                onclick: move |_| workspace.open_past_clicked(project.clone(), session_id.clone()),
                "{origin.title}"
            }
            if !time.is_empty() {
                span { " {time}" }
            }
        }
    }
}
