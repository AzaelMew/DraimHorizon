#!/usr/bin/env python3
"""Check the official Horizon and Sunrise Nightly pins, or refresh them from upstream checkouts.

Refresh before each task: python tools/compatibility.py --update --horizon PATH
and, for the Sunrise target: python tools/compatibility.py --update-sunrise --sunrise PATH
This updates identity and source hashes, not API adapters: native validation is required.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
NIGHTLY_REF = "refs/tags/nightly"


def resolve_nightly_revision(refs):
    """Use the official Nightly tag's commit, peeling annotated tags when present.

    Never fall back to main: it can differ from the published Nightly source.
    """
    entries = dict(line.split()[::-1] for line in refs.splitlines() if line.strip())
    revision = entries.get(NIGHTLY_REF + "^{}", entries.get(NIGHTLY_REF, ""))
    if re.fullmatch(r"[0-9a-f]{40}", revision) is None:
        raise ValueError("official Horizon Nightly tag is missing or has an invalid source revision")
    return revision


def latest_nightly(repository):
    refs = subprocess.check_output(
        ["git", "ls-remote", repository, NIGHTLY_REF, NIGHTLY_REF + "^{}"], text=True
    )
    return resolve_nightly_revision(refs)


def refresh_sunrise(compat, checkout):
    """Pin the official Sunrise Nightly commit and the hashes of the sources the adapter reads."""
    sunrise = compat["sunrise"]
    if checkout is None or not (checkout / ".git").exists():
        raise SystemExit("--update-sunrise needs --sunrise pointing to an existing Sunrise Git checkout")
    latest = latest_nightly(sunrise["repository"])
    subprocess.run(["git", "-C", str(checkout), "fetch", "--depth=1", sunrise["repository"], latest], check=True)
    for source in sunrise["source_sha256"]:
        data = subprocess.check_output(["git", "-C", str(checkout), "show", f"{latest}:{source}"])
        sunrise["source_sha256"][source] = hashlib.sha256(data).hexdigest()
    previous, sunrise["revision"], sunrise["channel"] = sunrise["revision"], latest, "nightly"
    return previous, latest


def check_sunrise(compat, strict):
    """Sunrise publishes daily and its support is experimental: a stale pin warns unless --strict-sunrise."""
    sunrise = compat.get("sunrise")
    if sunrise is None:
        return None
    latest = latest_nightly(sunrise["repository"])
    if sunrise["revision"] != latest or sunrise.get("channel") != "nightly":
        message = (f"Sunrise pin {sunrise['revision']} must target official Nightly {latest}; "
                   "refresh compatibility with --update-sunrise and validate before a Sunrise release.")
        if strict:
            raise SystemExit(message)
        print("warning: " + message)
    return latest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--update", action="store_true")
    parser.add_argument("--horizon", type=Path)
    parser.add_argument("--update-sunrise", action="store_true")
    parser.add_argument("--sunrise", type=Path)
    parser.add_argument("--strict-sunrise", action="store_true", help="fail when the Sunrise pin is not the published Nightly")
    args = parser.parse_args()
    path = ROOT / "compatibility.json"
    compat = json.loads(path.read_text(encoding="utf-8"))
    if args.update_sunrise:
        previous, latest = refresh_sunrise(compat, args.sunrise)
        path.write_text(json.dumps(compat, indent=2) + "\n", encoding="utf-8")
        print(f"Sunrise official Nightly {previous[:7]} -> {latest[:7]}; stage the Sunrise target and compile it before release.")
        return
    repo = compat["horizon_repository"]
    latest = latest_nightly(repo)
    current = compat["horizon_revision"]
    if args.update:
        if args.horizon is None or not (args.horizon / ".git").exists():
            parser.error("--update needs --horizon pointing to an existing upstream Git checkout")
        subprocess.run(["git", "-C", str(args.horizon), "fetch", "--depth=1", repo, latest], check=True)
        for source in compat["source_sha256"]:
            data = subprocess.check_output(["git", "-C", str(args.horizon), "show", f"{latest}:{source}"])
            compat["source_sha256"][source] = hashlib.sha256(data).hexdigest()
        compat["horizon_revision"] = latest
        compat["horizon_channel"] = "nightly"
        path.write_text(json.dumps(compat, indent=2) + "\n", encoding="utf-8")
        manifest = ROOT / "mixins/draimhorizon/horizon-mixin.toml"
        manifest.write_text(manifest.read_text(encoding="utf-8").replace(current, latest), encoding="utf-8")
        print(f"Horizon official Nightly {current[:7]} -> {latest[:7]}; apply mixins and compile the real client before release.")
    elif current != latest or compat.get("horizon_channel") != "nightly":
        raise SystemExit(f"Horizon pin {current} must target official Nightly {latest}; refresh compatibility and validate before release.")
    else:
        print(f"Horizon official Nightly: {latest}")
        sunrise = check_sunrise(compat, args.strict_sunrise)
        if sunrise:
            print(f"Sunrise official Nightly: {sunrise}")


if __name__ == "__main__":
    main()
