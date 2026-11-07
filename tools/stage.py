#!/usr/bin/env python3
"""Export pinned Horizon sources and stage the mod with official horizon-mixins.

Adapted from horizon-native-mod-template tools/horizon.py (MIT).
See notices/LICENSE-horizon-native-mod-template. Does not compile the full game.
"""
import argparse
import contextlib
try:
    import fcntl
except ImportError:  # Windows uses the existing portable toolchain too.
    fcntl = None
    import msvcrt
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(command, **kwargs):
    return subprocess.run(command, check=True, **kwargs)


def git(checkout, *args):
    return subprocess.check_output(["git", "-C", str(checkout), *args], text=True).strip()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def checkout_source(path, repository, revision):
    """Only create managed checkouts; never checkout/reset an existing user tree."""
    if path.exists():
        if not (path / ".git").exists():
            raise ValueError(f"existing source directory is not a Git checkout: {path}")
        return path
    path.parent.mkdir(parents=True, exist_ok=True)
    run(["git", "init", str(path)], stdout=sys.stderr)
    run(["git", "-C", str(path), "remote", "add", "origin", repository])
    run(["git", "-C", str(path), "fetch", "--depth", "1", "origin", revision], stdout=sys.stderr)
    run(["git", "-C", str(path), "checkout", "--detach", revision], stdout=sys.stderr)
    return path


def export_horizon(horizon, stages, compat):
    revision = compat["horizon_revision"]
    if not (horizon / ".git").exists():
        raise ValueError("--horizon must be a Git checkout, including a worktree")
    if git(horizon, "rev-parse", revision + "^{commit}") != revision:
        raise ValueError("Horizon pin must resolve to the recorded full commit SHA")
    # Export the committed pin, regardless of the user's working tree or current branch.
    out = stages / "original" / revision
    marker_name = ".draimhorizon-source.json"
    if out.exists():
        marker = json.loads((out / marker_name).read_text())
        if marker["revision"] != revision:
            raise ValueError(f"stage marker mismatch: {out}")
        for name, expected in marker["files"].items():
            if digest((out / name).read_bytes()) != expected:
                raise ValueError(f"exported source changed: {name}; remove generated stage to rebuild")
        return out
    out.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=".export-", dir=out.parent))
    try:
        process = subprocess.Popen(["git", "-c", "core.autocrlf=false", "-C", str(horizon), "archive", "--format=tar", revision], stdout=subprocess.PIPE)
        try:
            with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
                links = []
                for member in archive:
                    if member.issym():
                        links.append((member.name, member.linkname))
                    else:
                        archive.extract(member, temporary, filter="data")
                # Materialize documentation links after all targets exist. Windows
                # cannot reliably create symlinks, and stream tar cannot seek back.
                for name, link in links:
                    destination = temporary / name
                    source = (destination.parent / link).resolve()
                    if not source.is_relative_to(temporary.resolve()) or not source.is_file():
                        raise ValueError(f"unsupported source symlink: {name}")
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    destination.write_bytes(source.read_bytes())
        finally:
            process.stdout.close()
            code = process.wait()
        if code:
            raise ValueError(f"git archive exited {code}")
        for name, expected in compat.get("source_sha256", {}).items():
            if digest((temporary / name).read_bytes()) != expected:
                raise ValueError(f"pinned source hash mismatch: {name}")
        # Official CLI rejects symlinks in source snapshots. Materialize committed
        # documentation links only; no Rust rewriting is performed here.
        for path in sorted(temporary.rglob("*")):
            if path.is_symlink():
                if not path.resolve().is_relative_to(temporary.resolve()) or not path.is_file():
                    raise ValueError(f"unsupported source symlink: {path.relative_to(temporary)}")
                data = path.read_bytes()
                path.unlink()
                path.write_bytes(data)
        (temporary / ".horizon-revision").write_text(revision + "\n")
        files = {p.relative_to(temporary).as_posix(): digest(p.read_bytes())
                 for p in sorted(temporary.rglob("*")) if p.is_file()}
        (temporary / marker_name).write_text(json.dumps(dict(revision=revision, files=files), sort_keys=True, indent=2) + "\n")
        os.rename(temporary, out)
    except BaseException:
        shutil.rmtree(temporary, ignore_errors=True)
        raise
    return out


@contextlib.contextmanager
def stage_lock(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a") as handle:
        if fcntl is not None:
            fcntl.flock(handle, fcntl.LOCK_EX)
        else:
            handle.write(" ")
            handle.flush()
            handle.seek(0)
            msvcrt.locking(handle.fileno(), msvcrt.LK_LOCK, 1)
        try:
            yield
        finally:
            if fcntl is None:
                handle.seek(0)
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--horizon", type=Path, help="existing Git checkout containing the pinned Horizon commit")
    parser.add_argument("--mixins", type=Path, help="clean horizon-mixins checkout at the exact pinned commit")
    parser.add_argument("--offline", action="store_true", help="build horizon-mixins without fetching Cargo dependencies")
    args = parser.parse_args()
    compat = json.loads((ROOT / "compatibility.json").read_text())
    with stage_lock(ROOT / "target/stage.lock"):
        horizon = (args.horizon.resolve() if args.horizon else checkout_source(
            ROOT / ".local/horizon", compat["horizon_repository"], compat["horizon_revision"]))
        mixins = (args.mixins.resolve() if args.mixins else checkout_source(
            ROOT / ".local/horizon-mixins", compat["horizon_mixins_repository"], compat["horizon_mixins_revision"]))
        if git(mixins, "rev-parse", "HEAD") != compat["horizon_mixins_revision"]:
            raise ValueError("horizon-mixins HEAD differs from the recorded compatibility pin")
        if git(mixins, "status", "--porcelain", "--untracked-files=no"):
            raise ValueError("horizon-mixins checkout has tracked changes; use a clean pinned checkout")
        stages = ROOT / "target/mixins"
        original = export_horizon(horizon, stages, compat)
        cargo_target = ROOT / "target/mixins-cli"
        command = ["cargo", "build", "--locked", "-p", "horizon-mixins-cli", "--target-dir", str(cargo_target)]
        if args.offline:
            command.append("--offline")
        run(command, cwd=mixins, stdout=sys.stderr)
        cli = cargo_target / "debug/horizon-mixins"
        base = ["--workspace", str(original), "--mods", str(ROOT / "mixins")]
        plan = stages / "plan.json"
        output = stages / "modded"
        planned = run([str(cli), "plan", *base, "--out", str(plan)], capture_output=True, text=True)
        applied = run([str(cli), "apply", *base, "--out", str(output)], capture_output=True, text=True)
        report = dict(stage=str(output), original=str(original), plan=str(plan),
                      horizon_revision=compat["horizon_revision"],
                      horizon_mixins_revision=compat["horizon_mixins_revision"],
                      apply=json.loads(applied.stdout))
        # Keep plan's stdout available for troubleshooting without mixing it with the report.
        (stages / "plan-output.json").write_text(planned.stdout)
        print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        if isinstance(error, subprocess.CalledProcessError) and error.stderr:
            print(error.stderr, file=sys.stderr)
        print(f"Stage: {error}", file=sys.stderr)
        raise SystemExit(1)
