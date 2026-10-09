//! How far a running workflow is: where its journal lives, and what the journal says. The journal
//! (`journal.jsonl` in the workflow's transcript folder) is the SDK's own file, not a documented
//! format, so it is read loosely: a line that does not parse, and a kind of line we do not know, is
//! skipped, never an error.

use std::path::Path;

use serde_json::Value;

/// What the Workflow tool's result says about the run it started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRef {
    pub task_id: String,
    pub transcript_dir: String,
}

const LAUNCHED: &str = "Workflow launched in background. Task ID: ";
const DIR_MARK: &str = "Transcript dir: ";

/// Reads `Workflow launched in background. Task ID: <id>\nSummary: ...\nTranscript dir: <dir>\n...`.
/// Anything else is `None`.
pub fn workflow_ref(output: &str) -> Option<WorkflowRef> {
    let after_id = &output[output.find(LAUNCHED)? + LAUNCHED.len()..];
    let id_end = after_id.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(after_id.len());
    let task_id = &after_id[..id_end];
    let after_dir = &output[output.find(DIR_MARK)? + DIR_MARK.len()..];
    let dir = after_dir.lines().next().unwrap_or("").trim();
    (!task_id.is_empty() && !dir.is_empty()).then(|| WorkflowRef { task_id: task_id.to_string(), transcript_dir: dir.to_string() })
}

/// The journal file of a workflow, only when the folder is where the SDK keeps workflow runs:
/// absolute, no `..`, no NUL, and `…/subagents/workflows/wf_<run>`. Forge reads no other folder.
pub fn journal_path(transcript_dir: &str) -> Option<String> {
    if transcript_dir.contains('\0') || !forge_domain::is_absolute_path(transcript_dir) || forge_domain::has_parent_part(transcript_dir) {
        return None;
    }
    let normal = transcript_dir.replace('\\', "/");
    let mut parts = normal.trim_end_matches('/').rsplit('/');
    let run = parts.next()?;
    let valid = run.starts_with("wf_") && run.len() > 3 && parts.next() == Some("workflows") && parts.next() == Some("subagents");
    valid.then(|| Path::new(transcript_dir).join("journal.jsonl").to_string_lossy().to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Running,
    Done,
}

/// One agent the workflow started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalAgent {
    pub id: String,
    pub label: String,
    pub phase: String,
    pub state: AgentState,
}

/// The agents in the order they started; one that has a `result` line is done.
pub fn parse_journal(text: &str) -> Vec<JournalAgent> {
    let mut agents: Vec<JournalAgent> = Vec::new();
    for line in text.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else { continue };
        let field = |name: &str| entry.get(name).and_then(Value::as_str);
        let Some(id) = field("agentId").filter(|id| !id.is_empty()) else { continue };
        match field("type") {
            Some("started") if !agents.iter().any(|agent| agent.id == id) => agents.push(JournalAgent {
                id: id.to_string(),
                label: field("label").unwrap_or("").to_string(),
                phase: field("phase").unwrap_or("").to_string(),
                state: AgentState::Running,
            }),
            Some("result") => {
                if let Some(agent) = agents.iter_mut().find(|agent| agent.id == id) {
                    agent.state = AgentState::Done;
                }
            }
            _ => {}
        }
    }
    agents
}

/// One phase of a workflow with the agents it has started so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    pub agents: Vec<JournalAgent>,
}

impl Step {
    pub fn done(&self) -> usize {
        self.agents.iter().filter(|agent| agent.state == AgentState::Done).count()
    }
}

/// The phases the script lists, in its order, each with its agents; a phase only the journal knows
/// (a script whose titles could not be read) follows them.
pub fn steps(titles: &[String], agents: &[JournalAgent]) -> Vec<Step> {
    let mut names: Vec<String> = titles.to_vec();
    for agent in agents {
        if !names.contains(&agent.phase) {
            names.push(agent.phase.clone());
        }
    }
    names
        .into_iter()
        .map(|name| Step { agents: agents.iter().filter(|agent| agent.phase == name).cloned().collect(), name })
        .filter(|step| !step.name.is_empty() || !step.agents.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESULT_TEXT: &str = "Workflow launched in background. Task ID: wdzs26lja\nSummary: Three parallel agents analyze\nTranscript dir: /Users/me/.claude/projects/-tmp-x/6a34/subagents/workflows/wf_cd3743cd-a82\nScript file: /Users/me/.claude/projects/-tmp-x/6a34/workflows/scripts/s.js\n(Edit this file)\nRun ID: wf_cd3743cd-a82\n\nYou will be notified when it completes.";

    #[test]
    fn the_real_result_gives_the_task_id_and_the_folder() {
        assert_eq!(
            workflow_ref(RESULT_TEXT),
            Some(WorkflowRef {
                task_id: "wdzs26lja".to_string(),
                transcript_dir: "/Users/me/.claude/projects/-tmp-x/6a34/subagents/workflows/wf_cd3743cd-a82".to_string()
            })
        );
    }

    #[test]
    fn other_text_gives_no_workflow_ref() {
        for text in ["", "ok", "Workflow launched in background. Task ID: abc", "Transcript dir: /a/subagents/workflows/wf_x\n", "Workflow launched in background. Task ID: \nTranscript dir: /x"] {
            assert_eq!(workflow_ref(text), None, "{text}");
        }
    }

    #[test]
    fn only_a_workflow_run_folder_gives_a_journal_path() {
        let ok = journal_path("/Users/me/.claude/projects/-tmp-x/6a34/subagents/workflows/wf_cd3743cd-a82").unwrap();
        assert!(ok.ends_with("wf_cd3743cd-a82/journal.jsonl"), "{ok}");
        assert!(journal_path("/Users/me/.claude/projects/-tmp-x/6a34/subagents/workflows/wf_cd3743cd-a82/").is_some(), "a trailing slash is fine");
        for bad in [
            "",
            "relative/subagents/workflows/wf_a",
            "/a/subagents/workflows/wf_a/../../../etc",
            "/a/subagents/workflows/other",
            "/a/subagents/workflows/wf_",
            "/a/workflows/wf_a",
            "/a/subagents/other/wf_a",
            "/a/subagents/workflows/wf_a\0",
        ] {
            assert_eq!(journal_path(bad), None, "{bad}");
        }
    }

    const JOURNAL: &str = concat!(
        "{\"type\": \"launched\"}\n",
        "{\"type\": \"started\", \"key\": \"k1\", \"agentId\": \"a1\", \"label\": \"domain\", \"phase\": \"Analyze\"}\n",
        "{\"type\": \"started\", \"key\": \"k2\", \"agentId\": \"a2\", \"label\": \"features\", \"phase\": \"Analyze\"}\n",
        "{\"type\": \"result\", \"key\": \"k1\", \"agentId\": \"a1\", \"result\": {\"folder\": \"/x\"}}\n",
    );

    #[test]
    fn the_journal_gives_each_agent_and_whether_it_has_a_result() {
        let agents = parse_journal(JOURNAL);
        assert_eq!(agents.len(), 2);
        assert_eq!((agents[0].label.as_str(), agents[0].phase.as_str(), agents[0].state), ("domain", "Analyze", AgentState::Done));
        assert_eq!((agents[1].label.as_str(), agents[1].state), ("features", AgentState::Running));
    }

    #[test]
    fn a_journal_with_noise_is_read_as_far_as_it_goes() {
        let text = format!("{{\"type\": \"result\", \"agentId\": \"ghost\"}}\nnot json\n{{\"type\": \"mystery\", \"agentId\": \"a1\"}}\n\n{JOURNAL}{{\"type\": \"started\", \"agentId\": \"a1\", \"label\": \"again\", \"phase\": \"Other\"}}\n{{\"type\": \"started\", \"agentId\": \"\"}}\n{{\"type\": \"star");
        let agents = parse_journal(&text);
        assert_eq!(agents.len(), 2, "a result with no start, noise, an empty id, a repeat start and a cut line add nothing");
        assert_eq!(agents[0].label, "domain", "a repeated start keeps the first");
        assert_eq!(parse_journal(""), Vec::new());
    }

    #[test]
    fn steps_follow_the_script_and_count_what_is_done() {
        let agents = parse_journal(JOURNAL);
        let steps = steps(&["Analyze".to_string(), "Merge".to_string()], &agents);
        assert_eq!(steps.len(), 2);
        assert_eq!((steps[0].name.as_str(), steps[0].agents.len(), steps[0].done()), ("Analyze", 2, 1));
        assert_eq!((steps[1].name.as_str(), steps[1].agents.len(), steps[1].done()), ("Merge", 0, 0), "a phase not started yet is still listed");
    }

    #[test]
    fn a_phase_only_the_journal_knows_follows_the_listed_ones() {
        let agents = parse_journal(JOURNAL);
        let names: Vec<String> = steps(&["Merge".to_string()], &agents).into_iter().map(|step| step.name).collect();
        assert_eq!(names, vec!["Merge".to_string(), "Analyze".to_string()]);
        assert_eq!(steps(&[], &[]), Vec::new());
    }
}
