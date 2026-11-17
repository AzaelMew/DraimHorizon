#!/usr/bin/env python3
"""Check this prototype's native replacement prerequisites, not runtime compatibility."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tomllib

PROJECT = Path(__file__).resolve().parent
BRAND = "sunrise_client_lib::net::brand"
SIGNATURE = re.compile(
    r"\bfn\s+brand\s*\(\s*_address\s*:\s*&\s*str\s*,?\s*\)"
    r"\s*->\s*Vec\s*<\s*u8\s*>\s*\{"
)


def verify(manifest, sdk, functions, source):
    """Use SDK ownership and exact source bytes; never infer replaceability by a name search."""
    errors = []

    def reject(code, message, target=None):
        entry = {"code": code, "message": message}
        if target is not None:
            entry["target"] = target
        errors.append(entry)

    native = manifest["native"]
    expected = native["sunrise"]
    for name, document in (("SDK manifest", sdk), ("function index", functions)):
        if document.get("schema") != 1:
            reject("SDK_SCHEMA", f"{name}: expected schema 1")
        if document.get("build", {}).get("commit") != expected:
            reject("SDK_BUILD", f"{name}: expected build {expected}")
    if sdk.get("mixins", {}).get("rev") != native["mixins"]:
        reject("SDK_MIXINS", "SDK mixin tool differs from the native manifest pin")
    if errors:
        return result(errors, expected)

    root = source.resolve()
    for mixin in native.get("mixin", []):
        target = mixin["target"]
        entry = functions.get("functions", {}).get(target)
        if entry is None:
            reason = functions.get("refused", {}).get(target, {}).get("why")
            reject("SDK_TARGET_UNAVAILABLE", reason or (
                "Exact target is absent from the SDK replacement index. A matching source "
                "function or an empty Hooks compile is not sufficient."
            ), target)
            continue
        if target != BRAND or entry.get("module") != "sunrise_client_lib::net":
            reject("SDK_CONTRACT", "This checker only validates the address-aware brand hook", target)
            continue

        # The source path is deliberately independent of the owning crate's name.
        path = (root / entry["file"]).resolve()
        if not path.is_relative_to(root):
            reject("SDK_SOURCE_PATH", "Indexed source escapes the source checkout", target)
            continue
        if not path.is_file():
            reject("SDK_SOURCE_MISSING", f"Indexed source is missing: {entry['file']}", target)
            continue
        data = path.read_bytes()
        sources = functions.get("crates", {}).get("sunrise_client_lib", {}).get(
            "modules", {}
        ).get(entry["module"], [])
        matching = [row for row in sources if row.get("path") == entry["file"]]
        if not matching or any(
            row.get("sha256") != hashlib.sha256(data).hexdigest() for row in matching
        ):
            reject("SDK_SOURCE_HASH", "Source file differs from the SDK's module index", target)
            continue
        span = entry.get("span", [])
        if (len(span) != 2 or any(type(n) is not int for n in span)
                or not 0 <= span[0] < span[1] <= len(data)):
            reject("SDK_SOURCE_SPAN", "Invalid indexed function byte span", target)
            continue
        body = data[span[0]:span[1]]
        if hashlib.sha256(body).hexdigest() != entry.get("sha256"):
            reject("SDK_FUNCTION_HASH", "Function bytes differ from the SDK index", target)
            continue
        try:
            text = body.decode("utf-8")
        except UnicodeDecodeError:
            reject("SDK_SOURCE_SPAN", "Indexed span is not UTF-8 source", target)
            continue
        if not SIGNATURE.search(text):
            reject("SDK_SIGNATURE", "Mixin requires fn brand(_address: &str) -> Vec<u8>", target)
    if not native.get("mixin"):
        reject("SDK_CONTRACT", "No native mixin was declared")
    return result(errors, expected)


def result(errors, build):
    return {
        "ok": not errors,
        "stage": "sdk-prerequisites",
        "build": build,
        "runtime_verified": False,
        "errors": errors,
        "next": (
            "Build through Sunrise's launcher; then test SMP login and extension negotiation."
            if not errors else
            "Resolve the reported SDK contract before attempting a native build."
        ),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdk", type=Path, required=True, help="published sdk.json")
    parser.add_argument("--functions", type=Path, required=True, help="SDK engine functions.json")
    parser.add_argument("--source", type=Path, required=True, help="source of that exact SDK build")
    args = parser.parse_args()
    try:
        manifest = tomllib.loads((PROJECT / "sunrise-mod.toml").read_text(encoding="utf-8"))
        report = verify(
            manifest,
            json.loads(args.sdk.read_text(encoding="utf-8")),
            json.loads(args.functions.read_text(encoding="utf-8")),
            args.source,
        )
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        report = result([{"code": "SDK_INPUT", "message": str(error)}], None)
    print(json.dumps(report, indent=2))
    return 0 if report["ok"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
