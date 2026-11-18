"""The flat packages the current Horizon and Sunrise launchers install (the old layout is refused)."""
import argparse
import contextlib
import importlib.util
import io
from pathlib import Path
import shutil
import tempfile
import tomllib
import unittest
import zipfile

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

    def test_a_sunrise_package_is_the_launcher_built_smod_project(self):
        # Generate into a scratch tree: the real target/ may be in use by a launcher build.
        spec = importlib.util.spec_from_file_location("sunrise", package_flat.ROOT / "tools" / "sunrise.py")
        sunrise = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(sunrise)
        calls = []

        def run_smod(command, **kwargs):
            calls.append(command)
            self.assertEqual(Path(command[1]).name, "sunrise.py")
            self.assertEqual(command[2], "smod")
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                sunrise.command_smod(argparse.Namespace(sunrise=None))

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "target").mkdir()
            shutil.copyfile(ROOT / "LICENSE", root / "LICENSE")
            shutil.copyfile(ROOT / "Cargo.toml", root / "Cargo.toml")
            saved = package_flat.ROOT, package_flat.subprocess.run, sunrise.OUT
            package_flat.ROOT, package_flat.subprocess.run, sunrise.OUT = root, run_smod, root / "target" / "sunrise"
            try:
                project = package_flat.sunrise_project()
                artifact = package_flat.pack("sunrise", project)
            finally:
                package_flat.ROOT, package_flat.subprocess.run, sunrise.OUT = saved
            self.assertEqual(len(calls), 1)
            self.assertEqual(project, root / "target" / "sunrise" / "smod" / "draimhorizon")
            manifest = tomllib.loads((project / "sunrise-mod.toml").read_text(encoding="utf-8"))
            self.assertIn("sunrise", manifest["native"])
            self.assertNotIn("horizon", manifest["native"])
            self.assertTrue(manifest["native"]["mixin"])
            for row in manifest["native"]["mixin"]:
                self.assertTrue((project / row["helper"]).is_file(), row["id"])
            for row in manifest["native"]["module"]:
                self.assertTrue((project / row["source"]).is_file(), row["id"])
            cargo = tomllib.loads((project / "Cargo.toml").read_text(encoding="utf-8"))
            # The Sunrise launcher refuses these as mod-crate registry dependencies.
            for refused in ("ring", "rustls", "ureq"):
                self.assertNotIn(refused, cargo.get("dependencies", {}))
            self.assertIn("#[mod_api::sunrise_mod]", (project / "src" / "lib.rs").read_text(encoding="utf-8"))
            for name in package_flat.LICENSES.values():
                self.assertTrue((project / name).is_file(), name)
            self.assertLessEqual(len(package_flat.package_files(project)), package_flat.MAX_FILES)
            self.assertEqual(artifact, root / "target" / f"draimhorizons-{manifest['version']}.smod")
            with zipfile.ZipFile(artifact) as archive:
                names = archive.namelist()
                self.assertEqual(names[0], "sunrise-mod.toml")
                self.assertNotIn("horizon-mod.toml", names)
                self.assertEqual(tomllib.loads(archive.read("sunrise-mod.toml").decode("utf-8")), manifest)
            self.assertTrue(artifact.with_suffix(".smod.sha256").is_file())


if __name__ == "__main__":
    unittest.main()
