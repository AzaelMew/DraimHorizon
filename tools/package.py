#!/usr/bin/env python3
"""Create a deterministic, source-only Horizon native mod archive.

Adapted from horizon-native-mod-template tools/package.py (MIT).
See notices/LICENSE-horizon-native-mod-template.
"""
import hashlib
import json
from pathlib import Path
import re
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
GAME = "26.3"


def package():
    compat = json.loads((ROOT / "compatibility.json").read_text())
    manifests = sorted((ROOT / "mixins").glob("*/horizon-mixin.toml"))
    if len(manifests) != 1:
        raise ValueError("a .hnmod requires exactly one mixins/<id>/horizon-mixin.toml")
    bundle = manifests[0].parent
    mixin = tomllib.loads(manifests[0].read_text())
    mod = mixin["mod"]
    if re.fullmatch(r"[a-z0-9_]{1,64}", mod["id"]) is None:
        raise ValueError("mod id must contain 1-64 lowercase ASCII letters, digits or underscores")
    if bundle.name != mod["id"]:
        raise ValueError("bundle directory must match mod id")
    if mixin.get("compatibility", {}).get("horizon_revision") != compat["horizon_revision"]:
        raise ValueError("bundle and compatibility.json pin different Horizon revisions")
    for key in ("horizon_revision", "horizon_mixins_revision"):
        if re.fullmatch(r"[0-9a-f]{40}", compat[key]) is None:
            raise ValueError(f"{key} must be a full Git commit SHA")
    manifest = dict(schema=1, id=mod["id"], version=mod["version"], game=GAME,
                    horizon=compat["horizon_revision"], mixins=compat["horizon_mixins_revision"])
    files = {"horizon-native.json": (json.dumps(manifest, sort_keys=True, indent=2) + "\n").encode()}
    for path in sorted(bundle.rglob("*")):
        if path.is_symlink():
            raise ValueError(f"bundle must not contain symbolic links: {path.relative_to(ROOT)}")
        if path.is_file():
            if path.suffix not in (".rs", ".toml"):
                raise ValueError(f"source bundle contains unsupported file: {path.relative_to(ROOT)}")
            files[path.relative_to(ROOT).as_posix()] = path.read_bytes()
    out = ROOT / "target" / f"{mod['id']}-{mod['version']}.hnmod"
    out.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, content in sorted(files.items()):
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, content, compresslevel=9)
    report = dict(file=str(out), size=out.stat().st_size,
                  sha256=hashlib.sha256(out.read_bytes()).hexdigest())
    (out.with_suffix(out.suffix + ".sha256")).write_text(f"{report['sha256']}  {out.name}\n")
    print(json.dumps(report, indent=2))
    return out


if __name__ == "__main__":
    try:
        package()
    except (OSError, ValueError, KeyError) as error:
        raise SystemExit(f"Package: {error}") from error
