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

/// The script that puts the code look's stylesheet into the page, replacing the earlier one.
pub fn apply_code_look_script(look: &forge_domain::CodeLook) -> String {
    let css = serde_json::to_string(&forge_domain::code_look_css(look)).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "let sheet = document.getElementById('forge-code-look'); if (!sheet) {{ sheet = document.createElement('style'); sheet.id = 'forge-code-look'; document.head.appendChild(sheet); }} sheet.textContent = {css};"
    )
}

/// Applies the code colours, font and size to the open page at once.
pub fn apply_code_look(look: &forge_domain::CodeLook) {
    document::eval(&apply_code_look_script(look));
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
    fn the_code_look_goes_into_one_style_element_as_a_json_string() {
        let look = forge_domain::CodeLook { light: forge_domain::CodeTheme::Github, ..Default::default() };
        let script = apply_code_look_script(&look);
        assert!(script.contains("forge-code-look"), "{script}");
        assert!(script.contains("sheet.textContent = \":root:root"), "{script}");
    }

    #[test]
    fn the_script_sets_the_five_attributes_on_the_root_element() {
        let script = apply_appearance_script(&Appearance { theme: Theme::Dark, font: FontChoice::OpenDyslexic, text_size: TextSize::Large, chat_width: ChatWidth::Narrow, motion: MotionPref::Reduced });
        for expected in ["data-theme', 'dark'", "data-font', 'opendyslexic'", "data-text-size', 'large'", "data-width', 'narrow'", "data-motion', 'reduced'"] {
            assert!(script.contains(expected), "{script}");
        }
        assert!(script.contains("document.documentElement"));
    }
}
