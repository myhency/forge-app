#!/usr/bin/env python3
"""Adds preview samples to the harness fixture: files in `my-app/samples/` and a reply in the session
"Read the config" that names five of them (and one path outside the folder) by absolute path, so a link in the chat can be clicked.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_preview_samples.py scripts/harness/launch.sh
"""
import json
import os
import struct
import sys
import zlib
from pathlib import Path

import build_fixture as bf

SESSION_ID = "00000000-0000-4000-8000-000000000006"


def make_png(width=60, height=40):
    rows = b"".join(b"\x00" + bytes([200, 60, 60]) * width for _ in range(height))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b"")


def make_pdf(pages):
    objects = ["<</Type/Catalog/Pages 2 0 R>>"]
    kids = " ".join(f"{3 + 2 * i} 0 R" for i in range(pages))
    objects.append(f"<</Type/Pages/Kids[{kids}]/Count {pages}>>")
    font_id = 3 + 2 * pages
    for i in range(pages):
        objects.append(
            f"<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]/Contents {4 + 2 * i} 0 R/Resources<</Font<</F1 {font_id} 0 R>>>>>>"
        )
        stream = f"BT /F1 48 Tf 72 700 Td (Page {i + 1} of {pages}) Tj ET"
        objects.append(f"<</Length {len(stream)}>>stream\n{stream}\nendstream")
    objects.append("<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>")
    out = b"%PDF-1.4\n"
    offsets = []
    for number, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += f"{number} 0 obj\n{body}\nendobj\n".encode()
    xref = len(out)
    out += f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode()
    for offset in offsets:
        out += f"{offset:010d} 00000 n \n".encode()
    out += f"trailer<</Size {len(objects) + 1}/Root 1 0 R>>\nstartxref\n{xref}\n%%EOF\n".encode()
    return out


HTML = (
    '<!doctype html><html><body><h1>Hello HTML</h1><p id="out">script has not run</p><p>static text shows</p>'
    "<script>document.getElementById('out').textContent='script RAN';</script>"
    '<a href="https://example.com">link</a></body></html>\n'
)
SVG = (
    '<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect width="120" height="80" fill="#4a7"/>'
    '<circle cx="60" cy="40" r="25" fill="#fc3"/></svg>\n'
)


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    folder = cwd / "samples"
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "data.csv").write_text("name,age,city\nAda,36,London\nLinus,55,Portland\n")
    big = ["id,name,note"] + [f"{n},user{n},a longer note for row {n}" for n in range(700)]
    (folder / "big.csv").write_text("\n".join(big) + "\n")
    (folder / "data.json").write_text('{"name":"forge","nested":{"a":[1,2,3]}}\n')
    (folder / "page.html").write_text(HTML)
    (folder / "pic.svg").write_text(SVG)
    (folder / "pic.png").write_bytes(make_png())
    (folder / "doc.pdf").write_bytes(make_pdf(1))
    (folder / "multi.pdf").write_bytes(make_pdf(12))

    shown = ["data.csv", "page.html", "doc.pdf", "pic.png", "pic.svg"]
    # The last path is outside the session folder: a click on it must only show a notice.
    text = "Here are the files:\n\n" + "\n\n".join([*(str(folder / name) for name in shown), "/private/tmp/outside.txt"])
    transcript = Path(claude_home) / "projects" / bf.escape_project_path(cwd) / f"{SESSION_ID}.jsonl"
    line = {"type": "assistant", "sessionId": SESSION_ID, "message": {"role": "assistant", "content": [{"type": "text", "text": text}]}}
    with transcript.open("a") as handle:
        handle.write(json.dumps(line) + "\n")


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
    print(f"samples added under {bf.DEFAULT_PROJECTS_ROOT}/my-app/samples", file=sys.stderr)
