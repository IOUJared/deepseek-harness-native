#!/usr/bin/env python3
"""Serial, headless native checks with retained outcomes and source-drift detection."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
SKILL = ROOT / ".agents/skills/impeccable-rust"
SUITES = ("doctor", "clippy", "lint", "examples", "test", "all")


def commands(suite, offline=True):
    """Return explicit argv, never shell expressions or live GUI/Host invocations."""
    locked = ["--locked"] + (["--offline"] if offline else [])
    examples = [
        (name, ["cargo", "test", "--manifest-path", "app/Cargo.toml", *locked,
                "--features", "public-layout-fixture", "--example", name,
                "--", "--test-threads=2"])
        for name in ("layout_smoke", "scroll_app_smoke")
    ]
    if suite == "doctor":
        return [("tool-doctor", ["bash", str(SKILL / "scripts/impeccable"), "doctor"])]
    if suite == "examples":
        return examples
    lint = []
    for crate in ("core", "transport", "app"):
        manifest = f"{crate}/Cargo.toml"
        lint.append((f"{crate}-format", ["cargo", "fmt", "--manifest-path", manifest, "--all", "--", "--check"]))
        lint.append((f"{crate}-clippy", ["cargo", "clippy", "--manifest-path", manifest,
                     *locked, "--all-targets", "--all-features", "--",
                     "-D", "clippy::todo", "-D", "clippy::unimplemented", "-D", "unsafe_op_in_unsafe_fn"]))
    if suite == "clippy":
        return [(name, command) for name, command in lint if name.endswith("-clippy")]
    if suite == "lint":
        return lint
    tests = []
    for name, manifest, flags in (
        ("core", "core/Cargo.toml", []),
        ("core-transport", "core/Cargo.toml", ["--features", "transport"]),
        ("transport", "transport/Cargo.toml", []),
        ("app", "app/Cargo.toml", ["--bin", "dsh-native-app"]),
        ("app-public", "app/Cargo.toml", ["--features", "public-layout-fixture", "--bin", "dsh-native-app"]),
    ):
        tests.append((name, ["cargo", "test", "--manifest-path", manifest, *locked,
                            *flags, "--", "--test-threads=2"]))
    tests.append(("wayland-drops", ["cargo", "test", "--manifest-path", "app/Cargo.toml", *locked,
                                  "-p", "winit", "--lib", "data_device", "--", "--test-threads=2"]))
    tests.extend(examples)
    tests.append(("bridge", ["node", "--test", *sorted(str(p.relative_to(ROOT)) for p in (ROOT / "core/runtime").glob("*.test.mjs"))]))
    for name, directory, pattern in (("script-guards", "scripts", "test_*.py"),
                                     ("benchmark-guards", "benchmarks", "test_*.py"),
                                     ("app-guards", "app", "*_test.py"),
                                     ("release-guards", ".github/scripts", "test_*.py")):
        tests.append((name, ["python3", "-B", "-m", "unittest", "discover", "-s", directory, "-p", pattern]))
    if suite == "test":
        return tests
    if suite == "all":
        return lint + tests
    raise ValueError(f"unknown suite: {suite}")


def source_snapshot():
    """Hash tracked and nonignored untracked source; do not copy file bodies to evidence."""
    listing = subprocess.check_output(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"], cwd=ROOT)
    files = {}
    for raw in sorted(set(listing.split(b"\0")) - {b""}):
        name = os.fsdecode(raw)
        path = ROOT / name
        if path.is_symlink():
            data = os.fsencode(os.readlink(path))
        elif path.is_file():
            data = path.read_bytes()
        else:
            files[name] = "missing"
            continue
        files[name] = hashlib.sha256(data).hexdigest()
    digest = hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()
    return {"sha256": digest, "files": files}


def run_check(name, argv, output, env, timeout):
    """Retain failed and timed-out attempts; never rerun or silently skip a command."""
    start = time.monotonic()
    log = output / f"{name}.log"
    with log.open("xb") as stream:
        os.chmod(log, 0o600)
        timed_out = False
        try:
            process = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=stream,
                                       stderr=subprocess.STDOUT, start_new_session=True)
            try:
                code = process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                # This direct child is unreaped: its session/group id cannot be reused.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                code = process.wait()
            except BaseException:
                if process.returncode is None:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    process.wait()
                raise
            status = "timed-out" if timed_out else ("passed" if code == 0 else "failed")
        except OSError as error:
            stream.write(f"Cannot execute command: {type(error).__name__}\n".encode())
            code, status = None, "unavailable"
    return {"name": name, "command": argv, "status": status, "exitCode": code,
            "timedOut": timed_out, "signal": -code if code is not None and code < 0 else None,
            "seconds": round(time.monotonic() - start, 3), "log": str(log)}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", choices=SUITES, default="doctor")
    parser.add_argument("--plan", action="store_true", help="Print argv without executing or creating evidence")
    parser.add_argument("--online", action="store_true", help="Allow Cargo dependency access; does not install tools")
    parser.add_argument("--cargo-home", type=Path, help="Existing absolute Cargo cache directory")
    parser.add_argument("--timeout", type=int, default=900, help="Seconds per check (1–3600)")
    args = parser.parse_args(argv)
    if not 1 <= args.timeout <= 3600:
        parser.error("--timeout must be 1–3600")
    if args.cargo_home is not None and (not args.cargo_home.is_absolute() or not args.cargo_home.is_dir()):
        parser.error("--cargo-home must be an existing absolute directory")
    plan = commands(args.suite, offline=not args.online)
    if args.plan:
        print(json.dumps({"suite": args.suite, "offline": not args.online, "checks": [
            {"name": name, "command": command} for name, command in plan]}, indent=2))
        return 0
    env = {key: value for key, value in os.environ.items()
           if not any(word in key.upper() for word in ("KEY", "SECRET", "TOKEN", "PASSWORD"))}
    env.update(CARGO_BUILD_JOBS="2", RUSTUP_AUTO_INSTALL="0")
    if args.cargo_home:
        env["CARGO_HOME"] = str(args.cargo_home)
    if not args.online:
        env["CARGO_NET_OFFLINE"] = "true"
    evidence = ROOT / "app/evidence"
    evidence.mkdir(mode=0o700, parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="verification-", dir=evidence))
    before = source_snapshot()
    toolchain = [run_check(name, command, output, env, 30) for name, command in (
        ("rustc-version", ["rustc", "-vV"]), ("cargo-version", ["cargo", "--version"]),
        ("node-version", ["node", "--version"]), ("git-revision", ["git", "rev-parse", "HEAD"]))]
    results = []
    for name, command in plan:
        print(f"Checking {name}…", flush=True)
        result = run_check(name, command, output, env, args.timeout)
        results.append(result)
        print(f"{name}: {result['status']}", flush=True)
    after = source_snapshot()
    drift = before["sha256"] != after["sha256"]
    status = "source-changed" if drift else (
        "passed" if all(r["status"] == "passed" for r in results + toolchain) else "failed")
    report = {"status": status, "suite": args.suite, "offline": not args.online,
              "cargoJobs": 2, "toolchain": toolchain, "checks": results,
              "sourceBefore": before, "sourceAfter": after, "sourceChanged": drift,
              "scope": "Headless checks only; not GUI/physical input/provider/formal proof or complete compiled closure"}
    path = output / "report.json"
    with path.open("x", encoding="utf-8") as stream:
        os.chmod(path, 0o600)
        json.dump(report, stream, indent=2)
        stream.write("\n")
    print(f"Result: {status}; evidence: {path.relative_to(ROOT)}", flush=True)
    return 0 if status == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
