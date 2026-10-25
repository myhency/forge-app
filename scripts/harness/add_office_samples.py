#!/usr/bin/env python3
"""Adds office samples to the harness fixture: an xlsx (two sheets, numbers, a date, a formula nobody calculated),
a fake docx, and an xlsx outside the session folder, plus a reply in the session "Read the config" that names them
in inline code (the last message of the turn), so the `결과물` card and the links can be checked.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_office_samples.py scripts/harness/launch.sh
"""
import datetime
import json
import os
import sys
from pathlib import Path

import openpyxl

import build_fixture as bf

SESSION_ID = "00000000-0000-4000-8000-000000000006"


def make_book(path, title):
    book = openpyxl.Workbook()
    sheet = book.active
    sheet.title = "요약"
    sheet.append(["항목", "금액", "날짜"])
    sheet.append(["커피", 4500, datetime.date(2026, 10, 1)])
    sheet.append(["점심", 12000, datetime.date(2026, 10, 2)])
    sheet.append([title, "=SUM(B2:B3)", None])
    data = book.create_sheet("데이터")
    for row in range(1, 700):
        data.append([row, f"user{row}", row * 1.5])
    book.save(path)


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    folder = cwd / "office"
    folder.mkdir(parents=True, exist_ok=True)
    make_book(folder / "report.xlsx", "합계")
    (folder / "memo.docx").write_bytes(b"PK\x03\x04 not really a docx\x00\x00")
    outside = Path("/private/tmp/forge-outside-report.xlsx")
    make_book(outside, "밖")

    text = "만들었습니다.\n\n- `{}`\n- `{}`\n- `{}`\n".format(folder / "report.xlsx", folder / "memo.docx", outside)
    transcript = Path(claude_home) / "projects" / bf.escape_project_path(cwd) / f"{SESSION_ID}.jsonl"
    line = {"type": "assistant", "sessionId": SESSION_ID, "message": {"role": "assistant", "content": [{"type": "text", "text": text}]}}
    with transcript.open("a") as handle:
        handle.write(json.dumps(line) + "\n")


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
    print(f"office samples added under {bf.DEFAULT_PROJECTS_ROOT}/my-app/office", file=sys.stderr)
