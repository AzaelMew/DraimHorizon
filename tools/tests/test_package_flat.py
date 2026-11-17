"""The flat packages the current Horizon and Sunrise launchers install (the old layout is refused)."""
import importlib.util
from pathlib import Path
import tempfile
import tomllib
import unittest

spec = importlib.util.spec_from_file_location("package_flat", Path(__file__).resolve().parents[1] / "package_flat.py")
package_flat = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package_flat)

ROOT = package_flat.ROOT


class FlatPackageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.project = package_flat.build("horizon", package_flat.BUNDLE, Path(cls.tmp.name) / "horizon")
        cls.manifest = tomllib.loads((cls.project / "horizon-mod.toml").read_text(encoding="utf-8"))
        cls.old = tomllib.loads((package_flat.BUNDLE / "horizon-mixin.toml").read_text(encoding="utf-8"))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_the_old_layout_is_not_produced(self):
        names = {p.name for p in self.project.rglob("*")}
        self.assertNotIn("horizon-mixin.toml", names)
        self.assertNotIn("horizon-native.json", names)
        self.assertFalse((self.project / "native").exists() or (self.project / "rust").exists())

    def test_every_hook_and_module_is_carried_over_with_its_source(self):
        native = self.manifest["native"]
        self.assertEqual(len(native["mixin"]), len(self.old["injection"]))
        self.assertEqual(len(native["module"]), len(self.old["module"]))
        for row in native["mixin"]:
            self.assertTrue((self.project / row["helper"]).is_file(), row["id"])
            self.assertTrue(row["helper"].startswith("src/mixins/"))
        for row in native["module"]:
            self.assertTrue((self.project / row["source"]).is_file(), row["id"])
            self.assertTrue(row["source"].startswith("src/modules/"))

    def test_identity_and_pins_come_from_the_repository(self):
        compat = __import__("json").loads((ROOT / "compatibility.json").read_text(encoding="utf-8"))
        self.assertEqual(self.manifest["id"], "draimhorizon")
        self.assertEqual(self.manifest["version"], package_flat.project_version())
        self.assertEqual(self.manifest["native"]["horizon"], compat["horizon_revision"])
        self.assertEqual(self.manifest["native"]["mixins"], compat["sunrise"]["mixins_revision"])
        self.assertEqual(self.manifest["capabilities"], ["registries"])

    def test_the_relay_client_becomes_the_crate_root(self):
        lib = (self.project / "src" / "lib.rs").read_text(encoding="utf-8")
        self.assertIn("#[mod_api::horizon_mod]", lib)
        self.assertIn("pub mod translation;", lib)
        for path in (self.project / "src").rglob("*.rs"):
            self.assertNotIn("claim_borders_relay_client", path.read_text(encoding="utf-8"), path.name)
        cargo = tomllib.loads((self.project / "Cargo.toml").read_text(encoding="utf-8"))
        self.assertEqual(cargo["package"]["name"], "mod-draimhorizon")
        self.assertIn("horizon-mod-api", cargo["dependencies"])
        self.assertIn("ureq", cargo["dependencies"])

    def test_licence_texts_travel_as_root_files_and_the_package_fits_the_limits(self):
        for name in package_flat.LICENSES.values():
            self.assertTrue((self.project / name).is_file(), name)
            self.assertTrue(name.startswith(("LICENSE", "COPYING", "NOTICE")))
        files = package_flat.package_files(self.project)
        self.assertLessEqual(len(files), package_flat.MAX_FILES)

    def test_a_sunrise_package_uses_its_own_names(self):
        import subprocess
        import sys
        subprocess.run([sys.executable, str(ROOT / "tools" / "sunrise.py"), "generate"], check=True, stdout=subprocess.DEVNULL)
        with tempfile.TemporaryDirectory() as tmp:
            project = package_flat.build("sunrise", ROOT / "target" / "sunrise" / "mods" / "draimhorizon", Path(tmp) / "sunrise")
            manifest = tomllib.loads((project / "sunrise-mod.toml").read_text(encoding="utf-8"))
            self.assertIn("sunrise", manifest["native"])
            self.assertNotIn("horizon", manifest["native"])
            self.assertEqual(len(manifest["native"]["mixin"]), 75)
            self.assertIn("#[mod_api::sunrise_mod]", (project / "src" / "lib.rs").read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
