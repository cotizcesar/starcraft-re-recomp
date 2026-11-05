#!/usr/bin/env python3
"""Unit tests for scripts/vault_check.py (run: python3 -m unittest scripts/test_vault_check.py)."""
from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import vault_check  # noqa: E402

LOG = """# Log
## 2026-11-03 (81) — before the vault
## 2026-11-04 (82) — first covered
## 2026-11-05 (83) — second
"""


def note(name: str, area: str = "campaign", log: str = '["2026-11-04 (82)"]', open_count: int = 1, body: str = "- [ ] todo\n- [x] done\n") -> vault_check.Note:
    text = (
        f'---\ntitle: "t"\ndate: 2026-11-05\ntags: [session]\ntype: session\n'
        f"source: opencode\narea: {area}\nlog: {log}\nopen: {open_count}\n---\n\n{body}"
    )
    return vault_check.parse_note(Path(name), text)


class VaultCheckTests(unittest.TestCase):
    def setUp(self):
        self.keys = vault_check.log_keys(LOG)
        self.first = vault_check.FIRST_COVERED
        vault_check.FIRST_COVERED = "2026-11-04 (82)"

    def tearDown(self):
        vault_check.FIRST_COVERED = self.first

    def test_log_keys_keep_file_order(self):
        self.assertEqual(self.keys, ["2026-11-03 (81)", "2026-11-04 (82)", "2026-11-05 (83)"])

    def test_consistent_vault_passes(self):
        notes = [note("a.md"), note("b.md", log='["2026-11-05 (83)"]', open_count=0, body="- [x] x\n")]
        self.assertEqual(vault_check.check(notes, self.keys), [])

    def test_uncovered_entry_after_first_covered_fails(self):
        errors = vault_check.check([note("a.md")], self.keys)
        self.assertEqual(errors, ["session-log 2026-11-05 (83) has no vault note (add its key to a note's log)"])

    def test_stale_open_count_fails(self):
        notes = [note("a.md", log='["2026-11-04 (82)", "2026-11-05 (83)"]', open_count=3)]
        self.assertIn("a.md: open is 3 but the note has 1 unchecked items (run --write)", vault_check.check(notes, self.keys))

    def test_unknown_area_and_log_key_fail(self):
        notes = [note("a.md", area="misc", log='["2026-11-04 (82)", "2026-11-05 (83)", "2026-11-06 (84)"]')]
        errors = vault_check.check(notes, self.keys)
        self.assertTrue(any("unknown area 'misc'" in e for e in errors))
        self.assertTrue(any("'2026-11-06 (84)' not in session-log.md" in e for e in errors))

    def test_missing_fields_fail(self):
        bare = vault_check.parse_note(Path("x.md"), "---\ntitle: t\n---\n")
        errors = vault_check.check([bare], self.keys)
        self.assertTrue(errors[0].startswith("x.md: missing frontmatter date"))

    def test_write_fixes_open_count(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "a.md"
            path.write_text('---\ntitle: t\nopen: 9\n---\n- [ ] a\n- [ ] b\n', encoding="utf-8")
            notes = vault_check.load_notes(Path(tmp))
            self.assertEqual(vault_check.write_open_counts(notes), 1)
            self.assertIn("open: 2\n", path.read_text(encoding="utf-8"))

    def test_list_open_groups_by_area(self):
        out = vault_check.list_open([note("a.md"), note("b.md", area="tooling")], "tooling")
        self.assertEqual(out, "## tooling\n[[b]]\n  - [ ] todo")


if __name__ == "__main__":
    unittest.main()
