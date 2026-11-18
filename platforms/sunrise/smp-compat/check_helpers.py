#!/usr/bin/env python3
"""Type-check the actual native helpers against published SDK metadata, without game assets."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib

from verify_sdk import PROJECT, verify


def one(directory, pattern):
    matches = list(directory.glob(pattern))
    if len(matches) != 1:
        raise ValueError(f"expected exactly one {pattern} in {directory}")
    return matches[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdk", type=Path, required=True, help="published sdk.json")
    parser.add_argument("--api", type=Path, required=True, help="unpacked API slice")
    parser.add_argument("--engine", type=Path, required=True, help="unpacked engine slice")
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--rustc", default="rustc", help="SDK's exact Rust compiler")
    args = parser.parse_args()
    report = {"ok": False, "stage": "helper-typecheck", "runtime_verified": False}
    try:
        manifest = tomllib.loads((PROJECT / "sunrise-mod.toml").read_text(encoding="utf-8"))
        sdk = json.loads(args.sdk.read_text(encoding="utf-8"))
        prerequisites = verify(
            manifest, sdk,
            json.loads((args.engine / "functions.json").read_text(encoding="utf-8")),
            args.source,
            json.loads((args.api / "internals.json").read_text(encoding="utf-8")),
        )
        if not prerequisites["ok"]:
            raise ValueError(json.dumps(prerequisites["errors"]))
        version = subprocess.check_output([args.rustc, "--version"], text=True)
        if not version.startswith(f"rustc {sdk['toolchain']['rust']} "):
            raise ValueError("rustc differs from the SDK toolchain")
        search = []
        for root in (args.api, args.engine):
            for folder in ("deps", "host"):
                search += ["-L", f"dependency={(root / folder).resolve()}"]
        common = [args.rustc, "--edition", "2024", "--crate-type", "lib", "--emit", "metadata"]
        with tempfile.TemporaryDirectory(prefix="smp-typecheck-") as temporary:
            out = Path(temporary)
            own = out / "libmod_draimhorizon_smp_compat.rmeta"
            subprocess.run(common + search + [
                "--crate-name", "mod_draimhorizon_smp_compat", "-Dwarnings",
                "--extern", f"mod_api={one(args.api / 'deps', 'libmod_api.rmeta').resolve()}",
                str(PROJECT / "src/lib.rs"), "-o", str(own),
            ], check=True)
            report["entry_typechecked"] = True
            # A compile-only context for the real helpers, not a substitute game implementation.
            wrapper = out / "helpers.rs"
            wrapper.write_text(
                "#![allow(dead_code)]\n"
                "extern crate sunrise_client_lib;\nextern crate sunrise_protocol;\nextern crate log;\n"
                "extern crate mod_draimhorizon_smp_compat;\n"
                "use sunrise_client_lib::net::{Session, Event, ingress, queued_writer};\n"
                "use sunrise_protocol::packets::configuration as cfg;\n"
                + "\n".join((PROJECT / row["helper"]).read_text(encoding="utf-8")
                            for row in manifest["native"]["mixin"]),
                encoding="utf-8",
            )
            subprocess.run(common + search + [
                "-Dwarnings", "--extern", f"mod_draimhorizon_smp_compat={own}",
                "--extern", f"sunrise_client_lib={one(args.engine / 'deps', 'libsunrise_client_lib-*.rmeta').resolve()}",
                "--extern", f"sunrise_protocol={one(args.engine / 'deps', 'libsunrise_protocol-*.rmeta').resolve()}",
                # The copies compile with the game crate's dependencies; the helpers log through its `log`.
                "--extern", f"log={one(args.api / 'deps', 'liblog-*.rmeta').resolve()}",
                str(wrapper), "-o", str(out / "libhelpers.rmeta"),
            ], check=True)
            report.update(ok=True, helpers_typechecked=True)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        report["error"] = str(error)
    print(json.dumps(report, indent=2))
    return 0 if report["ok"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
