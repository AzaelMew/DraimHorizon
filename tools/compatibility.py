#!/usr/bin/env python3
"""Check the Horizon main pin, or refresh it from an existing upstream checkout.

Refresh before each task: python tools/compatibility.py --update --horizon PATH
This updates identity and source hashes, not API adapters: native validation is required.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--update", action="store_true")
    parser.add_argument("--horizon", type=Path)
    args = parser.parse_args()
    path = ROOT / "compatibility.json"
    compat = json.loads(path.read_text(encoding="utf-8"))
    repo = compat["horizon_repository"]
    latest = subprocess.check_output(["git", "ls-remote", repo, "refs/heads/main"], text=True).split()[0]
    current = compat["horizon_revision"]
    if args.update:
        if args.horizon is None or not (args.horizon / ".git").exists():
            parser.error("--update needs --horizon pointing to an existing upstream Git checkout")
        subprocess.run(["git", "-C", str(args.horizon), "fetch", "--depth=1", repo, latest], check=True)
        for source in compat["source_sha256"]:
            data = subprocess.check_output(["git", "-C", str(args.horizon), "show", f"{latest}:{source}"])
            compat["source_sha256"][source] = hashlib.sha256(data).hexdigest()
        compat["horizon_revision"] = latest
        path.write_text(json.dumps(compat, indent=2) + "\n", encoding="utf-8")
        manifest = ROOT / "mixins/claim_borders/horizon-mixin.toml"
        manifest.write_text(manifest.read_text(encoding="utf-8").replace(current, latest), encoding="utf-8")
        print(f"Horizon {current[:7]} -> {latest[:7]}; apply mixins and compile the real client before release.")
    elif current != latest:
        raise SystemExit(f"Horizon pin {current} is behind main {latest}; refresh compatibility and validate before release.")
    else:
        print(f"Horizon main: {latest}")


if __name__ == "__main__":
    main()
