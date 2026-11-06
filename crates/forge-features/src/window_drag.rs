//! Moving the window by its top row. On macOS the window has no title bar of its own (the desktop
//! app's look: the traffic lights sit in the sidebar's top row), so the empty parts of the sidebar's
//! top row and of the chat header move the window instead. The app provides the mover; without it
//! (Windows keeps its title bar, and tests) the spacer is only space.

use dioxus::prelude::*;

/// Starts moving the window with the pointer; provided by the app on macOS.
#[derive(Clone, Copy)]
pub struct WindowDrag(pub Callback<()>);

/// An empty, stretching part of a top row that moves the window when pressed.
#[component]
pub fn DragSpacer() -> Element {
    let drag = try_use_context::<WindowDrag>();
    rsx! {
        div {
            class: "drag-spacer",
            onmousedown: move |event: MouseEvent| {
                if let Some(drag) = drag {
                    if event.trigger_button() == Some(dioxus::html::input_data::MouseButton::Primary) {
                        drag.0.call(());
                    }
                }
            },
        }
    }
}
