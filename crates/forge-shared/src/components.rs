use dioxus::prelude::*;
use forge_domain::tr;

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

/// A row of mutually exclusive choices over `(value, label)` pairs, shown as one segmented control.
#[component]
pub fn Segmented(label: String, value: String, options: Vec<(String, String)>, onchange: EventHandler<String>) -> Element {
    rsx! {
        div { class: "view-switch", role: "radiogroup", "aria-label": "{label}",
            for (option_value, option_label) in options.iter().cloned() {
                button {
                    key: "{option_value}",
                    class: if option_value == value { "view-option selected" } else { "view-option" },
                    role: "radio",
                    "aria-checked": "{option_value == value}",
                    onclick: move |_| onchange.call(option_value.clone()),
                    "{option_label}"
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

/// Rows of the shape of the list that is on its way, shown in place of the list while it loads.
#[component]
pub fn LoadingRows() -> Element {
    rsx! { div { class: "skeleton-list", role: "status", "aria-label": tr!("Loading…"), div { class: "skeleton" } div { class: "skeleton" } div { class: "skeleton" } } }
}

/// A shape of an icon, drawn on a 16 by 16 grid with the same round stroke the file icons use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    Path(&'static str),
    Circle { cx: f32, cy: f32, r: f32 },
    Rect { x: f32, y: f32, width: f32, height: f32, rx: f32 },
    /// A filled dot (the strokeless mark of the "more" menu).
    Dot { cx: f32, cy: f32 },
}

/// The pictures that stand in for a glyph or an emoji. One chevron points right; the open state turns it with CSS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Close,
    Search,
    Sparkle,
    Clock,
    Folder,
    FolderPlus,
    Eye,
    Code,
    Trash,
    Archive,
    Menu,
    Sort,
    Pencil,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Send,
    More,
    Target,
    Chevron,
    Check,
}

impl IconKind {
    pub const ALL: [IconKind; 22] = [
        IconKind::Close,
        IconKind::Search,
        IconKind::Sparkle,
        IconKind::Clock,
        IconKind::Folder,
        IconKind::FolderPlus,
        IconKind::Eye,
        IconKind::Code,
        IconKind::Trash,
        IconKind::Archive,
        IconKind::Menu,
        IconKind::Sort,
        IconKind::Pencil,
        IconKind::ArrowUp,
        IconKind::ArrowDown,
        IconKind::ArrowLeft,
        IconKind::ArrowRight,
        IconKind::Send,
        IconKind::More,
        IconKind::Target,
        IconKind::Chevron,
        IconKind::Check,
    ];

    pub fn shapes(self) -> &'static [Shape] {
        const FOLDER: &str = "M2 4.5A1 1 0 0 1 3 3.5h3l1.5 1.5H13a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z";
        match self {
            IconKind::Close => &[Shape::Path("M4 4l8 8M12 4l-8 8")],
            IconKind::Search => &[Shape::Circle { cx: 7.0, cy: 7.0, r: 4.5 }, Shape::Path("M10.5 10.5L14 14")],
            IconKind::Sparkle => &[Shape::Path("M8 2l1.5 4.5L14 8l-4.5 1.5L8 14l-1.5-4.5L2 8l4.5-1.5z")],
            IconKind::Clock => &[Shape::Circle { cx: 8.0, cy: 8.0, r: 6.0 }, Shape::Path("M8 4.5V8l2.5 1.5")],
            IconKind::Folder => &[Shape::Path(FOLDER)],
            IconKind::FolderPlus => &[Shape::Path(FOLDER), Shape::Path("M8 7v4M6 9h4")],
            IconKind::Eye => &[Shape::Path("M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8s-2.5 4.5-6.5 4.5S1.5 8 1.5 8z"), Shape::Circle { cx: 8.0, cy: 8.0, r: 2.0 }],
            IconKind::Code => &[Shape::Path("M6 5L3 8l3 3M10 5l3 3-3 3")],
            IconKind::Trash => &[Shape::Path("M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.5 8h6l.5-8M7 7v3.5M9 7v3.5")],
            IconKind::Archive => &[Shape::Rect { x: 2.0, y: 3.0, width: 12.0, height: 3.0, rx: 1.0 }, Shape::Path("M3 6v6.5h10V6M6.5 9h3")],
            IconKind::Menu => &[Shape::Path("M3 4.5h10M3 8h10M3 11.5h10")],
            IconKind::Sort => &[Shape::Path("M5 13V3M5 3L2.5 5.5M5 3l2.5 2.5M11 3v10M11 13l-2.5-2.5M11 13l2.5-2.5")],
            IconKind::Pencil => &[Shape::Path("M3 13l.7-3L10.5 3.2a1.4 1.4 0 0 1 2 2L5.7 12.3zM9.5 4.2l2 2")],
            IconKind::ArrowUp => &[Shape::Path("M8 13V3M8 3L4 7M8 3l4 4")],
            IconKind::ArrowDown => &[Shape::Path("M8 3v10M8 13L4 9M8 13l4-4")],
            IconKind::ArrowLeft => &[Shape::Path("M13 8H3M3 8l4-4M3 8l4 4")],
            IconKind::ArrowRight => &[Shape::Path("M3 8h10M13 8l-4-4M13 8l-4 4")],
            IconKind::Send => &[Shape::Path("M12.5 3.5v5h-9M3.5 8.5l3-3M3.5 8.5l3 3")],
            IconKind::More => &[Shape::Dot { cx: 8.0, cy: 3.5 }, Shape::Dot { cx: 8.0, cy: 8.0 }, Shape::Dot { cx: 8.0, cy: 12.5 }],
            IconKind::Target => &[Shape::Circle { cx: 8.0, cy: 8.0, r: 6.0 }, Shape::Circle { cx: 8.0, cy: 8.0, r: 2.0 }],
            IconKind::Chevron => &[Shape::Path("M6 3.5L10.5 8 6 12.5")],
            IconKind::Check => &[Shape::Path("M3.5 8.5l3 3 6-7")],
        }
    }
}

/// The two sizes an icon takes: the default, and a small one for chevrons and marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconSize {
    #[default]
    Normal,
    Small,
}

/// An icon in the colour of the text around it. `open` turns a chevron to point down.
#[component]
pub fn Icon(kind: IconKind, #[props(default)] size: IconSize, #[props(default)] open: bool) -> Element {
    let class = format!("icon{}{}", if size == IconSize::Small { " sm" } else { "" }, if open { " open" } else { "" });
    rsx! {
        svg {
            class: "{class}",
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            for shape in kind.shapes() {
                match *shape {
                    Shape::Path(d) => rsx! { path { d } },
                    Shape::Circle { cx, cy, r } => rsx! { circle { cx: "{cx}", cy: "{cy}", r: "{r}" } },
                    Shape::Rect { x, y, width, height, rx } => rsx! { rect { x: "{x}", y: "{y}", width: "{width}", height: "{height}", rx: "{rx}" } },
                    Shape::Dot { cx, cy } => rsx! { circle { cx: "{cx}", cy: "{cy}", r: "1", fill: "currentColor", stroke: "none" } },
                }
            }
        }
    }
}

/// The run mark of a code block, as markup, for the chat's HTML. It sets its own size and colour because that page has no stylesheet of ours.
pub const RUN_ICON_SVG: &str = r#"<svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 3.5l7 4.5-7 4.5z"/></svg>"#;

/// The "running" mark shown next to a result while a block runs, as markup for the page of the code runner.
pub const PENDING_ICON_SVG: &str = r#"<svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true"><circle cx="8" cy="8" r="5.5" stroke-dasharray="2.5 2.5"/></svg>"#;

#[cfg(test)]
mod icon_tests {
    use super::*;

    #[test]
    fn every_icon_has_a_shape_and_every_path_is_drawn() {
        assert_eq!(IconKind::ALL.len(), 22);
        for kind in IconKind::ALL {
            let shapes = kind.shapes();
            assert!(!shapes.is_empty(), "{kind:?} has no shape");
            for shape in shapes {
                if let Shape::Path(d) = shape {
                    assert!(d.starts_with('M') && d.len() > 4, "{kind:?}: {d}");
                }
            }
        }
    }

    #[test]
    fn the_markup_icons_carry_their_own_size_stroke_and_colour() {
        for svg in [RUN_ICON_SVG, PENDING_ICON_SVG] {
            for attribute in ["width=\"12\"", "height=\"12\"", "stroke=\"currentColor\"", "stroke-width=\"1.5\"", "aria-hidden=\"true\""] {
                assert!(svg.contains(attribute), "{attribute} missing from {svg}");
            }
        }
    }
}
