#!/usr/bin/env python3
"""Build the Sunrise variant of the mod from the shared sources and platforms/sunrise.

The mod is written once, against Horizon's source (mixins/draimhorizon). Sunrise is a fork whose
crates, module layout and some native hook points differ, so this tool derives the Sunrise mixin
bundle instead of keeping a second copy of the 88k lines:

  resolve   list where each Horizon hook target lives in a Sunrise checkout
  generate  write target/sunrise/mods/<id>/ (manifest + translated sources) for horizon-mixins
  stage     generate, then apply it to the pinned Sunrise source with the pinned mixin tool

Source of truth for differences: platforms/sunrise/platform.toml and platforms/sunrise/overlay/.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "mixins" / "draimhorizon"
PLATFORM = ROOT / "platforms" / "sunrise"
OUT = ROOT / "target" / "sunrise"

# Horizon crate -> Sunrise crate. Longest names first so a prefix never wins.
CRATES = [
    ("horizon_client_render", "sunrise_client_render"),
    ("horizon_client_core", "sunrise_client_core"),
    ("horizon_client_input", "sunrise_client_input"),
    ("horizon_client_lib", "sunrise_client_lib"),
    ("horizon_client", "sunrise_client"),
    ("horizon_protocol", "sunrise_protocol"),
    ("horizon_world", "sunrise_world"),
    ("horizon_core", "sunrise_core"),
    ("horizon-client-render", "sunrise-client-render"),
    ("horizon-client-core", "sunrise-client-core"),
    ("horizon-client-input", "sunrise-client-input"),
    ("horizon-client-lib", "sunrise-client-lib"),
    ("horizon-client", "sunrise-client"),
    ("horizon-protocol", "sunrise-protocol"),
    ("horizon-world", "sunrise-world"),
    ("horizon-core", "sunrise-core"),
]
CRATE_DIRS = {
    "sunrise_client": "sunrise-client",
    "sunrise_client_render": "sunrise-client-render",
    "sunrise_client_lib": "sunrise-client-lib",
    "sunrise_client_core": "sunrise-client-core",
    "sunrise_client_input": "sunrise-client-input",
}


def rename_crates(text):
    for old, new in CRATES:
        text = re.sub(rf"(?<![A-Za-z0-9_-]){re.escape(old)}(?![A-Za-z0-9_-])", new, text)
    return text


def module_path(crate_dir, file):
    """The Rust module path of a source file; folders holding .game-area are not modules."""
    src = crate_dir / "src"
    parts = list(file.relative_to(src).with_suffix("").parts)
    out, current = [], src
    for index, part in enumerate(parts):
        current = current / part
        if index < len(parts) - 1 and (current / ".game-area").exists():
            continue
        out.append(part)
    if out and out[-1] == "mod":
        out.pop()
    return out


def sunrise_index(sunrise):
    """Every client-side source file by crate, with its text."""
    index = {}
    for crate, directory in CRATE_DIRS.items():
        base = sunrise / "crates" / directory
        for path in sorted((base / "src").rglob("*.rs")):
            index.setdefault(crate, []).append((path, base))
    return index


def split_target(target):
    """crate, module segments, owner (type or trait impl), function."""
    segments = re.split(r"::(?![^<]*>)", target)
    crate, rest = segments[0], segments[1:]
    function = rest[-1]
    owner_index = next((i for i, s in enumerate(rest[:-1]) if s[:1].isupper() or s.startswith("<")), len(rest) - 1)
    return crate, rest[:owner_index], "::".join(rest[owner_index:-1]), function


def resolve(sunrise, target):
    crate, modules, owner, function = split_target(rename_crates(target))
    index = sunrise_index(sunrise)
    pattern = re.compile(rf"\bfn\s+{re.escape(function)}\b")
    found = []
    for name in ([crate] if crate in index else list(index)):
        for path, base in index[name]:
            if pattern.search(path.read_text(encoding="utf-8", errors="ignore")):
                found.append((name, module_path(base, path), owner, path.relative_to(sunrise).as_posix()))
    return crate, modules, owner, function, found


def command_resolve(args):
    manifest = tomllib.loads((BUNDLE / "horizon-mixin.toml").read_text(encoding="utf-8"))
    rows = {}
    for injection in manifest["injection"]:
        crate, modules, owner, function, found = resolve(args.sunrise, injection["target"])
        direct = [f for f in found if f[0] == crate and f[1] == modules]
        rows[injection["id"]] = dict(
            target=rename_crates(injection["target"]),
            status="same" if direct else ("moved" if found else "missing"),
            candidates=[
                "::".join([f[0], *f[1], *([f[2]] if f[2] else []), function]) + f"  # {f[3]}"
                for f in found
            ][:6],
        )
    json.dump(rows, sys.stdout, indent=2, ensure_ascii=False)
    print()


def toml_value(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, list):
        return "[" + ", ".join(toml_value(v) for v in value) + "]"
    if isinstance(value, dict):
        return "{ " + ", ".join(f"{k} = {toml_value(v)}" for k, v in value.items()) + " }"
    raise ValueError(f"unsupported TOML value: {value!r}")


def emit_manifest(manifest):
    """Write the subset of TOML the mixin manifest uses; the platform tool owns this format."""
    lines = []
    for name in ("mod", "compatibility"):
        lines += [f"[{name}]", *(f"{k} = {toml_value(v)}" for k, v in manifest[name].items()), ""]
    for section in ("crate", "module", "injection"):
        for entry in manifest.get(section, []):
            lines.append(f"[[{section}]]")
            for key, value in entry.items():
                if key == "bindings":
                    lines.append("bindings = [")
                    lines += [f"  {toml_value(b)}," for b in value]
                    lines.append("]")
                else:
                    lines.append(f"{key} = {toml_value(value)}")
            lines.append("")
    return "\n".join(lines)


def choose_candidate(found):
    """The one non-test definition, or None when the platform file must decide."""
    real = [f for f in found if not re.search(r"(_tests?|/tests?)(\.rs|/)", f[3] + "/")]
    return real[0] if len(real) == 1 else None


def command_generate(args):
    platform = tomllib.loads((PLATFORM / "platform.toml").read_text(encoding="utf-8"))
    compat = json.loads((ROOT / "compatibility.json").read_text(encoding="utf-8"))["sunrise"]
    manifest = tomllib.loads((BUNDLE / "horizon-mixin.toml").read_text(encoding="utf-8"))
    rules = platform.get("injection", {})
    relocate = platform.get("module_file", {})
    out = OUT / "mods" / manifest["mod"]["id"]
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    injections, report = [], []
    for injection in manifest["injection"]:
        rule = rules.get(injection["id"], {})
        if rule.get("drop"):
            report.append((injection["id"], "dropped", rule.get("reason", "")))
            continue
        entry = {k: (rename_crates(v) if isinstance(v, str) else v) for k, v in injection.items()}
        if "target" in rule:
            entry["target"] = rule["target"]
        elif args.sunrise:
            crate, modules, owner, function, found = resolve(args.sunrise, injection["target"])
            if not [f for f in found if f[0] == crate and f[1] == modules]:
                best = choose_candidate(found)
                if best is None:
                    raise SystemExit(f"injection {injection['id']}: no unique Sunrise target; add it to platform.toml")
                entry["target"] = "::".join([best[0], *best[1], *([best[2]] if best[2] else []), function])
        for key in ("kind", "helper", "bindings", "cancellable"):
            if key in rule:
                entry[key] = rule[key]
        injections.append(entry)
    modules = []
    for module in manifest["module"]:
        entry = {k: (rename_crates(v) if isinstance(v, str) else v) for k, v in module.items()}
        entry["file"] = relocate.get(module["file"], entry["file"])
        modules.append(entry)
    manifest_out = dict(
        mod=manifest["mod"],
        compatibility=dict(horizon_revision=compat["revision"]),
        crate=[{k: rename_crates(v) for k, v in c.items()} for c in manifest.get("crate", [])],
        module=modules,
        injection=injections,
    )
    (out / "horizon-mixin.toml").write_text(emit_manifest(manifest_out), encoding="utf-8")
    for path in sorted(BUNDLE.rglob("*")):
        if path.is_file() and path.name != "horizon-mixin.toml":
            target = out / path.relative_to(BUNDLE)
            target.parent.mkdir(parents=True, exist_ok=True)
            if path.suffix == ".rs":
                target.write_text(rename_crates(path.read_text(encoding="utf-8")), encoding="utf-8")
            else:
                shutil.copyfile(path, target)
    overlay = PLATFORM / "overlay"
    if overlay.is_dir():
        for path in sorted(overlay.rglob("*")):
            if path.is_file():
                target = out / path.relative_to(overlay)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, target)
    print(json.dumps(dict(out=str(out), injections=len(injections), modules=len(modules), dropped=report), indent=2))


def run(command, **kwargs):
    return subprocess.run(command, check=True, **kwargs)


def git(checkout, *args):
    return subprocess.check_output(["git", "-C", str(checkout), *args], text=True).strip()


INCLUDE = re.compile(r'include!\(concat!\(env!\("OUT_DIR"\),\s*"/(?P<name>[a-z_]+\.rs)"\)\);')
# Generated while the game builds; the mixin tool reads source and cannot evaluate build scripts.
INLINED = {
    "game_modules.rs", "game_host_modules.rs", "lan_action_modules.rs", "native_hook_modules.rs",
    "mod_api_fire_generated.rs", "mod_api_goal_view_generated.rs",
}


def export_sunrise(sunrise, revision, destination):
    """Export the pinned commit like Horizon's stage: tracked files only, links materialised."""
    if git(sunrise, "rev-parse", revision + "^{commit}") != revision:
        raise SystemExit("the Sunrise checkout does not contain the pinned commit")
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    archive = subprocess.Popen(["git", "-c", "core.autocrlf=false", "-C", str(sunrise), "archive", "--format=tar", revision], stdout=subprocess.PIPE)
    run(["tar", "-x", "-C", str(destination)], stdin=archive.stdout)
    if archive.wait():
        raise SystemExit("git archive failed")
    for path in sorted(destination.rglob("*")):
        if path.is_symlink():
            target = path.resolve()
            if not target.is_relative_to(destination.resolve()) or not target.is_file():
                raise SystemExit(f"unsupported source symlink: {path.relative_to(destination)}")
            data = target.read_bytes()
            path.unlink()
            path.write_bytes(data)
    (destination / ".horizon-revision").write_text(revision + "\n")


def generated_directory(target):
    found = sorted(target.glob("*/build/sunrise-client-*/out/game_modules.rs"))
    if not found:
        raise SystemExit("no generated game_modules.rs: build sunrise-client once so its build script runs")
    return found[-1].parent


def inline_generated(root, generated):
    """Replace the build-script includes with their content, paths made relative to the includer."""
    count = 0
    for path in sorted((root / "crates/sunrise-client/src").rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        if "OUT_DIR" not in text:
            continue

        def replace(match):
            nonlocal count
            if match["name"] not in INLINED:
                return match[0]
            body = (generated / match["name"]).read_text(encoding="utf-8")
            body = re.sub(
                r'#\[path = "([^"]+)"\]',
                lambda m: '#[path = "%s"]' % os.path.relpath(m[1], path.parent),
                body,
            )
            count += 1
            return body

        updated = INCLUDE.sub(replace, text)
        if updated != text:
            path.write_text(updated, encoding="utf-8")
    return count


def ensure_mixins_tool(compat):
    """The mixin tool Sunrise runs: the pinned horizon-mixins revision plus Sunrise's one patch."""
    work = ROOT / ".local" / "horizon-mixins-sunrise"
    revision = compat["mixins_revision"]
    if not (work / ".git").exists():
        work.parent.mkdir(parents=True, exist_ok=True)
        run(["git", "init", str(work)], stdout=sys.stderr)
        run(["git", "-C", str(work), "remote", "add", "origin", compat["mixins_repository"]])
    head = subprocess.run(["git", "-C", str(work), "rev-parse", "--verify", "HEAD"], capture_output=True, text=True)
    if head.returncode != 0 or head.stdout.strip() != revision:
        run(["git", "-C", str(work), "fetch", "--depth", "1", "origin", revision], stdout=sys.stderr)
        run(["git", "-C", str(work), "checkout", "--detach", "--force", revision], stdout=sys.stderr)
    return work


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    generate_parser = sub.add_parser("generate")
    generate_parser.add_argument("--sunrise", type=Path, help="Sunrise checkout, to resolve moved targets")
    generate_parser.set_defaults(run=command_generate)
    resolve_parser = sub.add_parser("resolve")
    resolve_parser.add_argument("--sunrise", type=Path, required=True)
    resolve_parser.set_defaults(run=command_resolve)
    args = parser.parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
