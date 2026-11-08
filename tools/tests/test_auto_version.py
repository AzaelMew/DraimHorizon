import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import auto_version as av  # noqa: E402

CARGO = '[package]\nname = "x"\nversion = "{}"\n'


class AutoVersionTests(unittest.TestCase):
    def test_bump_only_when_version_is_unchanged(self):
        self.assertTrue(av.needs_bump(CARGO.format("0.29.3"), CARGO.format("0.29.3")))
        self.assertFalse(av.needs_bump(CARGO.format("0.29.3"), CARGO.format("0.29.4")))

    def test_changelog_entry_goes_first_and_is_not_duplicated(self):
        text = "# История версий\n\n## 0.29.3\n\n- old\n"
        out = av.add_changelog(text, "0.29.4", "Fix\nthings", "7")
        self.assertTrue(out.startswith("# История версий\n\n## 0.29.4\n\n- Fix things (#7)."))
        self.assertIn("## 0.29.3\n\n- old", out)
        self.assertEqual(av.add_changelog(out, "0.29.4", "Other", "8"), out)

    def test_release_notes_name_the_version(self):
        self.assertIn("DraimHorizon 0.29.4 - Fix", av.release_notes("0.29.4", " Fix "))


if __name__ == "__main__":
    unittest.main()
