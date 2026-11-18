#!/usr/bin/env python3
"""Build the Sunrise variant of the mod from the shared sources and platforms/sunrise.

The mod is written once, against Horizon's source (mixins/draimhorizon). Sunrise is a fork whose
crates, module layout and some native hook points differ, so this tool derives the Sunrise mixin
bundle instead of keeping a second copy of the 88k lines:

  resolve   list where each Horizon hook target lives in a Sunrise checkout
  generate  write target/sunrise/mods/<id>/ (manifest + translated sources) for horizon-mixins
  smod      write the flat Sunrise native project (the shipping path, packed by tools/package_flat.py)
  smod-check  offline validation of the packed .smod with the SDK tool and function index
  stage     generate, then apply it to the pinned Sunrise source with the pinned mixin tool

Source of truth for differences: platforms/sunrise/platform.toml and platforms/sunrise/overlay/.
"""
import argparse
import importlib.util
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tomllib
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "mixins" / "draimhorizon"
PLATFORM = ROOT / "platforms" / "sunrise"
# SUNRISE_OUT lets parallel work trees stage into their own folders.
OUT = Path(os.environ["SUNRISE_OUT"]).resolve() if os.environ.get("SUNRISE_OUT") else ROOT / "target" / "sunrise"

# Horizon crate -> Sunrise crate. Longest names first so a prefix never wins.
CRATES = [
    ("horizon_client_render", "sunrise_client_render"),
    ("horizon_client_core", "sunrise_client_core"),
    ("horizon_client_input", "sunrise_client_input"),
    ("horizon_client_lib", "sunrise_client"),
    ("horizon_client", "sunrise_client"),
    ("horizon_protocol", "sunrise_protocol"),
    ("horizon_world", "sunrise_world"),
    ("horizon_core", "sunrise_core"),
    ("horizon-client-render", "sunrise-client-render"),
    ("horizon-client-core", "sunrise-client-core"),
    ("horizon-client-input", "sunrise-client-input"),
    ("horizon-client-lib", "sunrise-client-lib"),
    ("horizon-client", "sunrise-client"),
    ("horizon-protocol", "sunrise-protocol"),
    ("horizon-world", "sunrise-world"),
    ("horizon-core", "sunrise-core"),
]
CRATE_DIRS = {
    "sunrise_client": "sunrise-client",
    "sunrise_client_render": "sunrise-client-render",
    "sunrise_client_lib": "sunrise-client-lib",
    "sunrise_client_core": "sunrise-client-core",
    "sunrise_client_input": "sunrise-client-input",
    "sunrise_mod_render": "sunrise-mod-render",
}


def load_platform():
    """platform.toml plus every platform.d/*.toml: tables merge by key, arrays append."""
    platform = tomllib.loads((PLATFORM / "platform.toml").read_text(encoding="utf-8"))
    for path in sorted((PLATFORM / "platform.d").glob("*.toml")):
        merge_platform(platform, tomllib.loads(path.read_text(encoding="utf-8")), path.name)
    return platform


def merge_platform(base, extra, name):
    for key, value in extra.items():
        if isinstance(value, list):
            base.setdefault(key, []).extend(value)
        elif isinstance(value, dict):
            table = base.setdefault(key, {})
            for row, content in value.items():
                if isinstance(content, dict) and isinstance(table.get(row), dict):
                    table[row].update(content)
                elif row in table and table[row] != content:
                    raise SystemExit(f"platform.d/{name}: {key}.{row} conflicts with another file")
                else:
                    table[row] = content
        else:
            base[key] = value


def rename_crates(text):
    for old, new in CRATES:
        text = re.sub(rf"(?<![A-Za-z0-9_-]){re.escape(old)}(?![A-Za-z0-9_-])", new, text)
    return text


TEXT_RULES = []


def load_text_rules(platform):
    """Regex rewrites (pattern -> replacement) for names Sunrise spells differently."""
    TEXT_RULES[:] = [(re.compile(rule["pattern"]), rule["replace"], rule.get("files")) for rule in platform.get("rewrite", [])]


def rename_text(text, name=None):
    text = rename_crates(text)
    for pattern, replacement, files in TEXT_RULES:
        if files is None or name in files:
            text = pattern.sub(replacement, text)
    return text


def rename_deep(value):
    if isinstance(value, str):
        return rename_text(value)
    if isinstance(value, list):
        return [rename_deep(v) for v in value]
    if isinstance(value, dict):
        return {k: rename_deep(v) for k, v in value.items()}
    return value


def module_path(crate_dir, file):
    """The Rust module path of a source file; folders holding .game-area are not modules."""
    src = crate_dir / "src"
    parts = list(file.relative_to(src).with_suffix("").parts)
    out, current = [], src
    for index, part in enumerate(parts):
        current = current / part
        if index < len(parts) - 1 and (current / ".game-area").exists():
            continue
        out.append(part)
    if out and out[-1] == "mod":
        out.pop()
    return out


def sunrise_index(sunrise):
    """Every client-side source file by crate, with its text."""
    index = {}
    for crate, directory in CRATE_DIRS.items():
        base = sunrise / "crates" / directory
        for path in sorted((base / "src").rglob("*.rs")):
            index.setdefault(crate, []).append((path, base))
    return index


def split_target(target):
    """crate, module segments, owner (type or trait impl), function."""
    segments = re.split(r"::(?![^<]*>)", target)
    crate, rest = segments[0], segments[1:]
    function = rest[-1]
    owner_index = next((i for i, s in enumerate(rest[:-1]) if s[:1].isupper() or s.startswith("<")), len(rest) - 1)
    return crate, rest[:owner_index], "::".join(rest[owner_index:-1]), function


def resolve(sunrise, target):
    crate, modules, owner, function = split_target(rename_crates(target))
    index = sunrise_index(sunrise)
    pattern = re.compile(rf"\bfn\s+{re.escape(function)}\b")
    found = []
    for name in ([crate] if crate in index else list(index)):
        for path, base in index[name]:
            if pattern.search(path.read_text(encoding="utf-8", errors="ignore")):
                found.append((name, module_path(base, path), owner, path.relative_to(sunrise).as_posix()))
    return crate, modules, owner, function, found


def command_resolve(args):
    manifest = tomllib.loads((BUNDLE / "horizon-mixin.toml").read_text(encoding="utf-8"))
    rows = {}
    for injection in manifest["injection"]:
        crate, modules, owner, function, found = resolve(args.sunrise, injection["target"])
        direct = [f for f in found if f[0] == crate and f[1] == modules]
        rows[injection["id"]] = dict(
            target=rename_crates(injection["target"]),
            status="same" if direct else ("moved" if found else "missing"),
            candidates=[
                "::".join([f[0], *f[1], *([f[2]] if f[2] else []), function]) + f"  # {f[3]}"
                for f in found
            ][:6],
        )
    json.dump(rows, sys.stdout, indent=2, ensure_ascii=False)
    print()


def toml_value(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, list):
        return "[" + ", ".join(toml_value(v) for v in value) + "]"
    if isinstance(value, dict):
        return "{ " + ", ".join(f"{k} = {toml_value(v)}" for k, v in value.items()) + " }"
    raise ValueError(f"unsupported TOML value: {value!r}")


def emit_manifest(manifest):
    """Write the subset of TOML the mixin manifest uses; the platform tool owns this format."""
    lines = []
    for name in ("mod", "compatibility"):
        lines += [f"[{name}]", *(f"{k} = {toml_value(v)}" for k, v in manifest[name].items()), ""]
    for section in ("crate", "module", "injection"):
        for entry in manifest.get(section, []):
            lines.append(f"[[{section}]]")
            for key, value in entry.items():
                if key == "bindings":
                    lines.append("bindings = [")
                    lines += [f"  {toml_value(b)}," for b in value]
                    lines.append("]")
                else:
                    lines.append(f"{key} = {toml_value(value)}")
            lines.append("")
    return "\n".join(lines)


def choose_candidate(found):
    """The one non-test definition, or None when the platform file must decide."""
    real = [f for f in found if not re.search(r"(_tests?|/tests?)(\.rs|/)", f[3] + "/")]
    return real[0] if len(real) == 1 else None


def platform_only_injections(rules, shared_ids):
    """Rules whose id the shared manifest lacks: Sunrise-only hooks, emitted as complete mixin rows."""
    rows = []
    for key, rule in rules.items():
        if key in shared_ids or rule.get("drop"):
            continue
        missing = [name for name in ("target", "helper") if name not in rule]
        if missing:
            raise SystemExit(f"platform injection {key}: not in the shared manifest, so it needs {' and '.join(missing)}")
        entry = dict(id=key, kind=rule.get("kind", "HEAD"))
        for name, value in rule.items():
            if name not in ("drop", "reason", "kind", "params", "helper_from"):
                entry[name] = value
        rows.append(entry)
    return rows


# GLFW button index (glfw_button) -> key.mouse.* as keybinds::mouse_name spells it; Other(n) is n + 1.
MOUSE_KEY_PRELUDE = """
    let key = if !game.native_playing() {
        "key.mouse.none"
    } else {
        match button {
            0 => "key.mouse.left",
            1 => "key.mouse.right",
            2 => "key.mouse.middle",
            3 => "key.mouse.4",
            4 => "key.mouse.5",
            5 => "key.mouse.6",
            6 => "key.mouse.7",
            7 => "key.mouse.8",
            _ => "key.mouse.none",
        }
    };
    let action = if pressed {
        mod_api::KeyAction::Press
    } else {
        mod_api::KeyAction::Release
    };"""
KEY_HELPER_SIGNATURE = re.compile(
    r"fn (\w+)\(\s*game: &mut crate::game::Game<'_>,\s*key: &str,\s*action: mod_api::KeyAction,\s*\) \{")


def derive_mouse_helpers(out, rules):
    """Write each `helper_from` rule's helper: the shared key helper fed from a GLFW button index."""
    for key, rule in rules.items():
        if "helper_from" not in rule or rule.get("drop"):
            continue
        source = (out / rule["helper_from"]).read_text(encoding="utf-8")
        text, count = KEY_HELPER_SIGNATURE.subn(
            lambda m: f"fn {m[1]}_mouse(game: &mut crate::game::Game<'_>, button: i32, pressed: bool) {{{MOUSE_KEY_PRELUDE}",
            source)
        if count != 1:
            raise SystemExit(f"platform injection {key}: {rule['helper_from']} has no (game, key, action) helper to derive from")
        target = out / rule["helper"]
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")


def command_generate(args):
    platform = load_platform()
    compat = json.loads((ROOT / "compatibility.json").read_text(encoding="utf-8"))["sunrise"]
    manifest = tomllib.loads((BUNDLE / "horizon-mixin.toml").read_text(encoding="utf-8"))
    load_text_rules(platform)
    frozen_path = PLATFORM / "targets.toml"
    frozen = tomllib.loads(frozen_path.read_text(encoding="utf-8")).get("target", {}) if frozen_path.exists() else {}
    rules = platform.get("injection", {})
    relocate = platform.get("module_file", {})
    out = OUT / "mods" / manifest["mod"]["id"]
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    injections, report = [], []
    for injection in manifest["injection"]:
        rule = rules.get(injection["id"], {})
        if rule.get("drop"):
            report.append((injection["id"], "dropped", rule.get("reason", "")))
            continue
        entry = {k: rename_deep(v) for k, v in injection.items()}
        if "target" in rule:
            entry["target"] = rule["target"]
        elif args.sunrise is None:
            if injection["id"] not in frozen:
                raise SystemExit(f"injection {injection['id']}: not in platforms/sunrise/targets.toml; run `freeze`")
            entry["target"] = frozen[injection["id"]]
        elif args.sunrise:
            crate, modules, owner, function, found = resolve(args.sunrise, injection["target"])
            if not [f for f in found if f[0] == crate and f[1] == modules]:
                best = choose_candidate(found)
                if best is None:
                    raise SystemExit(f"injection {injection['id']}: no unique Sunrise target; add it to platform.toml")
                entry["target"] = "::".join([best[0], *best[1], *([best[2]] if best[2] else []), function])
        for key in ("kind", "helper", "bindings", "cancellable", "selector", "require", "ordinal", "priority"):
            if key in rule:
                entry[key] = rule[key]
        for old, new in rule.get("params", {}).items():
            for binding in entry.get("bindings", []):
                binding["value"] = re.sub(rf"\b{re.escape(old)}\b", new, binding["value"])
        injections.append(entry)
    injections += platform_only_injections(rules, {i["id"] for i in manifest["injection"]})
    modules = []
    overrides = platform.get("module_override", {})
    for module in manifest["module"]:
        change = overrides.get(module["id"], {})
        if change.get("drop"):
            report.append((module["id"], "module dropped", change.get("reason", "")))
            continue
        entry = {k: rename_deep(v) for k, v in module.items()}
        entry["file"] = relocate.get(module["file"], entry["file"])
        for key in ("file", "name", "source", "visibility"):
            if key in change:
                entry[key] = change[key]
        modules.append(entry)
    for module in platform.get("module", []):
        modules.append(dict(module))
    manifest_out = dict(
        mod=manifest["mod"],
        compatibility=dict(horizon_revision=compat["revision"]),
        crate=[{k: rename_crates(v) for k, v in c.items()} for c in manifest.get("crate", [])],
        module=modules,
        injection=injections,
    )
    (out / "horizon-mixin.toml").write_text(emit_manifest(manifest_out), encoding="utf-8")
    for path in sorted(BUNDLE.rglob("*")):
        if path.is_file() and path.name != "horizon-mixin.toml":
            target = out / path.relative_to(BUNDLE)
            target.parent.mkdir(parents=True, exist_ok=True)
            if path.suffix == ".rs":
                target.write_text(rename_text(path.read_text(encoding="utf-8"), path.relative_to(BUNDLE).as_posix()), encoding="utf-8")
            else:
                shutil.copyfile(path, target)
    overlay = PLATFORM / "overlay"
    if overlay.is_dir():
        for path in sorted(overlay.rglob("*")):
            if path.is_file():
                target = out / path.relative_to(overlay)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, target)
    derive_mouse_helpers(out, rules)
    traits = traitify_game_impls(out, modules, args.sunrise)
    import_traits(out, modules, injections, traits)
    print(json.dumps(dict(out=str(out), injections=len(injections), modules=len(modules), dropped=report,
                          game_traits=traits), indent=2))


# Rust module paths of the non-`game.rs` files modules are mirrored into (`network/` and `runtime/`
# are areas, not modules), so a bundle can be generated without a Sunrise checkout.
PLATFORM_MODULE_PATHS = {
    "crates/sunrise-client/src/game/network/remote/mod.rs": "game::remote",
    "crates/sunrise-client/src/game/runtime/agent/mod.rs": "game::agent",
}
GAME_IMPL = re.compile(r"^impl (?P<path>(?:super::)?Game)<'_> \{\n", re.M)
METHOD = re.compile(r"^(?P<doc>(?:    ///[^\n]*\n)*)    (?:pub(?:\([^)]*\))? )?(?P<sig>fn [^{]*?)\s*\{", re.M | re.S)


def block_end(text, start):
    """Index just past the `}` closing the block whose `{` ends the line at `start`."""
    depth = 0
    for index in range(text.index("{", start), len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return index + 1
    raise SystemExit("unbalanced braces in an `impl Game` block")


def traitify(text, trait):
    """Turn each top-level `impl Game<'_> { ... }` into an extension trait and its impl: a Sunrise
    native mod compiles modules in another crate, where inherent impls on `Game` are refused (E0116)."""
    found = False
    while match := GAME_IMPL.search(text):
        found = True
        end = block_end(text, match.start())
        body = text[match.end():end - 1]
        sigs = [m["doc"] + "    " + re.sub(r"\s+", " ", m["sig"]).replace("( ", "(").replace(", )", ")") + ";"
                for m in METHOD.finditer(body)]
        body = re.sub(r"^    pub(?:\([^)]*\))? fn ", "    fn ", body, flags=re.M)
        replacement = (f"pub(crate) trait {trait} {{\n" + "\n".join(sigs) + "\n}\n\n"
                       f"impl {trait} for {match['path']}<'_> {{\n{body}}}")
        text = text[:match.start()] + replacement + text[end:]
        trait += "More"
    return text if found else None


def traitify_game_impls(out, modules, sunrise):
    """Convert modules that add methods to `Game`; return the trait paths callers must import."""
    traits = []
    for module in modules:
        path = out / module["source"]
        text = path.read_text(encoding="utf-8")
        if not GAME_IMPL.search(text):
            continue
        trait = "".join(part.title() for part in module["name"].split("_")) + "Game"
        if re.search(rf"\btrait {trait}\b", text):
            continue
        path.write_text(traitify(text, trait), encoding="utf-8")
        file = module["file"]
        known = PLATFORM_MODULE_PATHS.get(file)
        if file == "crates/sunrise-client/src/game.rs":
            owner = ["game"]
        elif known:
            owner = known.split("::")
        elif sunrise is not None:
            owner = module_path(Path(sunrise) / "crates" / "sunrise-client", Path(sunrise) / file)
        else:
            raise SystemExit(f"{module['id']}: pass --sunrise to place its Game trait")
        traits.append("::".join(["crate", *owner, module["name"], trait]))
    for module in modules:
        text = (out / module["source"]).read_text(encoding="utf-8")
        if module["name"] == "claim_borders_platform":
            traits += [f"crate::game::claim_borders_platform::{name}" for name in re.findall(r"pub\(crate\) trait (\w+)", text)]
    return sorted(set(traits))


def import_traits(out, modules, injections, traits):
    """Bring the extension traits into every module (file top) and helper (function body)."""
    if not traits:
        return
    uses = "".join(f"use {path} as _;\n" for path in traits)
    for module in modules:
        path = out / module["source"]
        path.write_text(uses + path.read_text(encoding="utf-8"), encoding="utf-8")
    for injection in injections:
        # Helpers below the game crate cannot name `crate::game`.
        if not injection["target"].startswith("sunrise_client::"):
            continue
        path = out / injection["helper"]
        text = path.read_text(encoding="utf-8")
        if all(f"use {p} as _;" in text for p in traits):
            continue
        body = re.search(r"^fn [^{]*\{\n", text, re.M | re.S)
        if body is None:
            raise SystemExit(f"{injection['id']}: helper has no function body")
        indented = "".join(f"    use {p} as _;\n" for p in traits)
        path.write_text(text[:body.end()] + indented + text[body.end():], encoding="utf-8")


SMOD_ID = "draimhorizon"
SMOD_DESCRIPTION = "Friends, map, chat, trades, waypoints. J: menu, M: map, B: trades, X: waypoint. /horizonmenu"
SMOD_GAME = "26.3"  # compatibility.json has no game field; override with sunrise.game when added
SMOD_CRATE = "mod-draimhorizon"
RELAY = "claim_borders_relay_client"
RELAY_PATH = f"crate::game::{RELAY}"
GAME_ROOTS = ("crates/sunrise-client/src/game.rs", "crates/sunrise-client/src/game/")
ROOTS_OLD = "rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned())"
ROOTS_NEW = (
    "rustls::RootCertStore {\n"
    "                roots: mod_draimhorizon::tls_roots_der()\n"
    "                    .into_iter()\n"
    "                    .map(|(subject, spki, constraints)| rustls::pki_types::TrustAnchor {\n"
    "                        subject: rustls::pki_types::Der::from_slice(subject),\n"
    "                        subject_public_key_info: rustls::pki_types::Der::from_slice(spki),\n"
    "                        name_constraints: constraints.map(rustls::pki_types::Der::from_slice),\n"
    "                    })\n"
    "                    .collect(),\n"
    "            }"
)
SMOD_CARGO = f"""[package]
name = "{SMOD_CRATE}"
version = "{{version}}"
edition = "2024"
license = "MIT"

[dependencies]
sunrise-mod-api = {{ path = ".sunrise/sdk/api/crates/sunrise-mod-api" }}
webpki-roots = "=1.0.10"
rustls-pki-types = {{ version = "=1.15.1", features = ["web"] }}
zeroize = "=1.9.1"
"""
INLINE_DECL = re.compile(r"^(?P<pub>pub\s+)?mod\s+(?P<name>\w+);[ \t]*$", re.M)


def inline_relay(src):
    """The relay_client crate as the items of one game module: files become nested `mod x { }`."""
    def load(name):
        text = (src / name).read_text(encoding="utf-8")
        text = re.sub(r"^//!", "//", text, flags=re.M)
        text = re.sub(r"\bcrate::", RELAY_PATH + "::", text)
        return text.replace(ROOTS_OLD, ROOTS_NEW)

    nested = {"crypto": "crypto.rs", "translation": "translation.rs", "websocket": "websocket.rs"}
    parts = {"languages": "translation_languages.rs"}

    def expand(text, files):
        text = re.sub(r"^#\[path\s*=\s*\"[^\"]+\"\]\n", "", text, flags=re.M)
        def swap(match):
            file = files.get(match["name"])
            if file is None:
                return match[0]
            body = load(file)
            if file == "translation.rs":
                body = expand(body, parts)
            return f"{match['pub'] or ''}mod {match['name']} {{\n{body}\n}}"
        return INLINE_DECL.sub(swap, text)

    return expand(load("lib.rs"), nested)


def smod_counts(modules, sources):
    """Known porting problems still left in the output."""
    inherent = re.compile(r"^\s*impl(?:<[^>]*>)?\s+Game(?:<[^>]*>)?\s*\{", re.M)
    declared = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*;", re.M)
    return dict(
        modules_outside_game_crate=sum(1 for m in modules if not m["file"].startswith(GAME_ROOTS)),
        sources_with_impl_game=sorted(n for n, t in sources.items() if inherent.search(t)),
        sources_with_mod_decl=sorted(n for n, t in sources.items() if declared.search(t)),
    )


SMP_BRAND = PLATFORM / "smp-compat"
SMP_BRAND_CRATE = "mod_draimhorizon_smp_compat"
SMP_BRAND_MODULE = "smp_brand"
SMP_BRAND_ROOT = f"""// Horizon SMP login brand for Sunrise, folded into this mod from platforms/sunrise/smp-compat.
// Its two mixins (src/mixins/smp/) call it from the network and game threads.
mod handshake;
mod policy;
pub use handshake::{{Outcome, Rendezvous, SendBrand, rendezvous}};
pub use policy::{{brand_for_address, is_horizon_smp}};
"""


def smp_brand_parts():
    """The brand adapter's mixins (helpers under src/mixins/smp/, calling this crate) and the SDK
    internals they read, from the adapter's declaration in platforms/sunrise/smp-compat."""
    declared = tomllib.loads((SMP_BRAND / "sunrise-mod.toml").read_text(encoding="utf-8"))
    pinned = json.loads((ROOT / "compatibility.json").read_text(encoding="utf-8"))["sunrise"]["revision"]
    if declared["native"]["sunrise"] != pinned:
        # Its internals fingerprints were checked against one build: recheck them before moving the pin.
        raise SystemExit(f"{SMP_BRAND / 'sunrise-mod.toml'} pins Sunrise {declared['native']['sunrise']}, "
                         f"compatibility.json {pinned}: rerun smp-compat/verify_sdk.py and move the pin")
    mixins, helpers = [], {}
    for row in declared["native"]["mixin"]:
        row = dict(row)
        name = f"src/mixins/smp/{Path(row['helper']).name}"
        text = (SMP_BRAND / row["helper"]).read_text(encoding="utf-8")
        helpers[name] = text.replace(f"{SMP_BRAND_CRATE}::", f"{SMOD_CRATE.replace('-', '_')}::{SMP_BRAND_MODULE}::")
        row["helper"] = name
        mixins.append(row)
    return mixins, helpers, declared.get("internals", [])


def smod_manifest(bundle, compat, injections, modules, internals=()):
    mod = bundle["mod"]
    lines = [
        f"id = {toml_value(mod['id'])}",
        f"name = {toml_value(mod.get('name', 'DraimHorizon'))}",
        f"version = {toml_value(mod['version'])}",
        f"description = {toml_value(mod.get('description') or SMOD_DESCRIPTION)}",
        'authors = ["DraimCiDo"]',
        'license = "MIT"',
        'api = "0.2"',
        'side = "client"',
        'capabilities = ["registries"]',
        "",
    ]
    if internals:
        lines += ["internals = [", *(f"  {toml_value(row)}," for row in internals), "]", ""]
    lines += [
        "[native]",
        f"game = {toml_value(compat.get('game', SMOD_GAME))}",
        f"sunrise = {toml_value(compat['revision'])}",
        f"mixins = {toml_value(compat['mixins_revision'])}",
        "",
    ]
    for section, rows in (("mixin", injections), ("module", modules)):
        for row in rows:
            lines.append(f"[[native.{section}]]")
            for key, value in row.items():
                if key == "bindings":
                    lines += ["bindings = [", *(f"  {toml_value(b)}," for b in value), "]"]
                else:
                    lines.append(f"{key} = {toml_value(value)}")
            lines.append("")
    return "\n".join(lines)


def command_smod(args):
    command_generate(args)
    compat = json.loads((ROOT / "compatibility.json").read_text(encoding="utf-8"))["sunrise"]
    generated = OUT / "mods" / "draimhorizon"
    bundle = tomllib.loads((generated / "horizon-mixin.toml").read_text(encoding="utf-8"))
    out = OUT / "smod" / "draimhorizon"
    if out.exists():
        shutil.rmtree(out)
    (out / "src" / "mixins").mkdir(parents=True)
    (out / "src" / "modules").mkdir(parents=True)
    sources = {}

    def fix(text):
        return re.sub(rf"(?<![\w:]){RELAY}\b", RELAY_PATH, text)

    injections = []
    skip = load_platform().get("smod", {}).get("skip", {})
    for injection in bundle["injection"]:
        if injection["id"] in skip:
            print(f"smod: skipping {injection['id']}: {skip[injection['id']]}", file=sys.stderr)
            continue
        row = dict(injection)
        # Keep the folder: `chat.rs` and `baritone/hooks/chat.rs` are different helpers.
        name = Path(row["helper"]).as_posix()
        text = fix((generated / row["helper"]).read_text(encoding="utf-8"))
        (out / "src" / "mixins" / name).parent.mkdir(parents=True, exist_ok=True)
        (out / "src" / "mixins" / name).write_text(text, encoding="utf-8")
        sources[f"src/mixins/{name}"] = text
        row["helper"] = f"src/mixins/{name}"
        injections.append(row)
    modules = []
    for module in bundle["module"]:
        row = dict(module)
        text = fix((generated / row["source"]).read_text(encoding="utf-8"))
        target = out / "src" / "modules" / row["source"]
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
        sources[f"src/modules/{row['source']}"] = text
        row["source"] = f"src/modules/{row['source']}"
        modules.append(row)
    relay = inline_relay(generated / "relay_client" / "src")
    (out / "src" / "modules" / f"{RELAY}.rs").write_text(relay, encoding="utf-8")
    sources[f"src/modules/{RELAY}.rs"] = relay
    modules.append(dict(
        id=RELAY, file="crates/sunrise-client/src/game.rs", name=RELAY,
        source=f"src/modules/{RELAY}.rs", visibility="pub(crate)",
    ))
    # The Horizon SMP login brand: two more mixins on the Sunrise network code, and their shared state
    # as a module of this crate, so no companion mod is needed.
    brand_mixins, brand_helpers, internals = smp_brand_parts()
    for name, text in brand_helpers.items():
        (out / name).parent.mkdir(parents=True, exist_ok=True)
        (out / name).write_text(text, encoding="utf-8")
        sources[name] = text
    injections += brand_mixins
    (out / "src" / SMP_BRAND_MODULE).mkdir()
    (out / "src" / f"{SMP_BRAND_MODULE}.rs").write_text(SMP_BRAND_ROOT, encoding="utf-8")
    for part in ("handshake.rs", "policy.rs"):
        shutil.copyfile(SMP_BRAND / "src" / part, out / "src" / SMP_BRAND_MODULE / part)
    (out / "sunrise-mod.toml").write_text(smod_manifest(bundle, compat, injections, modules, internals), encoding="utf-8")
    (out / "Cargo.toml").write_text(SMOD_CARGO.replace("{version}", bundle["mod"]["version"]), encoding="utf-8")
    lock = (PLATFORM / "smod" / "Cargo.lock").read_text(encoding="utf-8")
    lock = re.sub(r'name = "mod-draim-probe"\nversion = "[^"]*"', f'name = "{SMOD_CRATE}"\nversion = "{bundle["mod"]["version"]}"', lock)
    (out / "Cargo.lock").write_text(lock, encoding="utf-8")
    crate_files = sorted((generated / "crate").glob("*.rs"))
    lib = (PLATFORM / "smod" / "lib.rs").read_text(encoding="utf-8")
    lib += "".join(f"pub mod {path.stem};\n" for path in crate_files)
    lib += f"pub mod {SMP_BRAND_MODULE};\n"
    (out / "src" / "lib.rs").write_text(lib, encoding="utf-8")
    # State and hooks that only need public items (and that lower-crate helpers call) live in the mod crate.
    for path in crate_files:
        shutil.copyfile(path, out / "src" / path.name)
    print(json.dumps(dict(smod=str(out), mixins=len(injections), modules=len(modules), **smod_counts(modules, sources)), indent=2))


SDK_REPOSITORY = "https://github.com/sunrise-org/sunrise"
SDK_TARGET = "x86_64-unknown-linux-gnu"


def sdk_directory(compat):
    return ROOT / ".local" / f"sdk-{compat['revision'][:7]}"


def download(url, destination):
    with urllib.request.urlopen(url, timeout=120) as response:
        destination.write_bytes(response.read())


def sha256_file(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fetch_sdk(compat, directory):
    """The offline-check parts of the SDK release `sdk-<revision>`: sdk.json, the build data and the
    tool for this machine, each verified against the release's SHA256SUMS before it is unpacked."""
    base = f"{SDK_REPOSITORY}/releases/download/sdk-{compat['revision']}"
    directory.mkdir(parents=True, exist_ok=True)
    download(f"{base}/SHA256SUMS", directory / "SHA256SUMS")
    sums = {}
    for line in (directory / "SHA256SUMS").read_text(encoding="utf-8").splitlines():
        if line.strip():
            digest, name = line.split(None, 1)
            sums[name.strip().lstrip("*")] = digest
    for name, unpack in (("sdk.json", None), ("build-data.tar.xz", "build-data"), (f"tool-{SDK_TARGET}.tar.xz", "tool")):
        if name not in sums:
            raise SystemExit(f"SHA256SUMS of sdk-{compat['revision']} does not list {name}")
        path = directory / name
        if not path.exists() or sha256_file(path) != sums[name]:
            download(f"{base}/{name}", path)
        if sha256_file(path) != sums[name]:
            raise SystemExit(f"{name}: SHA-256 differs from the release's SHA256SUMS")
        if unpack:
            target = directory / unpack
            target.mkdir(exist_ok=True)
            with tarfile.open(path) as archive:
                archive.extractall(target, filter="data")
    tool = directory / "tool" / "cargo-sunrise-mod"
    tool.chmod(tool.stat().st_mode | 0o111)


def function_key(target):
    """The SDK's function index spells `Game<'_>` as `Game`: drop lifetime arguments."""
    return re.sub(r"<'[^>]*>", "", target)


def check_smod_manifest(manifest, functions, refused, names, internal_catalog=None):
    """Problems of one sunrise-mod.toml against the SDK's function index and the package's files."""
    problems = []
    if manifest.get("internals"):
        if internal_catalog is None:
            problems.append("internals declarations require the SDK internals.json catalog")
        else:
            # Share the adapter preflight's resolver semantics; raw catalog presence is not access.
            spec = importlib.util.spec_from_file_location("smp_sdk_contract", SMP_BRAND / "verify_sdk.py")
            checker = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(checker)
            problems.extend(row["message"] for row in checker.check_declared_internals(manifest, internal_catalog))
    native = manifest.get("native", {})
    for row in native.get("mixin", []):
        key = function_key(row["target"])
        if key in refused:
            problems.append(f"mixin {row['id']}: {key} is not replaceable ({refused[key].get('code')})")
        elif key not in functions:
            problems.append(f"mixin {row['id']}: {key} is not a replaceable function of this build")
        if row["helper"] not in names:
            problems.append(f"mixin {row['id']}: helper {row['helper']} is not in the package")
    for row in native.get("module", []):
        if not row["file"].startswith(GAME_ROOTS):
            problems.append(f"module {row['id']}: {row['file']} is outside the sunrise_client game crate")
        if row["source"] not in names:
            problems.append(f"module {row['id']}: source {row['source']} is not in the package")
    return problems


def command_smod_check(args):
    """Offline proxy for the launcher's validation of the packed .smod."""
    compat = json.loads((ROOT / "compatibility.json").read_text(encoding="utf-8"))["sunrise"]
    version = tomllib.loads((BUNDLE / "horizon-mixin.toml").read_text(encoding="utf-8"))["mod"]["version"]
    smod = args.smod or ROOT / "target" / f"draimhorizons-{version}.smod"
    if not smod.is_file():
        raise SystemExit(f"{smod} not found: run `python tools/package_flat.py sunrise` first")
    sdk = args.sdk or sdk_directory(compat)
    if args.fetch:
        fetch_sdk(compat, sdk)
    tool, data = sdk / "tool" / "cargo-sunrise-mod", sdk / "build-data" / "functions.json"
    if not tool.is_file() or not data.is_file():
        raise SystemExit(f"no SDK at {sdk}: pass --fetch to download sdk-{compat['revision']}")
    build = json.loads(data.read_text(encoding="utf-8"))
    if build["build"]["commit"] != compat["revision"]:
        raise SystemExit(f"{data} is for {build['build']['commit']}, not the pinned {compat['revision']}")
    result = subprocess.run([str(tool), "check", str(smod), "--json"], capture_output=True, text=True)
    print(result.stdout.strip())
    if result.returncode != 0 or not json.loads(result.stdout or "{}").get("ok"):
        raise SystemExit(f"cargo-sunrise-mod check refused {smod.name}:\n{result.stderr.strip()}")
    with zipfile.ZipFile(smod) as archive:
        manifest = tomllib.loads(archive.read("sunrise-mod.toml").decode("utf-8"))
        names = set(archive.namelist())
    internal_path = data.parent / "internals.json"
    internal_catalog = json.loads(internal_path.read_text(encoding="utf-8")) if internal_path.is_file() else None
    problems = check_smod_manifest(manifest, build["functions"], build["refused"], names, internal_catalog)
    if internal_catalog is not None and internal_catalog.get("build", {}).get("commit") != compat["revision"]:
        problems.append("the internals catalog belongs to another SDK build")
    if manifest["native"]["sunrise"] != compat["revision"] or manifest["native"]["mixins"] != compat["mixins_revision"]:
        problems.append("the package pins differ from compatibility.json")
    if problems:
        raise SystemExit("\n".join(problems))
    native = manifest["native"]
    print(f"{smod.name}: {len(native['mixin'])} mixins replaceable, {len(native['module'])} modules in the game crate")


def run(command, **kwargs):
    return subprocess.run(command, check=True, **kwargs)


def git(checkout, *args):
    return subprocess.check_output(["git", "-C", str(checkout), *args], text=True).strip()


INCLUDE = re.compile(r'include!\(concat!\(env!\("OUT_DIR"\),\s*"/(?P<name>[a-z_]+\.rs)"\)\);')
# Generated while the game builds; the mixin tool reads source and cannot evaluate build scripts.
INLINED = {
    "game_modules.rs", "game_host_modules.rs", "lan_action_modules.rs", "native_hook_modules.rs",
    "mod_api_fire_generated.rs", "mod_api_goal_view_generated.rs",
}


def export_sunrise(sunrise, revision, destination):
    """Export the pinned commit like Horizon's stage: tracked files only, links materialised."""
    if git(sunrise, "rev-parse", revision + "^{commit}") != revision:
        raise SystemExit("the Sunrise checkout does not contain the pinned commit")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    archive = subprocess.Popen(["git", "-c", "core.autocrlf=false", "-C", str(sunrise), "archive", "--format=tar", revision], stdout=subprocess.PIPE)
    run(["tar", "-x", "-C", str(destination)], stdin=archive.stdout)
    if archive.wait():
        raise SystemExit("git archive failed")
    for path in sorted(destination.rglob("*")):
        if path.is_symlink():
            target = path.resolve()
            if not target.is_relative_to(destination.resolve()) or not target.is_file():
                raise SystemExit(f"unsupported source symlink: {path.relative_to(destination)}")
            data = target.read_bytes()
            path.unlink()
            path.write_bytes(data)
    (destination / ".horizon-revision").write_text(revision + "\n")


def generated_directory(target):
    found = sorted(target.glob("*/build/sunrise-client-*/out/game_modules.rs"))
    if not found:
        raise SystemExit("no generated game_modules.rs: build sunrise-client once so its build script runs")
    return found[-1].parent


def inline_generated(root, generated):
    """Replace the build-script includes with their content, paths made relative to the includer."""
    count = 0
    for path in sorted((root / "crates/sunrise-client/src").rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        if "OUT_DIR" not in text:
            continue

        def replace(match):
            nonlocal count
            if match["name"] not in INLINED:
                return match[0]
            body = (generated / match["name"]).read_text(encoding="utf-8")
            body = re.sub(
                r'#\[path = "([^"]+)"\]',
                lambda m: '#[path = "%s"]' % os.path.relpath(root / "crates" / m[1].split("/crates/", 1)[1], path.parent),
                body,
            )
            count += 1
            return body

        updated = INCLUDE.sub(replace, text)
        if updated != text:
            path.write_text(updated, encoding="utf-8")
    return count


PATH_MOD = re.compile(
    r'(?P<attrs>(?:[ \t]*#\[[^\]\n]*\][ \t]*\n)*)'
    r'[ \t]*#\[path = "(?P<path>[^"]+)"\][ \t]*\n'
    r'(?P<more>(?:[ \t]*#\[[^\]\n]*\][ \t]*\n)*)'
    r'[ \t]*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+;[ \t]*\n'
)


def flatten_paths(root):
    """The mixin tool refuses `#[path]` with `..`. Sunrise uses it to pull sibling folders into a module:
    copy those files next to the includer; modules reaching outside the crate (test fixtures) are dropped."""
    src = (root / "crates/sunrise-client/src").resolve()
    copied = dropped = 0
    for path in sorted(src.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        if '#[path = "' not in text:
            continue

        def replace(match):
            nonlocal copied, dropped
            target = match["path"]
            if ".." not in Path(target).parts:
                return match[0]
            base = path.parent if path.name == "mod.rs" or path.parent == src else path.parent / path.stem
            resolved = (base / target).resolve()
            if not resolved.is_relative_to(src) or not resolved.is_file():
                dropped += 1
                return ""
            local = path.parent / "__linked" / resolved.name
            local.parent.mkdir(exist_ok=True)
            shutil.copyfile(resolved, local)
            copied += 1
            return match[0].replace(f'"{target}"', f'"__linked/{resolved.name}"')

        updated = PATH_MOD.sub(replace, text)
        if updated != text:
            path.write_text(updated, encoding="utf-8")
    return copied, dropped


LOCAL_INCLUDE = re.compile(r'^[ \t]*include!\("(?P<file>[^"]+\.rs)"\);[ \t]*$', re.M)


def inline_local_includes(root):
    """`include!("file.rs")` is invisible to the mixin tool; paste the file where it is included."""
    src = root / "crates/sunrise-client/src"
    pasted = 0
    for path in sorted(src.rglob("*.rs")):
        if path.name.endswith("_tests.rs") or not path.exists():
            continue
        text = path.read_text(encoding="utf-8")

        def replace(match):
            nonlocal pasted
            target = path.parent / match["file"]
            if not target.is_file():
                return match[0]
            pasted += 1
            body = target.read_text(encoding="utf-8")
            target.unlink()
            return body

        updated = LOCAL_INCLUDE.sub(replace, text)
        if updated != text:
            path.write_text(updated, encoding="utf-8")
    return pasted


MARKER = re.compile(r"^[ \t]*#\[sunrise_mod_api_macros::replaceable\][ \t]*\n", re.M)


def strip_markers(root):
    """`#[replaceable]` only reserves a function-replacement slot for the launcher's build; the
    mixin tool refuses unexpanded attribute macros, and a source-level check does not need the slot."""
    removed = 0
    for path in sorted((root / "crates").rglob("*.rs")):
        text = path.read_text(encoding="utf-8", errors="ignore")
        if "replaceable" not in text:
            continue
        updated, count = MARKER.subn("", text)
        if count:
            path.write_text(updated, encoding="utf-8")
            removed += count
    return removed


def ensure_mixins_tool(compat):
    """The mixin tool Sunrise runs: the pinned horizon-mixins revision plus Sunrise's one patch."""
    work = ROOT / ".local" / "horizon-mixins-sunrise"
    revision = compat["mixins_revision"]
    if not (work / ".git").exists():
        work.parent.mkdir(parents=True, exist_ok=True)
        run(["git", "init", str(work)], stdout=sys.stderr)
        run(["git", "-C", str(work), "remote", "add", "origin", compat["mixins_repository"]])
    head = subprocess.run(["git", "-C", str(work), "rev-parse", "--verify", "HEAD"], capture_output=True, text=True)
    if head.returncode != 0 or head.stdout.strip() != revision:
        run(["git", "-C", str(work), "fetch", "--depth", "1", "origin", revision], stdout=sys.stderr)
        run(["git", "-C", str(work), "checkout", "--detach", "--force", revision], stdout=sys.stderr)
    return work


def command_freeze(args):
    """Write where each injection lives in the Sunrise tree, so a bundle can be generated without one."""
    platform = load_platform()
    manifest = tomllib.loads((BUNDLE / "horizon-mixin.toml").read_text(encoding="utf-8"))
    load_text_rules(platform)
    rules = platform.get("injection", {})
    rows = {}
    for injection in manifest["injection"]:
        rule = rules.get(injection["id"], {})
        if rule.get("drop") or "target" in rule:
            continue
        crate, modules, owner, function, found = resolve(args.sunrise, injection["target"])
        if [f for f in found if f[0] == crate and f[1] == modules]:
            rows[injection["id"]] = rename_deep(injection["target"])
            continue
        best = choose_candidate(found)
        if best is None:
            raise SystemExit(f"injection {injection['id']}: no unique Sunrise target; add it to platform.toml")
        rows[injection["id"]] = "::".join([best[0], *best[1], *([best[2]] if best[2] else []), function])
    lines = ["# Where each injection's target lives in the pinned Sunrise tree (python tools/sunrise.py freeze).",
             "# Rules in platform.toml win over these.", "[target]"]
    lines += [f"{key} = {json.dumps(value)}" for key, value in sorted(rows.items())]
    (PLATFORM / "targets.toml").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"froze {len(rows)} targets")


def command_stage(args):
    compat = json.loads((ROOT / "compatibility.json").read_text(encoding="utf-8"))["sunrise"]
    revision = compat["revision"]
    original = OUT / "original" / revision
    indexed = OUT / "indexed" / revision
    if not original.exists():
        export_sunrise(args.sunrise, revision, original)
    check_target = OUT / "check-original"
    if args.generated is None:
        run(["cargo", "check", "-p", "sunrise-client", "--lib", "--locked", "-j", str(args.jobs),
             "--target-dir", str(check_target)], cwd=original)
        generated = generated_directory(check_target)
    else:
        generated = args.generated
    if indexed.exists():
        shutil.rmtree(indexed)
    shutil.copytree(original, indexed, symlinks=True, ignore=shutil.ignore_patterns("target"))
    print(f"inlined {inline_generated(indexed, generated)} generated includes", file=sys.stderr)
    print(f"pasted {inline_local_includes(indexed)} local includes", file=sys.stderr)
    print(f"stripped {strip_markers(indexed)} replaceable markers", file=sys.stderr)
    print("linked/dropped #[path] modules: %d/%d" % flatten_paths(indexed), file=sys.stderr)
    command_generate(argparse.Namespace(sunrise=indexed))
    # The launcher names the mod-render crate by its package; in source it is the `mod_render` library.
    for path in (OUT / "mods").rglob("*"):
        if path.is_file() and path.suffix in (".rs", ".toml"):
            text = path.read_text(encoding="utf-8")
            if "sunrise_mod_render::" in text:
                path.write_text(text.replace("sunrise_mod_render::", "mod_render::"), encoding="utf-8")
    retarget_for_stage(OUT / "mods")
    tool = ensure_mixins_tool(compat)
    cli_target = OUT / "mixins-cli"
    patch = ROOT / compat["mixins_patch"]
    if patch.exists() and not (tool / ".patched").exists():
        run(["git", "-C", str(tool), "apply", str(patch)])
        (tool / ".patched").write_text(compat["mixins_revision"])
    run(["cargo", "build", "--locked", "-p", "horizon-mixins-cli", "--target-dir", str(cli_target)], cwd=tool, stdout=sys.stderr)
    cli = cli_target / "debug" / "horizon-mixins"
    base = ["--workspace", str(indexed), "--mods", str(OUT / "mods")]
    run([str(cli), "plan", *base, "--out", str(OUT / "plan.json")])
    result = run([str(cli), "apply", *base, "--out", str(OUT / "modded")], capture_output=True, text=True)
    print(result.stdout)
    prepare_build(OUT / "modded")
    add_mod_crate(OUT / "modded", OUT / "mods" / "draimhorizon", compat)
    relax_for_launcher_access(OUT / "modded")
    if args.check:
        # Not --locked: the staged workspace gains the mod crate, so its lock file changes.
        run(["cargo", "check", "-p", "sunrise-client", "--lib", "-j", str(args.jobs),
             "--target-dir", str(OUT / "check-modded")], cwd=OUT / "modded")
        print("sunrise-client compiles with the bundle applied")


# The launcher (smod) and the source stage name a few targets differently. The launcher's function
# index spells a trait as `HumanoidSetup`; the source declares `impl anim::HumanoidSetup for Game`
# and the mixin tool wants the trait path exactly as written there.
STAGE_TARGETS = {
    "as HumanoidSetup>::before_arm_pose": "as anim::HumanoidSetup>::before_arm_pose",
}


def retarget_for_stage(mods):
    """Apply STAGE_TARGETS to the generated manifest of the source stage (smod keeps its own spelling)."""
    for path in mods.rglob("horizon-mixin.toml"):
        text = path.read_text(encoding="utf-8")
        updated = text
        for launcher, source in STAGE_TARGETS.items():
            updated = updated.replace(launcher, source)
        if updated != text:
            path.write_text(updated, encoding="utf-8")


# What the launcher grants a native mod and the source stage must grant by patching the Sunrise source
# (file, pattern, replacement, reason). Without them the check fails on access, not on a bad target.
LAUNCHER_ACCESS = [
    ("crates/sunrise-client/src/lib.rs", r"^#!\[forbid\(unsafe_code\)\]\n", "",
     "the launcher compiles mod modules without this lint; ghost_render.rs erases a pointer with `unsafe`"),
    ("crates/sunrise-client-render/src/minimap.rs", r"^    tiles: FxHashMap", "    pub tiles: FxHashMap",
     "the launcher's internals expose Minimap::tiles to terrain_bridge.rs"),
    ("crates/sunrise-client-render/src/minimap.rs", r"^struct Tile \{", "pub struct Tile {", "...and its Tile type"),
    ("crates/sunrise-client-render/src/minimap.rs", r"^    (color|height|depth|scanned): ", r"    pub \1: ", "...and the Tile fields it reads"),
]


def relax_for_launcher_access(modded):
    for name, pattern, replacement, reason in LAUNCHER_ACCESS:
        path = modded / name
        text = path.read_text(encoding="utf-8")
        updated, count = re.subn(pattern, replacement, text, flags=re.M)
        if not count:
            raise SystemExit(f"{name}: `{pattern}` not found ({reason}); update LAUNCHER_ACCESS")
        path.write_text(updated, encoding="utf-8")


def add_mod_crate(modded, bundle, compat):
    """The launcher builds `mod_draimhorizon` (platforms/sunrise/smod/lib.rs plus overlay/crate/*.rs) as the
    mod crate that game-side code calls into; add it to the staged workspace as a path crate so the source
    check compiles those calls, and make the crates that use it depend on it."""
    crate = modded / "crates" / "mod-draimhorizon"
    (crate / "src").mkdir(parents=True)
    version = tomllib.loads((bundle / "horizon-mixin.toml").read_text(encoding="utf-8"))["mod"]["version"]
    (crate / "Cargo.toml").write_text(
        f'[package]\nname = "mod-draimhorizon"\nversion = "{version}"\nedition = "2024"\npublish = false\n\n'
        '[dependencies]\nmod_api = { package = "sunrise-mod-api", path = "../sunrise-mod-api" }\n'
        'webpki-roots = "1"\nrustls-pki-types = { version = "1", features = ["web"] }\nzeroize = "1"\n',
        encoding="utf-8")
    files = sorted((bundle / "crate").glob("*.rs"))
    lib = (PLATFORM / "smod" / "lib.rs").read_text(encoding="utf-8") + "".join(f"pub mod {f.stem};\n" for f in files)
    (crate / "src" / "lib.rs").write_text(lib, encoding="utf-8")
    for path in files:
        shutil.copyfile(path, crate / "src" / path.name)
    users = {p.relative_to(modded / "crates").parts[0] for p in (modded / "crates").glob("*/src/**/*.rs")
             if "mod_draimhorizon" in p.read_text(encoding="utf-8", errors="ignore")} - {"mod-draimhorizon"}
    # sunrise-client-lib compiles sunrise-client's files through `#[path]`, so it needs the dependency too.
    users |= {"sunrise-client-lib"} if "sunrise-client" in users else set()
    for name in sorted(users):
        manifest = modded / "crates" / name / "Cargo.toml"
        text = manifest.read_text(encoding="utf-8")
        manifest.write_text(text.replace("[dependencies]\n", '[dependencies]\nmod_draimhorizon = { package = "mod-draimhorizon", path = "../mod-draimhorizon" }\n', 1), encoding="utf-8")
    root = modded / "Cargo.toml"
    root.write_text(root.read_text(encoding="utf-8").replace("members = [\n", 'members = [\n    "crates/mod-draimhorizon",\n', 1), encoding="utf-8")
    print(f"mod crate added; used by {', '.join(sorted(users))}", file=sys.stderr)


def prepare_build(modded):
    """The build script of sunrise-client regenerates the includes that were pasted into the
    source for the mixin tool; the pasted copies are the source of truth for this check."""
    script = modded / "crates/sunrise-client/build.rs"
    text = script.read_text(encoding="utf-8")
    call = re.search(r"    sunrise_mod_api_gen::generate\([^;]*\)\s*\.expect\(\"generating native mod API\"\);\n", text)
    if call is None:
        raise SystemExit("sunrise-client/build.rs changed: update prepare_build")
    script.write_text(text.replace(call[0], ""), encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    freeze_parser = sub.add_parser("freeze")
    freeze_parser.add_argument("--sunrise", type=Path, required=True, help="the staged Sunrise tree (target/sunrise/indexed/<revision>)")
    freeze_parser.set_defaults(run=command_freeze)
    stage_parser = sub.add_parser("stage")
    stage_parser.add_argument("--sunrise", type=Path, required=True, help="Sunrise Git checkout holding the pinned commit")
    stage_parser.add_argument("--generated", type=Path, help="OUT_DIR of a previous sunrise-client build (skips the baseline check)")
    stage_parser.add_argument("--jobs", type=int, default=2)
    stage_parser.add_argument("--check", action="store_true", help="cargo check the modified sunrise-client after applying")
    stage_parser.set_defaults(run=command_stage)
    generate_parser = sub.add_parser("generate")
    generate_parser.add_argument("--sunrise", type=Path, help="Sunrise checkout, to resolve moved targets")
    generate_parser.set_defaults(run=command_generate)
    smod_parser = sub.add_parser("smod")
    smod_parser.add_argument("--sunrise", type=Path, help="Sunrise checkout, to resolve moved targets")
    smod_parser.set_defaults(run=command_smod)
    check_parser = sub.add_parser("smod-check", help="offline validation of the packed .smod against the SDK")
    check_parser.add_argument("--smod", type=Path, help="default: target/draimhorizons-<version>.smod")
    check_parser.add_argument("--sdk", type=Path, help="default: .local/sdk-<revision[:7]>")
    check_parser.add_argument("--fetch", action="store_true", help="download the SDK release sdk-<revision> first (SHA-256 verified)")
    check_parser.set_defaults(run=command_smod_check)
    resolve_parser = sub.add_parser("resolve")
    resolve_parser.add_argument("--sunrise", type=Path, required=True)
    resolve_parser.set_defaults(run=command_resolve)
    args = parser.parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
