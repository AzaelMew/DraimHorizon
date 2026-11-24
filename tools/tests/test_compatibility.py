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


class SunriseCandidateTests(unittest.TestCase):
    def test_nightly_and_only_the_newest_dev_day_compete(self):
        a, b, c, d = ("a" * 40, "b" * 40, "c" * 40, "d" * 40)
        refs = (f"{a}\trefs/tags/nightly\n{b}\trefs/tags/dev-2026-11-23-bbbbbbb\n"
                f"{c}\trefs/tags/dev-2026-11-24-ccccccc\n{d}\trefs/tags/dev-2026-11-24-ddddddd\n")
        self.assertEqual(compatibility.sunrise_candidates(refs), {a: "nightly", c: "dev", d: "dev"})

    def test_annotated_dev_tag_resolves_to_its_commit(self):
        tag, source = "e" * 40, "f" * 40
        refs = f"{tag}\trefs/tags/dev-2026-11-24-fffffff\n{source}\trefs/tags/dev-2026-11-24-fffffff^{{}}\n"
        self.assertEqual(compatibility.sunrise_candidates(refs), {source: "dev"})

    def test_no_build_tags_is_rejected(self):
        with self.assertRaises(ValueError):
            compatibility.sunrise_candidates(f"{'a' * 40}\trefs/heads/main\n")


class PinCheckTests(unittest.TestCase):
    compat = {
        "horizon_repository": "https://example.test/horizon", "horizon_revision": "a" * 40, "horizon_channel": "nightly",
        "sunrise": {"repository": "https://example.test/sunrise", "revision": "b" * 40, "channel": "nightly"},
    }

    def check(self, horizon, sunrise, strict):
        with patch.object(compatibility, "latest_nightly", return_value=horizon), \
                patch.object(compatibility, "latest_sunrise", return_value=(sunrise, "nightly")):
            return compatibility.check_pins(self.compat, strict)

    def test_current_pins_pass_and_both_are_reported(self):
        lines, errors = self.check("a" * 40, "b" * 40, True)
        self.assertEqual(errors, [])
        self.assertEqual(len(lines), 2)

    def test_stale_horizon_is_always_an_error(self):
        _, errors = self.check("c" * 40, "b" * 40, False)
        self.assertEqual(len(errors), 1)
        self.assertIn("Horizon pin", errors[0])

    def test_stale_sunrise_warns_unless_strict(self):
        lines, errors = self.check("a" * 40, "c" * 40, False)
        self.assertEqual(errors, [])
        self.assertTrue(any(line.startswith("warning: Sunrise pin") for line in lines))
        _, errors = self.check("a" * 40, "c" * 40, True)
        self.assertEqual(len(errors), 1)

    def test_both_stale_pins_are_reported_together(self):
        _, errors = self.check("c" * 40, "d" * 40, True)
        self.assertEqual(len(errors), 2)


if __name__ == "__main__":
    unittest.main()
