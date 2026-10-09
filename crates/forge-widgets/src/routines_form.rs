use dioxus::prelude::*;
use forge_entities::{model_from_value, model_options, model_value};
use forge_features::{mode_options, mode_value, parse_mode, use_routines, use_settings, ScheduleKind};
use forge_domain::PermissionMode;
use forge_shared::{msg, tr, Button, ButtonKind, Select, TextArea, TextInput};

const KINDS: [(ScheduleKind, &str); 6] = [
    (ScheduleKind::Manual, msg!("Manual")),
    (ScheduleKind::Hourly, msg!("Hourly")),
    (ScheduleKind::Daily, msg!("Daily")),
    (ScheduleKind::Weekdays, msg!("Weekdays")),
    (ScheduleKind::Weekly, msg!("Weekly")),
    (ScheduleKind::Custom, msg!("Custom")),
];
const DAYS: [&str; 7] = [msg!("Sunday"), msg!("Monday"), msg!("Tuesday"), msg!("Wednesday"), msg!("Thursday"), msg!("Friday"), msg!("Saturday")];

/// The form of a new routine (`id` is `None`) or of the one being edited.
#[component]
pub fn RoutineForm(id: Option<String>) -> Element {
    let routines = use_routines();
    let settings = use_settings();
    let form = routines.form();
    let editing = id.is_some();
    // A routine already in bypass keeps its mode selected even when the switch is off.
    let allow_bypass = settings.allow_bypass_mode() || form.mode == PermissionMode::BypassPermissions;
    let problem = form.error();
    let started_typing = !form.name.is_empty() || !form.description.is_empty();
    let shows_time = matches!(form.kind, ScheduleKind::Daily | ScheduleKind::Weekdays | ScheduleKind::Weekly);
    let time_value = format!("{:02}:{:02}", form.hour, form.minute);
    let folder_text = form.folder.as_ref().map(|folder| folder.display().to_string()).unwrap_or_else(|| tr!("No folder chosen").to_string());
    let day_options: Vec<(String, String)> = DAYS.iter().enumerate().map(|(number, name)| (number.to_string(), forge_shared::i18n::text(name).to_string())).collect();
    rsx! {
        div { class: "routine-head",
            button { class: "link", onclick: move |_| routines.leave_form(), {tr!("Routines")} }
            span { if editing { {format!(" / {}", tr!("Edit routine"))} } else { {format!(" / {}", tr!("New routine"))} } }
        }
        p { class: "routines-notice", {tr!("Routines run only while Forge is open and the computer is awake.")} }
        div { class: "routine-form",
            label { {tr!("Name *")} }
            TextInput { value: form.name.clone(), placeholder: "daily-code-review", oninput: move |text: String| routines.edit_form(|f| f.name = text) }
            label { {tr!("Description *")} }
            TextInput {
                value: form.description.clone(),
                placeholder: tr!("Review yesterday's commits and flag concerns"),
                oninput: move |text: String| routines.edit_form(|f| f.description = text),
            }
            label { {tr!("Instructions")} }
            TextArea {
                value: form.instructions.clone(),
                placeholder: tr!("Look at the commits of the last 24 hours. Summarise what changed, point out risky patterns or missing tests, and note anything that needs a follow-up."),
                oninput: move |text: String| routines.edit_form(|f| f.instructions = text),
            }
            div { class: "routine-form-row",
                Select {
                    value: mode_value(form.mode).to_string(),
                    options: mode_options(allow_bypass),
                    onchange: move |value: String| routines.edit_form(|f| f.mode = parse_mode(&value)),
                }
                Select {
                    value: model_value(form.model.as_deref()),
                    options: model_options(form.model.as_deref()),
                    onchange: move |value: String| {
                        let chosen = model_from_value(&value);
                        routines.edit_form(|f| f.model = if chosen.is_empty() { None } else { Some(chosen) });
                    },
                }
                Button { label: tr!("Choose folder…"), onclick: move |_| routines.choose_folder() }
                span { class: "routine-folder", "{folder_text}" }
            }
            label { {tr!("Schedule")} }
            div { class: "customize-switch",
                for (kind, label) in KINDS {
                    button {
                        key: "{label}",
                        class: if form.kind == kind { "active" } else { "" },
                        onclick: move |_| routines.edit_form(|f| f.kind = kind),
                        {forge_shared::i18n::text(label)}
                    }
                }
            }
            if form.kind == ScheduleKind::Weekly {
                Select {
                    value: form.day.to_string(),
                    options: day_options,
                    onchange: move |value: String| routines.edit_form(|f| f.day = value.parse().unwrap_or(1)),
                }
            }
            if shows_time {
                label { {tr!("Time")} }
                input {
                    class: "customize-field",
                    r#type: "time",
                    value: "{time_value}",
                    oninput: move |event| {
                        if let Some((hour, minute)) = event.value().split_once(':') {
                            let (hour, minute) = (hour.parse::<u8>(), minute.parse::<u8>());
                            routines.edit_form(|f| {
                                f.hour = hour.unwrap_or(f.hour);
                                f.minute = minute.unwrap_or(f.minute);
                            });
                        }
                    },
                }
            }
            if form.kind == ScheduleKind::Custom {
                label { {tr!("Cron expression")} }
                TextInput { value: form.cron.clone(), placeholder: "0 9 * * *", oninput: move |text: String| routines.edit_form(|f| f.cron = text) }
            }
            if form.kind != ScheduleKind::Manual {
                p { class: "hint", {tr!("A run starts a few minutes after the set time (a fixed delay for each routine).")} }
            }
            if let Some(problem) = problem.clone() {
                if started_typing {
                    p { class: "routine-problem", "{problem}" }
                }
            }
            div { class: "routine-form-buttons",
                Button { label: tr!("Cancel"), onclick: move |_| routines.leave_form() }
                Button {
                    label: if editing { tr!("Save") } else { tr!("Create") },
                    kind: ButtonKind::Primary,
                    disabled: problem.is_some() || routines.busy(),
                    onclick: move |_| routines.submit_form(),
                }
            }
        }
    }
}
