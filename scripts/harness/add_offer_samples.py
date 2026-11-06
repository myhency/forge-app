#!/usr/bin/env python3
"""Adds a session "Offer sample" to the harness fixture: one turn that ends with a `start_session` proposal card,
so the card and its 옵션 popover can be looked at without a live turn. The folder `my-app` is made a git repository,
so the popover shows the 워크트리 row.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_offer_samples.py scripts/harness/launch.sh
"""
import json
import os
import subprocess
from pathlib import Path

import build_fixture as bf

SESSION_ID = "00000000-0000-4000-8000-000000000301"
TOOL = "mcp__ccd_session__start_session"


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    cwd.mkdir(parents=True, exist_ok=True)
    if not (cwd / ".git").exists():
        subprocess.run(["git", "init", "-q", str(cwd)], check=True)
    transcript = Path(claude_home) / "projects" / bf.escape_project_path(cwd) / f"{SESSION_ID}.jsonl"
    transcript.parent.mkdir(parents=True, exist_ok=True)
    offer = {"initiation": "own_initiative", "title": "Fix stats", "background": "Two tests fail in stats.py.",
             "prompt": "Fix mean in stats.py\nThen run pytest."}
    lines = [
        {"type": "user", "cwd": str(cwd), "sessionId": SESSION_ID, "message": {"role": "user", "content": "Offer sample"}},
        {"type": "assistant", "sessionId": SESSION_ID, "message": {"role": "assistant", "content": [
            {"type": "text", "text": "The failing tests are a separate job; here is a session for it."}]}},
        {"type": "assistant", "sessionId": SESSION_ID, "message": {"role": "assistant", "content": [
            {"type": "tool_use", "id": "toolu_offer1", "name": TOOL, "input": offer}]}},
        {"type": "user", "sessionId": SESSION_ID, "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": "toolu_offer1", "content": "Proposal shown."}]}},
    ]
    transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
    mtime = bf._epoch(30, 19)
    os.utime(transcript, (mtime, mtime))


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
