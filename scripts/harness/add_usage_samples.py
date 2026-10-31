#!/usr/bin/env python3
"""Adds a session "Usage sample" to the harness fixture: three assistant messages on two models, with input, output and
cache tokens, so the `/usage` card has a cost, a token table, a cache hit and a table by model to look at.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_usage_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf

SESSION_ID = "00000000-0000-4000-8000-000000000301"


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    cwd.mkdir(parents=True, exist_ok=True)
    transcript = Path(claude_home) / "projects" / bf.escape_project_path(cwd) / f"{SESSION_ID}.jsonl"

    def user(text):
        return {"type": "user", "cwd": str(cwd), "sessionId": SESSION_ID, "message": {"role": "user", "content": text}}

    def reply(message_id, model, text, input_tokens, output_tokens, cache_read, cache_write):
        return {"type": "assistant", "sessionId": SESSION_ID, "message": {
            "id": message_id, "role": "assistant", "model": model, "content": [{"type": "text", "text": text}],
            "usage": {"input_tokens": input_tokens, "output_tokens": output_tokens,
                      "cache_read_input_tokens": cache_read, "cache_creation_input_tokens": cache_write}}}

    lines = [
        user("Usage sample: summarise the repo"),
        reply("m1", "claude-sonnet-4-6", "Here is a summary.", 1200, 3400, 52000, 8000),
        user("And the tests?"),
        reply("m2", "claude-sonnet-4-6", "The tests cover the parser.", 300, 900, 61000, 0),
        user("One more check"),
        reply("m3", "claude-haiku-4-5", "Checked.", 150, 220, 9000, 0),
    ]
    transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
    mtime = bf._epoch(30, 19)
    os.utime(transcript, (mtime, mtime))


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
