#!/usr/bin/env python3
"""Adds a worktree session to the harness fixture, so the line under a session title (branch mark, branch, folder)
can be looked at: "Worktree session" runs in a worktree of `my-app` that Forge's config has a record for.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_worktree_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf

CONFIG = Path("/private/tmp/forge-harness-home/forge-config/config.json")
SESSION_ID = "00000000-0000-4000-8000-000000000201"
BRANCH = "claude/worktree-session-c1e83b"


def add(projects_root=bf.DEFAULT_PROJECTS_ROOT, claude_home=Path("/private/tmp/forge-harness-home/.claude"), config=CONFIG):
    repo = Path(os.path.realpath(projects_root)) / "my-app"
    tree = repo / ".claude" / "worktrees" / "worktree-session-c1e83b"
    tree.mkdir(parents=True, exist_ok=True)
    project_dir = Path(claude_home) / "projects" / bf.escape_project_path(tree)
    project_dir.mkdir(parents=True, exist_ok=True)
    lines = [
        {"type": "user", "cwd": str(tree), "sessionId": SESSION_ID, "message": {"role": "user", "content": "Try the next idea"}},
        {"type": "custom-title", "customTitle": "Worktree session", "sessionId": SESSION_ID},
    ]
    transcript = project_dir / f"{SESSION_ID}.jsonl"
    transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
    mtime = bf._epoch(30, 18)
    os.utime(transcript, (mtime, mtime))
    data = json.loads(config.read_text())
    data["worktrees"] = [{
        "name": "worktree-session-c1e83b", "path": str(tree), "repo": str(repo),
        "branch": BRANCH, "base_branch": "master", "created_at": 1,
    }]
    config.write_text(json.dumps(data, indent=2))


if __name__ == "__main__":
    add()
