#!/usr/bin/env python3
"""Adds two sessions to the harness fixture, for the task-list tools: "Next step card" ends in a wrap_up question
(a live end card) and "Answered card" has the same card answered with its first button (a spent card).

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_turn_cards_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf

OPEN_ID = "00000000-0000-4000-8000-000000000301"
SPENT_ID = "00000000-0000-4000-8000-000000000302"
PLAN = {"items": [{"content": "Read the code", "status": "completed"}, {"content": "Fix the bug", "status": "in_progress"}, {"content": "Run the tests", "status": "pending"}], "scope": "first_task"}
WRAP = {
    "recap": "The login bug is fixed and the tests pass. Nothing is committed yet.",
    "ask": "Commit the fix?",
    "actions": [{"label": "Commit", "message": "Commit the fix"}, {"label": "Not now", "message": "Not now"}],
}


def lines_for(session_id, cwd, answered):
    def user(text):
        return {"type": "user", "cwd": str(cwd), "sessionId": session_id, "message": {"role": "user", "content": text}}

    def assistant(block):
        return {"type": "assistant", "sessionId": session_id, "message": {"role": "assistant", "content": [block]}}

    def call(number, name, tool_input, output):
        tool_id = f"toolu_{session_id[-3:]}_{number}"
        return [
            assistant({"type": "tool_use", "id": tool_id, "name": name, "input": tool_input}),
            {"type": "user", "sessionId": session_id, "message": {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": tool_id, "content": output}]}},
        ]

    out = [user("Fix the login bug")]
    out += call(1, "mcp__ccd_turn__update_plan", PLAN, "Plan shown (3 steps).")
    out += call(2, "Bash", {"command": "cargo test", "description": "Run the tests"}, "ok")
    out += call(3, "mcp__ccd_turn__wrap_up", WRAP, "Shown in the next-step box. Your closing message is your answer to the user: write it now, in full, as the last thing you say this turn. The box asks your question, so do not ask it again.")
    out.append(assistant({"type": "text", "text": "I fixed the login bug and ran the tests; they pass."}))
    if answered:
        out.append(user("Commit the fix"))
        out.append(assistant({"type": "text", "text": "Committed."}))
    return out


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    project_dir = Path(claude_home) / "projects" / bf.escape_project_path(cwd)
    for session_id, title, answered, hour in ((OPEN_ID, "Next step card", False, 19), (SPENT_ID, "Answered card", True, 18)):
        transcript = project_dir / f"{session_id}.jsonl"
        lines = lines_for(session_id, cwd, answered)
        lines.insert(1, {"type": "custom-title", "customTitle": title, "sessionId": session_id})
        transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
        mtime = bf._epoch(30, hour)
        os.utime(transcript, (mtime, mtime))


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
