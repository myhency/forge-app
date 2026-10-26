use std::collections::HashMap;

use serde_json::{json, Value};

enum Line<'a> {
    Json(Value),
    Raw(&'a str),
}

fn is_title(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("custom-title")
}

/// The part of a transcript that comes before the line whose `uuid` is `uuid`, with that line's
/// file snapshot left out too. `None` when no line has that uuid.
pub(crate) fn truncated_before(source: &str, uuid: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let json = |line: &str| serde_json::from_str::<Value>(line).ok();
    let cut = lines.iter().position(|line| json(line).is_some_and(|value| value.get("uuid").and_then(Value::as_str) == Some(uuid)))?;
    let mut out = String::new();
    for line in &lines[..cut] {
        if json(line).is_some_and(|value| value.get("messageId").and_then(Value::as_str) == Some(uuid)) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    // The session's name is not part of the conversation: its last title lines (written near the end
    // of the file) stay, so the cut session keeps its name.
    for kind in ["custom-title", "ai-title"] {
        if let Some(line) = lines[cut..].iter().rev().find(|line| json(line).is_some_and(|value| value.get("type").and_then(Value::as_str) == Some(kind))) {
            out.push_str(line);
            out.push('\n');
        }
    }
    Some(out)
}

/// The text of a fork's transcript, made from the text of the source's. This is what the SDK's
/// `forkSession` does (seen on a probe transcript, spec 2026-10-03-fork-effort-design.md): every
/// line with a `uuid` gets a new one and a `forkedFrom: {sessionId, messageUuid}`; `parentUuid`
/// follows the new uuids (a parent that is not in the file stays); `sessionId` becomes the new id;
/// the old `custom-title` lines are dropped and one new one, with `title`, ends the file. A line
/// that is not a JSON object is copied as it is; blank lines are dropped.
pub(crate) fn forked_transcript(
    source: &str,
    old_id: &str,
    new_id: &str,
    title: &str,
    now: &str,
    mut new_uuid: impl FnMut() -> String,
) -> String {
    let lines: Vec<Line> = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| match serde_json::from_str::<Value>(line) {
            Ok(value @ Value::Object(_)) => Line::Json(value),
            _ => Line::Raw(line),
        })
        .collect();

    // Every uuid first, so a child that comes before its parent still finds it.
    let mut ids: HashMap<String, String> = HashMap::new();
    for line in &lines {
        if let Line::Json(value) = line {
            if is_title(value) {
                continue;
            }
            if let Some(old) = value.get("uuid").and_then(Value::as_str) {
                ids.entry(old.to_string()).or_insert_with(&mut new_uuid);
            }
        }
    }

    let mut out = String::new();
    for line in lines {
        match line {
            Line::Raw(text) => out.push_str(text),
            Line::Json(mut value) => {
                if is_title(&value) {
                    continue;
                }
                let old_uuid = value.get("uuid").and_then(Value::as_str).map(str::to_string);
                let object = value.as_object_mut().expect("only objects are parsed as JSON lines");
                if let Some(old) = &old_uuid {
                    object.insert("uuid".to_string(), json!(ids[old]));
                    object.insert("forkedFrom".to_string(), json!({"sessionId": old_id, "messageUuid": old}));
                }
                let parent = object.get("parentUuid").and_then(Value::as_str).and_then(|p| ids.get(p)).cloned();
                if let Some(parent) = parent {
                    object.insert("parentUuid".to_string(), json!(parent));
                }
                if object.contains_key("sessionId") {
                    object.insert("sessionId".to_string(), json!(new_id));
                }
                out.push_str(&value.to_string());
            }
        }
        out.push('\n');
    }
    out.push_str(&json!({"type": "custom-title", "customTitle": title, "sessionId": new_id, "uuid": new_uuid(), "timestamp": now}).to_string());
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn counter() -> impl FnMut() -> String {
        let mut n = 0;
        move || {
            n += 1;
            format!("new-{n}")
        }
    }

    fn parse(text: &str) -> Vec<Value> {
        text.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    fn source(lines: &[Value]) -> String {
        lines.iter().map(|line| format!("{line}\n")).collect()
    }

    #[test]
    fn a_transcript_is_cut_before_the_named_message_and_its_snapshot_goes_with_it() {
        let text = source(&[
            json!({"type": "file-history-snapshot", "messageId": "u1"}),
            json!({"type": "user", "uuid": "u1", "parentUuid": null}),
            json!({"type": "assistant", "uuid": "a1", "parentUuid": "u1"}),
            json!({"type": "file-history-snapshot", "messageId": "u2"}),
            json!({"type": "user", "uuid": "u2", "parentUuid": "a1"}),
            json!({"type": "assistant", "uuid": "a2", "parentUuid": "u2"}),
        ]);

        let cut = truncated_before(&text, "u2").unwrap();
        let kept: Vec<Value> = cut.lines().map(|line| serde_json::from_str(line).unwrap()).collect();

        assert_eq!(kept.len(), 3);
        assert_eq!(kept[2]["uuid"], "a1");
        assert!(truncated_before(&text, "missing").is_none());

        let titled = source(&[
            json!({"type": "user", "uuid": "u1"}),
            json!({"type": "ai-title", "aiTitle": "old guess"}),
            json!({"type": "assistant", "uuid": "a1"}),
            json!({"type": "ai-title", "aiTitle": "newer guess"}),
            json!({"type": "custom-title", "customTitle": "Mine"}),
        ]);
        let kept: Vec<Value> = truncated_before(&titled, "u1").unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        let titles: Vec<String> = kept.iter().filter_map(|v| v.get("customTitle").or(v.get("aiTitle")).and_then(Value::as_str).map(str::to_string)).collect();
        assert_eq!(titles, vec!["Mine", "newer guess"], "the last title lines survive a cut that removes everything else");
        assert_eq!(truncated_before(&text, "u1").unwrap(), "", "nothing came before the first message but its own snapshot");
    }

    #[test]
    fn every_message_gets_a_new_uuid_a_remapped_parent_the_new_session_id_and_a_forked_from() {
        let text = source(&[
            json!({"type": "user", "uuid": "u1", "parentUuid": null, "sessionId": "old"}),
            json!({"type": "assistant", "uuid": "u2", "parentUuid": "u1", "sessionId": "old"}),
        ]);

        let out = parse(&forked_transcript(&text, "old", "new", "T (fork)", "2026-10-03T00:00:00Z", counter()));

        assert_eq!(out[0]["uuid"], "new-1");
        assert!(out[0]["parentUuid"].is_null());
        assert_eq!(out[1]["uuid"], "new-2");
        assert_eq!(out[1]["parentUuid"], "new-1");
        assert_eq!(out[0]["sessionId"], "new");
        assert_eq!(out[1]["sessionId"], "new");
        assert_eq!(out[0]["forkedFrom"], json!({"sessionId": "old", "messageUuid": "u1"}));
        assert_eq!(out[1]["forkedFrom"], json!({"sessionId": "old", "messageUuid": "u2"}));
    }

    #[test]
    fn the_old_title_is_dropped_and_one_new_title_line_ends_the_file() {
        let text = source(&[
            json!({"type": "user", "uuid": "u1", "parentUuid": null, "sessionId": "old"}),
            json!({"type": "custom-title", "customTitle": "Old", "sessionId": "old"}),
        ]);

        let out = parse(&forked_transcript(&text, "old", "new", "Old (fork)", "2026-10-03T00:00:00Z", counter()));

        assert_eq!(out.len(), 2, "the old title line is not copied");
        assert_eq!(out[1]["type"], "custom-title");
        assert_eq!(out[1]["customTitle"], "Old (fork)");
        assert_eq!(out[1]["sessionId"], "new");
        assert_eq!(out[1]["uuid"], "new-2");
        assert_eq!(out[1]["timestamp"], "2026-10-03T00:00:00Z");
    }

    #[test]
    fn a_line_that_is_not_json_is_kept_as_it_is() {
        let text = format!("{}\nnot json at all\n", json!({"type": "user", "uuid": "u1", "parentUuid": null, "sessionId": "old"}));

        let out = forked_transcript(&text, "old", "new", "T", "now", counter());

        assert!(out.lines().any(|line| line == "not json at all"));
        assert_eq!(out.lines().count(), 3, "two copied lines and the title");
    }

    #[test]
    fn a_parent_that_is_not_in_the_file_is_left_as_it_is() {
        let text = source(&[json!({"type": "user", "uuid": "u2", "parentUuid": "gone", "sessionId": "old"})]);

        let out = parse(&forked_transcript(&text, "old", "new", "T", "now", counter()));

        assert_eq!(out[0]["parentUuid"], "gone");
        assert_eq!(out[0]["uuid"], "new-1");
    }

    #[test]
    fn a_line_without_a_uuid_only_gets_the_new_session_id() {
        let text = source(&[json!({"type": "permission-mode", "permissionMode": "plan", "sessionId": "old"})]);

        let out = parse(&forked_transcript(&text, "old", "new", "T", "now", counter()));

        assert_eq!(out[0]["sessionId"], "new");
        assert_eq!(out[0]["permissionMode"], "plan");
        assert!(out[0].get("forkedFrom").is_none());
        assert!(out[0].get("uuid").is_none());
    }

    #[test]
    fn a_source_without_a_final_newline_and_with_blank_lines_is_fine() {
        let text = format!(
            "{}\n\n{}",
            json!({"type": "user", "uuid": "u1", "parentUuid": null, "sessionId": "old"}),
            json!({"type": "user", "uuid": "u2", "parentUuid": "u1", "sessionId": "old"})
        );

        let out = parse(&forked_transcript(&text, "old", "new", "T", "now", counter()));

        assert_eq!(out.len(), 3);
        assert_eq!(out[1]["parentUuid"], "new-1");
    }
}
