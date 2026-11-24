"""The Sunrise bundle generator: renames, module paths and the source rewrites the mixin tool needs."""
import argparse
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib
import unittest
import zipfile

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


class SmodTests(unittest.TestCase):
    def test_manifest_keeps_native_fields_and_pins(self):
        bundle = {"mod": {"id": "draimhorizon", "version": "1.2.3"}}
        compat = {"revision": "a" * 40, "mixins_revision": "b" * 40}
        mixin = {"id": "m", "kind": "HEAD", "target": "sunrise_client::game::Game<'_>::tick", "helper": "src/mixins/m.rs",
                 "bindings": [{"name": "game", "ty": "&mut Game<'_>", "value": "&mut *self"}], "cancellable": True, "ordinal": 2}
        module = {"id": "x", "file": "crates/sunrise-client/src/game.rs", "name": "x", "source": "src/modules/x.rs", "visibility": "pub(crate)"}
        data = tomllib.loads(sunrise.smod_manifest(bundle, compat, [mixin], [module]))
        self.assertEqual((data["id"], data["version"], data["api"], data["side"]), ("draimhorizon", "1.2.3", "0.2", "client"))
        self.assertEqual(data["native"]["sunrise"], "a" * 40)
        self.assertEqual(data["native"]["mixins"], "b" * 40)
        self.assertEqual(data["native"]["mixin"], [mixin])
        self.assertEqual(data["native"]["module"], [module])

    def test_manifest_declares_no_internals(self):
        # The brand adapter was the only writer of `internals`, and `check_smod_manifest` refuses
        # manifests that declare any: the generator can no longer emit the table at all.
        bundle = {"mod": {"id": "draimhorizon", "version": "1.2.3"}}
        compat = {"revision": "a" * 40, "mixins_revision": "b" * 40}
        data = tomllib.loads(sunrise.smod_manifest(bundle, compat, [], []))
        self.assertNotIn("internals", data)

    def test_the_package_carries_no_brand_adapter(self):
        # Since Nightly ed3ac19 sunrise-official announces the Horizon identity itself.
        platform = sunrise.load_platform()
        self.assertTrue(platform["injection"]["brand-gate"]["drop"])
        self.assertTrue(platform["injection"]["brand-announce"]["drop"])
        self.assertFalse((sunrise.PLATFORM / "smp-compat" / "sunrise-mod.toml").exists())

    def test_relay_files_become_nested_modules_with_rewritten_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp)
            write(src, "lib.rs", "//! doc\nmod crypto;\npub mod translation;\nmod websocket;\npub struct Config;\n")
            write(src, "crypto.rs", "use crate::Config;\n")
            write(src, "websocket.rs", f"fn r() {{ {sunrise.ROOTS_OLD}; }}\n")
            write(src, "translation.rs", 'use crate::websocket::Url;\n#[path = "translation_languages.rs"]\nmod languages;\n')
            write(src, "translation_languages.rs", "use super::Language;\n")
            out = sunrise.inline_relay(src)
        prefix = "crate::game::claim_borders_relay_client"
        self.assertIn(f"mod crypto {{\nuse {prefix}::Config;", out)
        self.assertIn(f"pub mod translation {{\nuse {prefix}::websocket::Url;", out)
        self.assertIn("mod languages {\nuse super::Language;", out)
        self.assertIn("mod_draimhorizon::tls_roots_der()", out)
        self.assertNotIn("webpki_roots", out)
        self.assertNotIn("mod crypto;", out)
        self.assertNotIn("#[path", out)
        self.assertNotIn("//!", out)

    def test_counts_report_porting_problems(self):
        modules = [{"file": "crates/sunrise-client/src/game.rs"}, {"file": "crates/sunrise-client/src/mobs/a.rs"}]
        counts = sunrise.smod_counts(modules, {"a.rs": "impl<'a> Game<'a> {\n}", "b.rs": "mod x;\n", "c.rs": "fn f() {}"})
        self.assertEqual(counts["modules_outside_game_crate"], 1)
        self.assertEqual(counts["sources_with_impl_game"], ["a.rs"])
        self.assertEqual(counts["sources_with_mod_decl"], ["b.rs"])


class TraitifyTests(unittest.TestCase):
    SOURCE = (
        "use x;\n\n"
        "impl Game<'_> {\n"
        "    /// Doc one.\n"
        "    /// Doc two.\n"
        "    pub(crate) fn a(&mut self, v: [i32; 3], w: u8) -> [i32; 3] {\n"
        "        v\n"
        "    }\n\n"
        "    fn private(&self) {\n"
        "        if true { }\n"
        "    }\n"
        "}\n\n"
        "impl super::Game<'_> {\n"
        "    pub fn b(\n        &self,\n    ) -> bool {\n        true\n    }\n"
        "}\n"
    )

    def test_inherent_game_impls_become_extension_traits(self):
        out = sunrise.traitify(self.SOURCE, "FooGame")
        self.assertNotIn("impl Game<'_> {", out)
        self.assertIn("pub(crate) trait FooGame {", out)
        self.assertIn("impl FooGame for Game<'_> {", out)
        # A second block gets its own trait and keeps the `super::Game` path.
        self.assertIn("pub(crate) trait FooGameMore {", out)
        self.assertIn("impl FooGameMore for super::Game<'_> {", out)

    def test_signatures_keep_array_types_and_docs_and_lose_visibility(self):
        out = sunrise.traitify(self.SOURCE, "FooGame")
        trait = out[out.index("trait FooGame {"):out.index("impl FooGame for")]
        self.assertIn("    /// Doc one.\n    /// Doc two.\n    fn a(&mut self, v: [i32; 3], w: u8) -> [i32; 3];", trait)
        self.assertIn("fn private(&self);", trait)
        impl = out[out.index("impl FooGame for"):out.index("trait FooGameMore")]
        self.assertIn("    /// Doc one.\n    /// Doc two.\n    fn a(&mut self, v: [i32; 3], w: u8) -> [i32; 3] {", impl)
        self.assertNotIn("pub(crate) fn", impl)
        self.assertIn("fn b(&self) -> bool;", out)  # multi-line signature collapsed in the trait

    def test_text_without_a_game_impl_is_not_converted(self):
        self.assertIsNone(sunrise.traitify("fn f() {}\n", "FooGame"))

    def test_unbalanced_braces_stop_the_build(self):
        with self.assertRaises(SystemExit):
            sunrise.traitify("impl Game<'_> {\n    fn a(&self) {\n", "FooGame")


class ImportTraitsTests(unittest.TestCase):
    TRAITS = ["crate::game::a::AGame", "crate::game::b::BGame"]

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.out = Path(self.tmp.name)
        write(self.out, "module.rs", "fn m() {}\n")
        write(self.out, "game_hook.rs", "// note\nfn hook(\n    game: &mut Game,\n) {\n    body();\n}\n")
        write(self.out, "render_hook.rs", "fn hook() {\n    body();\n}\n")
        self.modules = [{"source": "module.rs"}]
        self.injections = [
            {"target": "sunrise_client::game::Game<'_>::tick", "helper": "game_hook.rs"},
            {"target": "sunrise_client_render::draw", "helper": "render_hook.rs"},
        ]

    def tearDown(self):
        self.tmp.cleanup()

    def test_modules_import_at_the_top_and_game_crate_helpers_inside_the_function(self):
        sunrise.import_traits(self.out, self.modules, self.injections, self.TRAITS)
        self.assertTrue((self.out / "module.rs").read_text().startswith(
            "use crate::game::a::AGame as _;\nuse crate::game::b::BGame as _;\nfn m()"))
        hook = (self.out / "game_hook.rs").read_text()
        self.assertIn(") {\n    use crate::game::a::AGame as _;\n    use crate::game::b::BGame as _;\n    body();", hook)
        # Crates below the game crate cannot name `crate::game`.
        self.assertEqual((self.out / "render_hook.rs").read_text(), "fn hook() {\n    body();\n}\n")

    def test_helpers_are_not_imported_twice_and_no_traits_changes_nothing(self):
        sunrise.import_traits(self.out, self.modules, self.injections, self.TRAITS)
        once = (self.out / "game_hook.rs").read_text()
        sunrise.import_traits(self.out, [], self.injections, self.TRAITS)
        self.assertEqual((self.out / "game_hook.rs").read_text(), once)
        sunrise.import_traits(self.out, self.modules, self.injections, [])
        self.assertEqual((self.out / "game_hook.rs").read_text(), once)

    def test_trait_paths_follow_the_module_that_holds_the_impl(self):
        write(self.out, "economy.rs", "impl Game<'_> {\n    pub fn f(&self) {}\n}\n")
        modules = [{"id": "economy", "name": "claim_borders_economy", "source": "economy.rs",
                    "file": "crates/sunrise-client/src/game/network/remote/mod.rs"}]
        traits = sunrise.traitify_game_impls(self.out, modules, None)
        self.assertEqual(traits, ["crate::game::remote::claim_borders_economy::ClaimBordersEconomyGame"])
        self.assertIn("impl ClaimBordersEconomyGame for Game<'_>", (self.out / "economy.rs").read_text())


class PlatformMergeTests(unittest.TestCase):
    def merged(self, base, *extras):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, "platform.toml", base)
            for name, text in extras:
                write(root, f"platform.d/{name}", text)
            old = sunrise.PLATFORM
            sunrise.PLATFORM = root
            try:
                return sunrise.load_platform()
            finally:
                sunrise.PLATFORM = old

    def test_tables_merge_by_key_and_arrays_append_in_file_order(self):
        platform = self.merged(
            '[[rewrite]]\npattern = "a"\nreplace = "b"\n[injection.one]\ndrop = true\n[smod.skip]\nx = "why"\n',
            ("b.toml", '[[rewrite]]\npattern = "c"\nreplace = "d"\n[injection.two]\ndrop = true\n[injection.one]\nreason = "r"\n'),
            ("a.toml", '[[module]]\nid = "m"\n[smod.skip]\ny = "other"\n'),
        )
        self.assertEqual([r["pattern"] for r in platform["rewrite"]], ["a", "c"])
        self.assertEqual(platform["injection"]["one"], {"drop": True, "reason": "r"})
        self.assertIn("two", platform["injection"])
        self.assertEqual(platform["module"], [{"id": "m"}])
        self.assertEqual(platform["smod"]["skip"], {"x": "why", "y": "other"})

    def test_rows_override_per_key_and_other_scalars_must_not_collide(self):
        platform = self.merged("[injection.one]\ndrop = true\n", ("a.toml", "[injection.one]\ndrop = false\n"))
        self.assertEqual(platform["injection"]["one"], {"drop": False})
        with self.assertRaises(SystemExit):
            self.merged("[smod]\nx = \"one\"\n", ("a.toml", "[smod]\nx = \"two\"\n"))

    def test_the_real_platform_carries_the_split_files(self):
        platform = sunrise.load_platform()
        self.assertTrue(list((sunrise.PLATFORM / "platform.d").glob("*.toml")))
        self.assertIn("map-images", [m["id"] for m in platform.get("module", [])])


class PlatformOnlyInjectionTests(unittest.TestCase):
    RULE = dict(target="sunrise_client::game::native_hooks::base::Game<'_>::native_mouse_button_post",
                helper="h.rs", bindings=[dict(name="key", ty="&str", value="key")])

    def test_rules_missing_from_the_shared_manifest_become_complete_rows(self):
        rows = sunrise.platform_only_injections(
            {"shared": dict(drop=True), "only": dict(self.RULE, reason="r"), "gone": dict(drop=True)}, {"shared"})
        self.assertEqual(rows, [dict(id="only", kind="HEAD", **self.RULE)])
        row = sunrise.platform_only_injections({"x": dict(self.RULE, kind="BEFORE_CALL", require=1)}, set())[0]
        self.assertEqual((row["kind"], row["require"]), ("BEFORE_CALL", 1))

    def test_overrides_of_shared_injections_are_not_duplicated(self):
        self.assertEqual(sunrise.platform_only_injections({"shared": dict(target="t")}, {"shared"}), [])

    def test_an_incomplete_platform_only_rule_stops_the_build(self):
        for missing in ("target", "helper"):
            rule = {k: v for k, v in self.RULE.items() if k != missing}
            with self.assertRaisesRegex(SystemExit, missing):
                sunrise.platform_only_injections({"broken": rule}, set())

    def test_generated_project_hooks_mouse_buttons_for_the_key_helpers(self):
        with tempfile.TemporaryDirectory() as tmp:
            old = sunrise.OUT
            sunrise.OUT = Path(tmp)
            try:
                with contextlib.redirect_stdout(io.StringIO()):
                    sunrise.command_generate(argparse.Namespace(sunrise=None))
                project = Path(tmp) / "mods" / "draimhorizon"
                bundle = tomllib.loads((project / "horizon-mixin.toml").read_text(encoding="utf-8"))
                helpers = {i["id"]: (project / i["helper"]).read_text(encoding="utf-8") for i in bundle["injection"]
                           if i["id"] in ("menu-mouse", "bar-click-mouse")}
            finally:
                sunrise.OUT = old
        rows = {i["id"]: i for i in bundle["injection"]}
        for id, fn in (("menu-mouse", "claim_borders_menu_open_mouse"), ("bar-click-mouse", "baritone_click_key_mouse")):
            self.assertTrue(rows[id]["target"].endswith("::native_mouse_button_post"), id)
            # Bindings may only be target parameters, so the helper turns the GLFW index into the key name.
            self.assertEqual([(b["name"], b["value"]) for b in rows[id]["bindings"]],
                             [("game", "&mut *self"), ("button", "button"), ("pressed", "pressed")])
            text = helpers[id]
            self.assertIn(f"fn {fn}(game: &mut crate::game::Game<'_>, button: i32, pressed: bool)", text)
            self.assertIn("native_playing()", text)
            self.assertIn('3 => "key.mouse.4"', text)
            self.assertIn('4 => "key.mouse.5"', text)
            self.assertIn("key.mouse.none", text)
            self.assertEqual(text.count("fn "), 1)

    def test_a_helper_without_the_key_signature_cannot_be_derived(self):
        with tempfile.TemporaryDirectory() as tmp:
            write(Path(tmp), "h.rs", "fn other(game: &mut Game) {}\n")
            with self.assertRaisesRegex(SystemExit, "no \\(game, key, action\\) helper"):
                sunrise.derive_mouse_helpers(Path(tmp), {"x": dict(helper="m.rs", helper_from="h.rs")})


def usage_accepts(usage, line):
    """Whether Sunrise's client chat tree takes `line` for `usage` (the rules of native_server::usage_nodes)."""
    name, *arguments = usage.split()
    words = line.split(" ", 1)[0], (line.split(" ", 1) + [""])[1]
    if words[0] != name:
        return False
    rest = words[1].split() if arguments and arguments[-1].endswith(":word]") else words[1]
    for index, argument in enumerate(arguments):
        optional = argument.startswith("[")
        kind = argument.strip("<>[]").split(":", 1)[1]
        if kind == "text":
            return bool(rest.strip()) or optional
        if not rest:
            return optional
        rest = rest[1:]
    return not rest


class ClientCommandUsageTests(unittest.TestCase):
    ENTRY = (sunrise.PLATFORM / "overlay" / "crate" / "mod_entry.rs").read_text(encoding="utf-8")
    SHARED = (sunrise.BUNDLE / "commands.rs").read_text(encoding="utf-8")

    def usages(self):
        block = re.search(r"pub const USAGES: \[&str; (\d+)\] = \[(.*?)\];", self.ENTRY, re.S)
        rows = re.findall(r'"([^"]+)"', block[2])
        self.assertEqual(len(rows), int(block[1]))
        return rows

    def test_every_shared_command_has_exactly_one_usage(self):
        shared = re.findall(r'"([a-z]+)"', self.SHARED.split("]")[0])
        self.assertEqual(sorted(u.split()[0] for u in self.usages()), sorted(shared))

    def test_usages_follow_the_grammar_of_the_client_command_tree(self):
        for usage in self.usages():
            name, *arguments = usage.split()
            self.assertRegex(name, r"^[a-z0-9_-]+$", usage)
            self.assertTrue(all(re.fullmatch(r"[<\[]\w+:(word|text)[>\]]", a) for a in arguments), usage)
            self.assertLessEqual(len(arguments), 1, usage)

    def test_every_form_the_handlers_take_is_valid_and_not_red(self):
        usages = {u.split()[0]: u for u in self.usages()}
        # The forms mixins/draimhorizon/{command,client}.rs read: the words after the name go to the handler.
        lines = ["r", "r hello", "r hello there  world", "reply hi", "reply", "party", "party invite Name",
                 "party accept Name", "friend", "friend add Name", "wp", "wp add #ff0000 Home", "waypoint list",
                 "dfarm", "dfarm pos1", "dfarm save wheat", "horizonmenu", "claimborders", "claimborders refresh",
                 "claimborders menu"]
        for line in lines:
            self.assertTrue(usage_accepts(usages[line.split()[0]], line), line)

    def test_a_bare_name_would_have_turned_arguments_red(self):
        self.assertFalse(usage_accepts("r", "r hello"))
        self.assertTrue(usage_accepts("r [message:text]", "r hello"))
        self.assertFalse(usage_accepts("horizonmenu", "horizonmenu now"))


class SunriseSuggestTests(unittest.TestCase):
    SOURCE = sunrise.PLATFORM / "overlay" / "crate" / "suggest.rs"

    def test_nickname_snapshot_logic_in_the_mod_crate(self):
        rustc = shutil.which("rustc")
        if rustc is None:
            self.skipTest("no rustc")
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp) / "suggest_tests"
            build = subprocess.run([rustc, "--edition", "2024", "--test", str(self.SOURCE), "-o", str(binary)],
                                   capture_output=True, text=True)
            self.assertEqual(build.returncode, 0, build.stderr)
            run = subprocess.run([str(binary), "--test-threads=1"], capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stdout + run.stderr)

    def test_the_hook_applies_only_a_list_built_for_the_text_it_holds(self):
        hook = (sunrise.PLATFORM / "overlay" / "baritone" / "hooks" / "suggest_hook.rs").read_text(encoding="utf-8")
        self.assertIn("suggest::nicknames(&input.value)", hook)
        self.assertNotIn("take_nicknames", hook)
        self.assertNotIn("request_nicknames", self.SOURCE.read_text(encoding="utf-8"))


class SmodSkipTests(unittest.TestCase):
    def test_skipped_injections_are_left_out_of_the_smod_project(self):
        platform = sunrise.load_platform()
        skip = platform.get("smod", {}).get("skip", {})
        bundle = tomllib.loads((sunrise.BUNDLE / "horizon-mixin.toml").read_text(encoding="utf-8"))
        ids = {i["id"] for i in bundle["injection"]}
        self.assertLessEqual(set(skip), ids, "[smod.skip] names an injection the bundle does not have")
        self.assertTrue(all(skip.values()), "every skip carries its reason")


class SmodCheckTests(unittest.TestCase):
    FUNCTIONS = {"sunrise_client::game::Game::tick": {}, "sunrise_mod_render::render::world_drawn": {}}
    REFUSED = {"sunrise_client::game::Game::id": {"code": "E_MIXIN_NOT_REPLACEABLE"}}
    GAME = "crates/sunrise-client/src/game.rs"

    def manifest(self, targets, files=(GAME,)):
        return {"native": {
            "mixin": [{"id": f"m{i}", "target": t, "helper": "src/mixins/h.rs"} for i, t in enumerate(targets)],
            "module": [{"id": f"x{i}", "file": f, "source": "src/modules/x.rs"} for i, f in enumerate(files)],
        }}

    def problems(self, targets, files=(GAME,), names=("src/mixins/h.rs", "src/modules/x.rs")):
        return sunrise.check_smod_manifest(self.manifest(targets, files), self.FUNCTIONS, self.REFUSED, set(names))

    def test_declared_internals_are_refused_now_that_the_adapter_is_gone(self):
        # The brand adapter was the only writer of `internals`; its preflight left with it, so the
        # check fails a package that declares any accessible internal rather than guessing.
        manifest = self.manifest([])
        manifest["internals"] = [{"path": "sunrise_client_lib::net::ingress::Producer",
                                  "kind": "struct", "hash": "same"}]
        names = {"src/modules/x.rs"}
        problems = sunrise.check_smod_manifest(manifest, {}, {}, names, {"internals": []})
        self.assertTrue(any("internals declarations need a preflight" in problem for problem in problems))
        self.assertTrue(sunrise.check_smod_manifest(manifest, {}, {}, names))

    def test_lifetimes_are_dropped_to_match_the_function_index(self):
        self.assertEqual(sunrise.function_key("sunrise_client::game::Game<'_>::tick"), "sunrise_client::game::Game::tick")
        self.assertEqual(sunrise.function_key("a::<Game<'_> as T>::f"), "a::<Game as T>::f")

    def test_replaceable_targets_in_the_game_crate_pass(self):
        self.assertEqual(self.problems(["sunrise_client::game::Game<'_>::tick", "sunrise_mod_render::render::world_drawn"],
                                       [self.GAME, "crates/sunrise-client/src/game/remote/mod.rs"]), [])

    def test_missing_refused_and_unpackaged_rows_are_reported(self):
        found = self.problems(["sunrise_client::game::Game<'_>::gone", "sunrise_client::game::Game<'_>::id"],
                              ["crates/sunrise-client-render/src/lib.rs"], names=())
        text = "\n".join(found)
        self.assertIn("m0", text)
        self.assertIn("not a replaceable function", text)
        self.assertIn("E_MIXIN_NOT_REPLACEABLE", text)
        self.assertIn("outside the sunrise_client game crate", text)
        self.assertIn("helper src/mixins/h.rs is not in the package", text)
        self.assertIn("source src/modules/x.rs is not in the package", text)

    def test_the_packed_smod_names_only_replaceable_targets_when_an_sdk_is_present(self):
        compat = json.loads((sunrise.ROOT / "compatibility.json").read_text(encoding="utf-8"))["sunrise"]
        data = sunrise.sdk_directory(compat) / "build-data" / "functions.json"
        if not data.is_file():
            self.skipTest("no SDK build data (python tools/sunrise.py smod-check --fetch)")
        build = json.loads(data.read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as tmp:
            old = sunrise.OUT
            sunrise.OUT = Path(tmp)
            try:
                with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                    sunrise.command_smod(argparse.Namespace(sunrise=None))
            finally:
                sunrise.OUT = old
            project = Path(tmp) / "smod" / "draimhorizon"
            manifest = tomllib.loads((project / "sunrise-mod.toml").read_text(encoding="utf-8"))
            names = {p.relative_to(project).as_posix() for p in project.rglob("*") if p.is_file()}
        catalog = data.parent / "internals.json"
        catalog_data = json.loads(catalog.read_text(encoding="utf-8")) if catalog.is_file() else None
        self.assertEqual(sunrise.check_smod_manifest(manifest, build["functions"], build["refused"], names, catalog_data), [])
        ids = {m["id"] for m in manifest["native"]["mixin"]}
        # The engine minimap that claim-minimap filled is gone; the mod draws its own image.
        self.assertNotIn("claim-minimap", ids)
        self.assertIn("minimap-scan", {m["id"] for m in manifest["native"]["module"]})
        map_image = next(m for m in manifest["native"]["mixin"] if m["id"] == "claim-map-image")
        self.assertEqual(map_image["target"],
                         "sunrise_client::game::native_hooks::render_frame_pre::Game<'_>::native_render_frame_pre")
        self.assertFalse(map_image.get("cancellable", False))
        self.assertNotIn("src/smp_brand/handshake.rs", names)
        catalog = data.parent / "internals.json"
        if catalog.is_file():
            known = {(i["path"], i["kind"]): i["hash"] for i in catalog_data["internals"]
                     if not i.get("shape_only", False) and not i.get("cfg", "")}
            for row in manifest.get("internals", []):
                self.assertEqual(known.get((row["path"], row["kind"])), row["hash"], row["path"])

    def test_a_package_for_another_build_is_refused_before_the_tool_compiles_it(self):
        compat = sunrise.target_compat()
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            ran = tmp / "tool-ran"
            tool = write(tmp, f"sdk/tool/cargo-{sunrise.S}-mod", f"#!/bin/sh\ntouch {ran}\n")
            tool.chmod(0o755)
            write(tmp, "sdk/build-data/functions.json", json.dumps(
                {"build": {"commit": compat["revision"]}, "functions": {}, "refused": {}}))
            smod = tmp / "old.smod"
            with zipfile.ZipFile(smod, "w") as archive:
                archive.writestr(sunrise.MANIFEST, f'[native]\n{sunrise.S} = "{"0" * 40}"\nmixins = "{"0" * 40}"\n'
                                                   "mixin = []\nmodule = []\n")
            with self.assertRaises(SystemExit) as refused:
                sunrise.command_smod_check(argparse.Namespace(smod=smod, sdk=tmp / "sdk", fetch=False))
            self.assertIn("pins differ", str(refused.exception))
            self.assertFalse(ran.exists())


class StageAdaptationTests(unittest.TestCase):
    def test_launcher_spellings_are_retargeted_for_the_source_stage_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            manifest = write(root, "draimhorizon/horizon-mixin.toml",
                             'target = "a::<Game<\'_> as HumanoidSetup>::before_arm_pose"\n')
            sunrise.retarget_for_stage(root)
            self.assertIn("as anim::HumanoidSetup>::before_arm_pose", manifest.read_text())

    def test_launcher_access_patches_apply_or_fail_loudly(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write(root, "crates/sunrise-client/src/lib.rs", "#![forbid(unsafe_code)]\nfn x() {}\n")
            sunrise.relax_for_launcher_access(root)
            self.assertNotIn("forbid", (root / "crates/sunrise-client/src/lib.rs").read_text())
            with self.assertRaises(SystemExit):
                sunrise.relax_for_launcher_access(root)  # already applied: the source changed shape


if __name__ == "__main__":
    unittest.main()
