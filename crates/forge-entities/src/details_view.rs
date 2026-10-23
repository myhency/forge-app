use forge_domain::{ContextBreakdown, ContextItem, ContextKind, ContextRow, ContextUsage, McpServerState, McpStatus};

use crate::usage_view::format_tokens;

/// Like `format_tokens`, but `1M` and `200k` instead of `1.0M` and `200.0k`, as the desktop app shows them.
pub fn format_tokens_short(count: u64) -> String {
    let text = format_tokens(count);
    match text.find(".0") {
        Some(at) if text[at + 2..].chars().all(|c| c.is_ascii_alphabetic()) => format!("{}{}", &text[..at], &text[at + 2..]),
        _ => text,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarSegment {
    /// An index into the usage ring's palette.
    pub color: usize,
    pub per_mille: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextRowView {
    pub name: String,
    pub tokens: String,
    /// `None` renders as a dash.
    pub percent: Option<String>,
    /// `None` for the grey rows.
    pub color: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextItemView {
    pub label: String,
    pub tokens: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextCardView {
    pub percent: u8,
    /// `78.1k / 1M`
    pub used_label: String,
    pub bar: Vec<BarSegment>,
    pub rows: Vec<ContextRowView>,
    pub deferred: Vec<ContextRowView>,
    pub mcp_tools: Vec<ContextItemView>,
    pub agents: Vec<ContextItemView>,
    /// The used part of the window in per-mille, for the folded card's small bar.
    pub folded_bar: u16,
}

fn per_mille(tokens: u64, window: u64) -> u64 {
    if window == 0 {
        return 0;
    }
    (tokens as u128 * 1000 / window as u128).min(1000) as u64
}

/// `tokens` out of `window` in tenths of a percent, rounded to nearest and capped at 1000.
fn rounded_tenths(tokens: u64, window: u64) -> u64 {
    if window == 0 {
        return 0;
    }
    ((tokens as u128 * 1000 + window as u128 / 2) / window as u128).min(1000) as u64
}

fn item_view(item: &ContextItem) -> ContextItemView {
    let label = match &item.group {
        Some(group) => format!("{group} · {}", item.name),
        None => item.name.clone(),
    };
    ContextItemView { label, tokens: format_tokens_short(item.tokens) }
}

/// `None` when there is no detail or the detail has no rows (the card then shows its sentence).
pub fn context_card(usage: Option<ContextUsage>, detail: Option<&ContextBreakdown>) -> Option<ContextCardView> {
    let detail = detail?;
    if detail.rows.is_empty() {
        return None;
    }
    let sum = |kinds: &[ContextKind]| detail.rows.iter().filter(|row| kinds.contains(&row.kind)).map(|row| row.tokens).sum::<u64>();
    let used = sum(&[ContextKind::Used]);
    // The deferred rows are not in the window, so they are left out of the fallback size.
    let (used_total, window) = match usage {
        Some(usage) => (usage.total_tokens, usage.max_tokens),
        None => (used, sum(&[ContextKind::Used, ContextKind::Free, ContextKind::Buffer])),
    };
    let percent = match usage {
        Some(usage) => usage.percent(),
        None => (if window == 0 { 0 } else { (used as u128 * 100 + window as u128 / 2) / window as u128 }).min(100) as u8,
    };

    let ordered = |kind: fn(ContextKind) -> bool| detail.rows.iter().filter(move |row| kind(row.kind));
    let used_rows = ordered(|kind| kind == ContextKind::Used);
    let other_rows = ordered(|kind| matches!(kind, ContextKind::Free | ContextKind::Buffer));
    let deferred_rows = ordered(|kind| kind == ContextKind::Deferred);

    let mut bar = Vec::new();
    let mut left = 1000u64;
    let mut rows = Vec::new();
    for (index, row) in used_rows.enumerate() {
        let share = per_mille(row.tokens, window).min(left);
        left -= share;
        if share > 0 {
            bar.push(BarSegment { color: index, per_mille: share as u16 });
        }
        rows.push(row_view(row, Some(window), Some(index)));
    }
    rows.extend(other_rows.map(|row| row_view(row, Some(window), None)));
    let deferred = deferred_rows.map(|row| row_view(row, None, None)).collect();

    Some(ContextCardView {
        percent,
        used_label: format!("{} / {}", format_tokens_short(used_total), format_tokens_short(window)),
        bar,
        rows,
        deferred,
        mcp_tools: detail.mcp_tools.iter().map(item_view).collect(),
        agents: detail.agents.iter().map(item_view).collect(),
        folded_bar: per_mille(used_total, window) as u16,
    })
}

fn row_view(row: &ContextRow, window: Option<u64>, color: Option<usize>) -> ContextRowView {
    let percent = window.map(|window| {
        let tenths = rounded_tenths(row.tokens, window);
        format!("{}.{}%", tenths / 10, tenths % 10)
    });
    ContextRowView { name: row.name.clone(), tokens: format_tokens_short(row.tokens), percent, color }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorState {
    SignIn,
    Reconnect,
    Connecting,
    Off,
}

fn state_of(status: McpStatus) -> Option<ConnectorState> {
    match status {
        McpStatus::Connected => None,
        McpStatus::NeedsAuth => Some(ConnectorState::SignIn),
        McpStatus::Failed => Some(ConnectorState::Reconnect),
        McpStatus::Pending => Some(ConnectorState::Connecting),
        McpStatus::Disabled => Some(ConnectorState::Off),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorRowView {
    pub name: String,
    pub initial: char,
    /// `None` for a connected server.
    pub state: Option<ConnectorState>,
}

fn capitalized(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Servers that need action first, then the rest in the SDK's order, `claudeai` ones last.
pub fn connector_rows(servers: &[McpServerState]) -> Vec<ConnectorRowView> {
    let mut ordered: Vec<&McpServerState> = servers.iter().collect();
    ordered.sort_by_key(|server| {
        if matches!(server.status, McpStatus::NeedsAuth | McpStatus::Failed) {
            0
        } else if server.scope.as_deref() == Some("claudeai") {
            2
        } else {
            1
        }
    });
    ordered
        .into_iter()
        .map(|server| {
            let name = capitalized(&server.name);
            ConnectorRowView { initial: name.chars().next().unwrap_or('?'), name, state: state_of(server.status) }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportLabel {
    /// A remote server.
    Web,
    /// A local process.
    Desktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerTag {
    Plugin,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableStatus {
    Connected,
    State(ConnectorState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorTableRow {
    pub name: String,
    /// The plugin's name, only for a server a plugin brought.
    pub subtitle: Option<String>,
    pub kind: Option<TransportLabel>,
    pub tag: Option<ServerTag>,
    pub status: TableStatus,
}

/// In the SDK's order. `plugin_names` maps a server's name to its plugin's name.
pub fn connector_table(servers: &[McpServerState], plugin_names: &dyn Fn(&str) -> Option<String>) -> Vec<ConnectorTableRow> {
    servers
        .iter()
        .map(|server| {
            let from_plugin = server.source.as_deref() == Some("plugin");
            let tag = if from_plugin {
                Some(ServerTag::Plugin)
            } else if server.scope.as_deref() == Some("user") {
                Some(ServerTag::User)
            } else {
                None
            };
            ConnectorTableRow {
                name: server.name.clone(),
                subtitle: if from_plugin { plugin_names(&server.name) } else { None },
                kind: server.remote.map(|remote| if remote { TransportLabel::Web } else { TransportLabel::Desktop }),
                tag,
                status: state_of(server.status).map_or(TableStatus::Connected, TableStatus::State),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, tokens: u64, kind: ContextKind) -> ContextRow {
        ContextRow { name: name.into(), tokens, kind }
    }

    fn detail(rows: Vec<ContextRow>) -> ContextBreakdown {
        ContextBreakdown { rows, mcp_tools: vec![], agents: vec![] }
    }

    fn server(name: &str, status: McpStatus, scope: Option<&str>) -> McpServerState {
        McpServerState { name: name.into(), status, scope: scope.map(String::from), source: None, remote: None, tool_count: 0, error: None }
    }

    #[test]
    fn the_context_card_has_the_percent_and_the_used_and_max_labels() {
        let usage = ContextUsage { total_tokens: 78_100, max_tokens: 1_000_000 };
        let card = context_card(Some(usage), Some(&detail(vec![row("Messages", 20_100, ContextKind::Used)]))).unwrap();
        assert_eq!(card.percent, 8);
        assert_eq!(card.used_label, "78.1k / 1M");
        assert_eq!(card.rows[0].tokens, "20.1k");
        assert_eq!(card.rows[0].percent.as_deref(), Some("2.0%"));
        assert_eq!(card.folded_bar, 78);
    }

    #[test]
    fn used_rows_come_before_free_and_buffer_and_deferred_rows_show_a_dash() {
        let rows = vec![
            row("Free space", 888_900, ContextKind::Free),
            row("MCP tools (deferred)", 123_900, ContextKind::Deferred),
            row("Messages", 20_100, ContextKind::Used),
            row("Autocompact buffer", 33_000, ContextKind::Buffer),
            row("System prompt", 5_000, ContextKind::Used),
        ];
        let usage = ContextUsage { total_tokens: 78_100, max_tokens: 1_000_000 };
        let card = context_card(Some(usage), Some(&detail(rows))).unwrap();
        let names: Vec<_> = card.rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["Messages", "System prompt", "Free space", "Autocompact buffer"]);
        assert_eq!(card.rows[0].color, Some(0));
        assert_eq!(card.rows[1].color, Some(1));
        assert_eq!(card.rows[2].color, None);
        assert_eq!(card.rows[2].percent.as_deref(), Some("88.9%"));
        assert_eq!(card.rows[3].tokens, "33k");
        assert_eq!(card.deferred.len(), 1);
        assert_eq!(card.deferred[0].percent, None);
        assert_eq!(card.deferred[0].tokens, "123.9k");
    }

    #[test]
    fn a_row_percent_rounds_to_the_nearest_tenth() {
        let usage = ContextUsage { total_tokens: 2_000, max_tokens: 1_000_000 };
        let rows = vec![row("A", 1_999, ContextKind::Used), row("B", 1, ContextKind::Used), row("C", 1_000_000, ContextKind::Free)];
        let card = context_card(Some(usage), Some(&detail(rows))).unwrap();
        assert_eq!(card.rows[0].percent.as_deref(), Some("0.2%"));
        assert_eq!(card.rows[1].percent.as_deref(), Some("0.0%"));
        assert_eq!(card.rows[2].percent.as_deref(), Some("100.0%"));
    }

    #[test]
    fn the_free_row_is_not_part_of_the_bar() {
        let rows = vec![row("Messages", 100, ContextKind::Used), row("Free space", 900, ContextKind::Free)];
        let card = context_card(None, Some(&detail(rows))).unwrap();
        assert_eq!(card.bar, vec![BarSegment { color: 0, per_mille: 100 }]);
    }

    #[test]
    fn bar_segments_never_add_to_more_than_a_thousand() {
        let rows = vec![row("A", 700, ContextKind::Used), row("B", 700, ContextKind::Used), row("C", 700, ContextKind::Used)];
        let usage = ContextUsage { total_tokens: 2_100, max_tokens: 1_000 };
        let card = context_card(Some(usage), Some(&detail(rows))).unwrap();
        assert!(card.bar.iter().map(|segment| u32::from(segment.per_mille)).sum::<u32>() <= 1000);
        assert!(card.folded_bar <= 1000);
    }

    #[test]
    fn an_unmeasured_window_gives_zero_percent_without_dividing_by_zero() {
        let usage = ContextUsage { total_tokens: 10, max_tokens: 0 };
        let card = context_card(Some(usage), Some(&detail(vec![row("Messages", 10, ContextKind::Used)]))).unwrap();
        assert_eq!(card.percent, 0);
        assert_eq!(card.rows[0].percent.as_deref(), Some("0.0%"));
        assert!(card.bar.is_empty());
        assert_eq!(card.folded_bar, 0);
    }

    #[test]
    fn rows_over_the_window_cap_the_percent_at_100() {
        let rows = vec![row("Messages", 5_000, ContextKind::Used), row("Free space", 100, ContextKind::Free)];
        let card = context_card(None, Some(&detail(rows))).unwrap();
        assert_eq!(card.percent, 98);
        let rows = vec![row("Messages", 5_000, ContextKind::Used)];
        assert_eq!(context_card(None, Some(&detail(rows))).unwrap().percent, 100);
        assert_eq!(context_card(None, Some(&detail(vec![]))), None, "no rows is no value");
        assert_eq!(context_card(None, None), None);
    }

    #[test]
    fn empty_mcp_and_agent_lists_give_no_fold_rows() {
        let card = context_card(None, Some(&detail(vec![row("Messages", 1, ContextKind::Used)]))).unwrap();
        assert!(card.mcp_tools.is_empty() && card.agents.is_empty());
    }

    #[test]
    fn an_item_label_joins_its_group_and_name() {
        let mut breakdown = detail(vec![row("Messages", 1, ContextKind::Used)]);
        breakdown.mcp_tools = vec![ContextItem { group: Some("figma".into()), name: "whoami".into(), tokens: 1_500 }];
        breakdown.agents = vec![ContextItem { group: None, name: "reviewer".into(), tokens: 80 }];
        let card = context_card(None, Some(&breakdown)).unwrap();
        assert_eq!(card.mcp_tools[0], ContextItemView { label: "figma · whoami".into(), tokens: "1.5k".into() });
        assert_eq!(card.agents[0], ContextItemView { label: "reviewer".into(), tokens: "80".into() });
    }

    #[test]
    fn servers_that_need_action_come_first_and_claudeai_ones_last() {
        let servers = vec![
            server("gmail", McpStatus::Connected, Some("claudeai")),
            server("figma", McpStatus::Connected, Some("user")),
            server("notion", McpStatus::NeedsAuth, Some("claudeai")),
            server("db", McpStatus::Failed, None),
            server("docs", McpStatus::Pending, None),
        ];
        let rows = connector_rows(&servers);
        let names: Vec<_> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["Notion", "Db", "Figma", "Docs", "Gmail"]);
        assert_eq!(rows[0].state, Some(ConnectorState::SignIn));
        assert_eq!(rows[1].state, Some(ConnectorState::Reconnect));
        assert_eq!(rows[3].state, Some(ConnectorState::Connecting));
        assert_eq!(rows[0].initial, 'N');
    }

    #[test]
    fn a_connected_server_has_no_state_text() {
        assert_eq!(connector_rows(&[server("a", McpStatus::Connected, None)])[0].state, None);
        assert_eq!(connector_rows(&[server("a", McpStatus::Disabled, None)])[0].state, Some(ConnectorState::Off));
    }

    #[test]
    fn an_empty_list_gives_no_rows() {
        assert!(connector_rows(&[]).is_empty());
        assert!(connector_table(&[], &|_| None).is_empty());
    }

    #[test]
    fn the_table_keeps_the_sdk_order_and_tags_plugin_and_user_servers() {
        let mut a = server("zeta", McpStatus::Connected, Some("user"));
        a.remote = Some(true);
        let mut b = server("alpha", McpStatus::NeedsAuth, None);
        b.source = Some("plugin".into());
        b.remote = Some(false);
        let c = server("mid", McpStatus::Failed, Some("project"));
        let table = connector_table(&[a, b, c], &|name| (name == "alpha").then(|| "Tools".to_string()));
        let names: Vec<_> = table.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["zeta", "alpha", "mid"]);
        assert_eq!((table[0].tag, table[0].kind, table[0].status), (Some(ServerTag::User), Some(TransportLabel::Web), TableStatus::Connected));
        assert_eq!((table[1].tag, table[1].kind), (Some(ServerTag::Plugin), Some(TransportLabel::Desktop)));
        assert_eq!(table[1].subtitle.as_deref(), Some("Tools"));
        assert_eq!(table[1].status, TableStatus::State(ConnectorState::SignIn));
        assert_eq!((table[2].tag, table[2].kind, table[2].subtitle.clone()), (None, None, None));
    }

    #[test]
    fn the_short_token_label_drops_a_trailing_point_zero() {
        assert_eq!(format_tokens_short(1_000_000), "1M");
        assert_eq!(format_tokens_short(200_000), "200k");
        assert_eq!(format_tokens_short(78_100), "78.1k");
        assert_eq!(format_tokens_short(999), "999");
    }
}
