use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonKind {
    #[default]
    Default,
    Primary,
    Danger,
}

impl ButtonKind {
    fn class(self) -> &'static str {
        match self {
            ButtonKind::Default => "button",
            ButtonKind::Primary => "button primary",
            ButtonKind::Danger => "button danger",
        }
    }
}

#[component]
pub fn Button(
    label: String,
    #[props(default)] kind: ButtonKind,
    #[props(default)] disabled: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: kind.class(),
            disabled,
            onclick: move |evt| onclick.call(evt),
            "{label}"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Muted,
    Active,
    Warning,
}

impl Tone {
    fn class(self) -> &'static str {
        match self {
            Tone::Muted => "status-dot muted",
            Tone::Active => "status-dot active",
            Tone::Warning => "status-dot warning",
        }
    }
}

#[component]
pub fn StatusDot(tone: Tone) -> Element {
    rsx! { span { class: tone.class() } }
}

/// A dropdown over `(value, label)` pairs.
#[component]
pub fn Select(
    value: String,
    options: Vec<(String, String)>,
    #[props(default)] disabled: bool,
    onchange: EventHandler<String>,
) -> Element {
    rsx! {
        select {
            class: "select",
            disabled,
            value: "{value}",
            onchange: move |evt| onchange.call(evt.value()),
            for (option_value, label) in options.iter() {
                option {
                    key: "{option_value}",
                    value: "{option_value}",
                    selected: *option_value == value,
                    "{label}"
                }
            }
        }
    }
}

#[component]
pub fn TextArea(
    value: String,
    #[props(default)] placeholder: String,
    #[props(default)] disabled: bool,
    oninput: EventHandler<String>,
    #[props(default)] onkeydown: EventHandler<KeyboardEvent>,
) -> Element {
    rsx! {
        textarea {
            class: "textarea",
            value: "{value}",
            placeholder: "{placeholder}",
            disabled,
            oninput: move |evt| oninput.call(evt.value()),
            onkeydown: move |evt| onkeydown.call(evt),
        }
    }
}
