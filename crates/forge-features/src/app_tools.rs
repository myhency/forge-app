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
