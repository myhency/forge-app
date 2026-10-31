#!/usr/bin/env python3
"""Turns a run of the evaluation (sidecar/scripts/eval_ui.mjs) into harness sessions, one per prompt that made a page,
so each page can be looked at in the real window and screenshotted.

    FORGE_EVAL_DIR=/tmp/forge-eval/run1 FORGE_HARNESS_EXTRA=scripts/harness/add_eval_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf
from add_generated_ui_samples import lines_for


def add(claude_home, projects_root):
    run_dir = Path(os.environ["FORGE_EVAL_DIR"])
    cwd = Path(projects_root) / "my-app"
    project_dir = Path(claude_home) / "projects" / bf.escape_project_path(cwd)
    runs = [json.loads(p.read_text()) for p in sorted(run_dir.glob("*.json")) if p.name != "summary.json"]
    shown = [r for r in runs if r.get("pages")]
    for index, run in enumerate(shown):
        session_id = f"00000000-0000-4000-8000-0000000007{index:02d}"
        transcript = project_dir / f"{session_id}.jsonl"
        lines = lines_for(session_id, cwd, run["prompt"], run["pages"][0], (run.get("text") or "").strip()[:300] or "…")
        lines.insert(1, {"type": "custom-title", "customTitle": "eval " + run["id"], "sessionId": session_id})
        transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
        mtime = bf._epoch(30, 23 - index % 24)
        os.utime(transcript, (mtime, mtime))
    print(f"added {len(shown)} evaluation sessions")


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
