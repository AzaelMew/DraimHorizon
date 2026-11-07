#!/usr/bin/env python3
"""Check or increment the three-part version of every native mod entry point."""
import argparse
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
FILES = {
    "Cargo.toml": r'(?m)^version = "(\d+\.\d+\.\d+)"$',
    "Cargo.lock": r'(?m)(?<=name = "claim-borders"\n)version = "(\d+\.\d+\.\d+)"$',
    "mixins/claim_borders/horizon-mixin.toml": r'(?m)^version = "(\d+\.\d+\.\d+)"$',
    "mixins/claim_borders/runtime.rs": r'(?m)^pub const VERSION: &str = "(\d+\.\d+\.\d+)";$',
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("check")
    bump = sub.add_parser("bump")
    bump.add_argument("part", choices=("patch", "minor", "major"))
    args = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    sources = []
    for name, pattern in FILES.items():
        path = ROOT / name
        data = path.read_text(encoding="utf-8")
        matches = list(re.finditer(pattern, data))
        if len(matches) != 1 or matches[0].group(1) != version:
            parser.error(f"{name} must contain the same version as Cargo.toml ({version})")
        sources.append((path, data, matches[0]))
    if args.command == "check":
        print(f"Version {version} is consistent across {len(FILES)} files.")
        return
    major, minor, patch = map(int, version.split("."))
    if args.part == "major":
        major, minor, patch = major + 1, 0, 0
    elif args.part == "minor":
        minor, patch = minor + 1, 0
    else:
        patch += 1
    updated = f"{major}.{minor}.{patch}"
    for path, data, match in sources:
        start, end = match.span(1)
        path.write_text(data[:start] + updated + data[end:], encoding="utf-8")
    print(f"{version} -> {updated}. Update CHANGELOG.md and docs/release-notes.md for this task.")


if __name__ == "__main__":
    main()
