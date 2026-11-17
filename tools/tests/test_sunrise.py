"""The Sunrise bundle generator: renames, module paths and the source rewrites the mixin tool needs."""
import importlib.util
from pathlib import Path
import tempfile
import tomllib
import unittest

spec = importlib.util.spec_from_file_location("sunrise", Path(__file__).resolve().parents[1] / "sunrise.py")
sunrise = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sunrise)


def write(root, name, text):
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path


class RenameTests(unittest.TestCase):
    def test_longest_crate_names_win_and_prefixes_are_untouched(self):
        self.assertEqual(sunrise.rename_crates("horizon_client_render::gfx"), "sunrise_client_render::gfx")
        self.assertEqual(sunrise.rename_crates("horizon_client_lib::mobs"), "sunrise_client::mobs")
        self.assertEqual(sunrise.rename_crates("not_horizon_client::x"), "not_horizon_client::x")

    def test_rewrite_rules_can_be_limited_to_one_file(self):
        sunrise.load_text_rules({"rewrite": [
            {"pattern": r"\bfoo\b", "replace": "bar", "files": ["only.rs"]},
            {"pattern": r"horizon_assets::", "replace": "sunrise_assets::"},
        ]})
        try:
            self.assertEqual(sunrise.rename_text("foo horizon_assets::x", "only.rs"), "bar sunrise_assets::x")
            self.assertEqual(sunrise.rename_text("foo", "other.rs"), "foo")
        finally:
            sunrise.load_text_rules({})

    def test_bindings_are_renamed_inside_lists(self):
        value = [{"name": "out", "ty": "&mut Vec<horizon_client::mesher::Vertex>", "value": "&mut *out"}]
        self.assertEqual(sunrise.rename_deep(value)[0]["ty"], "&mut Vec<sunrise_client::mesher::Vertex>")


class ManifestTests(unittest.TestCase):
    def test_emitted_manifest_parses_back_unchanged(self):
        manifest = dict(
            mod=dict(id="m", version="1.2.3", priority=100),
            compatibility=dict(horizon_revision="a" * 40),
            crate=[dict(id="c", path="c", dependent="sunrise-client")],
            module=[dict(id="x", file="crates/a.rs", name="n", source="x.rs", visibility="pub")],
            injection=[dict(id="i", kind="HEAD", target="a::b", helper="h.rs", cancellable=True,
                            bindings=[dict(name="game", ty="&Game<'_>", value="&*self")])],
        )
        self.assertEqual(tomllib.loads(sunrise.emit_manifest(manifest)), manifest)


class SourceTreeTests(unittest.TestCase):
    def test_area_folders_are_not_modules(self):
        with tempfile.TemporaryDirectory() as tmp:
            crate = Path(tmp) / "crates/sunrise-client"
            write(crate, "src/game/mobs/.game-area", "")
            file = write(crate, "src/game/mobs/creatures/mod.rs", "")
            plain = write(crate, "src/game/native_hooks/base.rs", "")
            self.assertEqual(sunrise.module_path(crate, file), ["game", "creatures"])
            self.assertEqual(sunrise.module_path(crate, plain), ["game", "native_hooks", "base"])

    def test_local_includes_are_pasted_and_the_file_removed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            src = root / "crates/sunrise-client/src"
            write(src, "hooks/mod.rs", 'mod a;\ninclude!("shared.rs");\n')
            shared = write(src, "hooks/shared.rs", "fn shared() {}\n")
            self.assertEqual(sunrise.inline_local_includes(root), 1)
            self.assertIn("fn shared() {}", (src / "hooks/mod.rs").read_text())
            self.assertFalse(shared.exists())

    def test_parent_path_modules_are_linked_or_dropped(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            src = root / "crates/sunrise-client/src"
            write(src, "game/creatures/biome.rs", "pub fn f() {}\n")
            write(src, "game/mobs/creatures/mod.rs",
                  '#[path = "../../creatures/biome.rs"]\npub mod biome;\n'
                  '#[cfg(test)]\n#[path = "../../../../sunrise-mod-wasm/tests/x.rs"]\nmod fixture;\n')
            self.assertEqual(sunrise.flatten_paths(root), (1, 1))
            text = (src / "game/mobs/creatures/mod.rs").read_text()
            self.assertIn('#[path = "__linked/biome.rs"]', text)
            self.assertNotIn("fixture", text)
            self.assertTrue((src / "game/mobs/creatures/__linked/biome.rs").is_file())

    def test_replaceable_markers_are_stripped(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            file = write(root, "crates/x/src/lib.rs",
                         "#[sunrise_mod_api_macros::replaceable]\npub fn f() {}\n")
            self.assertEqual(sunrise.strip_markers(root), 1)
            self.assertEqual(file.read_text(), "pub fn f() {}\n")


if __name__ == "__main__":
    unittest.main()
