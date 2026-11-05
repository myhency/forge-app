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
    pub fn serve_app_requests(&self, id: LiveSessionId, config: &AppConfig) {
        let Some(view) = self.live_view(id) else { return };
        let requests: Vec<AppRequest> = view.peek().app_requests.clone();
        for request in requests {
            if first_time(&request.id) {
                self.serve(id, request, config);
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

    fn serve(&self, id: LiveSessionId, request: AppRequest, config: &AppConfig) {
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

/// The settings `get_settings` reports, each with its value and one line on what it does.
fn settings_json(config: &AppConfig) -> String {
    let row = |value: Value, about: &str| json!({ "value": value, "about": about });
    json!({
        "defaultTranscriptView": row(json!(view_name(config.default_transcript_view)), "The view a session opens in."),
        "autoArchiveDays": row(json!(config.auto_archive_days), "Archives local sessions after this many idle days (null: never)."),
        "worktreeLocation": row(json!(format!("{:?}", config.worktree_location)), "Where isolated sessions keep their Git worktrees."),
        "promptSuggestions": row(json!(config.prompt_suggestions), "Shows a suggested next prompt after Claude answers."),
        "outputStyle": row(json!(config.output_style.map(|style| style.name().to_string()).unwrap_or_else(|| "default".to_string())), "The output style of new sessions."),
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
