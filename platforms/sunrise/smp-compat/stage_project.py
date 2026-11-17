#!/usr/bin/env python3
"""Stage only the native package inputs; research and test files are not package assets."""
import argparse
from pathlib import Path
import shutil
import tempfile

PROJECT = Path(__file__).resolve().parent


def stage(destination):
    destination = destination.resolve()
    if destination == PROJECT or destination.is_relative_to(PROJECT):
        raise ValueError("stage outside the source project")
    if destination.exists():
        raise ValueError("destination already exists; choose a fresh output directory")
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=".smp-stage-", dir=destination.parent))
    try:
        for name in ("Cargo.toml", "sunrise-mod.toml"):
            shutil.copyfile(PROJECT / name, temporary / name)
        shutil.copytree(PROJECT / "src", temporary / "src")
        temporary.rename(destination)
    finally:
        if temporary.exists():
            shutil.rmtree(temporary)
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    try:
        print(stage(args.destination))
    except (OSError, ValueError) as error:
        parser.exit(2, f"{error}\n")


if __name__ == "__main__":
    main()
