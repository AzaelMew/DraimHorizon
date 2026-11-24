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


class PinCheckTests(unittest.TestCase):
    compat = {
        "horizon_repository": "https://example.test/horizon", "horizon_revision": "a" * 40, "horizon_channel": "nightly",
        "sunrise": {"repository": "https://example.test/sunrise", "revision": "b" * 40, "channel": "nightly"},
    }

    def check(self, horizon, sunrise, strict):
        nightly = {"https://example.test/horizon": horizon, "https://example.test/sunrise": sunrise}
        with patch.object(compatibility, "latest_nightly", side_effect=lambda repository: nightly[repository]):
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

    def test_sunrise_only_never_contacts_horizon(self):
        with patch.object(compatibility, "latest_nightly", return_value="b" * 40) as query:
            lines, errors = compatibility.check_pins(self.compat, True, sunrise_only=True)
        query.assert_called_once_with(self.compat["sunrise"]["repository"])
        self.assertEqual(errors, [])
        self.assertIn("Sunrise official Nightly: " + "b" * 40, lines)

    def test_sunrise_only_still_rejects_a_stale_sunrise_pin(self):
        with patch.object(compatibility, "latest_nightly", return_value="c" * 40) as query:
            _, errors = compatibility.check_pins(self.compat, True, sunrise_only=True)
        query.assert_called_once_with(self.compat["sunrise"]["repository"])
        self.assertEqual(len(errors), 1)
        self.assertIn("Sunrise pin", errors[0])


class DevPinTests(unittest.TestCase):
    dev, nightly = "d" * 40, "e" * 40

    def check(self, tags):
        compat = {
            "horizon_repository": "https://example.test/horizon", "horizon_revision": "a" * 40, "horizon_channel": "nightly",
            "sunrise": {"repository": "https://example.test/sunrise", "revision": self.dev, "channel": "dev"},
        }
        nightly = {"https://example.test/horizon": "a" * 40, "https://example.test/sunrise": self.nightly}
        refs = "".join(f"{commit}\trefs/tags/{name}\n" for commit, name in tags)
        with patch.object(compatibility, "latest_nightly", side_effect=lambda repository: nightly[repository]), \
                patch.object(compatibility.subprocess, "check_output", return_value=refs):
            return compatibility.check_pins(compat, True)

    def test_published_dev_build_newer_than_the_nightly_passes(self):
        lines, errors = self.check([(self.dev, "dev-2026-11-24-ddddddd"), (self.nightly, "nightly-source/2026-11-20-eeeeeee")])
        self.assertEqual(errors, [])
        self.assertTrue(any(line.startswith("Sunrise Dev build") for line in lines))

    def test_unpublished_dev_build_fails(self):
        _, errors = self.check([("f" * 40, "dev-2026-11-24-fffffff"), (self.nightly, "nightly-source/2026-11-20-eeeeeee")])
        self.assertEqual(len(errors), 1)
        self.assertIn("not a published Dev build", errors[0])

    def test_annotated_dev_tag_names_its_commit_through_the_peeled_line(self):
        _, errors = self.check([("c" * 40, "dev-2026-11-24-ddddddd"), (self.dev, "dev-2026-11-24-ddddddd^{}"),
                                (self.nightly, "nightly-source/2026-11-20-eeeeeee")])
        self.assertEqual(errors, [])

    def test_the_nightly_commit_itself_is_not_a_dev_pin(self):
        self.dev = self.nightly
        _, errors = self.check([(self.nightly, "dev-2026-11-24-eeeeeee"), (self.nightly, "nightly-source/2026-11-20-eeeeeee")])
        self.assertEqual(len(errors), 1)
        self.assertIn("is the official Nightly", errors[0])

    def test_dev_build_fails_once_a_nightly_as_new_is_published(self):
        for date in ("2026-11-24", "2026-11-25"):
            with self.subTest(nightly=date):
                _, errors = self.check([(self.dev, "dev-2026-11-24-ddddddd"), (self.nightly, f"nightly-source/{date}-eeeeeee")])
                self.assertEqual(len(errors), 1)
                self.assertIn("not newer than official Nightly", errors[0])


if __name__ == "__main__":
    unittest.main()
