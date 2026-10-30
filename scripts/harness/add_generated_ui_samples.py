#!/usr/bin/env python3
"""Adds two sessions to the harness fixture, for the interactive page card: "Bill splitter" holds a page with
controls and a send button, and "Hostile page" holds a page that tries to reach the network, the parent page and
the app's IPC handler and writes what happened into itself. The log (/private/tmp/forge-harness.log) must say
`forge: dropped an IPC message from ...` for the hostile page.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_generated_ui_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf

BILL_ID = "00000000-0000-4000-8000-000000000401"
HOSTILE_ID = "00000000-0000-4000-8000-000000000402"

BILL = {
    "initialHeight": 260,
    "placeholderMessages": ["Setting up the calculator…"],
    "css": ".row{display:flex;gap:12px;align-items:center;margin:8px 0}input{width:90px;padding:4px 6px;border:1px solid var(--line);border-radius:6px;background:var(--bg)}button{padding:6px 12px;border:0;border-radius:8px;background:var(--accent);color:#fff;cursor:pointer}.big{font-size:24px;font-weight:600}",
    "html": "<h3 style='margin:0 0 8px'>Bill splitter</h3><div class='row'>Total <input id='t' type='number' value='120' oninput='calc()'> Tip % <input id='p' type='number' value='15' oninput='calc()'> People <input id='n' type='number' value='4' oninput='calc()'></div><div class='big' id='out'></div><div class='row'><button onclick='share()'>Send this split</button></div>",
    "jsFunctions": "function calc(){var t=+document.getElementById('t').value,p=+document.getElementById('p').value,n=Math.max(1,+document.getElementById('n').value);var each=t*(1+p/100)/n;document.getElementById('out').textContent='Each pays '+each.toFixed(2);return each}function share(){forge.send({label:'Bill split',values:{total:+document.getElementById('t').value,tip:document.getElementById('p').value+'%',people:+document.getElementById('n').value,each:calc().toFixed(2)}})}",
    "jsExpressions": "calc()",
}

HOSTILE = {
    "initialHeight": 220,
    "css": "pre{white-space:pre-wrap;font-size:12px}",
    "html": "<h3 style='margin:0 0 8px'>Hostile page</h3><pre id='log'>running…</pre>",
    "jsFunctions": "function log(m){document.getElementById('log').textContent+='\\n'+m}async function attack(){document.getElementById('log').textContent='';try{await fetch('https://example.com');log('fetch: ALLOWED')}catch(e){log('fetch: blocked')}try{log('parent document: '+window.parent.document.title)}catch(e){log('parent document: blocked')}try{window.webkit.messageHandlers.ipc.postMessage('{\"method\":\"query\",\"params\":{}}');log('ipc: posted (the app must log a dropped message)')}catch(e){log('ipc: no handler')}try{window.ipc.postMessage('{\"method\":\"query\",\"params\":{}}');log('window.ipc: posted')}catch(e){log('window.ipc: none')}try{localStorage.setItem('a','b');log('storage: ALLOWED')}catch(e){log('storage: blocked')}try{parent.postMessage({n:'guess',label:'x',values:{a:1}},'*');log('bridge guess: posted without a click (must be ignored)')}catch(e){}}",
    "jsExpressions": "attack()",
}


def lines_for(session_id, cwd, title, page, reply):
    def user(text):
        return {"type": "user", "cwd": str(cwd), "sessionId": session_id, "message": {"role": "user", "content": text}}

    def assistant(block):
        return {"type": "assistant", "sessionId": session_id, "message": {"role": "assistant", "content": [block]}}

    tool_id = f"toolu_{session_id[-3:]}_1"
    return [
        user(title),
        assistant({"type": "tool_use", "id": tool_id, "name": "mcp__forge_ui__generate_sandboxed_ui", "input": page}),
        {"type": "user", "sessionId": session_id, "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": tool_id, "content": "Shown in the chat as an interactive page."}]}},
        assistant({"type": "text", "text": reply}),
    ]


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    project_dir = Path(claude_home) / "projects" / bf.escape_project_path(cwd)
    samples = (
        (BILL_ID, "Bill splitter", BILL, "Here is a splitter: change the numbers and press the button to send the result.", 19),
        (HOSTILE_ID, "Hostile page", HOSTILE, "The page above tries to escape its frame.", 18),
    )
    for session_id, title, page, reply, hour in samples:
        transcript = project_dir / f"{session_id}.jsonl"
        lines = lines_for(session_id, cwd, "Make a " + title.lower(), page, reply)
        lines.insert(1, {"type": "custom-title", "customTitle": title, "sessionId": session_id})
        transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
        mtime = bf._epoch(30, hour)
        os.utime(transcript, (mtime, mtime))


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
