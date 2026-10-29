#!/usr/bin/env python3
"""Adds one session, "Run blocks", whose reply holds code blocks for the run button: a one-line bash block, a
two-line bash block, a block that fails with exit 3, a block that prints many lines, and a python block (no button).

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_code_run_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf

SESSION_ID = "00000000-0000-4000-8000-000000000401"

REPLY = """Here are some commands you can run.

```bash
echo forge-run-hello
```

Two lines:

```bash
cd /tmp
pwd
```

One that fails:

```sh
echo before; (exit 3)
```

Many lines:

```bash
seq 1 300
```

Not a shell block:

```python
print("no run button here")
```
"""


def lines_for(session_id, cwd):
    return [
        {"type": "user", "cwd": str(cwd), "sessionId": session_id, "message": {"role": "user", "content": "Give me some commands"}},
        {"type": "assistant", "sessionId": session_id, "message": {"role": "assistant", "content": [{"type": "text", "text": REPLY}]}},
    ]


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    project_dir = Path(claude_home) / "projects" / bf.escape_project_path(cwd)
    transcript = project_dir / f"{SESSION_ID}.jsonl"
    lines = lines_for(SESSION_ID, cwd)
    lines.insert(1, {"type": "custom-title", "customTitle": "Run blocks", "sessionId": SESSION_ID})
    transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
    mtime = bf._epoch(30, 20)
    os.utime(transcript, (mtime, mtime))


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
