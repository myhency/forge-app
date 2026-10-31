use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{use_workspace, EnvironmentRequest};
use forge_shared::tr;

/// The grey line above a session's first message: where it started, and either a link that saves
/// that folder as an environment or the name of the environment it already matches.
#[component]
pub fn StartLine(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let view = view.read().clone();
    if workspace.is_scratch_project(&view.project_path) {
        return rsx! {};
    }
    let origin = workspace.origin_of(&view.project_path);
    let folder = origin.file_name().map_or_else(|| origin.to_string_lossy().to_string(), |name| name.to_string_lossy().to_string());
    let lead = tr!("Starts in: {computer} · {folder} · ", computer = tr!("This computer"), folder = folder);
    let environment = workspace.session_environment(&view.project_path, view.claude_session_id.as_ref());
    rsx! {
        div { class: "start-line",
            span { "{lead}" }
            if let Some(environment) = environment {
                span { {tr!("Saved environment: \"{name}\"", name = environment.name)} }
            } else {
                button {
                    class: "start-line-link",
                    r#type: "button",
                    onclick: move |_| workspace.open_environment_dialog(EnvironmentRequest::NewFromSession { folder: origin.clone(), model: view.model.clone(), effort: view.effort }),
                    {tr!("Save as environment")}
                }
            }
        }
    }
}
