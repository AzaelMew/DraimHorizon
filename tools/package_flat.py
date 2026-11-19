#!/usr/bin/env python3
"""Build the flat native packages the current Horizon and Sunrise launchers install.

Both launchers dropped the old layout (`horizon-mixin.toml`, `.hnmod`): "Old format (.hnmod): ask its
author to republish it as .hmod". They read one flat Cargo project instead:

    horizon-mod.toml (sunrise-mod.toml)   identity, [native] pins, [[native.mixin]], [[native.module]]
    Cargo.toml, src/lib.rs                the mod crate (here: the relay client and an empty Hooks entry)
    src/mixins/**, src/modules/**         the hook helpers and the support modules of the manifest
    LICENSE*, NOTICE*, COPYING*           licence texts (the only non-source files a native package keeps)

The mod is still written once, in mixins/draimhorizon; tools/sunrise.py derives each launcher's project
from it and platforms/<platform> (DRAIM_TARGET picks the platform):

    python tools/package_flat.py horizon    -> target/horizon/smod/draimhorizon/ and target/draimhorizons-<v>.hmod
    python tools/package_flat.py sunrise    -> target/sunrise/smod/draimhorizon/ and target/draimhorizons-<v>.smod
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "mixins" / "draimhorizon"
# The launchers' own limits for a native package (horizon-mod-package, source_file).
MAX_FILES = 511
MAX_BYTES = 64 << 20

PLATFORMS = {
    "horizon": dict(manifest="horizon-mod.toml", extension="hmod"),
    "sunrise": dict(manifest="sunrise-mod.toml", extension="smod"),
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


def project_version():
    return tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]


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


def generated_project(platform):
    """The package is `tools/sunrise.py smod`'s project, the one built and run by the launcher: it inlines
    the relay client as game-side code (the launcher refuses ring, rustls and ureq as registry dependencies
    of a mod crate) and carries the hook and listener adaptations of platforms/<platform>."""
    environment = {**os.environ, "DRAIM_TARGET": platform}
    subprocess.run([sys.executable, str(ROOT / "tools" / "sunrise.py"), "smod"], check=True, stdout=subprocess.DEVNULL, env=environment)
    project = ROOT / "target" / platform / "smod" / "draimhorizon"
    shutil.copyfile(ROOT / "LICENSE", project / "LICENSE")
    for original, target in LICENSES.items():
        shutil.copyfile(BUNDLE / original, project / target)
    return project


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=sorted(PLATFORMS))
    args = parser.parse_args()
    project = generated_project(args.platform)
    artifact = pack(args.platform, project)
    print(json.dumps(dict(project=str(project), artifact=str(artifact), size=artifact.stat().st_size), indent=2))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError) as error:
        sys.exit(f"Package: {error}")
