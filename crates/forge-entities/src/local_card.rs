use forge_domain::{local_moment, ContextUsage, McpServerState, McpStatus, PriceTable, RateLevel, RateLimit, RateWindow, SessionUsage};
use forge_shared::tr;

use crate::rate_limit_view::reset_text;
use crate::usage_view::{ring_level, RingLevel, UsageView};

/// A command Forge answers on the spot: it shows a panel or a card and is never sent to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalCommand {
    /// `/tasks`: the background tasks panel.
    Tasks,
    /// `/diff`: the changes panel, shown or hidden.
    Diff,
    /// `/status`: a card about this session.
    Status,
    /// `/usage`: a card with what this session used and the account's limit.
    Usage,
}

/// The local command the message is, if it is just that command.
pub fn local_command(input: &str) -> Option<LocalCommand> {
    match input.trim() {
        "/tasks" => Some(LocalCommand::Tasks),
        "/diff" => Some(LocalCommand::Diff),
        "/status" => Some(LocalCommand::Status),
        "/usage" => Some(LocalCommand::Usage),
        _ => None,
    }
}

/// One part of a card in the chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardBlock {
    Heading(String),
    /// Label and value pairs; the value is drawn at the right.
    Rows(Vec<(String, String)>),
    /// A bar filled to `percent`, with the label above and `note` under it.
    Meter { label: String, percent: u8, level: RingLevel, note: String },
    Note(String),
}

/// What `/status` and `/usage` put in the chat. It belongs to the moment it was asked: the next message the
/// person sends takes it away, and it is not kept in the transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCard {
    pub title: String,
    pub blocks: Vec<CardBlock>,
}

/// What `/status` shows about a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusInfo {
    pub forge_version: String,
    pub model: String,
    pub effort: Option<String>,
    /// The words of the permission mode ("Auto").
    pub mode: String,
    pub messages: usize,
    pub session_id: Option<String>,
    pub folder: String,
    /// `None` until the sidecar has reported the servers.
    pub connectors: Option<Vec<McpServerState>>,
}

fn names(servers: &[&McpServerState]) -> String {
    servers.iter().map(|server| server.name.as_str()).collect::<Vec<_>>().join(", ")
}

/// "4 connected (figma, playwright) · 1 failed (m365)": only the groups that are not empty.
pub fn connectors_text(servers: &[McpServerState]) -> String {
    if servers.is_empty() {
        return tr!("None").to_string();
    }
    let of = |status: McpStatus| -> Vec<&McpServerState> { servers.iter().filter(|server| server.status == status).collect() };
    let mut parts: Vec<String> = Vec::new();
    let connected = of(McpStatus::Connected);
    if !connected.is_empty() {
        parts.push(tr!("{count} connected ({names})", count = connected.len(), names = names(&connected)));
    }
    let failed = of(McpStatus::Failed);
    if !failed.is_empty() {
        parts.push(tr!("{count} failed ({names})", count = failed.len(), names = names(&failed)));
    }
    let needs_sign_in = of(McpStatus::NeedsAuth);
    if !needs_sign_in.is_empty() {
        parts.push(tr!("{count} need sign-in ({names})", count = needs_sign_in.len(), names = names(&needs_sign_in)));
    }
    let rest = servers.len() - connected.len() - failed.len() - needs_sign_in.len();
    if rest > 0 {
        parts.push(tr!("{count} other", count = rest));
    }
    parts.join(" · ")
}

pub fn status_card(info: &StatusInfo) -> LocalCard {
    let mut session = vec![(tr!("Forge").to_string(), info.forge_version.clone()), (tr!("Model").to_string(), info.model.clone())];
    if let Some(effort) = &info.effort {
        session.push((tr!("Effort").to_string(), effort.clone()));
    }
    session.push((tr!("Permissions").to_string(), info.mode.clone()));
    session.push((tr!("Messages").to_string(), info.messages.to_string()));
    if let Some(id) = &info.session_id {
        // The first eight characters are enough to tell sessions apart.
        session.push((tr!("Session ID").to_string(), format!("{}…", id.chars().take(8).collect::<String>())));
    }
    let mut environment = vec![(tr!("Folder").to_string(), info.folder.clone())];
    environment.push((
        tr!("Connectors").to_string(),
        match &info.connectors {
            Some(servers) => connectors_text(servers),
            None => tr!("Not reported yet").to_string(),
        },
    ));
    LocalCard {
        title: tr!("Status").to_string(),
        blocks: vec![CardBlock::Heading(tr!("Session").to_string()), CardBlock::Rows(session), CardBlock::Heading(tr!("Environment").to_string()), CardBlock::Rows(environment)],
    }
}

/// The share of the input side that came from the cache, in percent; `None` when nothing was read or written.
pub fn cache_hit_percent(usage: &SessionUsage) -> Option<u8> {
    let total = usage.total();
    let input_side = total.input as u128 + total.cache_read as u128 + total.cache_write() as u128;
    if input_side == 0 {
        return None;
    }
    Some(((total.cache_read as u128 * 100 + input_side / 2) / input_side).min(100) as u8)
}

fn window_label(window: RateWindow) -> &'static str {
    match window {
        RateWindow::FiveHour => tr!("5-hour limit"),
        RateWindow::Weekly => tr!("Weekly limit"),
        RateWindow::Other => tr!("Usage limit"),
    }
}

pub fn usage_card(usage: &SessionUsage, context: Option<ContextUsage>, limit: Option<&RateLimit>, prices: &PriceTable) -> LocalCard {
    let view = UsageView::new(usage, context, prices);
    let mut blocks: Vec<CardBlock> = Vec::new();
    // The limit, when the SDK has told us a percent: only what it reported, nothing guessed.
    if let Some(limit) = limit {
        if let Some(percent) = limit.percent {
            let percent = percent.min(100) as u8;
            let level = match limit.level {
                RateLevel::Reached => RingLevel::Danger,
                RateLevel::Warning => RingLevel::Warn,
                RateLevel::Fine => ring_level(percent),
            };
            let note = match limit.resets_at.and_then(local_moment) {
                Some(moment) => reset_text(moment),
                None => tr!("{percent}% used", percent = percent),
            };
            blocks.push(CardBlock::Meter { label: window_label(limit.window).to_string(), percent, level, note });
        }
    }
    if let Some(context) = &view.context {
        blocks.push(CardBlock::Meter { label: tr!("Context window").to_string(), percent: context.percent, level: context.level, note: context.label.clone() });
    }
    if !view.has_usage {
        blocks.push(CardBlock::Note(tr!("Nothing has been used in this session yet.").to_string()));
        return LocalCard { title: tr!("Usage").to_string(), blocks };
    }
    let mut rows: Vec<(String, String)> = Vec::new();
    if let Some(cost) = &view.cost {
        let value = match &view.cost_note {
            Some(note) => format!("{cost} ({note})"),
            None => cost.clone(),
        };
        rows.push((tr!("Cost").to_string(), value));
    }
    rows.push((tr!("Input").to_string(), view.input.clone()));
    rows.push((tr!("Output").to_string(), view.output.clone()));
    rows.push((tr!("Cache read").to_string(), view.cache_read.clone()));
    rows.push((tr!("Cache write").to_string(), view.cache_write.clone()));
    if let Some(searches) = &view.web_searches {
        rows.push((tr!("Web searches").to_string(), searches.clone()));
    }
    if let Some(percent) = cache_hit_percent(usage) {
        rows.push((tr!("Cache hit").to_string(), format!("{percent}%")));
    }
    blocks.push(CardBlock::Heading(tr!("This session").to_string()));
    blocks.push(CardBlock::Rows(rows));
    if view.models.len() > 1 {
        blocks.push(CardBlock::Heading(tr!("By model").to_string()));
        blocks.push(CardBlock::Rows(view.models.iter().map(|line| (line.model.clone(), format!("{} · {}", line.tokens, line.cost))).collect()));
    }
    LocalCard { title: tr!("Usage").to_string(), blocks }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{Rates, TokenUsage};

    fn server(name: &str, status: McpStatus) -> McpServerState {
        McpServerState { name: name.to_string(), status, scope: None, source: None, remote: None, tool_count: 0, error: None }
    }

    fn prices() -> PriceTable {
        PriceTable::new(vec![("claude-test", Rates::per_mtok(3.0, 15.0, 0.30, 3.75, 6.0))])
    }

    fn used(input: u64, output: u64, cache_read: u64) -> SessionUsage {
        let mut usage = SessionUsage::default();
        usage.add("claude-test", TokenUsage { input, output, cache_read, ..TokenUsage::default() });
        usage
    }

    #[test]
    fn the_four_local_commands_are_the_whole_message_and_nothing_else() {
        assert_eq!(local_command("/tasks"), Some(LocalCommand::Tasks));
        assert_eq!(local_command("  /diff \n"), Some(LocalCommand::Diff));
        assert_eq!(local_command("/status"), Some(LocalCommand::Status));
        assert_eq!(local_command("/usage"), Some(LocalCommand::Usage));
        for text in ["", "/", "/task", "/tasks now", "tasks", "/diffs", "/btw /diff", "please /usage", "/usage all", "/stat"] {
            assert_eq!(local_command(text), None, "{text:?}");
        }
    }

    #[test]
    fn connectors_are_counted_by_state_and_named() {
        let servers = vec![server("figma", McpStatus::Connected), server("playwright", McpStatus::Connected), server("m365", McpStatus::Failed), server("drive", McpStatus::NeedsAuth)];
        assert_eq!(connectors_text(&servers), "2 connected (figma, playwright) · 1 failed (m365) · 1 need sign-in (drive)");
        assert_eq!(connectors_text(&[server("x", McpStatus::Pending)]), "1 other");
        assert_eq!(connectors_text(&[]), "None");
    }

    #[test]
    fn the_status_card_names_the_session_and_cuts_the_id() {
        let info = StatusInfo {
            forge_version: "0.1.0".to_string(),
            model: "Sonnet 5.5".to_string(),
            effort: Some("Medium".to_string()),
            mode: "Auto".to_string(),
            messages: 12,
            session_id: Some("0123456789abcdef".to_string()),
            folder: "/work/app".to_string(),
            connectors: None,
        };
        let card = status_card(&info);
        assert_eq!(card.title, "Status");
        let rows: Vec<&(String, String)> = card.blocks.iter().filter_map(|block| if let CardBlock::Rows(rows) = block { Some(rows) } else { None }).flatten().collect();
        let value = |label: &str| rows.iter().find(|(l, _)| l == label).map(|(_, v)| v.as_str());
        assert_eq!(value("Session ID"), Some("01234567…"));
        assert_eq!(value("Effort"), Some("Medium"));
        assert_eq!(value("Connectors"), Some("Not reported yet"));
        assert_eq!(value("Folder"), Some("/work/app"));
        // No effort and no id: those rows are left out, not shown empty.
        let bare = status_card(&StatusInfo { effort: None, session_id: None, ..info });
        let labels: Vec<&str> = bare.blocks.iter().filter_map(|block| if let CardBlock::Rows(rows) = block { Some(rows) } else { None }).flatten().map(|(l, _)| l.as_str()).collect();
        assert!(!labels.contains(&"Effort") && !labels.contains(&"Session ID"));
    }

    #[test]
    fn the_cache_hit_is_the_share_read_from_the_cache() {
        assert_eq!(cache_hit_percent(&SessionUsage::default()), None);
        assert_eq!(cache_hit_percent(&used(10, 5, 90)), Some(90));
        assert_eq!(cache_hit_percent(&used(100, 5, 0)), Some(0));
    }

    #[test]
    fn an_unused_session_says_so_and_has_no_table() {
        let card = usage_card(&SessionUsage::default(), None, None, &prices());
        assert_eq!(card.blocks, vec![CardBlock::Note("Nothing has been used in this session yet.".to_string())]);
    }

    #[test]
    fn the_usage_card_lists_cost_tokens_and_the_cache_hit() {
        let card = usage_card(&used(1_000, 2_000, 9_000), None, None, &prices());
        let rows = card.blocks.iter().find_map(|block| if let CardBlock::Rows(rows) = block { Some(rows.clone()) } else { None }).unwrap();
        let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["Cost", "Input", "Output", "Cache read", "Cache write", "Cache hit"]);
        assert_eq!(rows.last().unwrap().1, "90%");
        // One model: no per-model block.
        assert!(!card.blocks.iter().any(|block| matches!(block, CardBlock::Heading(text) if text == "By model")));
    }

    #[test]
    fn a_limit_with_a_percent_is_a_meter_and_one_without_is_left_out() {
        let with = RateLimit { level: RateLevel::Warning, window: RateWindow::FiveHour, percent: Some(83), resets_at: None };
        let card = usage_card(&used(1, 1, 0), None, Some(&with), &prices());
        assert!(matches!(&card.blocks[0], CardBlock::Meter { label, percent: 83, level: RingLevel::Warn, note } if label == "5-hour limit" && note == "83% used"));
        let without = RateLimit { level: RateLevel::Fine, window: RateWindow::Weekly, percent: None, resets_at: None };
        let card = usage_card(&used(1, 1, 0), None, Some(&without), &prices());
        assert!(!card.blocks.iter().any(|block| matches!(block, CardBlock::Meter { .. })));
    }
}
