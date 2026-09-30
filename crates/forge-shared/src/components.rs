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
    rsx! {
        span { class: tone.class(),
            if tone == Tone::Warning {
                "!"
            }
        }
    }
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

/// A one-line text field; `secret` hides the characters (an API key).
#[component]
pub fn TextInput(
    value: String,
    #[props(default)] placeholder: String,
    #[props(default)] secret: bool,
    oninput: EventHandler<String>,
) -> Element {
    rsx! {
        input {
            class: "text-input",
            r#type: if secret { "password" } else { "text" },
            value: "{value}",
            placeholder: "{placeholder}",
            autocomplete: "off",
            spellcheck: "false",
            oninput: move |evt| oninput.call(evt.value()),
        }
    }
}

/// One settings row: a title and a line of explanation on the left, the control (`children`) on the right.
#[component]
pub fn SettingRow(title: String, #[props(default)] description: String, children: Element) -> Element {
    rsx! {
        div { class: "setting-row",
            div { class: "setting-info",
                div { class: "setting-title", "{title}" }
                if !description.is_empty() {
                    div { class: "setting-desc", "{description}" }
                }
            }
            div { class: "setting-control", {children} }
        }
    }
}

/// An on/off switch. Clicking asks for the opposite of the current state via `onchange`.
#[component]
pub fn Switch(on: bool, #[props(default)] disabled: bool, onchange: EventHandler<bool>) -> Element {
    rsx! {
        button {
            class: if on { "switch on" } else { "switch" },
            role: "switch",
            "aria-checked": "{on}",
            disabled,
            onclick: move |_| onchange.call(!on),
            span { class: "switch-knob" }
        }
    }
}
