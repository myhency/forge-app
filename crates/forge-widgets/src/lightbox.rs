use base64::Engine;
use dioxus::prelude::*;
use forge_features::{run_blocking, use_workspace, Picture};
use forge_ports::FileContent;
use forge_shared::{tr, Icon, IconKind, IconSize};

/// A picture over the dimmed window, opened by a click on a picture path in the chat: the picture
/// in the middle, its path under it, `×` at its corner. A click anywhere, `×` or Escape closes it.
/// It draws nothing while no picture is asked for.
#[component]
pub fn Lightbox() -> Element {
    let workspace = use_workspace();
    match workspace.lightbox() {
        Some(picture) => rsx! { LightboxBody { key: "{picture.relative}", picture } },
        None => rsx! {},
    }
}

#[component]
fn LightboxBody(picture: Picture) -> Element {
    let workspace = use_workspace();
    let source = picture.clone();
    let image = use_resource(move || {
        let (files, folder, relative) = (workspace.files(), source.folder.clone(), source.relative.clone());
        async move { run_blocking(move || files.read(&folder, &relative)).await }
    });
    // A picture that cannot be read or is too large closes the lightbox with a notice, so the window
    // is never left dimmed with nothing in it.
    use_effect(move || {
        if let Some(done) = &*image.read() {
            if !matches!(done, Some(Ok(FileContent::Image { .. }))) {
                workspace.report(tr!("This picture cannot be shown.").to_string());
                workspace.close_lightbox();
            }
        }
    });
    let caption = picture.folder.join(&picture.relative).to_string_lossy().to_string();
    let data_url = match &*image.read() {
        Some(Some(Ok(FileContent::Image { bytes, mime }))) => {
            Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
        }
        _ => None,
    };
    rsx! {
        div {
            class: "lightbox-backdrop",
            tabindex: "0",
            role: "dialog",
            "aria-modal": "true",
            "aria-label": "Picture",
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onclick: move |_| workspace.close_lightbox(),
            onkeydown: move |event: KeyboardEvent| {
                // Nothing behind the picture takes keys while it is shown.
                event.stop_propagation();
                if event.key() == Key::Escape {
                    workspace.close_lightbox();
                } else if event.key() == Key::Tab {
                    event.prevent_default();
                }
            },
            if let Some(data_url) = data_url {
                figure { class: "lightbox-figure",
                    div { class: "lightbox-frame",
                        img { class: "lightbox-image", src: "{data_url}", alt: "{caption}" }
                        button {
                            class: "lightbox-close",
                            r#type: "button",
                            title: tr!("Close"),
                            "aria-label": "Close",
                            onclick: move |event| {
                                event.stop_propagation();
                                workspace.close_lightbox();
                            },
                            Icon { kind: IconKind::Close, size: IconSize::Small }
                        }
                    }
                    figcaption { class: "lightbox-caption", "{caption}" }
                }
            }
        }
    }
}
