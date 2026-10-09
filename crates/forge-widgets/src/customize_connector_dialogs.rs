use dioxus::prelude::*;
use forge_domain::{ConnectorDraft, ConnectorKind};
use forge_features::{sign_in_status, use_customize};
use forge_shared::{msg, tr, Button, ButtonKind};

use crate::terminal_panel::SignInTerminal;

fn kind_button(current: ConnectorKind, value: ConnectorKind, label: &'static str, mut kind: Signal<ConnectorKind>, disabled: bool) -> Element {
    rsx! {
        button {
            class: if current == value { "active" } else { "" },
            disabled,
            onclick: move |_| kind.set(value),
            {forge_shared::i18n::text(label)}
        }
    }
}

#[component]
pub fn AddConnector() -> Element {
    let customize = use_customize();
    let mut name = use_signal(String::new);
    let kind = use_signal(|| ConnectorKind::Http);
    let mut url = use_signal(String::new);
    let mut command = use_signal(String::new);
    let mut args = use_signal(String::new);
    let headers: Signal<Vec<(String, String)>> = use_signal(|| vec![(String::new(), String::new())]);
    let env: Signal<Vec<(String, String)>> = use_signal(|| vec![(String::new(), String::new())]);
    let adding = customize.connectors_busy();
    let current = kind();
    let draft = move || ConnectorDraft {
        name: name(),
        kind: kind(),
        url: url(),
        command: command(),
        args: args(),
        headers: headers(),
        env: env(),
    };
    rsx! {
        h2 { {tr!("Add connector")} }
        p { class: "hint", {tr!("A connector is an MCP server. It is added for you in every project.")} }
        input {
            class: "customize-field",
            r#type: "text",
            placeholder: tr!("Name (letters, numbers, - and _)"),
            value: "{name}",
            disabled: adding,
            oninput: move |event| name.set(event.value()),
        }
        div { class: "customize-switch",
            {kind_button(current, ConnectorKind::Http, msg!("Remote (http)"), kind, adding)}
            {kind_button(current, ConnectorKind::Sse, msg!("Remote (sse)"), kind, adding)}
            {kind_button(current, ConnectorKind::Stdio, msg!("Local"), kind, adding)}
        }
        if current == ConnectorKind::Stdio {
            input {
                class: "customize-field",
                r#type: "text",
                placeholder: tr!("Command, e.g. npx"),
                value: "{command}",
                disabled: adding,
                oninput: move |event| command.set(event.value()),
            }
            input {
                class: "customize-field",
                r#type: "text",
                placeholder: tr!("Arguments, e.g. -y some-mcp-server"),
                value: "{args}",
                disabled: adding,
                oninput: move |event| args.set(event.value()),
            }
            PairRows { label: tr!("Environment variables"), name_hint: tr!("NAME"), rows: env, disabled: adding }
        } else {
            input {
                class: "customize-field",
                r#type: "text",
                placeholder: "https://example.com/mcp",
                value: "{url}",
                disabled: adding,
                oninput: move |event| url.set(event.value()),
            }
            PairRows { label: tr!("Headers"), name_hint: tr!("Header name"), rows: headers, disabled: adding }
        }
        if let Some(message) = customize.dialog_error() {
            div { class: "status-line error customize-wrap", "{message}" }
        }
        div { class: "customize-dialog-buttons",
            if adding { span { class: "spinner", title: tr!("Adding…") } }
            Button { label: tr!("Cancel"), disabled: adding, onclick: move |_| customize.close_dialog() }
            Button { label: tr!("Add"), kind: ButtonKind::Primary, disabled: adding, onclick: move |_| customize.add_connector(draft()) }
        }
    }
}

// Rows of name and value. The value is a password field: a token typed here is not shown.
#[component]
fn PairRows(label: String, name_hint: String, rows: Signal<Vec<(String, String)>>, disabled: bool) -> Element {
    let count = rows.read().len();
    rsx! {
        div { class: "label", "{label}" }
        for index in 0..count {
            div { key: "{index}", class: "customize-pair",
                input {
                    class: "customize-field",
                    r#type: "text",
                    placeholder: "{name_hint}",
                    value: "{rows.read()[index].0}",
                    disabled,
                    oninput: move |event| rows.write()[index].0 = event.value(),
                }
                input {
                    class: "customize-field",
                    r#type: "password",
                    placeholder: tr!("Value"),
                    value: "{rows.read()[index].1}",
                    disabled,
                    oninput: move |event| rows.write()[index].1 = event.value(),
                }
            }
        }
        div {
            Button { label: tr!("+ Row"), disabled, onclick: move |_| rows.write().push((String::new(), String::new())) }
        }
    }
}

#[component]
pub fn ConfirmRemoveConnector(name: String) -> Element {
    let customize = use_customize();
    let busy = customize.connectors_busy();
    let target = name.clone();
    rsx! {
        h2 { {tr!("Remove {name}?", name = name)} }
        p { class: "hint", {tr!("It is removed from Claude Code on this computer, so the terminal's Claude Code loses it too.")} }
        if let Some(message) = customize.dialog_error() {
            div { class: "status-line error customize-wrap", "{message}" }
        }
        div { class: "customize-dialog-buttons",
            if busy { span { class: "spinner", title: tr!("Removing…") } }
            Button { label: tr!("Cancel"), disabled: busy, onclick: move |_| customize.close_dialog() }
            Button { label: tr!("Remove"), kind: ButtonKind::Primary, disabled: busy, onclick: move |_| customize.remove_connector(target.clone()) }
        }
    }
}

/// The sign-in dialog: a terminal that Forge types into, and one status line. Close ends the shell.
#[component]
pub fn SignInConnector(name: String) -> Element {
    let customize = use_customize();
    let Some(tap) = customize.sign_in_tap() else {
        return rsx! {};
    };
    let status = sign_in_status(&customize.sign_in_state(), &customize.connection());
    rsx! {
        h2 { {tr!("Sign in to {name}", name = name)} }
        p { class: "hint", {tr!("Forge types into Claude Code's /mcp screen for you. Finish the sign-in in your browser. If it stops, you can type here.")} }
        SignInTerminal { cwd: customize.sign_in_cwd(), tap }
        div { class: "status-line customize-wrap", "{status}" }
        div { class: "customize-dialog-buttons",
            Button { label: tr!("Close"), kind: ButtonKind::Primary, onclick: move |_| customize.close_sign_in() }
        }
    }
}
