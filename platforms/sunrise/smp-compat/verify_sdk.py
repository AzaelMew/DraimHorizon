#!/usr/bin/env python3
"""Check this prototype's native replacement prerequisites, not runtime compatibility."""
import argparse
import hashlib
import json
from pathlib import Path
import tomllib

PROJECT = Path(__file__).resolve().parent
POLL = "sunrise_client_lib::net::Session::poll"
RECEIVE = "sunrise_client_lib::net::receive_brand"
CONTRACTS = {POLL: "c8e6d608e3f14914", RECEIVE: "828097a95b45da8f"}
REQUIRED_INTERNALS = {
    ("sunrise_client_lib::net::Session", "struct"),
    ("sunrise_client_lib::net::ingress::Producer", "struct"),
    ("sunrise_mod_runtime::byte_channel::Sender", "struct"),
    ("sunrise_client_lib::net::queued_writer::QueuedWriter", "struct"),
    ("sunrise_client_lib::net::queued_writer::Inner", "struct"),
    ("sunrise_client_lib::net::queued_writer::WriteJob", "enum"),
}


def verify(manifest, sdk, functions, source, internals):
    """Use SDK ownership and exact source bytes; never infer replaceability by a name search."""
    errors = []

    def reject(code, message, target=None):
        entry = {"code": code, "message": message}
        if target is not None:
            entry["target"] = target
        errors.append(entry)

    native = manifest["native"]
    expected = native["sunrise"]
    for name, document in (("SDK manifest", sdk), ("function index", functions),
                           ("internals catalog", internals)):
        if document.get("schema") != 1:
            reject("SDK_SCHEMA", f"{name}: expected schema 1")
        if document.get("build", {}).get("commit") != expected:
            reject("SDK_BUILD", f"{name}: expected build {expected}")
    if sdk.get("mixins", {}).get("rev") != native["mixins"]:
        reject("SDK_MIXINS", "SDK mixin tool differs from the native manifest pin")
    if errors:
        return result(errors, expected)

    declared = {(row["path"], row.get("kind")): row for row in manifest.get("internals", [])}
    catalog = {(row["path"], row["kind"]): row for row in internals.get("internals", [])}
    for key in REQUIRED_INTERNALS:
        declaration = declared.get(key)
        actual = catalog.get(key)
        if declaration is None or actual is None or declaration.get("hash") != actual.get("hash"):
            reject("SDK_INTERNAL", "Required native internal is missing or has changed", key[0])
    if {mixin["target"] for mixin in native.get("mixin", [])} != set(CONTRACTS):
        reject("SDK_CONTRACT", "Both the session poll and server brand replacements are required")

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
        if target not in CONTRACTS or entry.get("module") != "sunrise_client_lib::net":
            reject("SDK_CONTRACT", "Unexpected replacement contract or owning module", target)
            continue
        if entry.get("signature_hash") != CONTRACTS[target]:
            reject("SDK_SIGNATURE", "Replacement signature differs from the tested SDK contract", target)
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
            body.decode("utf-8")
        except UnicodeDecodeError:
            reject("SDK_SOURCE_SPAN", "Indexed span is not UTF-8 source", target)
            continue
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
    parser.add_argument("--internals", type=Path, required=True, help="SDK API internals.json")
    parser.add_argument("--source", type=Path, required=True, help="source of that exact SDK build")
    args = parser.parse_args()
    try:
        manifest = tomllib.loads((PROJECT / "sunrise-mod.toml").read_text(encoding="utf-8"))
        report = verify(
            manifest,
            json.loads(args.sdk.read_text(encoding="utf-8")),
            json.loads(args.functions.read_text(encoding="utf-8")),
            args.source,
            json.loads(args.internals.read_text(encoding="utf-8")),
        )
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        report = result([{"code": "SDK_INPUT", "message": str(error)}], None)
    print(json.dumps(report, indent=2))
    return 0 if report["ok"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
