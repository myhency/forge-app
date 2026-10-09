# Settings window, appearance and notifications Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Settings becomes a grouped window with search; a General page holds Appearance (theme, font, text size, chat width, motion) and Notifications (four switches); the Claude Code page gets a Dock-bounce switch.

**Architecture:** New `AppConfig` fields (`appearance`, `notifications`, `attention_bounce`) saved through the existing `SettingsUseCase::save_code_settings` closure (one save path for all instant-save controls). The page root carries `data-*` attributes set by `document::eval`; `theme.css` keys its variables on them. Session notifications come from a watcher in `forge-features` that compares each live session's view with its previous snapshot through a pure rule function.

**Tech Stack:** Rust, Dioxus 0.7.10 desktop, serde, mockall, plain CSS.

**Spec:** `docs/superpowers/specs/2026-10-09-settings-window-design.md`

## Global Constraints

- Base branch is `master` (not `main`). Work on branch `plan-33`.
- Every new `AppConfig` field is `#[serde(default)]` (fields with a non-false default use a named default fn; enum-like values use `deserialize_with = "lenient"`) so an old config file loads with today's look.
- Controls save at once through `SettingsUseCase::save_code_settings`; saves use `dioxus::core::spawn_forever` (a tab switch must not cancel a save).
- Never store a menu row index across renders; store the value.
- Run tests with `cargo test -p <crate>`; `forge-adapter-claude-plugins --test round_trip` fails on master and is not ours.
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Code themes are not a choice; highlight colours stay the existing `--hl-*` variables, which flip with the theme.
- Fonts: System and OpenDyslexic only; OpenDyslexic is used if installed, never bundled.

## Review Focus

- Old config file without the new fields: loads, and looks/behaves as before (Task 1).
- A hand-edited unknown value (`"theme":"purple"`): reads as the default, rest of the file loads (Task 1).
- Theme `system` must follow the OS in both directions; `light` must stay light under a dark OS (Task 3 CSS test).
- A background session that finishes while the window is focused on another session still notifies; the session being looked at never does (Task 5).
- A permission request notifies once, not on every view update (Task 5).
- Search with no match shows an empty-state, and a search match in a row title opens that page (Task 4).
- Settings reopened after close lands on the last page, even when the search text was non-empty (Task 4).

---

### Task 1: Config fields

**Files:**
- Modify: `crates/forge-domain/src/config.rs` (AppConfig ~146-180, Default ~189-215, tests)
- Modify: `crates/forge-domain/src/lib.rs` (re-exports)

**Interfaces:**
- Produces: `forge_domain::{Theme, FontChoice, TextSize, ChatWidth, MotionPref, Appearance, NotificationPrefs}`; `AppConfig.appearance: Appearance`, `AppConfig.notifications: NotificationPrefs`, `AppConfig.attention_bounce: bool`. Each enum has `ALL: [Self; N]`, `label(self) -> &'static str`, `value(self) -> &'static str` (the string used in `data-*` attributes) and derives `Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize` with `#[serde(rename_all = "snake_case")]`.

- [ ] **Step 1: Write the failing tests** (append to `mod tests` in config.rs)

```rust
    #[test]
    fn an_old_config_loads_with_todays_look_and_all_notifications_on() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();
        assert_eq!(config.appearance, Appearance::default());
        assert_eq!(config.appearance.theme, Theme::System);
        assert_eq!(config.appearance.font, FontChoice::System);
        assert_eq!(config.appearance.text_size, TextSize::Normal);
        assert_eq!(config.appearance.chat_width, ChatWidth::Normal);
        assert_eq!(config.appearance.motion, MotionPref::System);
        assert!(config.notifications.reply_done && config.notifications.scheduled_task);
        assert!(config.notifications.code_notice && config.notifications.permission_request);
        assert!(!config.attention_bounce);
    }

    #[test]
    fn an_unknown_appearance_value_reads_as_the_default_and_the_rest_still_loads() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":"https://x","appearance":{"theme":"purple","text_size":"large"}}"#).unwrap();
        assert_eq!(config.base_url.as_deref(), Some("https://x"));
        assert_eq!(config.appearance.theme, Theme::System);
        assert_eq!(config.appearance.text_size, TextSize::Large);
    }

    #[test]
    fn appearance_and_notifications_survive_a_round_trip() {
        let mut config = AppConfig::default();
        config.appearance = Appearance { theme: Theme::Dark, font: FontChoice::OpenDyslexic, text_size: TextSize::Small, chat_width: ChatWidth::Wide, motion: MotionPref::Reduced };
        config.notifications.reply_done = false;
        config.attention_bounce = true;
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.appearance, config.appearance);
        assert!(!back.notifications.reply_done);
        assert!(back.attention_bounce);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p forge-domain appearance 2>&1 | tail -15`
Expected: compile error, `Appearance` not found.

- [ ] **Step 3: Implement.** Add above `AppConfig` in config.rs:

```rust
macro_rules! choice_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => ($value:literal, $label:literal)),+ $(,)? } default $default:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $(#[allow(missing_docs)] $variant),+
        }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];
            /// The text used in the page's `data-*` attribute.
            pub fn value(self) -> &'static str { match self { $($name::$variant => $value),+ } }
            pub fn label(self) -> &'static str { match self { $($name::$variant => $label),+ } }
            pub fn from_value(value: &str) -> Option<Self> { Self::ALL.iter().copied().find(|choice| choice.value() == value) }
        }
    };
}
```

The macro cannot mark a `#[default]` variant, so write the enums with a manual `Default`:

```rust
choice_enum!(/// Light, dark, or whatever the system says.
    Theme { System => ("system", "System"), Light => ("light", "Light"), Dark => ("dark", "Dark") } default System);
impl Default for Theme { fn default() -> Self { Theme::System } }
```

Remove `Default` from the macro's derive list and give each enum its manual `impl Default` (`FontChoice::System`, `TextSize::Normal`, `ChatWidth::Normal`, `MotionPref::System`). Define:

```rust
choice_enum!(FontChoice { System => ("system", "System"), OpenDyslexic => ("opendyslexic", "OpenDyslexic") } default System);
choice_enum!(TextSize { Small => ("small", "Small"), Normal => ("normal", "Normal"), Large => ("large", "Large") } default Normal);
choice_enum!(ChatWidth { Narrow => ("narrow", "Narrow"), Normal => ("normal", "Normal"), Wide => ("wide", "Wide") } default Normal);
choice_enum!(MotionPref { System => ("system", "System"), Reduced => ("reduced", "Reduced") } default System);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Appearance {
    #[serde(default, deserialize_with = "lenient")]
    pub theme: Theme,
    #[serde(default, deserialize_with = "lenient")]
    pub font: FontChoice,
    #[serde(default, deserialize_with = "lenient")]
    pub text_size: TextSize,
    #[serde(default, deserialize_with = "lenient")]
    pub chat_width: ChatWidth,
    #[serde(default, deserialize_with = "lenient")]
    pub motion: MotionPref,
}

/// Which notifications Forge sends. All on until the person turns one off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NotificationPrefs {
    #[serde(default = "default_true")]
    pub reply_done: bool,
    #[serde(default = "default_true")]
    pub scheduled_task: bool,
    #[serde(default = "default_true")]
    pub code_notice: bool,
    #[serde(default = "default_true")]
    pub permission_request: bool,
}
impl Default for NotificationPrefs {
    fn default() -> Self { Self { reply_done: true, scheduled_task: true, code_notice: true, permission_request: true } }
}
```

Add to `AppConfig` after `allow_bypass_mode`:

```rust
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub notifications: NotificationPrefs,
    /// Bounce the Dock icon when a notification says Claude needs the person.
    #[serde(default)]
    pub attention_bounce: bool,
```

and to `Default for AppConfig`: `appearance: Appearance::default(), notifications: NotificationPrefs::default(), attention_bounce: false,`. Export `Appearance, ChatWidth, FontChoice, MotionPref, NotificationPrefs, TextSize, Theme` in `lib.rs` beside `AppConfig`. (If the macro's doc-comment argument form does not compile, drop the `$(#[$meta:meta])*` part and write plain `///` comments above the invocations.)

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p forge-domain 2>&1 | tail -8`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain && git commit -m "feat: config fields for appearance, notifications and Dock bounce" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Settings state for the new controls

**Files:**
- Modify: `crates/forge-features/src/settings.rs` (`apply_code_change`, `CodeChange`, getters/setters, `after_save_code`, tests)

**Interfaces:**
- Consumes: Task 1 types.
- Produces on `Settings` (all `&self`): `appearance() -> Appearance`, `notifications() -> NotificationPrefs`, `attention_bounce() -> bool`, `set_theme(Theme)`, `set_font(FontChoice)`, `set_text_size(TextSize)`, `set_chat_width(ChatWidth)`, `set_motion(MotionPref)`, `set_notification(NotificationKind, bool)`, `set_attention_bounce(bool)`. `pub enum NotificationKind { ReplyDone, ScheduledTask, CodeNotice, PermissionRequest }`. `CodeChange` gains matching variants. A saved config is also what `Settings::saved_config()` returns: `pub fn saved_config(&self) -> AppConfig` (used by Task 3 and 5).

- [ ] **Step 1: Failing test** (in `mod tests`)

```rust
    #[test]
    fn the_new_controls_change_their_own_field_only() {
        let mut config = AppConfig::default();
        apply_code_change(&mut config, CodeChange::Theme(Theme::Dark));
        apply_code_change(&mut config, CodeChange::Font(FontChoice::OpenDyslexic));
        apply_code_change(&mut config, CodeChange::TextSize(TextSize::Large));
        apply_code_change(&mut config, CodeChange::ChatWidth(ChatWidth::Narrow));
        apply_code_change(&mut config, CodeChange::Motion(MotionPref::Reduced));
        apply_code_change(&mut config, CodeChange::Notification(NotificationKind::ReplyDone, false));
        apply_code_change(&mut config, CodeChange::AttentionBounce(true));
        assert_eq!(config.appearance.theme, Theme::Dark);
        assert_eq!(config.appearance.font, FontChoice::OpenDyslexic);
        assert_eq!(config.appearance.text_size, TextSize::Large);
        assert_eq!(config.appearance.chat_width, ChatWidth::Narrow);
        assert_eq!(config.appearance.motion, MotionPref::Reduced);
        assert!(!config.notifications.reply_done);
        assert!(config.notifications.permission_request, "the other switches stay on");
        assert!(config.attention_bounce);
        assert!(config.prompt_suggestions, "nothing else is touched");
    }
```

- [ ] **Step 2:** Run `cargo test -p forge-features the_new_controls 2>&1 | tail` → FAIL (variants missing).

- [ ] **Step 3: Implement.** Extend `CodeChange` (keep `Copy`) with `Theme(Theme)`, `Font(FontChoice)`, `TextSize(TextSize)`, `ChatWidth(ChatWidth)`, `Motion(MotionPref)`, `Notification(NotificationKind, bool)`, `AttentionBounce(bool)`; add the arms to `apply_code_change`:

```rust
        CodeChange::Theme(value) => config.appearance.theme = value,
        CodeChange::Font(value) => config.appearance.font = value,
        CodeChange::TextSize(value) => config.appearance.text_size = value,
        CodeChange::ChatWidth(value) => config.appearance.chat_width = value,
        CodeChange::Motion(value) => config.appearance.motion = value,
        CodeChange::Notification(kind, on) => match kind {
            NotificationKind::ReplyDone => config.notifications.reply_done = on,
            NotificationKind::ScheduledTask => config.notifications.scheduled_task = on,
            NotificationKind::CodeNotice => config.notifications.code_notice = on,
            NotificationKind::PermissionRequest => config.notifications.permission_request = on,
        },
        CodeChange::AttentionBounce(on) => config.attention_bounce = on,
```

Define `NotificationKind` (derive `Debug, Clone, Copy, PartialEq, Eq`) in settings.rs, export from `lib.rs`. Add getters/setters following the existing pattern (`self.saved.read().appearance`, `self.change_code(CodeChange::Theme(value))`). In `after_save_code` also copy `appearance`, `notifications`, `attention_bounce` from the saved config. Add `pub fn saved_config(&self) -> AppConfig { self.saved.read().clone() }`. Failures during a save of these controls report under `SettingsSection::Code` today; the section is renamed in Task 4 (`change_code` uses `SettingsSection::Code`; leave it, Task 4 maps pages to status).

- [ ] **Step 4:** `cargo test -p forge-features settings 2>&1 | tail` → PASS.
- [ ] **Step 5: Commit** `feat: Settings holds appearance, notification and Dock-bounce controls`.

---

### Task 3: Apply the appearance (CSS attributes)

**Files:**
- Modify: `crates/forge-shared/src/theme.css` (dark blocks at ~23, ~248, ~311; `--chat-column` ~103; `--font`/`--mono`; body font size; reduce-motion)
- Modify: `crates/forge-shared/src/theme.rs` (tests)
- Create: `crates/forge-features/src/appearance.rs`
- Modify: `crates/forge-features/src/lib.rs`, `crates/forge-features/src/settings.rs` (`show`/`after_save_code` call the applier)

**Interfaces:**
- Consumes: `Appearance` (Task 1), `Settings::saved_config` (Task 2).
- Produces: `forge_features::apply_appearance_script(&Appearance) -> String` (pure, JS text) and `forge_features::apply_appearance(&Appearance)` (runs it with `document::eval`).

- [ ] **Step 1: Failing tests.** In `theme.rs` tests:

```rust
    #[test]
    fn every_dark_block_has_a_forced_dark_twin_and_light_stays_light_under_a_dark_system() {
        // Each `prefers-color-scheme: dark` block must be repeated for data-theme="dark", and the
        // media version must not apply when the person chose light.
        let media = THEME_CSS.matches("@media (prefers-color-scheme: dark)").count();
        let forced = THEME_CSS.matches(":root[data-theme=\"dark\"]").count();
        assert_eq!(media, forced, "one forced-dark rule per media block");
        assert!(THEME_CSS.matches(":root:not([data-theme=\"light\"])").count() >= media);
    }

    #[test]
    fn the_stylesheet_has_a_rule_for_every_attribute_value() {
        for rule in [
            "[data-text-size=\"small\"]", "[data-text-size=\"large\"]",
            "[data-width=\"narrow\"]", "[data-width=\"wide\"]",
            "[data-font=\"opendyslexic\"]", "[data-motion=\"reduced\"]",
        ] {
            assert!(THEME_CSS.contains(rule), "missing {rule}");
        }
    }
```

In `appearance.rs`:

```rust
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
```

- [ ] **Step 2:** `cargo test -p forge-shared -p forge-features appearance 2>&1 | tail` → FAIL.

- [ ] **Step 3: Implement CSS.** For each of the three `@media (prefers-color-scheme: dark) { :root { … } }` blocks:
  1. change the media block's selector to `:root:not([data-theme="light"])`;
  2. add directly after it the same declarations under `:root[data-theme="dark"] { … }`.

Then add after the `--chat-column` rule (replace line `:root { --chat-column: 720px; }`):

```css
:root { --chat-column: 720px; --chat-size: 14px; }
:root[data-width="narrow"] { --chat-column: 560px; }
:root[data-width="wide"] { --chat-column: 880px; }
:root[data-text-size="small"] { --chat-size: 13px; }
:root[data-text-size="large"] { --chat-size: 16px; }
.turns { font-size: var(--chat-size); }
:root[data-font="opendyslexic"] { --font: "OpenDyslexic", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; --mono: "OpenDyslexicMono", "OpenDyslexic", ui-monospace, "SF Mono", Menlo, monospace; }
:root[data-motion="reduced"] *, :root[data-motion="reduced"] *::before, :root[data-motion="reduced"] *::after { animation-duration: 0s !important; animation-iteration-count: 1 !important; transition-duration: 0s !important; scroll-behavior: auto !important; }
@media (prefers-reduced-motion: reduce) {
  :root:not([data-motion="system"]):not([data-motion="reduced"]) * { animation-duration: 0s !important; }
}
```

(The last block is unnecessary; delete it. With `system` the browser's own setting applies and Forge adds nothing.) If `.turns` already sets its own `font-size`, merge rather than duplicate.

- [ ] **Step 4: Implement `appearance.rs`:**

```rust
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
```

Export from `lib.rs`. In `Settings::show` (after `saved.set(config)`) and in `after_save_code` (after copying fields) call `crate::appearance::apply_appearance(&self.saved.peek().appearance)`. `show` is the path `use_settings_provider` takes on start (`reload`), so the saved look is applied as soon as the config loads.

- [ ] **Step 5:** `cargo test -p forge-shared -p forge-features 2>&1 | tail` → PASS. Commit `feat: appearance choices drive the page through data attributes`.

---

### Task 4: The grouped window with search and the General page

**Files:**
- Create: `crates/forge-features/src/settings_pages.rs`
- Modify: `crates/forge-features/src/settings.rs`, `crates/forge-features/src/lib.rs`
- Modify: `crates/forge-widgets/src/settings_overlay.rs`
- Modify: `crates/forge-shared/src/theme.css` (search field, group headings, segmented choice reuse `.view-switch`)

**Interfaces:**
- Consumes: Task 2 setters.
- Produces: `SettingsSection::General` (new; the enum otherwise unchanged); `settings_pages::{NavGroup, nav_groups() -> Vec<NavGroup>, filter_pages(query: &str) -> Vec<SettingsSection>}` where `pub struct NavGroup { pub title: &'static str, pub pages: Vec<(SettingsSection, &'static str)> }`; `Settings::search() -> String`, `Settings::set_search(String)`.

- [ ] **Step 1: Failing tests** (`settings_pages.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_follow_the_real_app() {
        let groups = nav_groups();
        let titles: Vec<_> = groups.iter().map(|group| group.title).collect();
        assert_eq!(titles, ["Settings", "This computer", "Customize"]);
        assert_eq!(groups[0].pages.iter().map(|page| page.1).collect::<Vec<_>>(), ["General", "Claude Code"]);
        assert_eq!(groups[1].pages.iter().map(|page| page.1).collect::<Vec<_>>(), ["Connection", "Model"]);
        assert_eq!(groups[2].pages.iter().map(|page| page.1).collect::<Vec<_>>(), ["Skills", "Plugins", "MCP servers"]);
    }

    #[test]
    fn an_empty_search_keeps_every_page() {
        assert_eq!(filter_pages("  ").len(), 7);
    }

    #[test]
    fn a_search_matches_a_page_name_or_one_of_its_row_titles_ignoring_case() {
        assert_eq!(filter_pages("MODEL"), [SettingsSection::Model]);
        assert_eq!(filter_pages("dock"), [SettingsSection::Code]);
        assert_eq!(filter_pages("theme"), [SettingsSection::General]);
        assert_eq!(filter_pages("notif"), [SettingsSection::General]);
    }

    #[test]
    fn a_search_with_no_match_returns_nothing() {
        assert!(filter_pages("zzzz").is_empty());
    }
}
```

- [ ] **Step 2:** `cargo test -p forge-features settings_pages 2>&1 | tail` → FAIL.

- [ ] **Step 3: Implement `settings_pages.rs`:**

```rust
use crate::settings::SettingsSection;

pub struct NavGroup {
    pub title: &'static str,
    pub pages: Vec<(SettingsSection, &'static str)>,
}

pub fn nav_groups() -> Vec<NavGroup> {
    vec![
        NavGroup { title: "Settings", pages: vec![(SettingsSection::General, "General"), (SettingsSection::Code, "Claude Code")] },
        NavGroup { title: "This computer", pages: vec![(SettingsSection::Connection, "Connection"), (SettingsSection::Model, "Model")] },
        NavGroup {
            title: "Customize",
            pages: vec![(SettingsSection::Skills, "Skills"), (SettingsSection::Plugins, "Plugins"), (SettingsSection::McpServers, "MCP servers")],
        },
    ]
}

// The titles of the rows on each page, so a search finds a setting by its name.
fn row_titles(section: SettingsSection) -> &'static [&'static str] {
    match section {
        SettingsSection::General => &["Appearance", "Theme", "Font", "Text size", "Chat width", "Reduce motion", "Notifications", "Reply done", "Scheduled task", "Code notice", "Permission request"],
        SettingsSection::Code => &["Default transcript view", "Output style", "Prompt suggestions", "Allow bypass permissions mode", "Bounce the Dock icon"],
        SettingsSection::Connection => &["Base URL", "API key"],
        SettingsSection::Model => &["Default model", "Other model ID"],
        SettingsSection::Skills | SettingsSection::Plugins | SettingsSection::McpServers => &[],
    }
}

/// The pages whose name or row titles contain `query` (any case), in list order. A blank query keeps all.
pub fn filter_pages(query: &str) -> Vec<SettingsSection> {
    let needle = query.trim().to_lowercase();
    nav_groups()
        .into_iter()
        .flat_map(|group| group.pages)
        .filter(|(section, name)| {
            needle.is_empty() || name.to_lowercase().contains(&needle) || row_titles(*section).iter().any(|title| title.to_lowercase().contains(&needle))
        })
        .map(|(section, _)| section)
        .collect()
}
```

`SettingsSection` needs `General` added (derive stays `Debug, Clone, Copy, PartialEq, Eq`). Initial section stays `Connection` today; change the initial value in `use_settings_provider` to `SettingsSection::General`. `Settings` gets `search: Signal<String>` (init empty) with `search()`/`set_search()`; `open()` does **not** reset `section` (so it reopens on the last page) but clears `search`.

Search state must not hide the open page: when the filter result is non-empty and does not contain the current section, the overlay shows the first match (see the widget below) but does not change `section` until clicked.

- [ ] **Step 4: Widget.** Rewrite the nav in `settings_overlay.rs`: replace the hand-written six buttons with

```rust
                div { class: "settings-nav",
                    input {
                        class: "settings-search",
                        r#type: "text",
                        placeholder: "Search",
                        value: "{settings.search()}",
                        oninput: move |event| settings.set_search(event.value()),
                    }
                    {
                        let matches = filter_pages(&settings.search());
                        if matches.is_empty() {
                            rsx! { div { class: "hint settings-empty", "No settings match." } }
                        } else {
                            rsx! {
                                for group in nav_groups() {
                                    if group.pages.iter().any(|(page, _)| matches.contains(page)) {
                                        div { key: "{group.title}", class: "settings-nav-heading", "{group.title}" }
                                        for (page, name) in group.pages.clone() {
                                            if matches.contains(&page) {
                                                button {
                                                    key: "{name}",
                                                    class: if page == section { "settings-nav-item active" } else { "settings-nav-item" },
                                                    onclick: move |_| settings.set_section(page),
                                                    "{name}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    (keep the existing "Keyboard shortcuts" button below)
                }
```

Match arm adds `SettingsSection::General => rsx! { GeneralSection {} }`. Add `GeneralSection`:

```rust
#[component]
fn GeneralSection() -> Element {
    let settings = use_settings();
    let look = settings.appearance();
    let notes = settings.notifications();
    rsx! {
        h2 { "General" }
        h3 { class: "settings-group", "Appearance" }
        SettingRow { title: "Theme", description: "System follows the computer's light or dark mode.",
            Segmented { label: "Theme", value: look.theme.value().to_string(),
                options: Theme::ALL.iter().map(|choice| (choice.value().to_string(), choice.label().to_string())).collect::<Vec<_>>(),
                onchange: move |value: String| if let Some(choice) = Theme::from_value(&value) { settings.set_theme(choice) } }
        }
        SettingRow { title: "Font", description: "Used for the whole interface, the chat and code. OpenDyslexic must be installed on the computer.",
            Select { value: look.font.value().to_string(),
                options: FontChoice::ALL.iter().map(|choice| (choice.value().to_string(), choice.label().to_string())).collect::<Vec<_>>(),
                onchange: move |value: String| if let Some(choice) = FontChoice::from_value(&value) { settings.set_font(choice) } }
        }
        SettingRow { title: "Text size", description: "The size of the conversation text.",
            Segmented { label: "Text size", value: look.text_size.value().to_string(),
                options: TextSize::ALL.iter().map(|choice| (choice.value().to_string(), choice.label().to_string())).collect::<Vec<_>>(),
                onchange: move |value: String| if let Some(choice) = TextSize::from_value(&value) { settings.set_text_size(choice) } }
        }
        SettingRow { title: "Chat width", description: "How wide the conversation column is.",
            Segmented { label: "Chat width", value: look.chat_width.value().to_string(),
                options: ChatWidth::ALL.iter().map(|choice| (choice.value().to_string(), choice.label().to_string())).collect::<Vec<_>>(),
                onchange: move |value: String| if let Some(choice) = ChatWidth::from_value(&value) { settings.set_chat_width(choice) } }
        }
        SettingRow { title: "Reduce motion", description: "Turns off animation of streaming replies and other interface elements.",
            Switch { on: look.motion == MotionPref::Reduced,
                onchange: move |on| settings.set_motion(if on { MotionPref::Reduced } else { MotionPref::System }) }
        }
        h3 { class: "settings-group", "Notifications" }
        SettingRow { title: "Reply done", description: "When a session you are not looking at finishes a reply.",
            Switch { on: notes.reply_done, onchange: move |on| settings.set_notification(NotificationKind::ReplyDone, on) } }
        SettingRow { title: "Scheduled task", description: "When a routine needs you or cannot start.",
            Switch { on: notes.scheduled_task, onchange: move |on| settings.set_notification(NotificationKind::ScheduledTask, on) } }
        SettingRow { title: "Code notice", description: "When a session fails or cannot continue.",
            Switch { on: notes.code_notice, onchange: move |on| settings.set_notification(NotificationKind::CodeNotice, on) } }
        SettingRow { title: "Permission request", description: "When a session you are not looking at asks for approval.",
            Switch { on: notes.permission_request, onchange: move |on| settings.set_notification(NotificationKind::PermissionRequest, on) } }
    }
}
```

`Segmented` is the `.view-switch` radio group already in `CodeSection`; extract it into `forge-shared/src/components.rs` as `pub fn Segmented(label: String, value: String, options: Vec<(String, String)>, onchange: EventHandler<String>)` (same markup: `div.view-switch[role=radiogroup]` with `button.view-option[.selected][role=radio]`), and make `CodeSection` use it. "Reduce motion" is a switch (the two choices are System/Reduced; spec's wording "System / Reduced" maps to off/on). Add CSS:

```css
.settings-search { display: block; width: 100%; box-sizing: border-box; margin: 0 0 10px; padding: 6px 10px; border: 1px solid var(--border); border-radius: 8px; background: var(--bg); color: var(--fg); font: inherit; font-size: 13px; }
.settings-nav-heading { margin-top: 10px; }
.settings-group { margin: 22px 0 0; font-size: 13px; color: var(--fg-muted); font-weight: 600; }
.settings-empty { padding: 6px 10px; }
```

- [ ] **Step 5:** `cargo test -p forge-features -p forge-shared -p forge-widgets 2>&1 | tail` and `cargo build -p forge-app 2>&1 | tail -3` → PASS / builds. Commit `feat: Settings is a grouped window with search and a General page`.

---

### Task 5: Session notifications

**Files:**
- Create: `crates/forge-features/src/session_notices.rs`
- Modify: `crates/forge-features/src/lib.rs`, `crates/forge-features/src/routines_state.rs` (scheduled_task switch), `crates/forge-pages/src/workspace_page.rs` (install the watcher)

**Interfaces:**
- Consumes: `NotificationPrefs`, `Settings::saved_config`, `Workspace::{live_ids?, live_view, active}` (see step 3), `WorkspaceServices.routines.notify: Arc<dyn NotifyPort>`.
- Produces: `pub struct Snapshot { pub phase: Phase, pub waiting: usize, pub failed: bool }`; `pub enum Notice { ReplyDone, PermissionRequest, Failed }`; `pub fn notice_between(before: &Snapshot, after: &Snapshot) -> Option<Notice>`; `pub fn should_send(notice: Notice, prefs: &NotificationPrefs, in_view: bool) -> bool`; `pub fn text(notice: Notice, session_title: &str) -> (String, String)`; `pub fn use_session_notices(workspace: Workspace, settings: Settings)`.

- [ ] **Step 1: Failing tests** (`session_notices.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_application::Phase;
    use forge_domain::NotificationPrefs;

    fn snap(phase: Phase, waiting: usize) -> Snapshot { Snapshot { phase, waiting, failed: phase == Phase::Failed } }

    #[test]
    fn a_turn_that_ends_is_a_reply_done() {
        assert_eq!(notice_between(&snap(Phase::Running, 0), &snap(Phase::Idle, 0)), Some(Notice::ReplyDone));
    }

    #[test]
    fn a_session_that_starts_waiting_notifies_once_not_on_every_update() {
        assert_eq!(notice_between(&snap(Phase::Running, 0), &snap(Phase::Running, 1)), Some(Notice::PermissionRequest));
        assert_eq!(notice_between(&snap(Phase::Running, 1), &snap(Phase::Running, 1)), None);
        assert_eq!(notice_between(&snap(Phase::Running, 1), &snap(Phase::Running, 2)), None, "a second request while one waits is not a new notice");
    }

    #[test]
    fn a_waiting_session_that_then_ends_its_turn_is_not_a_reply_done_while_a_request_remains() {
        assert_eq!(notice_between(&snap(Phase::Running, 1), &snap(Phase::Idle, 1)), None);
    }

    #[test]
    fn a_session_that_fails_notifies() {
        assert_eq!(notice_between(&snap(Phase::Running, 0), &snap(Phase::Failed, 0)), Some(Notice::Failed));
    }

    #[test]
    fn nothing_happens_for_an_idle_session_staying_idle() {
        assert_eq!(notice_between(&snap(Phase::Idle, 0), &snap(Phase::Idle, 0)), None);
    }

    #[test]
    fn a_notice_is_sent_only_when_its_switch_is_on_and_the_session_is_not_in_view() {
        let on = NotificationPrefs::default();
        assert!(should_send(Notice::ReplyDone, &on, false));
        assert!(!should_send(Notice::ReplyDone, &on, true), "the person is looking at it");
        let off = NotificationPrefs { reply_done: false, ..on };
        assert!(!should_send(Notice::ReplyDone, &off, false));
        assert!(!should_send(Notice::PermissionRequest, &NotificationPrefs { permission_request: false, ..on }, false));
        assert!(!should_send(Notice::Failed, &NotificationPrefs { code_notice: false, ..on }, false));
    }

    #[test]
    fn the_text_names_the_session() {
        let (title, body) = text(Notice::PermissionRequest, "Fix login");
        assert_eq!(title, "Approval needed");
        assert!(body.contains("Fix login"));
    }
}
```

- [ ] **Step 2:** `cargo test -p forge-features session_notices 2>&1 | tail` → FAIL.

- [ ] **Step 3: Implement the pure part:**

```rust
use forge_application::Phase;
use forge_domain::NotificationPrefs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot { pub phase: Phase, pub waiting: usize, pub failed: bool }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice { ReplyDone, PermissionRequest, Failed }

/// What changed between two looks at one session that the person should hear about.
pub fn notice_between(before: &Snapshot, after: &Snapshot) -> Option<Notice> {
    if !before.failed && after.failed {
        return Some(Notice::Failed);
    }
    if before.waiting == 0 && after.waiting > 0 {
        return Some(Notice::PermissionRequest);
    }
    if before.phase == Phase::Running && after.phase == Phase::Idle && after.waiting == 0 {
        return Some(Notice::ReplyDone);
    }
    None
}

pub fn should_send(notice: Notice, prefs: &NotificationPrefs, in_view: bool) -> bool {
    !in_view
        && match notice {
            Notice::ReplyDone => prefs.reply_done,
            Notice::PermissionRequest => prefs.permission_request,
            Notice::Failed => prefs.code_notice,
        }
}

pub fn text(notice: Notice, session_title: &str) -> (String, String) {
    match notice {
        Notice::ReplyDone => ("Reply done".to_string(), format!("{session_title} finished its reply")),
        Notice::PermissionRequest => ("Approval needed".to_string(), format!("{session_title} is waiting for your approval")),
        Notice::Failed => ("Session stopped".to_string(), format!("{session_title} could not continue")),
    }
}
```

Then the hook. "In view" means: the session is `workspace.active()` **and** the window has focus. Window focus is read through JS `document.hasFocus()` each time a notice is about to be sent (one `document::eval("return document.hasFocus();")` awaited). Write:

```rust
/// Watches every open session and sends the notifications of the Notifications switches.
pub fn use_session_notices(workspace: Workspace, settings: Settings) {
    use_hook(move || {
        let notify = workspace.services_notify(); // see below
        dioxus::core::spawn_forever(async move {
            let mut before: HashMap<LiveSessionId, Snapshot> = HashMap::new();
            loop {
                for (id, view) in workspace.live_views() {
                    let now = { let view = view.peek(); Snapshot { phase: view.phase, waiting: view.pending_permissions.len(), failed: view.phase == Phase::Failed } };
                    let notice = before.insert(id, now).and_then(|previous| notice_between(&previous, &now));
                    if let Some(notice) = notice {
                        let focused = document::eval("return document.hasFocus();").join::<bool>().await.unwrap_or(true);
                        let in_view = focused && workspace.active() == Active::Live(id);
                        if should_send(notice, &settings.saved_config().notifications, in_view) {
                            let (title, body) = text(notice, &workspace.title_of(id));
                            let notify = notify.clone();
                            tokio::task::spawn_blocking(move || { let _ = notify.notify(&title, &body); });
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });
    });
}
```

Two small helpers on `Workspace` (add in `workspace.rs`, near `live_view`): `pub fn live_views(&self) -> Vec<(LiveSessionId, Signal<LiveSessionView>)>` (map over `self.live.read()`), `pub fn title_of(&self, id) -> String` (the view's `session.title` or the project label when empty — read how the sidebar names a live entry and reuse that function), and `pub fn services_notify(&self) -> Arc<dyn NotifyPort>` returning `self.services.peek().routines.notify.clone()`. Sessions that routines own are covered too (the watcher sees every live entry); to avoid a double notice for a routine session waiting for approval, the routine path (`notices` in routines_state) stays as is and the watcher skips ids for which `services.routines.host` tracks a routine: add `RegistryHost::is_routine_session(&self, id: LiveSessionId) -> bool` (`self.tracked.lock().unwrap().contains_key(&id)`) and skip those ids in the loop. Prune `before` of ids no longer present each pass.

Install in `workspace_page.rs` after `use_routines_provider(workspace);`: `use_session_notices(workspace, settings);`. Export from `lib.rs`.

- [ ] **Step 4: scheduled_task switch.** In `routines_state.rs` where `notices(...)` results are sent (line ~345), skip the loop body when `!this_settings.notifications.scheduled_task`. `RoutinesState` has no `Settings`; pass the flag by reading config: the timer already runs in the page scope, so give `start_timer` a `prefs: Signal<NotificationPrefs>`-like closure — concretely change `use_routines_provider(workspace)` to `use_routines_provider(workspace, settings)` and read `settings.saved_config().notifications.scheduled_task` just before the `for (title, body) in …` loop (compute `let allow = …;` and `if allow { … }`; the notices are still drained from the host, so they are not delayed to a later turn-on). Add a test in `routines.rs` is not needed (the rule is a one-line guard); cover by the real-window check in Task 7.

- [ ] **Step 5:** `cargo test -p forge-features -p forge-application 2>&1 | tail` → PASS. Commit `feat: sessions notify when a reply is done, a request waits or a session stops`.

---

### Task 6: Dock bounce

**Files:**
- Modify: `crates/forge-ports/src/notify_port.rs`, `crates/forge-adapter-notify/src/lib.rs`
- Modify: `crates/forge-app/src/main.rs`
- Modify: `crates/forge-features/src/session_notices.rs`, `crates/forge-features/src/routines_state.rs`
- Modify: `crates/forge-widgets/src/settings_overlay.rs` (switch in CodeSection)

**Interfaces:**
- Produces: `NotifyPort::request_attention(&self) -> Result<(), String>` with a default body `Err("not available here".into())` so other implementers keep compiling. `forge-app` implements it for the macOS notifier through a `DesktopContext`.

- [ ] **Step 1: Failing test** (`notify_port.rs` tests)

```rust
    #[test]
    fn the_default_request_for_attention_says_it_is_not_available() {
        struct Plain;
        impl NotifyPort for Plain {
            fn notify(&self, _: &str, _: &str) -> Result<(), String> { Ok(()) }
        }
        assert!(Plain.request_attention().is_err());
    }
```

- [ ] **Step 2:** `cargo test -p forge-ports notify 2>&1 | tail` → FAIL.

- [ ] **Step 3: Implement.** In the trait add

```rust
    /// Bounces the Dock icon until the person comes back. Only macOS has one.
    fn request_attention(&self) -> Result<(), String> {
        Err("attention requests are not available here".to_string())
    }
```

(`automock` supports default methods.) In `forge-app/src/main.rs` add a wrapper that owns the window:

```rust
// Notifies through osascript and bounces the Dock icon through the window (macOS).
struct DesktopNotify {
    inner: Arc<dyn forge_ports::NotifyPort>,
    window: std::sync::Mutex<Option<dioxus::desktop::DesktopContext>>,
}
```

Simplest honest shape: keep `notifier()` as is, and implement `request_attention` in `OsascriptNotify` by calling a function pointer set at startup is overengineered. Use instead: `OsascriptNotify` gets `attention: Arc<dyn Fn() + Send + Sync>` (default no-op returning Err), set via `OsascriptNotify::with_attention(f)`. In `App()` (forge-app, which has the desktop feature) obtain `let window = dioxus::desktop::use_window();` and store in a `static ATTENTION: OnceLock<Box<dyn Fn() + Send + Sync>>` that calls `window.request_user_attention(Some(dioxus::desktop::tao::window::UserAttentionType::Informational))`. `OsascriptNotify::request_attention` calls the static if set, else `Err`. (If `DesktopContext` is not `Send + Sync`, capture the `WindowId` and call through `dioxus::desktop::window()` on the main thread via `window.proxy`; check at implementation time with a compile and use the form that compiles — the observable contract is the test below.)

Test in `forge-adapter-notify`:

```rust
    #[test]
    fn a_request_for_attention_without_a_window_is_an_error_not_a_panic() {
        assert!(OsascriptNotify::new().request_attention().is_err());
    }
```

Then bounce from both senders. In `session_notices.rs` after sending a notice: `if settings.saved_config().attention_bounce { let _ = notify.request_attention(); }` inside the same `spawn_blocking`. Same in `routines_state.rs` next to `notify.notify`. Only `PermissionRequest` and `Failed` (and routine "needs approval"/"could not start") bounce, not `ReplyDone` — this matches the row's wording "when attention is needed"; encode as `Notice::needs_attention(self) -> bool` with a test:

```rust
    #[test]
    fn only_notices_that_need_the_person_bounce_the_dock() {
        assert!(Notice::PermissionRequest.needs_attention() && Notice::Failed.needs_attention());
        assert!(!Notice::ReplyDone.needs_attention());
    }
```

Widget: in `CodeSection` after the bypass row add (macOS only — `cfg!(target_os = "macos")` evaluated in the component):

```rust
        if cfg!(target_os = "macos") {
            SettingRow { title: "Bounce the Dock icon", description: "When a session needs your approval or stops, the Dock icon bounces until you come back.",
                Switch { on: settings.attention_bounce(), onchange: move |on| settings.set_attention_bounce(on) } }
        }
```

- [ ] **Step 4:** `cargo test -p forge-ports -p forge-adapter-notify -p forge-features 2>&1 | tail` and `cargo build -p forge-app 2>&1 | tail -3` → PASS / builds.
- [ ] **Step 5: Commit** `feat: bounce the Dock icon when a session needs the person`.

---

### Task 7: Docs, roadmap, real-window check

**Files:**
- Modify: `docs/superpowers/roadmap/backlog.md` (SET-01, SET-02, SET-03, SET-24 rows)
- Modify: `docs/superpowers/specs/2026-10-09-settings-window-design.md` (one line: appearance is applied as soon as the config loads, not before first paint)

- [ ] **Step 1: Real-window check.** Ask the user to unlock the Mac and keep it awake first. Use the Quartz harness and `FORGE_CLAUDE_HOME` fixture (see memory `driving-the-forge-app-visually`; capture after every click). Check, with screenshots:
  1. ⌘, opens Settings on General; groups and search show; typing `dock` leaves only Claude Code; `zzzz` shows "No settings match."; close and reopen lands on the last page.
  2. Theme Light/Dark/System changes the whole window at once and code blocks recolour; text size and chat width change the conversation; Reduce motion on; font OpenDyslexic changes text (falls back if not installed).
  3. Quit and restart: all choices are kept. Delete the new keys from the config file: Forge opens with the old look.
  4. Start a session in a second folder, switch to another session, let it finish: a "Reply done" notification arrives; with the switch off none arrives. A permission request in a background session notifies once; with "Bounce the Dock icon" on, the Dock icon bounces.
  5. A routine that needs approval: obeys the Scheduled task switch.
- [ ] **Step 2: Update the backlog.** SET-01, SET-02, SET-03 get Forge column `has` (or `partial` with what is missing: voice, trusted folders, default browser; code themes; Anthropic fonts), Plan column `Plan 33 (built)`. If no Code notices beyond "session stopped" exist, say so in the SET-03 note. SET-24: Settings now keeps Skills, Plugins, MCP servers under `Customize`.
- [ ] **Step 3: Run the whole suite**

Run: `cargo test --workspace --exclude forge-adapter-claude-plugins 2>&1 | tail -15` (then `cargo test -p forge-adapter-claude-plugins` alone, ignoring `round_trip`, which fails on master).
Expected: all pass.

- [ ] **Step 4: Commit** `docs: Plan 33 is built; SET-01..03 updated in the backlog`.

---

## Self-review

- **Spec coverage:** SET-01 groups/search/last page (Task 4), SET-02 five appearance controls + dark/light code highlight via CSS variables (Tasks 1, 3, 4), SET-03 four switches + Dock bounce (Tasks 2, 4, 5, 6), backlog and real-window check (Task 7). The "Code notice" switch is wired to the session-failed notice, so it is not left out.
- **Types:** `NotificationKind`, `CodeChange` variants, `Settings` method names, and `Snapshot`/`Notice` are used with the same names across tasks. `Segmented` is added in Task 4 and used only there.
- **Known soft spots, to settle at implementation time with a compile, not a decision:** the `choice_enum!` macro's doc-comment argument; whether `DesktopContext` is `Send + Sync` for the Dock bounce (Task 6 names the fallback); `Workspace::title_of` must reuse the sidebar's existing live-session naming.
