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


def horizon_problem(compat):
    """The published Horizon Nightly and what is wrong with the pin, if anything."""
    latest = latest_nightly(compat["horizon_repository"])
    if compat["horizon_revision"] != latest or compat.get("horizon_channel") != "nightly":
        return latest, (f"Horizon pin {compat['horizon_revision']} must target official Nightly {latest}; "
                        "refresh compatibility and validate before release.")
    return latest, None


def tag_dates(refs, prefix):
    """{commit: [date, ...]} of the `<prefix><YYYY-MM-DD>-<short>` tags in `git ls-remote` output; an
    annotated tag's peeled line (`^{}`) names its commit."""
    dates = {}
    for line in refs.splitlines():
        found = re.fullmatch(
            rf"([0-9a-f]{{40}})\trefs/tags/{re.escape(prefix)}(\d{{4}}-\d{{2}}-\d{{2}})-[0-9a-f]+(?:\^\{{\}})?", line)
        if found:
            dates.setdefault(found[1], []).append(found[2])
    return dates


def dev_problem(sunrise, latest):
    """What is wrong with a Sunrise Dev pin, if anything. A Dev build is allowed only while it is a published
    Dev build (a `dev-<date>-<short>` tag) newer than the published Nightly (its `nightly-source/<date>-<short>`
    tag): it carries something the port needs that no Nightly has yet. Once a Nightly as new is published,
    the pin moves to it."""
    if sunrise["revision"] == latest:
        return f"Sunrise pin {latest} is the official Nightly: set its channel to \"nightly\"."
    refs = subprocess.check_output(
        ["git", "ls-remote", sunrise["repository"], "refs/tags/dev-*", "refs/tags/nightly-source/*"], text=True)
    dev = tag_dates(refs, "dev-").get(sunrise["revision"])
    if not dev:
        return f"Sunrise Dev pin {sunrise['revision']} is not a published Dev build."
    nightly = tag_dates(refs, "nightly-source/").get(latest, [])
    if not nightly or min(nightly) >= min(dev):
        return (f"Sunrise Dev pin {sunrise['revision']} is not newer than official Nightly {latest}; "
                "refresh compatibility with --update-sunrise and validate before a Sunrise release.")
    return None


def sunrise_problem(compat):
    """The published Sunrise Nightly and what is wrong with the pin, if anything (None without a Sunrise pin)."""
    sunrise = compat.get("sunrise")
    if sunrise is None:
        return None, None
    latest = latest_nightly(sunrise["repository"])
    if sunrise.get("channel") == "dev":
        return latest, dev_problem(sunrise, latest)
    if sunrise["revision"] != latest or sunrise.get("channel") != "nightly":
        return latest, (f"Sunrise pin {sunrise['revision']} must target official Nightly {latest}; "
                        "refresh compatibility with --update-sunrise and validate before a Sunrise release.")
    return latest, None


def check_pins(compat, strict_sunrise, sunrise_only=False):
    """Check selected pins. A stale Sunrise pin is a warning unless strict_sunrise.
    sunrise_only avoids contacting Horizon during an upstream outage."""
    lines, errors = [], []
    if sunrise_only:
        lines.append("Horizon pin check skipped (--sunrise-only).")
    else:
        latest, problem = horizon_problem(compat)
        if problem:
            errors.append(problem)
        else:
            lines.append(f"Horizon official Nightly: {latest}")
    latest, problem = sunrise_problem(compat)
    if problem:
        (errors if strict_sunrise else lines).append(problem if strict_sunrise else "warning: " + problem)
    elif latest and compat["sunrise"].get("channel") == "dev":
        lines.append(f"Sunrise Dev build: {compat['sunrise']['revision']} (newer than official Nightly {latest})")
    elif latest:
        lines.append(f"Sunrise official Nightly: {latest}")
    return lines, errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--update", action="store_true")
    parser.add_argument("--horizon", type=Path)
    parser.add_argument("--update-sunrise", action="store_true")
    parser.add_argument("--sunrise", type=Path)
    parser.add_argument("--sunrise-only", action="store_true", help="check only Sunrise without contacting Horizon")
    parser.add_argument("--strict-sunrise", action="store_true", help="fail when the Sunrise pin is not the published Nightly or a published Dev build newer than it")
    args = parser.parse_args()
    if args.sunrise_only and (args.update or args.update_sunrise):
        parser.error("--sunrise-only is a check-only option")
    path = ROOT / "compatibility.json"
    compat = json.loads(path.read_text(encoding="utf-8"))
    if args.update_sunrise:
        previous, latest = refresh_sunrise(compat, args.sunrise)
        path.write_text(json.dumps(compat, indent=2) + "\n", encoding="utf-8")
        print(f"Sunrise official Nightly {previous[:7]} -> {latest[:7]}; stage the Sunrise target and compile it before release.")
        return
    if not args.update:
        lines, errors = check_pins(compat, args.strict_sunrise, args.sunrise_only)
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
