#!/usr/bin/env python3
"""Build the flat native packages the current Horizon and Sunrise launchers install.

Both launchers dropped the old layout (`horizon-mixin.toml`, `.hnmod`): "Old format (.hnmod): ask its
author to republish it as .hmod". They read one flat Cargo project instead:

    horizon-mod.toml (sunrise-mod.toml)   identity, [native] pins, [[native.mixin]], [[native.module]]
    Cargo.toml, src/lib.rs                the mod crate (here: the relay client and an empty Hooks entry)
    src/mixins/**, src/modules/**         the hook helpers and the support modules of the manifest
    LICENSE*, NOTICE*, COPYING*           licence texts (the only non-source files a native package keeps)

The mod is still written once, in mixins/draimhorizon. This tool only re-lays it out:

    python tools/package_flat.py horizon    -> target/flat/horizon/ and target/draimhorizons-<v>.hmod
    python tools/package_flat.py sunrise    -> target/sunrise/smod/draimhorizon/ (tools/sunrise.py smod)
                                               and target/draimhorizons-<v>.smod
"""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "mixins" / "draimhorizon"
OUT = ROOT / "target" / "flat"
CRATE = "mod-draimhorizon"
LIB = CRATE.replace("-", "_")
GAME = "26.3"
# The launchers' own limits for a native package (horizon-mod-package, source_file).
MAX_FILES = 511
MAX_BYTES = 64 << 20

PLATFORMS = {
    "horizon": dict(
        manifest="horizon-mod.toml", extension="hmod", pin="horizon",
        api_crate="horizon-mod-api", sdk=".horizon/sdk/api/crates/horizon-mod-api", entry="horizon_mod",
    ),
    "sunrise": dict(
        manifest="sunrise-mod.toml", extension="smod", pin="sunrise",
        api_crate="sunrise-mod-api", sdk=".sunrise/sdk/api/crates/sunrise-mod-api", entry="sunrise_mod",
    ),
}
# Licence texts of the ported code: native packages keep only root files with these prefixes.
LICENSES = {
    "baritone/LICENSE": "LICENSE-baritone",
    "baritone/COPYING": "COPYING-baritone",
    "baritone/LICENSE-Part-2.jpg": "LICENSE-baritone-Part-2.jpg",
    "baritone/NOTICE.md": "NOTICE-baritone.md",
    "litematica/LICENSE": "LICENSE-litematica",
    "litematica/NOTICE.md": "NOTICE-litematica.md",
}


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


def emit_row(table, row):
    lines = [f"[[{table}]]"]
    for key, value in row.items():
        if key == "bindings":
            lines.append("bindings = [")
            lines += [f"  {toml_value(b)}," for b in value]
            lines.append("]")
        else:
            lines.append(f"{key} = {toml_value(value)}")
    return lines + [""]


def project_version():
    return tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]


def pins(platform, compat):
    """The exact commits `[native]` names: the game build, and the horizon-mixins tool."""
    if platform == "horizon":
        return compat["horizon_revision"], compat["sunrise"]["mixins_revision"]
    return compat["sunrise"]["revision"], compat["sunrise"]["mixins_revision"]


def manifest_text(platform, bundle_manifest, compat, helpers, modules):
    spec = PLATFORMS[platform]
    game, mixins = pins(platform, compat)
    head = [
        'id = "draimhorizon"',
        'name = "DraimHorizon"',
        f'version = "{project_version()}"',
        'description = "Client tools for Horizon SMP: claims, map and navigation, trading, chat translation, Baritone and schematics."',
        'authors = ["DraimCiDo"]',
        'license = "MIT"',
        'homepage = "https://github.com/DraimCiDo/DraimHorizon"',
        'issues = "https://github.com/DraimCiDo/DraimHorizon/issues"',
        'api = "0.2"',
        'side = "client"',
        '# A native package must name a capability; registries is the only one the manifest defines.',
        'capabilities = ["registries"]',
        "",
        "[native]",
        f'game = "{GAME}"',
        f'{spec["pin"]} = "{game}"',
        f'mixins = "{mixins}"',
        "",
    ]
    lines = list(head)
    for module in bundle_manifest["module"]:
        row = dict(module)
        row["source"] = modules[module["source"]]
        lines += emit_row("native.module", row)
    for injection in bundle_manifest["injection"]:
        row = dict(injection)
        row["helper"] = helpers[injection["helper"]]
        # horizon-mixins' [[injection]] keys, in the order the manifest documents them.
        ordered = {k: row[k] for k in ("id", "target", "kind", "helper") if k in row}
        ordered.update({k: v for k, v in row.items() if k not in ordered})
        lines += emit_row("native.mixin", ordered)
    return "\n".join(lines).rstrip() + "\n"


def crate_text(platform):
    spec = PLATFORMS[platform]
    relay = tomllib.loads((BUNDLE / "relay_client" / "Cargo.toml").read_text(encoding="utf-8"))
    deps = []
    for name, value in relay["dependencies"].items():
        deps.append(f"{name} = {toml_value(value)}")
    return "\n".join([
        "[package]",
        f'name = "{CRATE}"',
        f'version = "{project_version()}"',
        'edition = "2024"',
        'license = "MIT"',
        "",
        "[dependencies]",
        f'# The pinned SDK\'s API crate, fetched into the project by any `cargo {PLATFORMS[platform]["pin"]}-mod` command.',
        f'{spec["api_crate"]} = {{ path = "{spec["sdk"]}" }}',
        "# The relay client's registry dependencies, exact: pin them with the launcher's `native-lock`.",
        *deps,
        "",
    ])


def lib_text(platform):
    """The relay client is the crate root, so its `crate::` paths stay valid; the entry point is appended."""
    root = (BUNDLE / "relay_client" / "src" / "lib.rs").read_text(encoding="utf-8")
    entry = PLATFORMS[platform]["entry"]
    return root.rstrip() + f'''

/// The native entry point. The mod works through the mixins and support modules its manifest declares;
/// it subscribes to no hook of its own.
#[derive(Default)]
struct Mod;

#[mod_api::{entry}]
impl mod_api::Hooks for Mod {{}}
'''


def rename(text):
    return text.replace("claim_borders_relay_client", LIB)


def layout(bundle_manifest, bundle):
    """Where each helper and each module source goes inside src/."""
    helpers = {h: f"src/mixins/{h}" for h in sorted({i["helper"] for i in bundle_manifest["injection"]})}
    modules = {m: f"src/modules/{m}" for m in sorted({m["source"] for m in bundle_manifest["module"]})}
    for table, base in ((helpers, "helper"), (modules, "source")):
        for original in table:
            if not (bundle / original).is_file():
                raise SystemExit(f"{base} file is missing from the bundle: {original}")
    return helpers, modules


def build(platform, bundle, out):
    compat = json.loads((ROOT / "compatibility.json").read_text(encoding="utf-8"))
    bundle_manifest = tomllib.loads((bundle / "horizon-mixin.toml").read_text(encoding="utf-8"))
    helpers, modules = layout(bundle_manifest, bundle)
    if out.exists():
        shutil.rmtree(out)
    (out / "src").mkdir(parents=True)
    spec = PLATFORMS[platform]
    (out / spec["manifest"]).write_text(manifest_text(platform, bundle_manifest, compat, helpers, modules), encoding="utf-8")
    (out / "Cargo.toml").write_text(crate_text(platform), encoding="utf-8")
    (out / "src" / "lib.rs").write_text(lib_text(platform), encoding="utf-8")
    for name in ("crypto.rs", "translation.rs", "translation_languages.rs", "websocket.rs"):
        shutil.copyfile(BUNDLE / "relay_client" / "src" / name, out / "src" / name)
    for original, target in {**helpers, **modules}.items():
        destination = out / target
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(rename((bundle / original).read_text(encoding="utf-8")), encoding="utf-8")
    shutil.copyfile(ROOT / "LICENSE", out / "LICENSE")
    for original, target in LICENSES.items():
        shutil.copyfile(BUNDLE / original, out / target)
    (out / "README.md").write_text(
        "# DraimHorizon\n\nClient tools for Horizon SMP. Source and documentation: "
        "https://github.com/DraimCiDo/DraimHorizon\n", encoding="utf-8")
    return out


def package_files(project):
    """The files a launcher packs, as it selects them: Cargo.toml, src/**, root licence and readme files."""
    files = {}
    for path in sorted(project.rglob("*")):
        if not path.is_file():
            continue
        name = path.relative_to(project).as_posix()
        files[name] = path.read_bytes()
    if len(files) > MAX_FILES or sum(map(len, files.values())) > MAX_BYTES:
        raise SystemExit("the native package exceeds the launchers' entry or size limit")
    return files


def pack(platform, project):
    spec = PLATFORMS[platform]
    files = package_files(project)
    manifest = spec["manifest"]
    name = f"draimhorizons-{project_version()}.{spec['extension']}"
    out = ROOT / "target" / name
    with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for entry in [manifest, *sorted(n for n in files if n != manifest)]:
            info = zipfile.ZipInfo(entry, (2026, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, files[entry], compresslevel=9)
    import hashlib
    out.with_suffix(out.suffix + ".sha256").write_text(f"{hashlib.sha256(out.read_bytes()).hexdigest()}  {out.name}\n")
    return out


def sunrise_project():
    """The Sunrise package is `tools/sunrise.py smod`'s project, the one built and run by the Sunrise
    launcher: it inlines the relay client as game-side code (the launcher refuses ring, rustls and ureq
    as registry dependencies of a mod crate) and carries the Sunrise hook and listener adaptations."""
    subprocess.run([sys.executable, str(ROOT / "tools" / "sunrise.py"), "smod"], check=True, stdout=subprocess.DEVNULL)
    project = ROOT / "target" / "sunrise" / "smod" / "draimhorizon"
    shutil.copyfile(ROOT / "LICENSE", project / "LICENSE")
    for original, target in LICENSES.items():
        shutil.copyfile(BUNDLE / original, project / target)
    return project


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=sorted(PLATFORMS))
    args = parser.parse_args()
    if args.platform == "sunrise":
        project = sunrise_project()
    else:
        project = build(args.platform, BUNDLE, OUT / args.platform)
    artifact = pack(args.platform, project)
    print(json.dumps(dict(project=str(project), artifact=str(artifact), size=artifact.stat().st_size), indent=2))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError) as error:
        sys.exit(f"Package: {error}")
