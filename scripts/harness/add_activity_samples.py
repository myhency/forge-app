#!/usr/bin/env python3
"""Adds a session "Activity wording" to the harness fixture, so the folded lines can be looked at: a turn that read a
memory file, ran a skill, loaded a tool, ran a command and started a background one; and a turn that read one image.
It also adds the fork samples, folded (the line `└ 중첩된 세션 N개`).

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_activity_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import add_fork_samples as forks
import add_preview_samples as previews
import build_fixture as bf

SESSION_ID = "00000000-0000-4000-8000-000000000201"
MEMORY = "/Users/me/.claude/projects/-Users-me-forge/memory/MEMORY.md"
BACKGROUND = (
    "Command running in background with ID: bk1abc. Output is being written to: "
    "/private/tmp/claude/tasks/bk1abc.output. You will be notified when it completes."
)


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    cwd.mkdir(parents=True, exist_ok=True)
    picture = cwd / "shot.png"
    picture.write_bytes(previews.make_png(120, 80))
    transcript = Path(claude_home) / "projects" / bf.escape_project_path(cwd) / f"{SESSION_ID}.jsonl"

    def user(text):
        return {"type": "user", "cwd": str(cwd), "sessionId": SESSION_ID, "message": {"role": "user", "content": text}}

    def assistant(block):
        return {"type": "assistant", "sessionId": SESSION_ID, "message": {"role": "assistant", "content": [block]}}

    def call(number, name, tool_input, output="ok"):
        tool_id = f"toolu_{number}"
        return [
            assistant({"type": "tool_use", "id": tool_id, "name": name, "input": tool_input}),
            {"type": "user", "sessionId": SESSION_ID, "message": {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": tool_id, "content": output}]}},
        ]

    lines = [user("Check the notes and the build")]
    lines += call(1, "Read", {"file_path": MEMORY})
    lines += call(2, "Skill", {"skill": "superpowers:brainstorming"})
    lines += call(3, "ToolSearch", {"query": "select:Foo"})
    lines += call(4, "Bash", {"command": "ls -la", "description": "List the folder"})
    lines += call(5, "Bash", {"command": "sleep 4 && make", "run_in_background": True}, BACKGROUND)
    lines.append(assistant({"type": "text", "text": "Done with the notes and the build."}))
    lines.append(user("Now look at the screenshot"))
    lines += call(6, "Read", {"file_path": str(picture)})
    lines.append(assistant({"type": "text", "text": "The screenshot is a red box."}))
    transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
    mtime = bf._epoch(30, 18)
    os.utime(transcript, (mtime, mtime))


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
    forks.add()
    forks.fold()
