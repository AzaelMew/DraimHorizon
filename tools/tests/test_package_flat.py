"""The flat packages the current Horizon and Sunrise launchers install (the old layout is refused)."""
import argparse
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import tempfile
import tomllib
import unittest
import zipfile

TOOLS = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("package_flat", TOOLS / "package_flat.py")
package_flat = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package_flat)

ROOT = package_flat.ROOT
ENTRY = {"horizon": "horizon_mod", "sunrise": "sunrise_mod"}
EXTENSION = {"horizon": "hmod", "sunrise": "smod"}
# The launchers refuse these as registry dependencies of a mod crate.
REFUSED_DEPENDENCIES = ("ring", "rustls", "ureq")


def load_sunrise(platform):
    """tools/sunrise.py selects its product when it is imported, so load a copy per platform."""
    saved = os.environ.get("DRAIM_TARGET")
    os.environ["DRAIM_TARGET"] = platform
    try:
        spec = importlib.util.spec_from_file_location(f"sunrise_{platform}", TOOLS / "sunrise.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    finally:
        if saved is None:
            os.environ.pop("DRAIM_TARGET")
        else:
            os.environ["DRAIM_TARGET"] = saved
    return module


class FlatPackageTests(unittest.TestCase):
    """Generate each platform's project into a scratch tree: the real target/ may be in use by a launcher build."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.packages = {}
        for platform in ("horizon", "sunrise"):
            sunrise = load_sunrise(platform)
            root = Path(cls.tmp.name) / platform
            (root / "target").mkdir(parents=True)
            shutil.copyfile(ROOT / "LICENSE", root / "LICENSE")
            shutil.copyfile(ROOT / "Cargo.toml", root / "Cargo.toml")

            def run_smod(command, _sunrise=sunrise, **kwargs):
                with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                    _sunrise.command_smod(argparse.Namespace(sunrise=None))

            saved = package_flat.ROOT, package_flat.subprocess.run, sunrise.OUT
            package_flat.ROOT, package_flat.subprocess.run, sunrise.OUT = root, run_smod, root / "target" / platform
            try:
                project = package_flat.generated_project(platform)
                artifact = package_flat.pack(platform, project)
            finally:
                package_flat.ROOT, package_flat.subprocess.run, sunrise.OUT = saved
            manifest = tomllib.loads((project / package_flat.PLATFORMS[platform]["manifest"]).read_text(encoding="utf-8"))
            cls.packages[platform] = dict(root=root, project=project, artifact=artifact, manifest=manifest)
        cls.compat = json.loads((ROOT / "compatibility.json").read_text(encoding="utf-8"))
        cls.bundle = tomllib.loads((package_flat.BUNDLE / "horizon-mixin.toml").read_text(encoding="utf-8"))

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def each(self):
        for platform, package in self.packages.items():
            with self.subTest(platform=platform):
                yield platform, package

    def test_the_old_layout_is_not_produced(self):
        for _, package in self.each():
            names = {p.name for p in package["project"].rglob("*")}
            self.assertNotIn("horizon-mixin.toml", names)
            self.assertNotIn("horizon-native.json", names)
            self.assertFalse((package["project"] / "native").exists() or (package["project"] / "rust").exists())

    def test_every_hook_and_module_is_in_the_package_with_its_source(self):
        for _, package in self.each():
            native = package["manifest"]["native"]
            self.assertTrue(native["mixin"])
            for row in native["mixin"]:
                self.assertTrue((package["project"] / row["helper"]).is_file(), row["id"])
                self.assertTrue(row["helper"].startswith("src/mixins/"))
            for row in native["module"]:
                self.assertTrue((package["project"] / row["source"]).is_file(), row["id"])
                self.assertTrue(row["source"].startswith("src/modules/"))

    def test_no_shared_hook_is_lost_without_a_reason(self):
        """A platform may drop a hook, but only through platforms/<platform> with a stated reason."""
        for platform, package in self.each():
            sunrise = load_sunrise(platform)
            rules = sunrise.load_platform().get("injection", {})
            dropped = {i for i, rule in rules.items() if rule.get("drop")}
            for injection in dropped:
                self.assertTrue(rules[injection].get("reason"), f"{platform}: {injection} is dropped without a reason")
            skipped = set(sunrise.load_platform().get("smod", {}).get("skip", {}))
            carried = {row["id"] for row in package["manifest"]["native"]["mixin"]}
            shared = {row["id"] for row in self.bundle["injection"]}
            self.assertEqual(shared - carried - dropped - skipped, set())
            self.assertFalse(dropped - shared)

    def test_identity_and_pins_come_from_the_repository(self):
        horizon = self.packages["horizon"]["manifest"]
        sunrise = self.packages["sunrise"]["manifest"]
        for manifest in (horizon, sunrise):
            self.assertEqual(manifest["id"], "draimhorizon")
            self.assertEqual(manifest["version"], package_flat.project_version())
            self.assertEqual(manifest["capabilities"], ["registries"])
        self.assertEqual(horizon["native"]["horizon"], self.compat["horizon_revision"])
        self.assertEqual(horizon["native"]["mixins"], self.compat["horizon_mixins_revision"])
        self.assertNotIn("sunrise", horizon["native"])
        self.assertEqual(sunrise["native"]["sunrise"], self.compat["sunrise"]["revision"])
        self.assertEqual(sunrise["native"]["mixins"], self.compat["sunrise"]["mixins_revision"])
        self.assertNotIn("horizon", sunrise["native"])

    def test_the_mod_crate_is_the_launcher_built_project(self):
        for platform, package in self.each():
            lib = (package["project"] / "src" / "lib.rs").read_text(encoding="utf-8")
            self.assertIn(f"#[mod_api::{ENTRY[platform]}]", lib)
            cargo = tomllib.loads((package["project"] / "Cargo.toml").read_text(encoding="utf-8"))
            self.assertEqual(cargo["package"]["name"], "mod-draimhorizon")
            self.assertIn(f"{platform}-mod-api", cargo["dependencies"])
            for refused in REFUSED_DEPENDENCIES:
                self.assertNotIn(refused, cargo["dependencies"])

    def test_licence_texts_travel_as_root_files_and_the_package_fits_the_limits(self):
        for _, package in self.each():
            for name in package_flat.LICENSES.values():
                self.assertTrue((package["project"] / name).is_file(), name)
                self.assertTrue(name.startswith(("LICENSE", "COPYING", "NOTICE")))
            self.assertLessEqual(len(package_flat.package_files(package["project"])), package_flat.MAX_FILES)

    def test_the_packed_file_is_named_and_laid_out_for_its_launcher(self):
        for platform, package in self.each():
            manifest_name = package_flat.PLATFORMS[platform]["manifest"]
            version = package["manifest"]["version"]
            self.assertEqual(package["artifact"], package["root"] / "target" / f"draimhorizons-{version}.{EXTENSION[platform]}")
            with zipfile.ZipFile(package["artifact"]) as archive:
                names = archive.namelist()
                self.assertEqual(names[0], manifest_name)
                other = "sunrise-mod.toml" if platform == "horizon" else "horizon-mod.toml"
                self.assertNotIn(other, names)
                self.assertEqual(tomllib.loads(archive.read(manifest_name).decode("utf-8")), package["manifest"])
            self.assertTrue(package["artifact"].with_suffix(f".{EXTENSION[platform]}.sha256").is_file())

    def test_the_horizon_mod_crate_carries_the_relay_crypto(self):
        horizon = self.packages["horizon"]
        cargo = tomllib.loads((horizon["project"] / "Cargo.toml").read_text(encoding="utf-8"))
        for name in ("curve25519-dalek", "chacha20poly1305", "hkdf", "sha2"):
            self.assertIn(name, cargo["dependencies"])
        lib = (horizon["project"] / "src" / "lib.rs").read_text(encoding="utf-8")
        for name in ("chacha20poly1305", "curve25519_dalek", "hkdf", "sha2"):
            self.assertIn(f"pub use {name};", lib)
        relay = (horizon["project"] / "src" / "modules" / "claim_borders_relay_client.rs").read_text(encoding="utf-8")
        self.assertIn("mod_draimhorizon::chacha20poly1305::", relay)
        self.assertNotRegex(relay, r"(?<![\w:])chacha20poly1305::")
        lock = (horizon["project"] / "Cargo.lock").read_text(encoding="utf-8")
        self.assertIn('name = "mod-draimhorizon"', lock)

    def test_horizon_mod_does_not_redeclare_game_modules_provided_by_the_sdk(self):
        game_modules = {"api_values", "minecarts", "execution_role", "entity_culling"}
        modules = self.packages["horizon"]["manifest"]["native"]["module"]
        for row in modules:
            if row["file"] == "crates/horizon-client/src/game.rs":
                self.assertNotIn(row["name"], game_modules)


if __name__ == "__main__":
    unittest.main()
