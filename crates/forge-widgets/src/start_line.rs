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
    // The desktop app (2026-11-06), from a folder's second session on without an environment:
    // "이 컴퓨터 · forge-observe 위치의 두 번째 세션입니다. 환경을 만들어 두면 클릭 한 번으로 시작할 수
    // 있습니다 · 환경으로 저장 · 나중에". Which sessions it counts was not seen; Forge counts the folder's.
    let count = workspace.folder_session_count(&origin);
    if environment.is_none() && count >= 2 && !workspace.env_hint_dismissed(&origin) {
        let hint = tr!("This is the {nth} session in {computer} · {folder}. Save it as an environment to start it in one click", nth = ordinal(count), computer = tr!("This computer"), folder = folder);
        let later = origin.clone();
        return rsx! {
            div { class: "start-line",
                span { "{hint} · " }
                button {
                    class: "start-line-link",
                    r#type: "button",
                    onclick: move |_| workspace.open_environment_dialog(EnvironmentRequest::NewFromSession { folder: origin.clone(), model: view.model.clone(), effort: view.effort }),
                    {tr!("Save as environment")}
                }
                span { " · " }
                button { class: "start-line-link", r#type: "button", onclick: move |_| workspace.dismiss_env_hint(later.clone()), {tr!("Later")} }
            }
        };
    }
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

/// "second", "third"… in the language on screen; past the tenth, a number.
fn ordinal(n: usize) -> String {
    const KO: [&str; 9] = ["두", "세", "네", "다섯", "여섯", "일곱", "여덟", "아홉", "열"];
    const EN: [&str; 9] = ["second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth"];
    let korean = forge_shared::i18n::current() == forge_domain::Language::Korean;
    match (n, korean) {
        (2..=10, true) => format!("{} 번째", KO[n - 2]),
        (2..=10, false) => EN[n - 2].to_string(),
        (_, true) => format!("{n}번째"),
        (_, false) => format!("{n}th"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals_read_as_words_up_to_the_tenth() {
        forge_shared::i18n::set_current(forge_domain::Language::English);
        assert_eq!(ordinal(2), "second");
        assert_eq!(ordinal(11), "11th");
        forge_shared::i18n::set_current(forge_domain::Language::Korean);
        assert_eq!(ordinal(2), "두 번째");
        assert_eq!(ordinal(12), "12번째");
        forge_shared::i18n::set_current(forge_domain::Language::English);
    }
}
