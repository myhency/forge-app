#!/usr/bin/env python3
"""Adds a session "Table and edits" to the harness fixture: a turn that edits one file and writes another, then answers
with a markdown table, so the table look and the end card's file rows can be checked.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_table_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf

SESSION_ID = "00000000-0000-4000-8000-000000000701"


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    cwd.mkdir(parents=True, exist_ok=True)
    (cwd / "stats.py").write_text("def mean(xs):\n    return sum(xs) / len(xs)\n")
    (cwd / "summary.md").write_text("# Sales\n\n| Region | Total |\n|---|---:|\n| Seoul | 255 |\n")
    edit = {"file_path": str(cwd / "stats.py"), "old_string": "(len(xs) - 1)", "new_string": "len(xs)"}
    write = {"file_path": str(cwd / "summary.md"), "content": "# Sales\n\n| Region | Total |\n|---|---:|\n| Seoul | 255 |\n"}
    text = (
        "Fixed the failing test and wrote the totals.\n\n"
        "| Region | Total |\n|---|---:|\n| Seoul | 255 |\n| Busan | 175 |\n| All | 430 |\n\n"
        "No commit was made."
    )
    transcript = Path(claude_home) / "projects" / bf.escape_project_path(cwd) / f"{SESSION_ID}.jsonl"
    lines = [
        {"type": "user", "cwd": str(cwd), "sessionId": SESSION_ID, "message": {"role": "user", "content": "Table and edits: fix stats and sum sales"}},
        {"type": "assistant", "sessionId": SESSION_ID, "message": {"role": "assistant", "content": [
            {"type": "tool_use", "id": "e1", "name": "Edit", "input": edit},
            {"type": "tool_use", "id": "w1", "name": "Write", "input": write},
        ]}},
        {"type": "user", "sessionId": SESSION_ID, "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": "e1", "content": "ok"},
            {"type": "tool_result", "tool_use_id": "w1", "content": "ok"},
        ]}},
        {"type": "assistant", "sessionId": SESSION_ID, "message": {"role": "assistant", "content": [{"type": "text", "text": text}]}},
    ]
    transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
    mtime = bf._epoch(30, 22)
    os.utime(transcript, (mtime, mtime))


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
