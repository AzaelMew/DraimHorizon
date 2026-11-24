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
import tempfile

ROOT = Path(__file__).resolve().parents[1]
NIGHTLY_REF = "refs/tags/nightly"
DEV_REF_PATTERN = "refs/tags/dev-*"
DEV_TAG = re.compile(r"refs/tags/dev-(\d{4}-\d{2}-\d{2})-[0-9a-f]+")


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


def sunrise_candidates(refs):
    """The Sunrise builds worth pinning: the Nightly tag and the newest day's Dev tags, as {sha: channel}."""
    entries = {}
    for line in refs.splitlines():
        if line.strip():
            sha, ref = line.split()
            entries[ref.removesuffix("^{}")] = sha if ref.endswith("^{}") or ref not in entries else entries[ref]
    found = {}
    nightly = entries.get(NIGHTLY_REF)
    if nightly and re.fullmatch(r"[0-9a-f]{40}", nightly):
        found[nightly] = "nightly"
    days = {ref: m.group(1) for ref in entries if (m := DEV_TAG.fullmatch(ref))}
    newest = max(days.values(), default=None)
    for ref, day in days.items():
        if day == newest and re.fullmatch(r"[0-9a-f]{40}", entries[ref]):
            found[entries[ref]] = "dev"
    if not found:
        raise ValueError("Sunrise has neither a Nightly nor a Dev tag with a valid source revision")
    return found


def latest_sunrise(repository):
    """(revision, channel) of the most recently committed official Sunrise Nightly or Dev build."""
    refs = subprocess.check_output(
        ["git", "ls-remote", repository, NIGHTLY_REF, NIGHTLY_REF + "^{}", DEV_REF_PATTERN, DEV_REF_PATTERN + "^{}"], text=True
    )
    candidates = sunrise_candidates(refs)
    if len(candidates) == 1:
        return next(iter(candidates.items()))
    with tempfile.TemporaryDirectory() as scratch:
        subprocess.run(["git", "init", "-q", "--bare", scratch], check=True)
        subprocess.run(["git", "-C", scratch, "fetch", "-q", "--depth=1", repository, *candidates], check=True)
        dated = [(int(subprocess.check_output(["git", "-C", scratch, "log", "-1", "--format=%ct", sha], text=True)),
                  channel == "dev", sha) for sha, channel in candidates.items()]
    _, _, sha = max(dated)
    return sha, candidates[sha]


def refresh_sunrise(compat, checkout):
    """Pin the newest official Sunrise build (Nightly or Dev) and the hashes of the sources the adapter reads."""
    sunrise = compat["sunrise"]
    if checkout is None or not (checkout / ".git").exists():
        raise SystemExit("--update-sunrise needs --sunrise pointing to an existing Sunrise Git checkout")
    latest, channel = latest_sunrise(sunrise["repository"])
    subprocess.run(["git", "-C", str(checkout), "fetch", "--depth=1", sunrise["repository"], latest], check=True)
    for source in sunrise["source_sha256"]:
        data = subprocess.check_output(["git", "-C", str(checkout), "show", f"{latest}:{source}"])
        sunrise["source_sha256"][source] = hashlib.sha256(data).hexdigest()
    previous, sunrise["revision"], sunrise["channel"] = sunrise["revision"], latest, channel
    return previous, latest


def horizon_problem(compat):
    """The published Horizon Nightly and what is wrong with the pin, if anything."""
    latest = latest_nightly(compat["horizon_repository"])
    if compat["horizon_revision"] != latest or compat.get("horizon_channel") != "nightly":
        return latest, (f"Horizon pin {compat['horizon_revision']} must target official Nightly {latest}; "
                        "refresh compatibility and validate before release.")
    return latest, None


def sunrise_problem(compat):
    """The newest published Sunrise build and what is wrong with the pin, if anything (None without a Sunrise pin)."""
    sunrise = compat.get("sunrise")
    if sunrise is None:
        return None, None
    latest, channel = latest_sunrise(sunrise["repository"])
    if sunrise["revision"] != latest or sunrise.get("channel") != channel:
        return latest, (f"Sunrise pin {sunrise['revision']} must target the newest official {channel} build {latest}; "
                        "refresh compatibility with --update-sunrise and validate before a Sunrise release.")
    return latest, None


def check_pins(compat, strict_sunrise):
    """Check both pins and report both. Horizon is always an error; Sunrise is experimental, so a stale
    pin is a warning unless strict_sunrise. Returns (lines to print, errors)."""
    lines, errors = [], []
    latest, problem = horizon_problem(compat)
    if problem:
        errors.append(problem)
    else:
        lines.append(f"Horizon official Nightly: {latest}")
    latest, problem = sunrise_problem(compat)
    if problem:
        (errors if strict_sunrise else lines).append(problem if strict_sunrise else "warning: " + problem)
    elif latest:
        lines.append(f"Sunrise newest official build: {latest}")
    return lines, errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--update", action="store_true")
    parser.add_argument("--horizon", type=Path)
    parser.add_argument("--update-sunrise", action="store_true")
    parser.add_argument("--sunrise", type=Path)
    parser.add_argument("--strict-sunrise", action="store_true", help="fail when the Sunrise pin is not the newest published build")
    args = parser.parse_args()
    path = ROOT / "compatibility.json"
    compat = json.loads(path.read_text(encoding="utf-8"))
    if args.update_sunrise:
        previous, latest = refresh_sunrise(compat, args.sunrise)
        path.write_text(json.dumps(compat, indent=2) + "\n", encoding="utf-8")
        print(f"Sunrise official {compat['sunrise']['channel']} build {previous[:7]} -> {latest[:7]}; stage the Sunrise target and compile it before release.")
        return
    if not args.update:
        lines, errors = check_pins(compat, args.strict_sunrise)
        print("\n".join(lines))
        if errors:
            raise SystemExit("\n".join(errors))
        return
    repo = compat["horizon_repository"]
    latest = latest_nightly(repo)
    current = compat["horizon_revision"]
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


if __name__ == "__main__":
    main()
