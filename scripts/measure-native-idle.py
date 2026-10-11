#!/usr/bin/env python3
"""Sample owned native-app idle resources over a longer window than the 8s package smoke.

Scope and limits, deliberately narrow:

- One owned release binary plus the isolated alpha Host it starts; fresh private
  XDG/HOME/workspace and a new data container per run. No credentials, no prompts.
- CPU is identity-checked: if the observed pid set changes inside the warm window, or any
  tick counter decreases, the run reports `unknown` instead of a number.
- `cpuPercentOneCore` is the endpoint delta over the warm span, expressed as percentage of
  one core, matching `scripts/qualify-development-package.py`. It is not a profiler, GC
  attribution, presented-frame latency, input latency or a toolkit ranking.
- RSS/PSS come from `/proc/<pid>/smaps_rollup` for the whole owned tree and are medians over
  the warm window, so they exclude most startup allocation; peak RSS is reported separately.

    python3 -B scripts/measure-native-idle.py --seconds 60 --repeats 3 --output app/evidence/idle-<label>
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
HZ = os.sysconf("SC_CLK_TCK")
PAGE_KIB = os.sysconf("SC_PAGE_SIZE") // 1024
MAX_SAMPLES = 4096


def parse_stat(text):
    """Return (utime, stime) ticks from one /proc/<pid>/stat body; comm may contain spaces."""
    close = text.rfind(")")
    fields = text[close + 2:].split()
    return int(fields[11]), int(fields[12])


def parse_smaps_rollup(text):
    """Return (rssKib, pssKib) from an smaps_rollup body, or None when either is absent."""
    values = {}
    for line in text.splitlines():
        key, _, rest = line.partition(":")
        if key in ("Rss", "Pss"):
            values[key] = int(rest.split()[0])
    return (values["Rss"], values["Pss"]) if "Rss" in values and "Pss" in values else None


def read_tree(root, ppid_map, limit=64):
    """Return {pid: (utime, stime)} for root and its descendants, bounded."""
    tree, pending = {}, [root]
    while pending and len(tree) < limit:
        pid = pending.pop()
        if pid in tree:
            continue
        try:
            tree[pid] = parse_stat(Path(f"/proc/{pid}/stat").read_text())
        except (FileNotFoundError, ProcessLookupError, PermissionError, IndexError, ValueError):
            continue
        pending.extend(child for child, parent in ppid_map.items() if parent == pid and child not in tree)
    return tree


def ppid_map():
    """Current pid -> ppid for every readable process."""
    mapping = {}
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            text = (entry / "stat").read_text()
            mapping[int(entry.name)] = int(text[text.rfind(")") + 2:].split()[1])
        except (FileNotFoundError, ProcessLookupError, PermissionError, IndexError, ValueError):
            continue
    return mapping


def memory(pid):
    try:
        return parse_smaps_rollup(Path(f"/proc/{pid}/smaps_rollup").read_text())
    except (FileNotFoundError, ProcessLookupError, PermissionError, OSError):
        return None


def sample(root):
    mapping = ppid_map()
    ticks = read_tree(root, mapping)
    rss = pss = 0
    for pid in ticks:
        value = memory(pid)
        if value is None:
            return None
        rss += value[0]
        pss += value[1]
    return {"at": time.monotonic(), "pids": len(ticks), "rssKib": rss, "pssKib": pss,
            "ticks": {str(pid): values[0] + values[1] for pid, values in ticks.items()}}


def warm_window(samples, warm_start_s):
    """Identity-checked endpoint CPU plus median RSS/PSS over the warm span."""
    unknown = {"identitiesStable": False, "seconds": None, "cpuPercentOneCore": None,
               "sampleCount": 0, "rssKibMedian": None, "pssKibMedian": None}
    warm = [s for s in samples if s["at"] - samples[0]["at"] >= warm_start_s] if samples else []
    if len(warm) < 2 or len(warm) > MAX_SAMPLES:
        return unknown
    identities = set(warm[0]["ticks"])
    if not identities or any(set(s["ticks"]) != identities for s in warm):
        return unknown
    if any(b["at"] <= a["at"] or any(b["ticks"][key] < a["ticks"][key] for key in identities)
           for a, b in zip(warm, warm[1:])):
        return unknown
    elapsed = warm[-1]["at"] - warm[0]["at"]
    delta = sum(warm[-1]["ticks"][key] - warm[0]["ticks"][key] for key in identities)
    return {"identitiesStable": True, "seconds": elapsed, "sampleCount": len(warm),
            "cpuPercentOneCore": (delta / HZ) / elapsed * 100,
            "rssKibMedian": median([s["rssKib"] for s in warm]),
            "pssKibMedian": median([s["pssKib"] for s in warm])}


def median(values):
    ordered = sorted(values)
    if not ordered:
        return None
    middle = len(ordered) // 2
    return ordered[middle] if len(ordered) % 2 else (ordered[middle - 1] + ordered[middle]) / 2


def run_once(binary, runtime, seconds, scale, output, warm_start_s):
    run = output / f"run-{time.strftime('%H%M%S')}"
    for name, mode in (("home", 0o700), ("user", 0o700), ("workspace", 0o700),
                       ("xdg/config", 0o700), ("xdg/data", 0o700), ("xdg/cache", 0o700)):
        (run / name).mkdir(parents=True, mode=mode)
    data = run / "data"
    report, screenshot = run / "smoke.json", run / "own-window.png"
    env = {k: v for k, v in os.environ.items()
           if k in {"WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS", "LANG", "LC_ALL",
                    "XDG_SESSION_TYPE", "XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP"}}
    env.update(PATH="/usr/bin:/bin", HOME=str(run / "home"), ICED_BACKEND="wgpu",
               XDG_CONFIG_HOME=str(run / "xdg/config"), XDG_DATA_HOME=str(run / "xdg/data"),
               XDG_CACHE_HOME=str(run / "xdg/cache"))
    command = [str(binary), "--runtime", str(runtime), "--expected-version", "0.2.1-alpha.1",
               "--home", str(run / "home"), "--user-home", str(run / "user"), "--cwd", str(run / "workspace"),
               "--scale", str(scale), "--smoke-new-session", "--exit-after-seconds", str(seconds),
               "--smoke-report", str(report), "--smoke-screenshot", str(screenshot)]
    log = (run / "run.log").open("xb")
    os.chmod(run / "run.log", 0o600)
    child = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    samples = []
    try:
        deadline = time.monotonic() + seconds + 45
        while child.poll() is None and time.monotonic() < deadline and len(samples) < MAX_SAMPLES:
            current = sample(child.pid)
            if current is not None:
                samples.append(current)
            time.sleep(0.25)
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGKILL)
    finally:
        child.wait()
        log.close()
    smoke = json.loads(report.read_text()) if report.is_file() else {}
    result = {"secondsRequested": seconds, "exitCode": child.returncode, "sampleCount": len(samples),
              "peakRssKib": max((s["rssKib"] for s in samples), default=None),
              "warm": warm_window(samples, warm_start_s),
              "samples": [{"at": round(s["at"] - samples[0]["at"], 3), "pids": s["pids"],
                           "rssKib": s["rssKib"], "pssKib": s["pssKib"]} for s in samples] if samples else [],
              "smoke": {key: smoke.get(key) for key in
                        ("status", "windowBackend", "coreReady", "paintVerified", "modelPrompts",
                         "modelCatalogRequested", "deadlineReached", "stop")}}
    (run / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    os.chmod(run / "result.json", 0o600)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default=str(ROOT / "app/target/release/dsh-native-app"))
    parser.add_argument("--runtime", default=str(ROOT.parent / "deepseek-harness-linux/apps/cli"))
    parser.add_argument("--seconds", type=int, default=60)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--scale", type=float, default=1.0)
    parser.add_argument("--warm-start", type=float, default=3.0)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    if not 8 <= args.seconds <= 600 or not 1 <= args.repeats <= 10:
        parser.error("--seconds must be 8-600 and --repeats 1-10")

    binary, runtime = Path(args.binary).resolve(strict=True), Path(args.runtime).resolve(strict=True)
    output = args.output.resolve()
    if output.exists() or output.is_symlink() or not output.parent.is_dir():
        parser.error("--output must be a new directory inside an existing parent")
    output.mkdir(mode=0o700)
    runs = [run_once(binary, runtime, args.seconds, args.scale, output, args.warm_start)
            for _ in range(args.repeats)]
    warm = [r["warm"] for r in runs if r["warm"]["identitiesStable"]]
    summary = {
        "status": "measured" if len(warm) == len(runs) else "partial",
        "binarySha256": __import__("hashlib").sha256(binary.read_bytes()).hexdigest(),
        "binaryBytes": binary.stat().st_size, "secondsRequested": args.seconds,
        "repeats": args.repeats, "scale": args.scale, "warmStartSeconds": args.warm_start,
        "cpuPercentOneCoreMedian": median([w["cpuPercentOneCore"] for w in warm]),
        "cpuPercentOneCoreRange": [min((w["cpuPercentOneCore"] for w in warm), default=None),
                                   max((w["cpuPercentOneCore"] for w in warm), default=None)],
        "rssKibMedian": median([w["rssKibMedian"] for w in warm]),
        "pssKibMedian": median([w["pssKibMedian"] for w in warm]),
        "peakRssKibMax": max((r["peakRssKib"] for r in runs if r["peakRssKib"]), default=None),
        "scope": "Owned-tree sampled idle resources over one owned blank session; not presented-frame, "
                 "input, VRAM, provider or full-dependency-closure measurement",
        "runs": runs,
    }
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    os.chmod(output / "summary.json", 0o600)
    print(json.dumps({k: summary[k] for k in ("status", "secondsRequested", "repeats",
                                              "cpuPercentOneCoreMedian", "cpuPercentOneCoreRange",
                                              "rssKibMedian", "pssKibMedian", "peakRssKibMax")}, indent=2))
    return 0 if summary["status"] == "measured" else 1


if __name__ == "__main__":
    sys.exit(main())
