#!/usr/bin/env python3
"""Raise the patch version of a pull request that left it unchanged.

Used by .github/workflows/auto-version.yml so a forgotten bump never blocks a release. The
pull request title becomes a stub CHANGELOG entry and release note; edit them afterwards.
"""
import argparse
from pathlib import Path
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def read_version(cargo_toml: str) -> str:
    return tomllib.loads(cargo_toml)["package"]["version"]


def needs_bump(base_cargo: str, head_cargo: str) -> bool:
    return read_version(base_cargo) == read_version(head_cargo)


def clean_title(title: str) -> str:
    return " ".join(title.split())[:200] or "Update"


def add_changelog(text: str, version: str, title: str, number: str) -> str:
    if f"## {version}\n" in text or text.endswith(f"## {version}"):
        return text
    entry = f"## {version}\n\n- {clean_title(title)} (#{number}). Replace this line with a concrete entry.\n\n"
    head, separator, rest = text.partition("\n## ")
    if not separator:
        return text.rstrip("\n") + "\n\n" + entry
    return head.rstrip("\n") + "\n\n" + entry + "## " + rest


def release_notes(version: str, title: str) -> str:
    return (
        f"**DraimHorizon {version} - {clean_title(title)} (unreleased)**\n\n"
        "Version raised automatically because the pull request did not change it. "
        "Replace this text with the real notes.\n"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-cargo", required=True, help="Cargo.toml of the target branch")
    parser.add_argument("--title", required=True)
    parser.add_argument("--number", required=True)
    args = parser.parse_args()
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    if not needs_bump(Path(args.base_cargo).read_text(encoding="utf-8"), cargo):
        print("Version already differs from the target branch; nothing to do.")
        return 0
    subprocess.run([sys.executable, str(ROOT / "tools" / "version.py"), "bump", "patch"], check=True)
    version = read_version((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    changelog = ROOT / "CHANGELOG.md"
    changelog.write_text(
        add_changelog(changelog.read_text(encoding="utf-8"), version, args.title, args.number),
        encoding="utf-8",
    )
    notes = ROOT / "docs" / "release-notes.md"
    if f"DraimHorizon {version}" not in notes.read_text(encoding="utf-8"):
        notes.write_text(release_notes(version, args.title), encoding="utf-8")
    print(f"Raised the version to {version}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
