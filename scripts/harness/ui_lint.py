#!/usr/bin/env python3
"""Scores an interactive page (the arguments of one generate_sandboxed_ui call) against what the frame allows and what
the Forge UI kit expects, and reports a run of the evaluation (sidecar/scripts/eval_ui.mjs).

    python3 scripts/harness/ui_lint.py report <run-dir>     a table of every prompt of a run
    python3 scripts/harness/ui_lint.py page <page.json>     the findings for one page

The allowed hosts and the kit classes are read from the sources that enforce them (crates/forge-entities/src/
generated_ui.rs and ui_kit.css), so this file cannot drift from them.
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
RS = ROOT / "crates" / "forge-entities" / "src"

MAX_PAGE_CHARS = 200_000
BLOCKED = ["leaflet", "tile.openstreetmap.org", "arcgisonline", "nationalmap.gov", "cartocdn", "mapbox", "googleapis", "naver", "kakao"]


def allowed_hosts():
    """The hosts of the three consts that feed the frame's policy: libraries, the map host, photos."""
    source = (RS / "generated_ui.rs").read_text()
    hosts = set()
    for name in ("LIBRARY_HOSTS", "MAP_HOST", "PHOTO_HOSTS"):
        match = re.search(rf'const {name}: &str = "([^"]+)";', source)
        if not match:
            raise RuntimeError(f"{name} not found in generated_ui.rs")
        hosts |= {h.removeprefix("https://") for h in match.group(1).split()}
    return hosts


def kit_classes():
    return set(re.findall(r"\.(fx-[a-z0-9-]+)", (RS / "ui_kit.css").read_text()))


def lint_page(page):
    """Returns {'score': 0-100, 'findings': [{'severity', 'rule', 'message', 'cost'}]} for one page."""
    parts = {k: (page.get(k) or "") for k in ("css", "html", "jsFunctions", "jsExpressions")}
    everything = "\n".join(parts.values())
    findings = []

    def add(severity, rule, message, cost):
        findings.append({"severity": severity, "rule": rule, "message": message, "cost": cost})

    if not parts["html"].strip():
        add("error", "empty", "the page has no html", 100)
    if len(everything) > MAX_PAGE_CHARS:
        add("error", "size", f"{len(everything)} characters is over the {MAX_PAGE_CHARS} limit", 100)
    elif len(everything) > 60_000:
        add("warn", "size", f"{len(everything)} characters is large", 5)

    hosts = allowed_hosts()
    for url in sorted(set(re.findall(r"https://[^\"'\s)<>\\`]+", everything))):
        host = url.removeprefix("https://").split("/")[0]
        if host in ("www.w3.org",):
            continue
        if host not in hosts:
            add("error", "host", f"{host} is not allowed (the frame blocks it)", 25)
    lowered = everything.lower()
    for word in BLOCKED:
        if word in lowered:
            add("error", "blocked-map", f"'{word}' is a blocked or key-only map provider; use MapLibre with OpenFreeMap", 25)
            break

    known = kit_classes()
    used = set(re.findall(r"(?<![\w-])(fx-[a-z0-9-]+)", everything))
    if len(used & known) < 3:
        add("warn", "kit", "uses fewer than three Forge UI kit classes (fx-*): it will look unfinished", 15)
    for unknown in sorted(used - known - {"fx-tabset"}):
        add("warn", "kit-unknown", f"{unknown} is not a kit class", 3)

    if re.search(r"\b\d+(\.\d+)?vh\b", everything):
        add("warn", "vh", "sizes with vh: the card follows the content, so vh units feed back and grow", 10)
    for img in re.findall(r"<img\b[^>]*>", parts["html"], flags=re.I):
        if "onerror" not in img.lower():
            add("warn", "img-onerror", "an <img> has no onerror handler", 8)
            break
    height = page.get("initialHeight")
    if not isinstance(height, (int, float)) or not 100 <= height <= 800:
        add("warn", "height", "initialHeight is missing or outside 100-800", 5)
    if len(re.findall(r"#[0-9a-fA-F]{3,6}\b|\b(?:white|black|gray|grey)\b", parts["css"] + parts["html"])) > 8:
        add("warn", "colors", "many hard-coded colours; use the theme variables so dark mode works", 8)
    if re.search(r"<script\b(?![^>]*\bsrc=)[^>]*>", parts["html"], flags=re.I):
        add("warn", "inline-script", "inline <script> in html; scripts belong in jsFunctions/jsExpressions", 5)
    if re.search(r"\b(alert|confirm|prompt)\s*\(", parts["jsFunctions"] + parts["jsExpressions"]):
        add("warn", "dialog", "uses alert/confirm/prompt, which the frame does not show", 10)
    if "maplibregl" in everything and "tiles.openfreemap.org" not in everything:
        add("warn", "map-style", "MapLibre without an OpenFreeMap style has no tiles", 15)

    score = max(0, 100 - sum(f["cost"] for f in findings))
    return {"score": score, "findings": findings}


def report(run_dir):
    """Prints one row per prompt of a run and the totals; returns the rows."""
    rows = []
    for path in sorted(Path(run_dir).glob("*.json")):
        if path.name == "summary.json":
            continue
        run = json.loads(path.read_text())
        pages = run.get("pages", [])
        scores = [lint_page(p) for p in pages]
        got = "page" if pages else "text"
        best = max((s["score"] for s in scores), default=None)
        rows.append({
            "id": run["id"], "expect": run["expect"], "got": got, "guide": run.get("called_guide", False),
            "score": best, "ok": got == run["expect"], "findings": [f["rule"] for s in scores for f in s["findings"]],
            "cost": run.get("cost_usd"), "error": run.get("error"),
        })
    width = max((len(r["id"]) for r in rows), default=4)
    print(f"{'id':<{width}}  expect  got   guide  score  findings")
    for r in rows:
        score = "-" if r["score"] is None else str(r["score"])
        print(f"{r['id']:<{width}}  {r['expect']:<6}  {r['got']:<4}  {'yes' if r['guide'] else 'no':<5}  {score:>5}  {', '.join(sorted(set(r['findings']))) or r['error'] or ''}")
    pages = [r for r in rows if r["expect"] == "page"]
    hit = sum(1 for r in rows if r["ok"])
    scored = [r["score"] for r in rows if r["score"] is not None]
    summary = {
        "prompts": len(rows), "right_choice": hit,
        "pages_expected": len(pages), "pages_made": sum(1 for r in pages if r["got"] == "page"),
        "guide_called": sum(1 for r in pages if r["guide"]),
        "mean_score": round(sum(scored) / len(scored), 1) if scored else None,
        "cost_usd": round(sum(r["cost"] or 0 for r in rows), 4),
    }
    print("\n" + json.dumps(summary))
    Path(run_dir, "summary.json").write_text(json.dumps({"summary": summary, "rows": rows}, ensure_ascii=False, indent=2))
    return rows


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "report":
        report(sys.argv[2])
    elif len(sys.argv) == 3 and sys.argv[1] == "page":
        print(json.dumps(lint_page(json.loads(Path(sys.argv[2]).read_text())), indent=2))
    else:
        print(__doc__)
        sys.exit(2)
