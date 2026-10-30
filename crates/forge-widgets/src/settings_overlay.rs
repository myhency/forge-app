use dioxus::prelude::*;
use forge_domain::{ChatWidth, FontChoice, MotionPref, OutputStyle, TextSize, Theme, TranscriptView};
use forge_entities::{model_from_value, model_options, model_value};
use forge_features::{filter_pages, nav_groups, use_settings, use_workspace, ExtensionKind, NotificationKind, SettingsSection, SettingsStatus};
use crate::developer_section::DeveloperSection;
use forge_shared::{tr, Button, ButtonKind, Segmented, Select, SettingRow, Switch, TextInput};

/// The settings overlay (spec §16): a left navigation and rows on the right. Renders nothing while
/// closed. Esc and ⌘, are handled by the page's key handler.
#[component]
pub fn SettingsOverlay() -> Element {
    let settings = use_settings();
    let workspace = use_workspace();
    if !settings.is_open() {
        return rsx! {};
    }
    let section = settings.section();

    rsx! {
        div {
            class: "settings-backdrop",
            onclick: move |_| settings.close(),
            div {
                class: "settings-panel",
                // A click inside the panel must not reach the backdrop and close it.
                onclick: move |event| event.stop_propagation(),
                button { class: "settings-close", title: tr!("Close (Esc)"), onclick: move |_| settings.close(), "✕" }
                div { class: "settings-nav",
                    input {
                        class: "settings-search",
                        r#type: "text",
                        placeholder: tr!("Search"),
                        value: "{settings.search()}",
                        oninput: move |event| settings.set_search(event.value()),
                    }
                    {
                        let matches = filter_pages(&settings.search());
                        if matches.is_empty() {
                            rsx! { div { class: "hint settings-empty", {tr!("No settings match.")} } }
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
                    button {
                        class: "settings-nav-item",
                        style: "margin-top: 16px;",
                        onclick: move |_| {
                            settings.close();
                            workspace.open_shortcuts();
                        },
                        {tr!("Keyboard shortcuts")}
                    }
                }
                div { class: "settings-content",
                    match section {
                        SettingsSection::General => rsx! { GeneralSection {} },
                        SettingsSection::Connection => rsx! { ConnectionSection {} },
                        SettingsSection::Model => rsx! { ModelSection {} },
                        // Temporary: Task 8 renders these tabs.
                        SettingsSection::Skills => rsx! { ExtensionSection { kind: ExtensionKind::Skill } },
                        SettingsSection::Plugins => rsx! { ExtensionSection { kind: ExtensionKind::Plugin } },
                        SettingsSection::McpServers => rsx! { ExtensionSection { kind: ExtensionKind::McpServer } },
                        SettingsSection::Code => rsx! { CodeSection {} },
                        SettingsSection::Developer => rsx! { DeveloperSection {} },
                    }
                    StatusLine {}
                }
            }
            TrustedFoldersDialog {}
        }
    }
}

#[component]
fn GeneralSection() -> Element {
    let settings = use_settings();
    let look = settings.appearance();
    let notes = settings.notifications();
    rsx! {
        h2 { {tr!("General")} }
        h3 { class: "settings-group", {tr!("Appearance")} }
        SettingRow { title: tr!("Theme"), description: tr!("System follows the computer's light or dark mode."),
            Segmented {
                label: tr!("Theme"),
                value: look.theme.value().to_string(),
                options: Theme::ALL.iter().map(|choice| (choice.value().to_string(), forge_shared::i18n::text(choice.label()).to_string())).collect::<Vec<_>>(),
                onchange: move |value: String| {
                    if let Some(choice) = Theme::from_value(&value) {
                        settings.set_theme(choice);
                    }
                },
            }
        }
        SettingRow {
            title: tr!("Font"),
            description: tr!("Used for the whole interface, the chat and code. OpenDyslexic must be installed on the computer."),
            Select {
                value: look.font.value().to_string(),
                options: FontChoice::ALL.iter().map(|choice| (choice.value().to_string(), forge_shared::i18n::text(choice.label()).to_string())).collect::<Vec<_>>(),
                onchange: move |value: String| {
                    if let Some(choice) = FontChoice::from_value(&value) {
                        settings.set_font(choice);
                    }
                },
            }
        }
        SettingRow { title: tr!("Text size"), description: tr!("The size of the conversation text."),
            Segmented {
                label: tr!("Text size"),
                value: look.text_size.value().to_string(),
                options: TextSize::ALL.iter().map(|choice| (choice.value().to_string(), forge_shared::i18n::text(choice.label()).to_string())).collect::<Vec<_>>(),
                onchange: move |value: String| {
                    if let Some(choice) = TextSize::from_value(&value) {
                        settings.set_text_size(choice);
                    }
                },
            }
        }
        SettingRow { title: tr!("Chat width"), description: tr!("How wide the conversation column is."),
            Segmented {
                label: tr!("Chat width"),
                value: look.chat_width.value().to_string(),
                options: ChatWidth::ALL.iter().map(|choice| (choice.value().to_string(), forge_shared::i18n::text(choice.label()).to_string())).collect::<Vec<_>>(),
                onchange: move |value: String| {
                    if let Some(choice) = ChatWidth::from_value(&value) {
                        settings.set_chat_width(choice);
                    }
                },
            }
        }
        SettingRow { title: tr!("Reduce motion"), description: tr!("Turns off animation of streaming replies and other interface elements."),
            Switch {
                on: look.motion == MotionPref::Reduced,
                onchange: move |on| settings.set_motion(if on { MotionPref::Reduced } else { MotionPref::System }),
            }
        }
        h3 { class: "settings-group", {tr!("Work")} }
        SettingRow {
            title: tr!("Trusted folders"),
            description: tr!("Trusting a folder on this computer lets Claude use that folder and its subfolders. Trusting a folder does not add it to new sessions. When you add a folder to a session, you may be asked to allow it again."),
            Button { label: tr!("Manage"), onclick: move |_| settings.open_trusted_dialog() }
        }
        h3 { class: "settings-group", {tr!("Notifications")} }
        SettingRow { title: tr!("Reply done"), description: tr!("When a session you are not looking at finishes a reply."),
            Switch { on: notes.reply_done, onchange: move |on| settings.set_notification(NotificationKind::ReplyDone, on) }
        }
        SettingRow { title: tr!("Scheduled task"), description: tr!("When a routine needs you or cannot start."),
            Switch { on: notes.scheduled_task, onchange: move |on| settings.set_notification(NotificationKind::ScheduledTask, on) }
        }
        SettingRow { title: tr!("Code notice"), description: tr!("When a session fails or cannot continue."),
            Switch { on: notes.code_notice, onchange: move |on| settings.set_notification(NotificationKind::CodeNotice, on) }
        }
        SettingRow { title: tr!("Permission request"), description: tr!("When a session you are not looking at asks for approval."),
            Switch { on: notes.permission_request, onchange: move |on| settings.set_notification(NotificationKind::PermissionRequest, on) }
        }
    }
}

// Over the settings panel, not inside it: the backdrop is as big as the whole window.
#[component]
fn TrustedFoldersDialog() -> Element {
    let settings = use_settings();
    if !settings.trusted_dialog_open() {
        return rsx! {};
    }
    let folders = settings.trusted_folders();
    rsx! {
        div {
            class: "settings-backdrop trusted-backdrop",
            onclick: move |event| {
                event.stop_propagation();
                settings.close_trusted_dialog();
            },
            div { class: "customize-dialog", onclick: move |event| event.stop_propagation(),
                h2 { {tr!("Trusted folders")} }
                p { class: "hint", {tr!("Trusting a folder on this computer lets Claude use that folder and its subfolders. Trusting a folder does not add it to new sessions. When you add a folder to a session, you may be asked to allow it again.")} }
                if let Some(SettingsStatus::Error(message)) = settings.status() {
                    div { class: "status-line error", "{message}" }
                }
                if folders.is_empty() {
                    div { class: "hint", {tr!("No trusted folders yet")} }
                }
                for path in folders {
                    div { key: "{path.display()}", class: "trusted-row",
                        span { "📁" }
                        span { class: "trusted-path", title: "{path.display()}", "{path.display()}" }
                        button {
                            class: "chip-remove",
                            r#type: "button",
                            title: tr!("Remove"),
                            onclick: {
                                let path = path.clone();
                                move |_| settings.remove_trusted_folder(path.clone())
                            },
                            "×"
                        }
                    }
                }
                div { class: "customize-dialog-buttons trusted-buttons",
                    Button { label: format!("+ {}", tr!("Add folder")), onclick: move |_| settings.add_trusted_folder() }
                    Button { label: tr!("Done"), kind: ButtonKind::Primary, onclick: move |_| settings.close_trusted_dialog() }
                }
            }
        }
    }
}

const DEFAULT_STYLE_VALUE: &str = "default";
const PROJECT_LOCATION: &str = "\u{0}project";
const CUSTOM_LOCATION: &str = "\u{0}custom";

#[component]
fn CodeSection() -> Element {
    let settings = use_settings();
    let view = settings.default_transcript_view();
    let location = settings.worktree_location();
    let (location_value, mut location_options) = match &location {
        forge_domain::WorktreeLocation::Project => (PROJECT_LOCATION.to_string(), Vec::new()),
        forge_domain::WorktreeLocation::Folder(folder) => (folder.display().to_string(), vec![(folder.display().to_string(), folder.display().to_string())]),
    };
    location_options.insert(0, (PROJECT_LOCATION.to_string(), tr!("Inside the project (.claude/worktrees)").to_string()));
    location_options.push((CUSTOM_LOCATION.to_string(), tr!("Custom…").to_string()));
    let style_value = settings.output_style().map_or(DEFAULT_STYLE_VALUE, OutputStyle::name).to_string();
    let mut style_options = vec![(DEFAULT_STYLE_VALUE.to_string(), tr!("Default").to_string())];
    style_options.extend(OutputStyle::ALL.iter().map(|style| (style.name().to_string(), forge_shared::i18n::text(style.label()).to_string())));
    rsx! {
        h2 { {tr!("Code")} }
        SettingRow {
            title: tr!("Default transcript view"),
            description: tr!("The view sessions open in. The view menu of a session changes that session only."),
            Segmented {
                label: tr!("Default transcript view"),
                value: view.label().to_string(),
                options: TranscriptView::ALL.iter().map(|option| (option.label().to_string(), forge_shared::i18n::text(option.label()).to_string())).collect::<Vec<_>>(),
                onchange: move |value: String| {
                    if let Some(option) = TranscriptView::ALL.iter().copied().find(|option| option.label() == value) {
                        settings.set_default_transcript_view(option);
                    }
                },
            }
        }
        SettingRow {
            title: tr!("Archive inactive sessions"),
            description: tr!("Archives local sessions with no activity for a while. A session that is running or has background work is not archived, and a worktree with uncommitted changes stays on disk."),
            Select {
                value: forge_features::auto_archive_value(settings.auto_archive_days()),
                options: forge_domain::AUTO_ARCHIVE_CHOICES.iter().map(|days| (forge_features::auto_archive_value(*days), forge_features::auto_archive_text(*days))).collect::<Vec<_>>(),
                onchange: move |value: String| settings.set_auto_archive_days(value.parse::<u32>().ok()),
            }
        }
        SettingRow {
            title: tr!("Worktree location"),
            description: tr!("Where the Git worktrees of isolated sessions on this computer are kept. A folder inside the project is not used."),
            Select {
                key: "{settings.location_nonce()}",
                value: location_value,
                options: location_options,
                onchange: move |value: String| match value.as_str() {
                    PROJECT_LOCATION => settings.set_worktree_location(forge_domain::WorktreeLocation::Project),
                    CUSTOM_LOCATION => settings.choose_worktree_folder(),
                    folder => settings.set_worktree_location(forge_domain::WorktreeLocation::Folder(folder.into())),
                },
            }
        }
        SettingRow { title: tr!("Output style"), description: tr!("Applies to new sessions."),
            Select {
                value: style_value,
                options: style_options,
                onchange: move |value: String| settings.set_output_style(OutputStyle::from_name(&value)),
            }
        }
        SettingRow {
            title: tr!("Prompt suggestions"),
            description: tr!("Show a suggested next prompt in the message box after Claude answers. Suggestions use a little more of your usage."),
            Switch { on: settings.prompt_suggestions(), onchange: move |on| settings.set_prompt_suggestions(on) }
        }
        SettingRow {
            title: tr!("Allow bypass permissions mode"),
            description: tr!("Adds Bypass permissions to the mode menu, which skips every permission check in a session. A session that is already in that mode keeps it."),
            Switch { on: settings.allow_bypass_mode(), onchange: move |on| settings.set_allow_bypass_mode(on) }
        }
        SettingRow {
            title: tr!("Dynamic workflows"),
            description: tr!("Lets Claude run several agents in parallel for a complex task. Workflows can use up your usage limit quickly."),
            Switch { on: settings.dynamic_workflows(), onchange: move |on| settings.set_dynamic_workflows(on) }
        }
        SettingRow {
            title: tr!("Task list and next step"),
            description: tr!("Claude shows its plan as a list while it works and ends a turn with a summary and a question."),
            Switch { on: settings.turn_tools(), onchange: move |on| settings.set_turn_tools(on) }
        }
        SettingRow {
            title: tr!("Interactive pages"),
            description: tr!("Claude can draw a small interactive page in the chat, such as a calculator or a chart, when text would explain less."),
            Switch { on: settings.generative_ui(), onchange: move |on| settings.set_generative_ui(on) }
        }
        if cfg!(target_os = "macos") {
            SettingRow {
                title: tr!("Bounce the Dock icon"),
                description: tr!("When a session needs your approval or stops, the Dock icon bounces until you come back."),
                Switch { on: settings.attention_bounce(), onchange: move |on| settings.set_attention_bounce(on) }
            }
        }
        div { class: "hint", style: "margin-top: 12px;",
            {tr!("Output style, prompt suggestions, the bypass switch, dynamic workflows and the task list apply to sessions that start after the change. The default view also applies to open sessions you have not changed.")}
        }
    }
}

#[component]
fn ConnectionSection() -> Element {
    let settings = use_settings();
    let key_label = if settings.key_visible() { tr!("Hide") } else { tr!("Show") };
    rsx! {
        h2 { {tr!("Connection")} }
        SettingRow { title: tr!("Base URL"), description: tr!("Leave empty for https://api.anthropic.com."),
            TextInput {
                value: settings.base_url(),
                placeholder: "https://api.anthropic.com",
                oninput: move |value| settings.set_base_url(value),
            }
        }
        SettingRow { title: tr!("API key"), description: tr!("Stored in Forge's own config file, never in ~/.claude."),
            TextInput {
                value: settings.api_key(),
                placeholder: "sk-ant-…",
                secret: !settings.key_visible(),
                oninput: move |value| settings.set_api_key(value),
            }
            Button { label: key_label, onclick: move |_| settings.toggle_key_visible() }
        }
        div { class: "hint", style: "margin-top: 12px;",
            {tr!("Sessions that are already open keep the old values until they are started again.")}
        }
        div { style: "margin-top: 16px;",
            Button {
                label: tr!("Save"),
                kind: ButtonKind::Primary,
                disabled: !settings.connection_dirty(),
                onclick: move |_| settings.save_connection(),
            }
        }
    }
}

#[component]
fn ModelSection() -> Element {
    let settings = use_settings();
    let mut custom = use_signal(String::new);
    let saved = settings.model();
    let options = model_options(saved.as_deref());
    let selected = model_value(saved.as_deref());
    let custom_empty = custom.read().trim().is_empty();
    rsx! {
        h2 { {tr!("Model")} }
        SettingRow { title: tr!("Default model"), description: tr!("New sessions start with this model. A choice is saved at once."),
            Select {
                value: selected,
                options,
                onchange: move |value: String| settings.choose_model(model_from_value(&value)),
            }
        }
        SettingRow { title: tr!("Other model ID"), description: tr!("Use an ID that is not in the list."),
            TextInput {
                value: custom(),
                placeholder: "model-id",
                oninput: move |value| custom.set(value),
            }
            Button {
                label: tr!("Use"),
                disabled: custom_empty,
                onclick: move |_| {
                    settings.choose_model(custom.read().clone());
                    custom.set(String::new());
                },
            }
        }
        div { class: "hint", style: "margin-top: 12px;",
            {tr!("Sessions that are already open keep their model; a change applies to the next new session.")}
        }
    }
}

#[component]
fn StatusLine() -> Element {
    let settings = use_settings();
    match settings.status() {
        Some(SettingsStatus::Saved) => rsx! { div { class: "status-line", {tr!("Saved.")} } },
        Some(SettingsStatus::Error(message)) => rsx! { div { class: "status-line error", "{message}" } },
        None => rsx! { div { class: "status-line" } },
    }
}

/// One of the Skills / Plugins / MCP servers tabs: a row per definition with a switch that saves at once.
#[component]
fn ExtensionSection(kind: ExtensionKind) -> Element {
    let settings = use_settings();
    let (heading, tab, scoped) = match kind {
        ExtensionKind::Skill => (tr!("Skills"), settings.skill_tab(), true),
        ExtensionKind::Plugin => (tr!("Plugins"), settings.plugin_tab(), false),
        ExtensionKind::McpServer => (tr!("MCP servers"), settings.mcp_tab(), true),
    };
    let body = match tab {
        Err(message) => rsx! { div { class: "status-line error", "{message}" } },
        Ok(rows) if rows.is_empty() => rsx! { div { class: "hint", {tr!("Nothing found.")} } },
        Ok(rows) => rsx! {
            for row in rows {
                SettingRow { key: "{row.key}", title: row.title.clone(), description: row.detail.clone(),
                    Switch {
                        on: row.enabled,
                        onchange: {
                            let key = row.key.clone();
                            move |_| settings.toggle(kind, key.clone())
                        },
                    }
                }
            }
        },
    };
    rsx! {
        h2 { "{heading}" }
        if scoped {
            div { class: "hint", {tr!("Showing what is available for {project}.", project = settings.project_label())} }
        }
        {body}
        div { class: "hint", style: "margin-top: 12px;",
            {tr!("Sessions that are already open keep what they started with; a change applies to the next new session.")}
        }
        div { class: "hint",
            {tr!("Forge sessions do not use the hooks or permission rules from ~/.claude/settings.json; project settings still apply.")}
        }
    }
}
