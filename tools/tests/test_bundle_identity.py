"""The launcher ID may change; existing user data must keep its old namespace."""
from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUNDLE = ROOT / "mixins/draimhorizon"


class BundleIdentityTests(unittest.TestCase):
    def test_archive_and_runtime_identity_agree_with_preserved_storage(self):
        manifest = tomllib.loads((BUNDLE / "horizon-mixin.toml").read_text(encoding="utf-8"))
        runtime = (BUNDLE / "runtime.rs").read_text(encoding="utf-8")
        identity = re.search(r'pub const ID: &str = "([^"]+)";', runtime).group(1)
        storage = re.search(r'pub const SETTINGS_ID: &str = "([^"]+)";', runtime).group(1)
        self.assertEqual(identity, manifest["mod"]["id"])
        self.assertEqual(identity, BUNDLE.name)
        self.assertEqual(storage, "claim_borders")
        self.assertNotEqual(identity, storage)

    def test_all_native_settings_and_user_data_calls_use_legacy_namespace(self):
        calls = 0
        pattern = r"\.native_(?:setting|set_setting|data_dir)\s*\(\s*([^,)]*)"
        for source in BUNDLE.rglob("*.rs"):
            for call in re.finditer(pattern, source.read_text(encoding="utf-8")):
                calls += 1
                with self.subTest(source=str(source.relative_to(ROOT)), call=call.group(0)):
                    self.assertIn(call.group(1).strip(), (
                        "claims::SETTINGS_ID", "crate::game::claim_borders::SETTINGS_ID"))
        self.assertGreater(calls, 0)


if __name__ == "__main__":
    unittest.main()
