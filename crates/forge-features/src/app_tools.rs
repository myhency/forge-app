//! The app's side of the tools the desktop app gives Claude to look at and steer the app itself
//! (`ccd_session_mgmt`, `ccd_sidebar`, `ccd_view`, `ccd_settings`; schemas read from a desktop
//! session, 2026-11-06). The sidecar forwards each call as an `AppRequest`; this answers it from the
//! workspace: the sessions in the sidebar, the open ones, the pins, the transcript views and the
//! settings.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use forge_application::{AppRequest, LiveSessionId, Phase};
use forge_domain::{AppConfig, SessionId, TranscriptView};
use dioxus::prelude::ReadableExt;
use serde_json::{json, Value};

use crate::sidebar::EntryKey;
use crate::workspace::Workspace;

/// Requests already being answered: the view keeps a request until the answer reaches the actor,
/// and the watcher looks again before that.
static SERVED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn first_time(request: &str) -> bool {
    let mut served = SERVED.lock().unwrap();
    served.get_or_insert_with(HashSet::new).insert(request.to_string())
}

/// One session as the tools describe it.
#[derive(Debug, Clone)]
struct Row {
    session_id: Option<SessionId>,
    live: Option<LiveSessionId>,
    title: String,
    folder: PathBuf,
    updated_at: Option<String>,
    archived: bool,
    pinned: bool,
}

impl Row {
    fn id_text(&self) -> String {
        match (&self.session_id, self.live) {
            (Some(id), _) => id.0.clone(),
            (None, Some(live)) => format!("open-{}", live.0),
            (None, None) => String::new(),
        }
    }
}

fn view_name(view: TranscriptView) -> &'static str {
    match view {
        TranscriptView::Normal => "normal",
        TranscriptView::Thinking => "thinking",
        TranscriptView::Detailed => "verbose",
    }
}

impl Workspace {
    /// Answers every app tool call the open sessions are waiting on. Called by the session watcher.
    pub fn serve_app_requests(&self, id: LiveSessionId, config: &AppConfig, settings: crate::Settings) {
        let Some(view) = self.live_view(id) else { return };
        let requests: Vec<AppRequest> = view.peek().app_requests.clone();
        for request in requests {
            if first_time(&request.id) {
                self.serve(id, request, config, settings);
            }
        }
    }

    fn answer(&self, id: LiveSessionId, request: &AppRequest, result: Result<String, String>) {
        let Some(handle) = self.handle(id) else { return };
        match result {
            Ok(text) => handle.answer_app_request(request.id.clone(), text, false),
            Err(text) => handle.answer_app_request(request.id.clone(), text, true),
        }
    }

    fn serve(&self, id: LiveSessionId, request: AppRequest, config: &AppConfig, settings: crate::Settings) {
        let input = &request.input;
        let text = |key: &str| input.get(key).and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty());
        let flag = |key: &str| input.get(key).and_then(Value::as_bool).unwrap_or(false);
        let limit = input.get("limit").and_then(Value::as_u64).map(|n| n.clamp(1, 200) as usize).unwrap_or(20);
        let result = match request.tool.as_str() {
            "list_sessions" => Ok(self.list_sessions_json(id, flag("include_archived"), flag("linked"), limit)),
            "get_session" => self.get_session_json(id, text("session_id").unwrap_or("self")),
            "send_message" => self.send_to_session(id, text("session_id").unwrap_or_default(), text("message").unwrap_or_default()),
            "search_session_transcripts" => return self.search_transcripts(id, request.clone(), flag("include_archived"), limit),
            "set_pinned" => self.set_pinned_by_tool(id, text("session_id").unwrap_or("self"), input.get("pinned").and_then(Value::as_bool)),
            "set_transcript_view" => self.set_view_by_tool(id, text("session_id").unwrap_or("self"), text("view").unwrap_or_default()),
            "get_settings" => Ok(settings_json(config)),
            "list_sections" => Ok(self.sections_json()),
            "export_transcript" => return self.export_by_tool(id, request.clone(), text("session_id").unwrap_or("self").to_string()),
            "show_pane" | "close_pane" => self.pane_by_tool(id, request.tool == "show_pane", text("pane").unwrap_or_default(), text("path")),
            "get_layout" => Ok(self.layout_json(id)),
            "preview_start" => self.preview_start_by_tool(id, text("name"), text("url")),
            "preview_list" => Ok(self.preview_list_json(id)),
            "tabs_context_mcp" => {
                if input.get("createIfEmpty").and_then(Value::as_bool) == Some(true) && self.previews(id).is_empty() {
                    self.open_empty_preview_tab(id);
                }
                Ok(self.preview_tabs_json(id))
            }
            "navigate" => self.navigate_by_tool(id, input.get("tabId").and_then(Value::as_u64), text("url").unwrap_or_default()),
            "get_page_text" => return self.eval_by_tool(id, request.clone(), input.get("tabId").and_then(Value::as_u64), PAGE_TEXT_SCRIPT.to_string()),
            "computer" => match text("action").unwrap_or_default() {
                "screenshot" => return self.screenshot_by_tool(id, request.clone(), input.get("tabId").and_then(Value::as_u64)),
                "scroll" => {
                    let ticks = input.get("scroll_amount").and_then(Value::as_i64).unwrap_or(3).clamp(1, 50) * 100;
                    let (x, y) = match text("scroll_direction").unwrap_or("down") {
                        "up" => (0, -ticks),
                        "left" => (-ticks, 0),
                        "right" => (ticks, 0),
                        _ => (0, ticks),
                    };
                    return self.eval_by_tool(id, request.clone(), input.get("tabId").and_then(Value::as_u64), format!("(function(){{window.scrollBy({x},{y});return 'Scrolled to '+Math.round(window.scrollX)+','+Math.round(window.scrollY)+' of '+document.documentElement.scrollWidth+'x'+document.documentElement.scrollHeight;}})()"));
                }
                "wait" => {
                    let seconds = input.get("duration").and_then(Value::as_f64).unwrap_or(1.0).clamp(0.0, 30.0);
                    let this = *self;
                    let request = request.clone();
                    dioxus::core::spawn_forever(async move {
                        tokio::time::sleep(std::time::Duration::from_secs_f64(seconds)).await;
                        this.answer(id, &request, Ok(format!("Waited {seconds} seconds.")));
                    });
                    return;
                }
                action @ ("left_click" | "double_click" | "triple_click" | "right_click" | "hover" | "type" | "key" | "scroll_to" | "left_click_drag") => {
                    let args = json!({ "action": action, "coordinate": input.get("coordinate"), "start": input.get("start_coordinate"), "text": text("text"), "ref": text("ref") });
                    return self.eval_by_tool(id, request.clone(), input.get("tabId").and_then(Value::as_u64), COMPUTER_SCRIPT.replace("__ARGS__", &args.to_string()));
                }
                other => Err(format!("Forge's browser tool does not do \"{other}\"; use read_page with form_input, or javascript_tool.")),
            },
            "find" => {
                let query = text("query").unwrap_or_default().to_string();
                if query.trim().is_empty() {
                    Err("Say what to find, e.g. \"login button\".".to_string())
                } else {
                    let args = json!({ "interactive": false, "depth": 60, "ref": Value::Null, "max": 400_000 });
                    let script = READ_PAGE_SCRIPT.replace("__ARGS__", &args.to_string());
                    return self.eval_then(id, request.clone(), input.get("tabId").and_then(Value::as_u64), script, move |outline| find_in_outline(&outline, &query));
                }
            }
            "read_page" => {
                let args = json!({
                    "interactive": text("filter") == Some("interactive"),
                    "depth": input.get("depth").and_then(Value::as_u64).unwrap_or(15).clamp(1, 60),
                    "ref": text("ref_id"),
                    "max": input.get("max_chars").and_then(Value::as_u64).unwrap_or(50_000).clamp(1_000, 500_000),
                });
                return self.eval_by_tool(id, request.clone(), input.get("tabId").and_then(Value::as_u64), READ_PAGE_SCRIPT.replace("__ARGS__", &args.to_string()));
            }
            "gif_creator" => return self.gif_by_tool(id, request.clone(), input.get("tabId").and_then(Value::as_u64), text("action").unwrap_or_default(), text("filename").unwrap_or_default().to_string()),
            "file_upload" => {
                let paths: Vec<std::path::PathBuf> = input.get("paths").and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_str).map(std::path::PathBuf::from).collect()).unwrap_or_default();
                match (text("ref"), paths.is_empty()) {
                    (Some(reference), false) => {
                        let reference = reference.to_string();
                        let tab = input.get("tabId").and_then(Value::as_u64);
                        let this = *self;
                        let request = request.clone();
                        dioxus::core::spawn_forever(async move {
                            match crate::run_blocking(move || upload_files(&paths)).await.unwrap_or_else(|| Err("The files could not be read.".to_string())) {
                                Ok(files) => {
                                    let args = json!({ "ref": reference, "files": files });
                                    this.eval_by_tool(id, request, tab, FILE_UPLOAD_SCRIPT.replace("__ARGS__", &args.to_string()));
                                }
                                Err(message) => this.answer(id, &request, Err(message)),
                            }
                        });
                        return;
                    }
                    _ => Err("Pass ref (a file input, from read_page or find) and paths (absolute paths of files on this computer).".to_string()),
                }
            }
            "form_input" => match (text("ref"), input.get("value")) {
                (Some(reference), Some(value)) => {
                    let args = json!({ "ref": reference, "value": value });
                    return self.eval_by_tool(id, request.clone(), input.get("tabId").and_then(Value::as_u64), FORM_INPUT_SCRIPT.replace("__ARGS__", &args.to_string()));
                }
                _ => Err("Pass ref (from read_page) and value.".to_string()),
            },
            "read_console_messages" => {
                let pattern = text("pattern").unwrap_or_default();
                let only_errors = input.get("onlyErrors").and_then(Value::as_bool).unwrap_or(false);
                let clear = input.get("clear").and_then(Value::as_bool).unwrap_or(false);
                let limit = input.get("limit").and_then(Value::as_u64).unwrap_or(100).clamp(1, 500);
                return self.eval_by_tool(id, request.clone(), input.get("tabId").and_then(Value::as_u64), console_script(pattern, only_errors, clear, limit));
            }
            "read_network_requests" => {
                let args = json!({
                    "pattern": text("urlPattern").unwrap_or_default(),
                    "clear": input.get("clear").and_then(Value::as_bool).unwrap_or(false),
                    "limit": input.get("limit").and_then(Value::as_u64).unwrap_or(100).clamp(1, 500),
                });
                return self.eval_by_tool(id, request.clone(), input.get("tabId").and_then(Value::as_u64), NETWORK_READ_SCRIPT.replace("__ARGS__", &args.to_string()));
            }
            "javascript_tool" => {
                let code = text("text").unwrap_or_default();
                if code.trim().is_empty() {
                    Err("Pass the JavaScript to run as text.".to_string())
                } else {
                    return self.eval_by_tool(id, request.clone(), input.get("tabId").and_then(Value::as_u64), script_runner(code));
                }
            }
            "preview_stop" => {
                let server = input.get("serverId").and_then(Value::as_u64).or_else(|| text("serverId").and_then(|id| id.parse().ok()));
                match server {
                    Some(server) if server > crate::workspace::BACKGROUND_ID_BASE && self.background().stop(server - crate::workspace::BACKGROUND_ID_BASE) => Ok(format!("Server {server} is stopped.")),
                    Some(server) if self.code_runs(id).request_stop(server) => Ok(format!("Server {server} is stopping (Ctrl-C in its terminal).")),
                    Some(server) => Err(format!("No running server {server}; preview_list lists them.")),
                    None => Err("Pass serverId from preview_list.".to_string()),
                }
            }
            "preview_logs" => {
                let server = input.get("serverId").and_then(Value::as_u64).or_else(|| text("serverId").and_then(|id| id.parse().ok()));
                let output = server.and_then(|server| {
                    if server > crate::workspace::BACKGROUND_ID_BASE && self.background_servers_of(id).iter().any(|run| run.id == server - crate::workspace::BACKGROUND_ID_BASE) {
                        self.background().output(server - crate::workspace::BACKGROUND_ID_BASE)
                    } else {
                        self.code_runs(id).server_output(server)
                    }
                });
                match output {
                    Some(output) => Ok(server_log(&output, text("level"), text("search"))),
                    None => Err("No such server; preview_list lists them with their serverId.".to_string()),
                }
            }
            "create_section" => self.create_section_by_tool(text("name").unwrap_or_default()),
            "rename_section" => {
                let section = text("section_id").unwrap_or_default().to_string();
                let name = text("name").unwrap_or_default().to_string();
                if !self.prefs().sections.iter().any(|known| known.id == section) {
                    Err("Only a section you or the user made can be renamed.".to_string())
                } else if name.is_empty() {
                    Err("Give the new name.".to_string())
                } else {
                    self.rename_section(&section, name.clone());
                    Ok(format!("The section is now called {name}."))
                }
            }
            "delete_section" => {
                let section = text("section_id").unwrap_or_default().to_string();
                if !self.prefs().sections.iter().any(|known| known.id == section) {
                    Err("Only a section you or the user made can be deleted.".to_string())
                } else {
                    self.delete_section(&section);
                    Ok("Deleted the section; its sessions are back in their usual groups.".to_string())
                }
            }
            "move_sessions" => self.move_sessions_by_tool(id, input),
            "set_unread" => {
                let wanted = text("session_id").unwrap_or("self");
                match (self.find_row(id, wanted), input.get("unread").and_then(Value::as_bool)) {
                    (Some(row), Some(unread)) => match row.session_id.clone() {
                        Some(session) => {
                            let now = self.is_unread(&session);
                            if now != unread {
                                self.toggle_unread(session);
                            }
                            Ok(format!("{} is {}.", row.title, if unread { "unread" } else { "read" }))
                        }
                        None => Err("That session has no transcript yet.".to_string()),
                    },
                    (None, _) => Err(format!("No session {wanted}.")),
                    (_, None) => Err("Say unread: true or false.".to_string()),
                }
            }
            "set_setting" => set_setting(settings, config, &self.available_output_styles(id), text("key").unwrap_or_default(), input.get("value")),
            "list_events" => return self.list_events(id, request.clone(), text("session_id").unwrap_or_default().to_string(), input.get("limit").and_then(Value::as_u64).map(|n| n.clamp(1, 200) as usize).unwrap_or(40)),
            "get_usage" => self.usage_json(id, text("session_id").unwrap_or("self")),
            "set_session_title" => self.rename_by_tool(id, text("session_id").unwrap_or("self"), text("title").unwrap_or_default()),
            "set_session_model" => self.open_other(id, text("session_id").unwrap_or_default()).and_then(|(handle, title)| {
                let model = text("model").ok_or_else(|| "Name a model.".to_string())?;
                handle.set_model(Some(model.to_string()));
                Ok(format!("{title} runs on {model} from its next turn."))
            }),
            "set_session_effort" => self.open_other(id, text("session_id").unwrap_or_default()).and_then(|(handle, title)| {
                let effort = match text("effort").unwrap_or_default() {
                    "low" => forge_domain::EffortLevel::Low,
                    "medium" => forge_domain::EffortLevel::Medium,
                    "high" => forge_domain::EffortLevel::High,
                    "xhigh" => forge_domain::EffortLevel::XHigh,
                    "max" => forge_domain::EffortLevel::Max,
                    other => return Err(format!("No effort level called {other}.")),
                };
                handle.set_effort(Some(effort));
                Ok(format!("{title} runs at {} effort from its next turn.", text("effort").unwrap_or_default()))
            }),
            "set_session_permission_mode" => self.set_mode_by_tool(id, text("session_id").unwrap_or("self"), text("mode").unwrap_or_default(), config),
            "archive_session" => self.archive_by_tool(id, text("session_id").unwrap_or_default()),
            "set_session_output_style" => {
                let wanted = text("session_id").unwrap_or("self");
                let name = text("style").unwrap_or_default();
                let available = self.available_output_styles(id);
                let style = if name.eq_ignore_ascii_case("default") { Ok(None) } else { forge_domain::OutputStyle::find(name, &available).map(Some).ok_or_else(|| format!("No output style called {name}.")) };
                style.and_then(|style| {
                    let row = self.find_row(id, wanted).ok_or_else(|| format!("No session {wanted}."))?;
                    let handle = row.live.and_then(|live| self.handle(live)).ok_or_else(|| format!("{} is not open; open it first.", row.title))?;
                    let shown = style.as_ref().map(|style| style.display()).unwrap_or_else(|| "default".to_string());
                    handle.set_output_style(style);
                    Ok(format!("{} answers in the {shown} style from now on.", row.title))
                })
            }
            "unarchive_session" => self.unarchive_by_tool(id, text("session_id").unwrap_or_default()),
            "stop_session" => self.open_other(id, text("session_id").unwrap_or_default()).map(|(handle, title)| {
                if self.live_view(handle.id()).is_some_and(|view| view.peek().phase == Phase::Running) {
                    handle.interrupt();
                    format!("Stopped the turn in {title}; it is idle now.")
                } else {
                    format!("{title} is idle; it was left alone.")
                }
            }),
            other => Err(format!("Forge has no app tool called {other}.")),
        };
        self.answer(id, &request, result);
    }

    /// Every session the sidebar knows, newest first, as the tools describe them.
    fn rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = self
            .sidebar()
            .into_iter()
            .flat_map(|group| {
                let folder = group.project.path.clone();
                group.entries.into_iter().map(move |entry| (folder.clone(), entry))
            })
            .map(|(folder, entry)| Row {
                live: match entry.key {
                    EntryKey::Live(live) => Some(live),
                    EntryKey::Past { .. } => None,
                },
                pinned: entry.session_id.as_ref().is_some_and(|session| self.is_pinned(&folder, session)),
                session_id: entry.session_id,
                title: entry.title,
                folder,
                updated_at: entry.updated_at,
                archived: entry.archived,
            })
            .collect();
        rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        rows
    }

    /// The open session that started `child` (a card's Start or a hand-off), if any.
    fn started_by(&self, child: LiveSessionId) -> Option<LiveSessionId> {
        self.offer_sessions_snapshot().into_iter().find(|(_, started)| *started == child).map(|(parent, _)| parent)
    }

    fn is_self(row: &Row, id: LiveSessionId, own: Option<&SessionId>) -> bool {
        row.live == Some(id) || (own.is_some() && row.session_id.as_ref() == own)
    }

    fn own_session_id(&self, id: LiveSessionId) -> Option<SessionId> {
        self.live_view(id).and_then(|view| view.peek().claude_session_id.clone())
    }

    /// Finds a session by the id a tool was given: "self", "parent", a Claude session id or the
    /// `open-<n>` id of an open session with no transcript yet.
    fn find_row(&self, id: LiveSessionId, wanted: &str) -> Option<Row> {
        let rows = self.rows();
        let own = self.own_session_id(id);
        let live = match wanted {
            "self" => Some(id),
            "parent" => self.started_by(id),
            other => other.strip_prefix("open-").and_then(|n| n.parse::<u64>().ok()).map(LiveSessionId),
        };
        if let Some(live) = live {
            if let Some(row) = rows.iter().find(|row| row.live == Some(live)) {
                return Some(row.clone());
            }
            if live == id {
                return rows.into_iter().find(|row| own.is_some() && row.session_id == own);
            }
            return None;
        }
        rows.into_iter().find(|row| row.session_id.as_ref().is_some_and(|session| session.0 == wanted))
    }

    fn row_json(&self, row: &Row) -> Value {
        let mut value = json!({
            "sessionId": row.id_text(),
            "title": row.title,
            "folder": row.folder.display().to_string(),
            "open": row.live.is_some(),
            "archived": row.archived,
            "pinned": row.pinned,
        });
        if let Some(at) = &row.updated_at {
            value["updatedAt"] = json!(at);
        }
        if let Some(live) = row.live {
            let running = self.live_view(live).is_some_and(|view| view.peek().phase == Phase::Running);
            value["running"] = json!(running);
            if let Some(parent) = self.started_by(live) {
                if let Some(parent_row) = self.rows().into_iter().find(|candidate| candidate.live == Some(parent)) {
                    value["startedBy"] = json!(parent_row.id_text());
                }
            }
        }
        value
    }

    fn list_sessions_json(&self, id: LiveSessionId, include_archived: bool, linked: bool, limit: usize) -> String {
        let own = self.own_session_id(id);
        let parent = self.started_by(id);
        let children: Vec<LiveSessionId> = self.offer_sessions_snapshot().into_iter().filter(|(parent, _)| *parent == id).map(|(_, child)| child).collect();
        let rows: Vec<Value> = self
            .rows()
            .into_iter()
            .filter(|row| !Self::is_self(row, id, own.as_ref()))
            .filter(|row| include_archived || !row.archived)
            .filter(|row| !linked || row.live.is_some_and(|live| Some(live) == parent || children.contains(&live)))
            .take(limit)
            .map(|row| self.row_json(&row))
            .collect();
        Value::Array(rows).to_string()
    }

    fn get_session_json(&self, id: LiveSessionId, wanted: &str) -> Result<String, String> {
        let row = self.find_row(id, wanted).ok_or_else(|| format!("No session {wanted}."))?;
        let mut value = self.row_json(&row);
        if let Some(view) = row.live.and_then(|live| self.live_view(live)) {
            let view = view.peek();
            value["model"] = json!(view.model.clone().unwrap_or_else(|| "default".to_string()));
            value["effort"] = json!(view.effort.map(|effort| format!("{effort:?}").to_lowercase()).unwrap_or_else(|| "default".to_string()));
            value["permissionMode"] = json!(format!("{:?}", view.session.permission_mode));
            value["transcriptView"] = json!(view_name(self.transcript_view(view.id, TranscriptView::Normal)));
        }
        if let Some(parent) = row.live.and_then(|live| self.started_by(live)) {
            if let Some(parent_row) = self.rows().into_iter().find(|candidate| candidate.live == Some(parent)) {
                value["parentSessionId"] = json!(parent_row.id_text());
            }
        }
        Ok(value.to_string())
    }

    /// Sends `message` to another session as a user turn that says where it came from: at once
    /// when that session is idle ("delivered"), after its turn when it is working ("queued"). A
    /// session that is not open is opened first.
    fn send_to_session(&self, id: LiveSessionId, wanted: &str, message: &str) -> Result<String, String> {
        if wanted.is_empty() || message.is_empty() {
            return Err("A message needs a session_id and a message.".to_string());
        }
        let row = self.find_row(id, wanted).ok_or_else(|| format!("No session {wanted}."))?;
        let own = self.own_session_id(id);
        if Self::is_self(&row, id, own.as_ref()) {
            return Err("That is this session.".to_string());
        }
        let from = self.find_row(id, "self").map(|row| row.title).unwrap_or_default();
        let text = format!("From {from}:\n\n{message}");
        if let Some(handle) = row.live.and_then(|live| self.handle(live)) {
            let running = self.live_view(handle.id()).is_some_and(|view| view.peek().phase == Phase::Running);
            if running {
                handle.queue(text, Vec::new());
                return Ok("queued: it runs when that session's current work finishes.".to_string());
            }
            handle.send_with(text, Vec::new());
            return Ok("delivered: that session's turn has started on your message.".to_string());
        }
        let session = row.session_id.clone().ok_or_else(|| "That session has no transcript yet.".to_string())?;
        self.open_and_send(row.folder, session, text);
        Ok("delivered: the session was opened and its turn has started on your message.".to_string())
    }

    fn search_transcripts(&self, id: LiveSessionId, request: AppRequest, include_archived: bool, limit: usize) {
        let query = request.input.get("query").and_then(Value::as_str).unwrap_or_default().trim().to_string();
        if query.chars().count() < 2 {
            return self.answer(id, &request, Err("The query needs at least 2 characters.".to_string()));
        }
        let this = *self;
        let services = self.services_snapshot();
        let own = self.own_session_id(id);
        let archived: Vec<SessionId> = self.rows().into_iter().filter(|row| row.archived).filter_map(|row| row.session_id).collect();
        dioxus::core::spawn_forever(async move {
            let result = match services.sessions.search_sessions(&query, limit * 2).await {
                Ok(hits) => {
                    let found: Vec<Value> = hits
                        .into_iter()
                        .filter(|hit| own.as_ref() != Some(&hit.session.id))
                        .filter(|hit| include_archived || !archived.contains(&hit.session.id))
                        .take(limit)
                        .map(|hit| {
                            json!({
                                "sessionId": hit.session.id.0,
                                "title": hit.session.title.clone().unwrap_or_default(),
                                "folder": hit.project.display().to_string(),
                                "snippet": hit.snippet.map(|snippet| format!("{}{}{}", snippet.before, snippet.matched, snippet.after)).unwrap_or_default(),
                            })
                        })
                        .collect();
                    Ok(Value::Array(found).to_string())
                }
                Err(error) => Err(error.to_string()),
            };
            this.answer(id, &request, result);
        });
    }

    fn set_pinned_by_tool(&self, id: LiveSessionId, wanted: &str, pinned: Option<bool>) -> Result<String, String> {
        let pinned = pinned.ok_or_else(|| "Say pinned: true or false.".to_string())?;
        let row = self.find_row(id, wanted).ok_or_else(|| format!("No session {wanted}."))?;
        let session = row.session_id.clone().ok_or_else(|| "That session has no transcript yet, so it cannot be pinned.".to_string())?;
        if self.is_pinned(&row.folder, &session) != pinned {
            self.toggle_pin(row.folder.clone(), session);
        }
        Ok(format!("{} is {}.", row.title, if pinned { "pinned" } else { "not pinned" }))
    }

    fn set_view_by_tool(&self, id: LiveSessionId, wanted: &str, view: &str) -> Result<String, String> {
        let view = match view {
            "normal" => TranscriptView::Normal,
            "thinking" => TranscriptView::Thinking,
            "verbose" => TranscriptView::Detailed,
            other => return Err(format!("No view called {other}; use normal, thinking or verbose.")),
        };
        let target = match wanted {
            "self" => id,
            other => {
                let row = self.find_row(id, other).ok_or_else(|| format!("No session {other}."))?;
                let live = row.live.ok_or_else(|| "That session is not open in any window, so it was left alone.".to_string())?;
                if self.started_by(live) != Some(id) {
                    return Err("Only this session or one it started can be switched; the user switches others from their own menu.".to_string());
                }
                live
            }
        };
        self.set_transcript_view(target, view);
        Ok(format!("The transcript view is now {}.", view_name(view)))
    }
}

impl Workspace {
    /// Another open session, by the id a tool was given, with its title. Refuses this session.
    fn open_other(&self, id: LiveSessionId, wanted: &str) -> Result<(forge_application::LiveSessionHandle, String), String> {
        if wanted.is_empty() {
            return Err("Name the session with session_id.".to_string());
        }
        let row = self.find_row(id, wanted).ok_or_else(|| format!("No session {wanted}."))?;
        let own = self.own_session_id(id);
        if Self::is_self(&row, id, own.as_ref()) {
            return Err("Not for this session.".to_string());
        }
        let handle = row.live.and_then(|live| self.handle(live)).ok_or_else(|| format!("{} is not open; open it first.", row.title))?;
        Ok((handle, row.title))
    }

    fn rename_by_tool(&self, id: LiveSessionId, wanted: &str, title: &str) -> Result<String, String> {
        if title.is_empty() {
            return Err("Give the new title.".to_string());
        }
        let row = self.find_row(id, wanted).ok_or_else(|| format!("No session {wanted}."))?;
        let session = row.session_id.clone().ok_or_else(|| "That session has no transcript yet, so it cannot be renamed.".to_string())?;
        self.rename_session(row.folder.clone(), session, title.to_string());
        Ok(format!("Renamed {} to {title}.", row.title))
    }

    fn set_mode_by_tool(&self, id: LiveSessionId, wanted: &str, mode: &str, config: &AppConfig) -> Result<String, String> {
        use forge_domain::PermissionMode;
        let mode = match mode {
            "default" => PermissionMode::Default,
            "acceptEdits" => PermissionMode::AcceptEdits,
            "plan" => PermissionMode::Plan,
            "auto" => PermissionMode::Auto,
            "bypassPermissions" if config.allow_bypass_mode => PermissionMode::BypassPermissions,
            "bypassPermissions" => return Err("Bypass permissions is off in Settings, so that mode is unavailable.".to_string()),
            other => return Err(format!("No permission mode called {other}.")),
        };
        let row = self.find_row(id, wanted).ok_or_else(|| format!("No session {wanted}."))?;
        let handle = row.live.and_then(|live| self.handle(live)).ok_or_else(|| format!("{} is not open; open it first.", row.title))?;
        handle.set_permission_mode(mode);
        Ok(format!("{} is now in {mode:?} mode.", row.title))
    }

    fn archive_by_tool(&self, id: LiveSessionId, wanted: &str) -> Result<String, String> {
        let row = self.find_row(id, if wanted.is_empty() { "self" } else { wanted }).ok_or_else(|| format!("No session {wanted}."))?;
        let session = row.session_id.clone().ok_or_else(|| "That session has no transcript yet, so it cannot be archived.".to_string())?;
        let own = self.own_session_id(id);
        let is_self = Self::is_self(&row, id, own.as_ref());
        if let Some(live) = row.live {
            if !is_self && self.live_view(live).is_some_and(|view| view.peek().phase == Phase::Running) {
                return Err(format!("{} is still working, so it was not archived.", row.title));
            }
        }
        if is_self {
            // The answer has to reach this session before it closes.
            let this = *self;
            let folder = row.folder.clone();
            dioxus::core::spawn_forever(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                this.archive_session(folder, session);
                this.close_remembering(id);
            });
            return Ok("This session is archived when this turn ends; the conversation ends here.".to_string());
        }
        self.archive_session(row.folder.clone(), session);
        if let Some(live) = row.live {
            self.close_remembering(live);
        }
        Ok(format!("Archived {}.", row.title))
    }

    fn unarchive_by_tool(&self, id: LiveSessionId, wanted: &str) -> Result<String, String> {
        let row = self.find_row(id, wanted).ok_or_else(|| format!("No session {wanted}."))?;
        if !row.archived {
            return Ok(format!("{} is not archived.", row.title));
        }
        let session = row.session_id.clone().ok_or_else(|| "That session has no transcript.".to_string())?;
        self.unarchive_session(row.folder.clone(), session);
        Ok(format!("Restored {} to the session list.", row.title))
    }

    fn usage_json(&self, id: LiveSessionId, wanted: &str) -> Result<String, String> {
        let row = self.find_row(id, wanted).ok_or_else(|| format!("No session {wanted}."))?;
        let mut value = json!({ "sessionId": row.id_text() });
        match row.live.and_then(|live| self.live_view(live)) {
            Some(view) => {
                let view = view.peek();
                match &view.context {
                    Some(context) => {
                        value["context"] = json!({ "tokens": context.total_tokens, "window": context.max_tokens, "percent": context.percent() });
                    }
                    None => value["context"] = json!("unavailable: no turn has finished yet"),
                }
                match &view.rate_limit {
                    Some(limit) => value["plan"] = json!({ "window": format!("{:?}", limit.window), "level": format!("{:?}", limit.level), "percent": limit.percent, "resetsAt": limit.resets_at.and_then(|at| time::OffsetDateTime::from_unix_timestamp(at).ok()).and_then(|at| at.format(&time::format_description::well_known::Rfc3339).ok()) }),
                    None => value["plan"] = json!("unavailable: the account has not reported its limits in this session"),
                }
            }
            None => value["context"] = json!("unavailable: the session is not open"),
        }
        Ok(value.to_string())
    }

    /// The last messages of another session as plain text: who said what, and one line per tool call.
    fn list_events(&self, id: LiveSessionId, request: AppRequest, wanted: String, limit: usize) {
        let row = match self.find_row(id, &wanted) {
            Some(row) => row,
            None => return self.answer(id, &request, Err(format!("No session {wanted}."))),
        };
        let own = self.own_session_id(id);
        if Self::is_self(&row, id, own.as_ref()) {
            return self.answer(id, &request, Err("Not for this session.".to_string()));
        }
        if let Some(view) = row.live.and_then(|live| self.live_view(live)) {
            let text = render_events(&view.peek().session.messages, limit);
            return self.answer(id, &request, Ok(text));
        }
        let Some(session) = row.session_id.clone() else {
            return self.answer(id, &request, Err("That session has no transcript yet.".to_string()));
        };
        let this = *self;
        let services = self.services_snapshot();
        dioxus::core::spawn_forever(async move {
            let result = match services.sessions.load_session(&row.folder, &session).await {
                Ok(loaded) => Ok(render_events(&loaded.messages, limit)),
                Err(error) => Err(error.to_string()),
            };
            this.answer(id, &request, result);
        });
    }
}

fn render_events(messages: &[forge_domain::Message], limit: usize) -> String {
    let start = messages.len().saturating_sub(limit);
    let mut out = String::new();
    for message in &messages[start..] {
        let who = match message.role {
            forge_domain::MessageRole::User => "User",
            forge_domain::MessageRole::Assistant => "Claude",
        };
        for block in &message.blocks {
            match block {
                forge_domain::Block::Text(text) if !text.trim().is_empty() => out.push_str(&format!("{who}: {}\n", text.trim())),
                forge_domain::Block::Tool(call) => {
                    let failed = call.result.as_ref().is_some_and(|result| result.is_error);
                    out.push_str(&format!("  [tool {}{}]\n", call.name, if failed { ", failed" } else { "" }));
                }
                _ => {}
            }
        }
    }
    if out.is_empty() {
        "The session has no messages yet.".to_string()
    } else {
        out
    }
}

/// The pane a view tool names, as Forge's dock has it.
fn pane_of(name: &str) -> Option<crate::Panel> {
    Some(match name {
        "diff" => crate::Panel::Changes,
        "file" => crate::Panel::Files,
        "terminal" => crate::Panel::Terminal,
        "tasks" => crate::Panel::Tasks,
        "plan" => crate::Panel::Plan,
        "preview" => crate::Panel::Preview,
        "details" => crate::Panel::Details,
        _ => return None,
    })
}

const PANE_NAMES: [(&str, crate::Panel); 7] = [
    ("diff", crate::Panel::Changes),
    ("file", crate::Panel::Files),
    ("terminal", crate::Panel::Terminal),
    ("tasks", crate::Panel::Tasks),
    ("plan", crate::Panel::Plan),
    ("preview", crate::Panel::Preview),
    ("details", crate::Panel::Details),
];

impl Workspace {
    fn pane_by_tool(&self, id: LiveSessionId, show: bool, name: &str, path: Option<&str>) -> Result<String, String> {
        let pane = pane_of(name).ok_or_else(|| format!("Forge has no {name} pane."))?;
        if !self.pane_targets().contains(&crate::Active::Live(id)) {
            return Ok("This session is not on screen, so nothing changed; tell the user what to look at.".to_string());
        }
        if !show {
            self.change_dock(id, |dock| dock.close(pane));
            return Ok(format!("Closed the {name} pane."));
        }
        if let (crate::Panel::Files, Some(path)) = (pane, path) {
            let folder = self.live_view(id).map(|view| view.peek().project_path.clone()).unwrap_or_default();
            let full = if std::path::Path::new(path).is_absolute() { PathBuf::from(path) } else { folder.join(path) };
            if !full.starts_with(&folder) {
                return Err("That file is outside the session's folder.".to_string());
            }
            if !full.is_file() {
                return Err(format!("There is no file {}.", full.display()));
            }
            self.open_file(id, full.display().to_string());
            return Ok(format!("Opened {} in the Files pane.", full.display()));
        }
        self.change_dock(id, |dock| dock.open(pane));
        Ok(format!("Showing the {name} pane."))
    }

    fn layout_json(&self, id: LiveSessionId) -> String {
        let on_screen = self.pane_targets().contains(&crate::Active::Live(id));
        let dock = self.dock(id);
        let open: Vec<&str> = PANE_NAMES.iter().filter(|(_, pane)| dock.is_open(*pane)).map(|(name, _)| *name).collect();
        let view = match self.transcript_view(id, TranscriptView::Normal) {
            TranscriptView::Normal => "normal",
            TranscriptView::Thinking => "thinking",
            TranscriptView::Detailed => "detailed",
        };
        json!({ "on_screen": on_screen, "open_panes": if on_screen { open } else { Vec::new() }, "transcript_view": view }).to_string()
    }

    fn export_by_tool(&self, id: LiveSessionId, request: AppRequest, wanted: String) {
        let Some(row) = self.find_row(id, &wanted) else {
            return self.answer(id, &request, Err(format!("No session {wanted}.")));
        };
        let Some(session) = row.session_id.clone() else {
            return self.answer(id, &request, Err("That session has no transcript yet.".to_string()));
        };
        let caller = format!("{id:?}");
        if let Err(refusal) = export_limits().lock().map_err(|_| "Exports are unavailable just now.".to_string()).and_then(|mut limits| limits.entry(caller.clone()).or_default().begin(std::time::Instant::now())) {
            return self.answer(id, &request, Err(refusal));
        }
        let this = *self;
        let services = self.services_snapshot();
        dioxus::core::spawn_forever(async move {
            let result = services.sessions.export_session(&row.folder, &session).await.map(|path| format!("Exported to {}", path.display())).map_err(|error| error.to_string());
            if let Ok(mut limits) = export_limits().lock() {
                limits.entry(caller).or_default().finish();
            }
            this.answer(id, &request, result);
        });
    }

    /// The desktop app's `preview_start`: a `url` opens in a new Preview tab; a `name` from
    /// `.claude/launch.json` starts that server (unless its port answers) and shows it.
    fn preview_start_by_tool(&self, id: LiveSessionId, name: Option<&str>, url: Option<&str>) -> Result<String, String> {
        if let Some(url) = url.filter(|url| !url.trim().is_empty()) {
            let address = crate::preview_tabs::normalize_url(url).ok_or_else(|| format!("{url} is not a web address."))?;
            self.open_preview_tab(id, &address);
            return Ok(format!("The Browser pane shows {address}."));
        }
        let name = name.filter(|name| !name.trim().is_empty()).ok_or_else(|| "Pass url, or name (a server from .claude/launch.json).".to_string())?;
        let servers: Vec<_> = self.detect_dev_servers(id).into_iter().filter(|server| server.saved).collect();
        let server = servers.iter().find(|server| server.name.eq_ignore_ascii_case(name)).cloned().ok_or_else(|| {
            if servers.is_empty() {
                format!("No server called {name}: .claude/launch.json has none. Create it first with this format: {{\"version\": \"0.0.1\", \"configurations\": [{{\"name\": \"<unique-name>\", \"runtimeExecutable\": \"<command>\", \"runtimeArgs\": [\"<args>\"], \"port\": <port>}}]}}")
            } else {
                format!("No server called {name}. Available servers: {}.", servers.iter().map(|server| server.name.as_str()).collect::<Vec<_>>().join(", "))
            }
        })?;
        let tab = self.open_empty_preview_tab(id).ok_or_else(|| "This session is not open on screen.".to_string())?;
        self.use_dev_server(id, tab, server.clone())?;
        Ok(format!("{} ({}) shows in the Browser pane at {} once it answers.", server.name, server.command_line(), server.url()))
    }

    /// `tabs_context_mcp`: this session's Preview tabs, each with its id, address and title.
    fn preview_tabs_json(&self, id: LiveSessionId) -> String {
        let tabs = self.previews(id);
        let active = tabs.active();
        let list: Vec<Value> = tabs
            .ids()
            .into_iter()
            .map(|tab| json!({ "tabId": tab.0, "url": tabs.current(tab).unwrap_or_else(|| "about:blank".to_string()), "title": tabs.title(tab).unwrap_or_default(), "active": Some(tab) == active }))
            .collect();
        json!({ "tabs": list }).to_string()
    }

    // The tab a browser tool names, or the session's active tab; a tab of another session is refused.
    fn tool_tab(&self, id: LiveSessionId, wanted: Option<u64>) -> Result<crate::preview_tabs::PreviewTabId, String> {
        let tabs = self.previews(id);
        match wanted {
            Some(number) => {
                let tab = crate::preview_tabs::PreviewTabId(number);
                if tabs.contains(tab) {
                    Ok(tab)
                } else {
                    Err(format!("No tab {number} in this session; tabs_context_mcp lists them."))
                }
            }
            None => tabs.active().ok_or_else(|| "This session has no Browser tab; call tabs_context_mcp with createIfEmpty: true.".to_string()),
        }
    }

    /// `navigate`: an address in the tab (the session's active one, or a new one when it has none);
    /// "back" and "forward" move in its history.
    fn navigate_by_tool(&self, id: LiveSessionId, wanted: Option<u64>, url: &str) -> Result<String, String> {
        let tab = match (wanted, self.previews(id).active()) {
            (None, None) => self.open_empty_preview_tab(id).ok_or_else(|| "This session is not open on screen.".to_string())?,
            _ => self.tool_tab(id, wanted)?,
        };
        match url.trim() {
            "back" => {
                self.preview_back(tab);
                Ok(format!("Tab {} went back.", tab.0))
            }
            "forward" => {
                self.preview_forward(tab);
                Ok(format!("Tab {} went forward.", tab.0))
            }
            "" => Err("Pass url (or \"back\" / \"forward\").".to_string()),
            address => {
                self.preview_go(id, tab, address)?;
                Ok(format!("Tab {} shows {}. {}", tab.0, self.previews(id).current(tab).unwrap_or_else(|| address.to_string()), self.preview_tabs_json(id)))
            }
        }
    }

    /// Runs a script in the tab's page and answers with what it gave back.
    fn eval_by_tool(&self, id: LiveSessionId, request: AppRequest, wanted: Option<u64>, script: String) {
        self.eval_then(id, request, wanted, script, |text| text);
    }

    /// Runs a script in the tab's page and answers with `shape` of what it gave back.
    fn eval_then(&self, id: LiveSessionId, request: AppRequest, wanted: Option<u64>, script: String, shape: impl FnOnce(String) -> String + 'static) {
        let tab = match self.tool_tab(id, wanted) {
            Ok(tab) => tab,
            Err(message) => return self.answer(id, &request, Err(message)),
        };
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.webviews().evaluate(forge_ports::WebViewTabId(tab.0), &script, sender);
        let this = *self;
        dioxus::core::spawn_forever(async move {
            let result = match tokio::time::timeout(std::time::Duration::from_secs(20), receiver).await {
                Ok(Ok(result)) => result.map(|json| shape(page_result(&json))),
                _ => Err("The page did not answer in time.".to_string()),
            };
            this.answer(id, &request, result);
        });
    }

    /// `computer` `screenshot`: a picture of the tab's page, sent to Claude as an image.
    /// `gif_creator`: start_recording, stop_recording, export (a GIF in Downloads) and clear.
    fn gif_by_tool(&self, id: LiveSessionId, request: AppRequest, wanted: Option<u64>, action: &str, filename: String) {
        use crate::gif_recording::{recordings, Recording};
        let key = format!("{id:?}");
        let Ok(mut all) = recordings().lock() else {
            return self.answer(id, &request, Err("Recording is unavailable just now.".to_string()));
        };
        let result = match action {
            "start_recording" => match self.tool_tab(id, wanted) {
                Ok(tab) => {
                    let generation = all.get(&key).map_or(0, |old| old.generation) + 1;
                    all.insert(key.clone(), Recording { tab: tab.0, running: true, generation, frames: Vec::new() });
                    drop(all);
                    self.record_frames(id, key, generation);
                    return self.answer(id, &request, Ok(format!("Recording tab {}. Take your actions, then stop_recording and export.", tab.0)));
                }
                Err(message) => Err(message),
            },
            "stop_recording" => match all.get_mut(&key) {
                Some(recording) => {
                    // One last picture, so a change made just before stopping is in the GIF.
                    recording.running = false;
                    let (tab, generation) = (recording.tab, recording.generation);
                    drop(all);
                    let (sender, receiver) = tokio::sync::oneshot::channel();
                    self.webviews().snapshot(forge_ports::WebViewTabId(tab), sender);
                    let this = *self;
                    dioxus::core::spawn_forever(async move {
                        let last = tokio::time::timeout(std::time::Duration::from_secs(5), receiver).await;
                        let count = recordings().lock().ok().and_then(|mut all| {
                            let recording = all.get_mut(&key).filter(|recording| recording.generation == generation)?;
                            if let Ok(Ok(Ok(png))) = last {
                                if !png_width(&png).is_some_and(|width| width < 16) {
                                    crate::gif_recording::push_frame(recording, png);
                                }
                            }
                            Some(recording.frames.len())
                        });
                        let result = count.map(|count| format!("Stopped with {count} frames; export writes them as a GIF.")).ok_or_else(|| "The recording was cleared.".to_string());
                        this.answer(id, &request, result);
                    });
                    return;
                }
                None => Err("Nothing is being recorded; start_recording first.".to_string()),
            },
            "clear" => {
                all.remove(&key);
                Ok("Cleared the recording.".to_string())
            }
            "export" => match all.get_mut(&key) {
                Some(recording) if !recording.frames.is_empty() => {
                    recording.running = false;
                    let frames = recording.frames.clone();
                    drop(all);
                    let this = *self;
                    dioxus::core::spawn_forever(async move {
                        let result = crate::run_blocking(move || {
                            let gif = crate::gif_recording::encode_gif(&frames)?;
                            let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
                            let folder = crate::gif_recording::downloads_folder(home, std::env::temp_dir());
                            let path = crate::gif_recording::free_gif_path(&folder, &filename);
                            std::fs::write(&path, gif).map_err(|error| format!("The GIF could not be written: {error}"))?;
                            Ok(format!("Saved a GIF of {} frames to {}", frames.len(), path.display()))
                        })
                        .await
                        .unwrap_or_else(|| Err("The GIF could not be made.".to_string()));
                        this.answer(id, &request, result);
                    });
                    return;
                }
                _ => Err("Nothing was recorded; start_recording, take actions, then export.".to_string()),
            },
            other => Err(format!("gif_creator does not do \"{other}\"; use start_recording, stop_recording, export or clear.")),
        };
        self.answer(id, &request, result);
    }

    /// Pictures the recorded tab about once a second until the recording stops or another starts.
    fn record_frames(&self, id: LiveSessionId, key: String, generation: u64) {
        use crate::gif_recording::recordings;
        let this = *self;
        dioxus::core::spawn_forever(async move {
            loop {
                let tab = match recordings().lock().ok().and_then(|all| all.get(&key).filter(|recording| recording.running && recording.generation == generation).map(|recording| recording.tab)) {
                    Some(tab) => tab,
                    None => return,
                };
                let (sender, receiver) = tokio::sync::oneshot::channel();
                this.webviews().snapshot(forge_ports::WebViewTabId(tab), sender);
                if let Ok(Ok(Ok(png))) = tokio::time::timeout(std::time::Duration::from_secs(5), receiver).await {
                    // A covered pane gives a tiny picture: not a frame.
                    if !png_width(&png).is_some_and(|width| width < 16) {
                        if let Ok(mut all) = recordings().lock() {
                            if let Some(recording) = all.get_mut(&key).filter(|recording| recording.generation == generation) {
                                crate::gif_recording::push_frame(recording, png);
                                if recording.frames.len() >= crate::gif_recording::MAX_FRAMES {
                                    recording.running = false;
                                }
                            }
                        }
                    }
                }
                if this.live_view(id).is_none() {
                    if let Ok(mut all) = recordings().lock() {
                        all.remove(&key);
                    }
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
    }

    fn screenshot_by_tool(&self, id: LiveSessionId, request: AppRequest, wanted: Option<u64>) {
        let tab = match self.tool_tab(id, wanted) {
            Ok(tab) => tab,
            Err(message) => return self.answer(id, &request, Err(message)),
        };
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.webviews().snapshot(forge_ports::WebViewTabId(tab.0), sender);
        let this = *self;
        dioxus::core::spawn_forever(async move {
            use base64::Engine;
            let result = match tokio::time::timeout(std::time::Duration::from_secs(20), receiver).await {
                // A covered or hidden pane gives a picture a few pixels wide.
                Ok(Ok(Ok(png))) if png_width(&png).is_some_and(|width| width < 16) => Err("The Browser pane is covered or hidden, so its page cannot be pictured now. Ask the user to bring it into view, or use get_page_text or read_page.".to_string()),
                Ok(Ok(Ok(png))) => Ok(format!("forge-image:png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))),
                Ok(Ok(Err(message))) => Err(message),
                _ => Err("The page did not answer in time.".to_string()),
            };
            this.answer(id, &request, result);
        });
    }

    fn preview_list_json(&self, id: LiveSessionId) -> String {
        let servers: Vec<Value> = self
            .detect_dev_servers(id)
            .into_iter()
            .filter(|server| server.saved)
            .map(|server| json!({ "name": server.name, "command": server.command_line(), "port": server.port, "url": server.url(), "running": crate::workspace::port_is_answering(server.port) }))
            .collect();
        let background = self.background();
        let in_background = self.background_servers_of(id).into_iter().map(|run| {
            let status = match background.exit_code(run.id) {
                Some(None) => "running".to_string(),
                Some(Some(code)) => format!("exited ({code})"),
                None => "ended".to_string(),
            };
            json!({ "serverId": run.id + crate::workspace::BACKGROUND_ID_BASE, "name": run.name, "command": run.command, "status": status })
        });
        let running: Vec<Value> = in_background.chain(self
            .code_runs(id)
            .servers()
            .into_iter()
            .map(|run| {
                let status = match run.state {
                    crate::code_runs::RunState::Waiting => "starting".to_string(),
                    crate::code_runs::RunState::Running => "running".to_string(),
                    crate::code_runs::RunState::Done { code } => format!("exited ({code})"),
                    crate::code_runs::RunState::Ended => "ended".to_string(),
                };
                json!({ "serverId": run.id, "name": run.name, "command": run.command, "status": status })
            }))
            .collect();
        json!({ "servers": running, "launch_json": servers }).to_string()
    }

    fn sections_json(&self) -> String {
        let prefs = self.prefs();
        let mut out = vec![json!({ "id": "pinned", "kind": "builtin", "name": "", "order": 0, "session_count": prefs.pinned.len() })];
        for (index, section) in prefs.sections.iter().enumerate() {
            out.push(json!({ "id": section.id, "kind": "manual", "name": section.name, "order": index + 1, "session_count": section.sessions.len() }));
        }
        out.push(json!({ "id": "sessions", "kind": "builtin", "name": "", "order": prefs.sections.len() + 1 }));
        Value::Array(out).to_string()
    }

    fn create_section_by_tool(&self, name: &str) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 200 {
            return Err("A section's name is 1 to 200 characters.".to_string());
        }
        let id = format!("section-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_millis()).unwrap_or(0));
        let name = name.to_string();
        let made = id.clone();
        self.update_prefs(|prefs| prefs.sections.push(forge_domain::SidebarSection { id: made, name, sessions: Vec::new() }));
        Ok(json!({ "section_id": id }).to_string())
    }

    fn move_sessions_by_tool(&self, id: LiveSessionId, input: &Value) -> Result<String, String> {
        let target = input.get("section_id").and_then(Value::as_str).unwrap_or("sessions").to_string();
        let target = (target != "sessions").then_some(target);
        if let Some(section) = &target {
            if !self.prefs().sections.iter().any(|known| known.id == *section) {
                return Err(format!("No section {section}; list_sections names them."));
            }
        }
        let wanted: Vec<String> = input.get("session_ids").and_then(Value::as_array).map(|ids| ids.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
        if wanted.is_empty() || wanted.len() > 100 {
            return Err("Name 1 to 100 sessions.".to_string());
        }
        let mut moved = Vec::new();
        for each in wanted {
            let row = self.find_row(id, &each).ok_or_else(|| format!("No session {each}."))?;
            let session = row.session_id.clone().ok_or_else(|| format!("{} has no transcript yet.", row.title))?;
            self.move_to_section(session, target.clone());
            moved.push(row.title);
        }
        Ok(format!("Moved {}.", moved.join(", ")))
    }
}

/// Changes one setting for `set_setting`; refuses a value that is not allowed or already set.
fn set_setting(settings: crate::Settings, config: &AppConfig, available: &[String], key: &str, value: Option<&Value>) -> Result<String, String> {
    let value = value.ok_or_else(|| "Give the new value.".to_string())?;
    let switch = || value.as_bool().ok_or_else(|| format!("{key} takes true or false."));
    let same = || Err(format!("{key} already has that value."));
    match key {
        "prompt_suggestions" => {
            let on = switch()?;
            if on == config.prompt_suggestions {
                return same();
            }
            settings.set_prompt_suggestions(on);
        }
        "auto_archive_inactive_days" => {
            let days = value.as_u64().ok_or_else(|| "auto_archive_inactive_days takes a whole number of days (0: never).".to_string())?;
            let days = (days > 0).then_some(days as u32);
            if days == config.auto_archive_days {
                return same();
            }
            settings.set_auto_archive_days(days);
        }
        "task_complete_notifications" => {
            let on = switch()?;
            if on == config.notifications.reply_done {
                return same();
            }
            settings.set_notification(crate::NotificationKind::ReplyDone, on);
        }
        "permission_request_notifications" => {
            let on = switch()?;
            if on == config.notifications.permission_request {
                return same();
            }
            settings.set_notification(crate::NotificationKind::PermissionRequest, on);
        }
        "draw_attention_on_notifications" => {
            let on = switch()?;
            if on == config.attention_bounce {
                return same();
            }
            settings.set_attention_bounce(on);
        }
        "output_style" => {
            let name = value.as_str().ok_or_else(|| "output_style takes a style name, or \"default\".".to_string())?;
            let style = if name.eq_ignore_ascii_case("default") { None } else { Some(forge_domain::OutputStyle::find(name, available).ok_or_else(|| format!("No output style called {name}."))?) };
            if style == config.output_style {
                return same();
            }
            settings.set_output_style(style);
        }
        other => return Err(format!("{other} cannot be changed here; the user changes it in Settings.")),
    }
    Ok(format!("{key} is changed; it applies from the next session where it is a session setting."))
}

/// The settings `get_settings` reports, each with its value and one line on what it does.
fn settings_json(config: &AppConfig) -> String {
    let row = |value: Value, about: &str| json!({ "value": value, "about": about });
    json!({
        "defaultTranscriptView": row(json!(view_name(config.default_transcript_view)), "The view a session opens in."),
        "autoArchiveDays": row(json!(config.auto_archive_days), "Archives local sessions after this many idle days (null: never)."),
        "worktreeLocation": row(json!(format!("{:?}", config.worktree_location)), "Where isolated sessions keep their Git worktrees."),
        "promptSuggestions": row(json!(config.prompt_suggestions), "Shows a suggested next prompt after Claude answers."),
        "outputStyle": row(json!(config.output_style.as_ref().map(|style| style.name().to_string()).unwrap_or_else(|| "default".to_string())), "The output style of new sessions."),
        "dynamicWorkflows": row(json!(config.dynamic_workflows), "Lets Claude run several agents in parallel."),
        "taskListAndNextStep": row(json!(config.turn_tools), "Claude shows its plan as a list and ends a turn with a summary."),
        "unrelatedRequests": row(json!(config.unrelated_task.value()), "What happens when the user asks for unrelated work: ask, here or new_session."),
        "allowBypassMode": row(json!(config.allow_bypass_mode), "Whether Bypass permissions is in the mode menu."),
        "readOnlyHere": "These are changed by the user in Settings; this tool only reads them.",
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn upload_files_reads_absolute_files_within_the_limits() {
        let dir = std::env::temp_dir().join(format!("forge-upload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let picture = dir.join("a.PNG");
        std::fs::write(&picture, b"png").unwrap();
        let files = upload_files(&[picture.clone()]).unwrap();
        assert_eq!(files[0]["name"], "a.PNG");
        assert_eq!(files[0]["type"], "image/png");
        assert_eq!(files[0]["data"], "cG5n");
        assert!(upload_files(&[std::path::PathBuf::from("a.png")]).unwrap_err().contains("not an absolute path"));
        assert!(upload_files(&[dir.clone()]).unwrap_err().contains("not a file"));
        let big = dir.join("big.bin");
        std::fs::File::create(&big).unwrap().set_len(UPLOAD_FILE_LIMIT + 1).unwrap();
        assert!(upload_files(&[big]).unwrap_err().contains("8 MB"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    use super::*;
    use forge_domain::{Block, Message, MessageRole, ToolCall, ToolResult};

    #[test]
    fn events_read_as_who_said_what_with_a_line_per_tool() {
        let mut answer = Message::new(MessageRole::Assistant);
        answer.blocks = vec![
            Block::Tool(ToolCall { id: "t".into(), name: "Bash".into(), input: json!({}), result: Some(ToolResult { output: "x".into(), is_error: true }) }),
            Block::Text("Done.".into()),
        ];
        let text = render_events(&[Message::user("fix it"), answer], 40);
        assert_eq!(text, "User: fix it\n  [tool Bash, failed]\nClaude: Done.\n");
        assert_eq!(render_events(&[], 40), "The session has no messages yet.");
    }
}

/// `get_page_text`: the page's text, its article or main part when it has one, at most 50,000 characters.
const PAGE_TEXT_SCRIPT: &str = "(function(){var root=document.querySelector('article')||document.querySelector('main')||document.body;var text=(root&&root.innerText)||'';return 'Title: '+document.title+'\\nURL: '+location.href+'\\n\\n'+text.slice(0,50000);})()";

/// `javascript_tool`: the code runs as the body of a script; its last value comes back as JSON
/// (at most 20,000 characters), a thrown error as its message.
fn script_runner(code: &str) -> String {
    let quoted = serde_json::to_string(code).unwrap_or_else(|_| "\"\"".to_string());
    format!("(function(){{try{{var value=(0,eval)({quoted});if(value===undefined)return 'undefined';if(typeof value==='string')return value.slice(0,20000);var json=JSON.stringify(value);return (json===undefined?String(value):json).slice(0,20000);}}catch(error){{return 'Error: '+(error&&error.message||String(error));}}}})()")
}

/// `read_page`: an outline of the page, one element per line with its role, name and a `ref_N`
/// that stays the element's for the page's life (form_input and ref_id take it).
const READ_PAGE_SCRIPT: &str = include_str!("page_scripts/read_page.js");
/// `computer`'s mouse and keyboard actions, done with page events: a coordinate is in screenshot
/// pixels (divided by the page's pixel ratio), typing goes to the focused field.
const COMPUTER_SCRIPT: &str = include_str!("page_scripts/computer.js");
/// `read_network_requests`: the page's document, its loads from the resource timing list, and the
/// fetch and XHR calls the start script kept (with method and status), oldest first.
const NETWORK_READ_SCRIPT: &str = r#"(function(){var A=__ARGS__;var kept=window.__forgeNetwork||[];var rows=[];var nav=performance.getEntriesByType('navigation')[0];rows.push({t:0,line:'document GET '+location.href+(nav&&nav.responseStatus?' '+nav.responseStatus:'')});var seen={};kept.forEach(function(e){seen[e.url]=1;});performance.getEntriesByType('resource').forEach(function(r){if(r.initiatorType==='fetch'||r.initiatorType==='xmlhttprequest')return;rows.push({t:r.startTime,line:r.initiatorType+' GET '+r.name+(r.responseStatus?' '+r.responseStatus:'')+' '+Math.round(r.duration)+'ms'});});var origin=performance.timeOrigin;kept.forEach(function(e){rows.push({t:e.time-origin,line:e.type+' '+e.method+' '+e.url+' '+(e.status===null?'pending':e.status)+(e.ms!=null?' '+e.ms+'ms':'')});});rows.sort(function(a,b){return a.t-b.t;});var out=rows.map(function(r){return r.line;}).filter(function(l){return !A.pattern||l.indexOf(A.pattern)>=0;});out=out.slice(-A.limit);if(A.clear){kept.length=0;if(performance.clearResourceTimings)performance.clearResourceTimings();}return out.length?out.join('\n'):'No requests'+(A.pattern?' match '+A.pattern:'')+'.';})()"#;

/// `file_upload`: puts files into the file input a `ref_N` names and fires its input and change events.
const FILE_UPLOAD_SCRIPT: &str = include_str!("page_scripts/file_upload.js");
const UPLOAD_FILE_LIMIT: u64 = 8 * 1024 * 1024;
const UPLOAD_TOTAL_LIMIT: u64 = 20 * 1024 * 1024;

/// The files for `file_upload`, each as `{name, type, data}` (base64): absolute paths of files, at
/// most 8 MB each and 20 MB in all (they go into the page as text).
fn upload_files(paths: &[std::path::PathBuf]) -> Result<Vec<Value>, String> {
    use base64::Engine;
    let mut total = 0;
    let mut files = Vec::new();
    for path in paths {
        if !path.is_absolute() {
            return Err(format!("{} is not an absolute path.", path.display()));
        }
        let meta = std::fs::metadata(path).map_err(|_| format!("{} cannot be read.", path.display()))?;
        if !meta.is_file() {
            return Err(format!("{} is not a file.", path.display()));
        }
        if meta.len() > UPLOAD_FILE_LIMIT {
            return Err(format!("{} is larger than 8 MB.", path.display()));
        }
        total += meta.len();
        if total > UPLOAD_TOTAL_LIMIT {
            return Err("The files are larger than 20 MB together.".to_string());
        }
        let bytes = std::fs::read(path).map_err(|_| format!("{} cannot be read.", path.display()))?;
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        files.push(json!({ "name": name, "type": upload_type(path), "data": base64::engine::general_purpose::STANDARD.encode(bytes) }));
    }
    Ok(files)
}

/// The media type a browser gives a picked file of this kind; empty when it would not know.
fn upload_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        Some("csv") => "text/csv",
        Some("json") => "application/json",
        Some("html" | "htm") => "text/html",
        Some("zip") => "application/zip",
        _ => "",
    }
}

/// `form_input`: sets the field a `ref_N` names and fires its input and change events.
const FORM_INPUT_SCRIPT: &str = include_str!("page_scripts/form_input.js");

/// `read_console_messages`: the kept messages that match, oldest first, one per line.
fn console_script(pattern: &str, only_errors: bool, clear: bool, limit: u64) -> String {
    let quoted = serde_json::to_string(pattern).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "(function(){{var kept=window.__forgeConsole||[];var re=null;try{{re={quoted}?new RegExp({quoted},'i'):null;}}catch(e){{return 'Error: the pattern is not a valid regular expression';}}var out=kept.filter(function(m){{return (!{only_errors}||m.level==='error'||m.level==='exception')&&(!re||re.test(m.text));}});out=out.slice(-{limit});if({clear})kept.length=0;if(!out.length)return 'No console messages'+(re?' match the pattern.':'.');return out.map(function(m){{return '['+m.level+'] '+m.text;}}).join('\\n');}})()"
    )
}

/// The width a PNG's header gives.
fn png_width(png: &[u8]) -> Option<u32> {
    (png.len() >= 24 && png.starts_with(b"\x89PNG")).then(|| u32::from_be_bytes([png[16], png[17], png[18], png[19]]))
}

/// What a page script returned: wry hands it over as JSON, so a string comes back quoted.
fn page_result(json: &str) -> String {
    match serde_json::from_str::<Value>(json) {
        Ok(Value::String(text)) => text,
        Ok(Value::Null) | Err(_) if json.trim().is_empty() => "(nothing)".to_string(),
        Ok(other) => other.to_string(),
        Err(_) => json.to_string(),
    }
}

/// `find`: the outline lines of `read_page` that share the most words with the query (at most 20),
/// best first. The desktop app asks a model; this matches words.
fn find_in_outline(outline: &str, query: &str) -> String {
    let words: Vec<String> = query.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|word| word.len() > 1).map(str::to_string).collect();
    let mut hits: Vec<(usize, usize, &str)> = outline
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("[ref_"))
        .filter_map(|(at, line)| {
            let lower = line.to_lowercase();
            let score = words.iter().filter(|word| lower.contains(word.as_str())).count();
            (score > 0).then_some((score, at, line.trim()))
        })
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    if hits.is_empty() {
        return format!("Nothing on the page matches \"{query}\". Try read_page.");
    }
    hits.into_iter().take(20).map(|(_, _, line)| line).collect::<Vec<_>>().join("\n")
}

/// `preview_logs`: the last 8 KB of a server's output, only the lines with an error when `level` is
/// "error", only those holding `search` when it is given.
fn server_log(output: &str, level: Option<&str>, search: Option<&str>) -> String {
    let errors_only = level.is_some_and(|level| level.eq_ignore_ascii_case("error"));
    let search = search.map(str::to_lowercase).filter(|search| !search.is_empty());
    let kept: Vec<&str> = output
        .lines()
        .filter(|line| !errors_only || { let lower = line.to_lowercase(); lower.contains("error") || lower.contains("failed") || lower.contains("exception") })
        .filter(|line| search.as_ref().is_none_or(|search| line.to_lowercase().contains(search.as_str())))
        .collect();
    let text = kept.join("\n");
    let limit = 8 * 1024;
    if text.len() <= limit {
        return if text.is_empty() { "(no output)".to_string() } else { text };
    }
    let mut from = text.len() - limit;
    while !text.is_char_boundary(from) {
        from += 1;
    }
    text[from..].to_string()
}

/// The desktop app's limits on `export_transcript`, per calling session: one export at a time and
/// at most six in an hour (its answer points the person at the files already in Downloads).
#[derive(Debug, Default)]
struct ExportLimit {
    started: Vec<std::time::Instant>,
    in_flight: bool,
}

impl ExportLimit {
    fn begin(&mut self, now: std::time::Instant) -> Result<(), String> {
        let hour = std::time::Duration::from_secs(3600);
        self.started.retain(|at| now.duration_since(*at) < hour);
        if self.in_flight {
            return Err("An export from this session is still being written; wait for it to finish before exporting again.".to_string());
        }
        if self.started.len() >= 6 {
            return Err("This session has already exported 6 transcripts in the past hour. Point the user at the files already in their Downloads folder, or at the session menu's Export, instead of exporting again.".to_string());
        }
        self.started.push(now);
        self.in_flight = true;
        Ok(())
    }

    fn finish(&mut self) {
        self.in_flight = false;
    }
}

fn export_limits() -> &'static std::sync::Mutex<std::collections::HashMap<String, ExportLimit>> {
    static LIMITS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, ExportLimit>>> = std::sync::OnceLock::new();
    LIMITS.get_or_init(Default::default)
}

#[cfg(test)]
mod export_limit_tests {
    use super::{server_log, ExportLimit};

    #[test]
    fn page_results_unquote_strings_and_scripts_are_quoted_safely() {
        assert_eq!(super::page_result("\"hello\""), "hello");
        assert_eq!(super::page_result("42"), "42");
        let script = super::script_runner("document.title + \"'\"");
        assert!(script.contains(r#"(0,eval)("document.title + \"'\"")"#), "{script}");
    }

    #[test]
    fn find_ranks_outline_lines_by_shared_words() {
        let outline = "Title: x\n\nbutton \"Log in\" [ref_1]\nlink \"Login help\" [ref_2]\ntextbox \"Search\" [ref_3]";
        assert_eq!(super::find_in_outline(outline, "log in button"), "button \"Log in\" [ref_1]\nlink \"Login help\" [ref_2]");
        assert!(super::find_in_outline(outline, "basket").starts_with("Nothing"));
    }

    #[test]
    fn a_png_header_gives_its_width() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&[0, 0, 2, 0, 0, 0, 1, 0]);
        assert_eq!(super::png_width(&png), Some(512));
        assert_eq!(super::png_width(b"nope"), None);
    }

    #[test]
    fn server_logs_keep_the_tail_and_filter_errors_or_a_search() {
        let output = "ready on 5173\nGET / 200\nError: boom\nGET /x 404";
        assert_eq!(server_log(output, None, None), output);
        assert_eq!(server_log(output, Some("error"), None), "Error: boom");
        assert_eq!(server_log(output, None, Some("get")), "GET / 200\nGET /x 404");
        assert_eq!(server_log("", None, None), "(no output)");
        assert_eq!(server_log(&"x".repeat(9000), None, None).len(), 8192);
    }
    use std::time::{Duration, Instant};

    #[test]
    fn one_export_at_a_time_and_six_an_hour() {
        let start = Instant::now();
        let mut limit = ExportLimit::default();
        assert!(limit.begin(start).is_ok());
        assert!(limit.begin(start).unwrap_err().contains("still being written"));
        limit.finish();
        for n in 1..6 {
            assert!(limit.begin(start + Duration::from_secs(n)).is_ok());
            limit.finish();
        }
        assert!(limit.begin(start + Duration::from_secs(10)).unwrap_err().contains("6 transcripts"));
        assert!(limit.begin(start + Duration::from_secs(3601)).is_ok(), "an hour later the first export no longer counts");
    }
}
