import json
import tempfile
import unittest
from pathlib import Path

import ui_lint

EXAMPLES = Path(__file__).resolve().parent.parent.parent / "sidecar" / "src" / "ui_examples"


def page(**kw):
    base = {"initialHeight": 300, "css": "", "html": '<div class="fx-card fx-stack"><h1 class="fx-title">x</h1></div>', "jsFunctions": "", "jsExpressions": ""}
    base.update(kw)
    return base


def rules(result):
    return {f["rule"] for f in result["findings"]}


class LintTests(unittest.TestCase):
    def test_the_hosts_come_from_the_policy_source(self):
        hosts = ui_lint.allowed_hosts()
        self.assertEqual(hosts, {"cdnjs.cloudflare.com", "cdn.jsdelivr.net", "unpkg.com", "tiles.openfreemap.org", "upload.wikimedia.org", "thumb.wikimedia.org", "commons.wikimedia.org"})
        self.assertIn("fx-card", ui_lint.kit_classes())

    def test_every_reviewed_example_scores_well_with_no_errors(self):
        for path in sorted(EXAMPLES.glob("*.json")):
            if path.name == "kit.json":
                continue
            data = json.loads(path.read_text())
            result = ui_lint.lint_page(data["page"])
            self.assertGreaterEqual(result["score"], 90, (data["name"], result["findings"]))
            self.assertFalse([f for f in result["findings"] if f["severity"] == "error"], data["name"])

    def test_a_clean_page_scores_100(self):
        self.assertEqual(ui_lint.lint_page(page())["score"], 100)

    def test_a_host_outside_the_allowlist_is_an_error(self):
        result = ui_lint.lint_page(page(html='<img src="https://images.unsplash.com/a.jpg" onerror="this.remove()">'))
        self.assertIn("host", rules(result))
        self.assertLessEqual(result["score"], 75)

    def test_blocked_map_providers_are_errors(self):
        result = ui_lint.lint_page(page(jsFunctions="L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png')"))
        self.assertIn("blocked-map", rules(result))

    def test_a_page_that_ignores_the_kit_and_uses_vh_is_marked_down(self):
        result = ui_lint.lint_page(page(html="<div style='height:100vh'>hi</div>"))
        self.assertTrue({"kit", "vh"} <= rules(result))

    def test_an_image_without_onerror_and_an_unknown_kit_class(self):
        result = ui_lint.lint_page(page(html='<div class="fx-card fx-stack fx-row fx-nope"><img src="https://commons.wikimedia.org/wiki/Special:FilePath/A.jpg"></div>'))
        self.assertTrue({"img-onerror", "kit-unknown"} <= rules(result))

    def test_dialogs_inline_scripts_height_and_empty_pages(self):
        result = ui_lint.lint_page(page(initialHeight=2000, html='<div class="fx-card fx-stack fx-row"><script>x()</script></div>', jsFunctions="alert(1)"))
        self.assertTrue({"height", "inline-script", "dialog"} <= rules(result))
        self.assertEqual(ui_lint.lint_page(page(html=""))["score"], 0)

    def test_maplibre_needs_an_openfreemap_style(self):
        result = ui_lint.lint_page(page(jsFunctions="new maplibregl.Map({container:'m'})"))
        self.assertIn("map-style", rules(result))

    def test_report_counts_choices_pages_and_scores(self):
        with tempfile.TemporaryDirectory() as tmp:
            good = {"id": "a", "expect": "page", "pages": [page()], "called_guide": True, "cost_usd": 0.1}
            text_ok = {"id": "b", "expect": "text", "pages": []}
            missed = {"id": "c", "expect": "page", "pages": []}
            for run in (good, text_ok, missed):
                Path(tmp, run["id"] + ".json").write_text(json.dumps(run))
            rows = ui_lint.report(tmp)
            summary = json.loads(Path(tmp, "summary.json").read_text())["summary"]
        self.assertEqual([r["ok"] for r in rows], [True, True, False])
        self.assertEqual(summary["right_choice"], 2)
        self.assertEqual(summary["pages_made"], 1)
        self.assertEqual(summary["guide_called"], 1)
        self.assertEqual(summary["mean_score"], 100.0)


if __name__ == "__main__":
    unittest.main()
