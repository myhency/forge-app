use dioxus::prelude::*;
use forge_domain::Appearance;

/// The script that puts the person's choices on the root element, where `theme.css` reads them.
pub fn apply_appearance_script(appearance: &Appearance) -> String {
    format!(
        "const root = document.documentElement; root.setAttribute('data-theme', '{}'); root.setAttribute('data-font', '{}'); root.setAttribute('data-text-size', '{}'); root.setAttribute('data-width', '{}'); root.setAttribute('data-motion', '{}');",
        appearance.theme.value(),
        appearance.font.value(),
        appearance.text_size.value(),
        appearance.chat_width.value(),
        appearance.motion.value(),
    )
}

/// Applies the choices to the open page at once.
pub fn apply_appearance(appearance: &Appearance) {
    document::eval(&apply_appearance_script(appearance));
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{ChatWidth, FontChoice, MotionPref, TextSize, Theme};

    #[test]
    fn the_script_sets_the_five_attributes_on_the_root_element() {
        let script = apply_appearance_script(&Appearance { theme: Theme::Dark, font: FontChoice::OpenDyslexic, text_size: TextSize::Large, chat_width: ChatWidth::Narrow, motion: MotionPref::Reduced });
        for expected in ["data-theme', 'dark'", "data-font', 'opendyslexic'", "data-text-size', 'large'", "data-width', 'narrow'", "data-motion', 'reduced'"] {
            assert!(script.contains(expected), "{script}");
        }
        assert!(script.contains("document.documentElement"));
    }
}
