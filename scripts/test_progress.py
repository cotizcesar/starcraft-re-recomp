#!/usr/bin/env python3
"""Unit tests for scripts/progress.py (run: python3 -m unittest scripts/test_progress.py)."""
from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import progress  # noqa: E402

DOC = """# P
<!-- progress:summary:start -->
stale
<!-- progress:summary:end -->

## Alpha area
<!-- area: weight=60 -->
- [x] one
- [ ] two

## Beta (b)
<!-- area: weight=40 -->
- [x] three
"""


class ProgressTests(unittest.TestCase):
    def test_counts_and_weighted_overall(self):
        areas = progress.parse(DOC)
        self.assertEqual([(a.done, a.total) for a in areas], [(1, 2), (1, 1)])
        self.assertEqual([a.percent for a in areas], [50, 100])
        self.assertEqual(progress.overall(areas), 70)  # 0.6*50 + 0.4*100

    def test_anchor_matches_github_slug(self):
        self.assertEqual(progress.Area("Beta (b)", 1).anchor, "beta-b")
        self.assertEqual(progress.Area("Playable client (`sc_view`)", 1).anchor, "playable-client-sc_view")

    def test_weights_must_sum_to_100(self):
        with self.assertRaises(ValueError):
            progress.parse(DOC.replace("weight=40", "weight=30"))

    def test_area_without_items_is_rejected(self):
        with self.assertRaises(ValueError):
            progress.parse(DOC.replace("- [x] three\n", ""))

    def test_check_fails_when_stale_and_passes_after_write(self):
        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "PROGRESS.md"
            f.write_text(DOC, encoding="utf-8")
            self.assertEqual(progress.main(["--file", str(f), "--check"]), 1)
            self.assertEqual(progress.main(["--file", str(f), "--write", "--date", "2026-01-01"]), 0)
            self.assertEqual(progress.main(["--file", str(f), "--check"]), 0)
            self.assertIn("**70 %**", f.read_text(encoding="utf-8"))
            # Ticking an item without regenerating makes the check fail again.
            f.write_text(f.read_text(encoding="utf-8").replace("- [ ] two", "- [x] two"), encoding="utf-8")
            self.assertEqual(progress.main(["--file", str(f), "--check"]), 1)

    def test_repository_progress_file_is_current(self):
        self.assertEqual(progress.main(["--check"]), 0)


if __name__ == "__main__":
    unittest.main()
