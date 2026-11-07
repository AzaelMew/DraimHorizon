"""Protect the published Nightly source selection from a regression to main."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("compatibility", Path(__file__).resolve().parents[1] / "compatibility.py")
compatibility = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compatibility)


class NightlyRevisionTests(unittest.TestCase):
    def test_published_tag_wins_over_main(self):
        main, nightly = "a" * 40, "b" * 40
        self.assertEqual(compatibility.resolve_nightly_revision(
            f"{main}\trefs/heads/main\n{nightly}\trefs/tags/nightly\n"), nightly)

    def test_annotated_tag_uses_source_commit_not_tag_object(self):
        tag, source = "c" * 40, "d" * 40
        self.assertEqual(compatibility.resolve_nightly_revision(
            f"{source}\trefs/tags/nightly^{{}}\n{tag}\trefs/tags/nightly\n"), source)

    def test_missing_or_invalid_tag_fails_without_main_fallback(self):
        for refs in ("", f"{'a' * 40}\trefs/heads/main\n", "invalid\trefs/tags/nightly\n"):
            with self.subTest(refs=refs), self.assertRaises(ValueError):
                compatibility.resolve_nightly_revision(refs)

    def test_remote_query_only_requests_official_nightly(self):
        with patch.object(compatibility.subprocess, "check_output", return_value=f"{'e' * 40}\trefs/tags/nightly\n") as query:
            self.assertEqual(compatibility.latest_nightly("https://example.test/horizon"), "e" * 40)
        self.assertEqual(query.call_args.args[0], ["git", "ls-remote", "https://example.test/horizon", "refs/tags/nightly", "refs/tags/nightly^{}"])


if __name__ == "__main__":
    unittest.main()
