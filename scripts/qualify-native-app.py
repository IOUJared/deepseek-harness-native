#!/usr/bin/env python3
"""Qualify only the owned real native app. No desktop settings, model calls or screenshots."""
import argparse
import asyncio
import importlib.util
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import statistics
import time

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("owned_benchmark_metrics", BASE / "benchmarks/run.py")
METRICS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(METRICS)


def prepare(output):
    output = Path(output).absolute()
    allowed = (BASE / "app/evidence").resolve()
    if output.exists() or not output.parent.resolve().is_relative_to(allowed):
        raise ValueError("output must be a NEW directory below native app/evidence")
    output.mkdir(mode=0o700)
    for name in ("harness", "workspace", "user", "config", "cache", "data", "state"):
        (output / name).mkdir(mode=0o700)
    return output


def environment(output):
    # Preserve only native display/driver identity; no ambient model/account credentials.
    names = ("PATH", "LANG", "LC_ALL", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "HYPRLAND_INSTANCE_SIGNATURE", "AQ_DRM_DEVICES", "WGPU_BACKEND")
    env = {name: os.environ[name] for name in names if name in os.environ}
    env.update(HOME=str(output / "user"), XDG_CONFIG_HOME=str(output / "config"),
               XDG_CACHE_HOME=str(output / "cache"), XDG_DATA_HOME=str(output / "data"),
               XDG_STATE_HOME=str(output / "state"), WINIT_UNIX_BACKEND="wayland", ICED_BACKEND="wgpu")
    return env


def owned_threads(root_pid, root_start):
    result = {}
    for process in METRICS.process_tree(root_pid, root_start):
        try:
            threads = list(Path(f"/proc/{process['pid']}/task").iterdir())
        except OSError:
            continue
        for thread in threads:
            try:
                raw = (thread / 'stat').read_text()
                fields = raw[raw.rindex(')') + 2:].split()
                ticks = int(fields[11]) + int(fields[12])
                started = int(fields[19])
                name = (thread / 'comm').read_text().strip()[:64]
            except (OSError, ValueError, IndexError):
                continue
            key = f"{process['pid']}:{int(thread.name)}:{started}"
            result[key] = {"ticks": ticks, "name": name}
    return result


async def run(options):
    output = prepare(options.output)
    binary = (BASE / "app/target/release/dsh-native-app").resolve()
    runtime = (BASE.parent / "deepseek-harness-linux/apps/cli").resolve()
    if not binary.is_file():
        raise ValueError("release app binary is not ready")
    command = [str(binary), "--runtime", str(runtime), "--expected-version", "0.2.1-alpha.1",
               "--home", str(output / "harness"), "--cwd", str(output / "workspace"),
               "--user-home", str(output / "user"), "--scale", str(options.scale),
               "--smoke-new-session", "--exit-after-seconds", str(options.seconds),
               "--smoke-report", str(output / "app.json")]
    if options.screenshot:
        command += ["--smoke-screenshot", str(output / "own-window.png")]
    binary_sha256 = hashlib.sha256(binary.read_bytes()).hexdigest()
    start = time.monotonic()
    child = await asyncio.create_subprocess_exec(*command, env=environment(output),
        stdout=asyncio.subprocess.DEVNULL, stderr=asyncio.subprocess.DEVNULL, start_new_session=True)
    root = METRICS.proc_stat(child.pid)
    if root is None:
        await child.wait()
        raise RuntimeError("app exited before process identity was acquired")
    root_start = root["start_ticks"]
    identities, pidfds, samples = {}, {}, []
    facts, mapped_ms, forced, interrupted = None, None, False, False
    cancelled = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, cancelled.set)
    deadline = start + options.seconds + 30
    try:
        while child.returncode is None:
            sample = METRICS.sample_tree(child.pid, root_start)
            sample["elapsed_ms"] = (sample["at"] - start) * 1000
            sample["threads"] = owned_threads(child.pid, root_start)
            if samples:
                sample["cpu_percent_one_core"] = METRICS.cpu_percent(samples[-1], sample)
                elapsed = sample['at'] - samples[-1]['at']
                old_threads = samples[-1]['threads']
                for key, thread in sample['threads'].items():
                    if key in old_threads and elapsed > 0:
                        thread['cpu_percent_one_core'] = (thread['ticks'] - old_threads[key]['ticks']) / METRICS.HZ / elapsed * 100
            samples.append(sample)
            for stat in METRICS.process_tree(child.pid, root_start):
                identity = (stat["pid"], stat["start_ticks"], stat["pgrp"])
                identities[identity] = stat
                if identity not in pidfds:
                    try:
                        fd = os.pidfd_open(stat["pid"])
                        current = METRICS.proc_stat(stat["pid"])
                        if current is not None and (current["pid"], current["start_ticks"], current["pgrp"]) == identity:
                            pidfds[identity] = fd
                        else:
                            os.close(fd)
                    except (OSError, AttributeError):
                        pass
            if facts is None:
                candidate = await METRICS.native_window(child.pid)
                if candidate and candidate.get("mapped") is True:
                    facts = candidate
                    mapped_ms = (time.monotonic() - start) * 1000
            if cancelled.is_set() or time.monotonic() >= deadline:
                forced = True
                interrupted = cancelled.is_set()
                # Signal only identity-checked pidfds captured from this owned tree.
                for fd in pidfds.values():
                    try:
                        signal.pidfd_send_signal(fd, signal.SIGTERM)
                    except OSError:
                        pass
                try:
                    await asyncio.wait_for(child.wait(), 3)
                except asyncio.TimeoutError:
                    for fd in pidfds.values():
                        try:
                            signal.pidfd_send_signal(fd, signal.SIGKILL)
                        except OSError:
                            pass
                    await asyncio.wait_for(child.wait(), 3)
                break
            try:
                await asyncio.wait_for(child.wait(), 0.5)
            except asyncio.TimeoutError:
                pass
        await child.wait()
        retained = []
        for identity, fd in pidfds.items():
            if not select.select([fd], [], [], 0)[0]:
                retained.append({"pid": identity[0], "startTicks": identity[1], "group": identity[2]})
        try:
            data = (output / "app.json").read_bytes()
            app = json.loads(data) if len(data) <= 65536 else {"status": "oversize-report"}
        except (OSError, ValueError):
            app = {"status": "missing-or-invalid-report"}
        valid = (child.returncode == 0 and not forced and not retained and app.get("status") == "passed"
            and facts and facts.get("xwayland") is False and facts.get("class") == "ai.deepseek.harness.native.app")
        # Use only late bounded keyless-idle samples. This is NOT a matched long-chat benchmark.
        idle = [s for s in samples if 3000 <= s["elapsed_ms"] <= (options.seconds - 1) * 1000]
        def median(key):
            values = [s[key] for s in idle if s.get(key) is not None]
            return statistics.median(values) if values else None
        report = {"status": "passed" if valid else "failed", "scope": "owned native app keyless startup/lifecycle, not full parity or matched chat performance",
            "binarySha256": binary_sha256, "scale": options.scale, "exitCode": child.returncode, "forced": forced, "interrupted": interrupted,
            "ownedWindow": facts, "compositorMappedProxyMs": mapped_ms, "presentedFrameTiming": False,
            "remainingObservedProcesses": retained, "app": app, "samples": samples,
            "keylessIdle": {"sampleCount": len(idle), "rssKiBMedian": median("rss_kib"), "pssKiBMedian": median("pss_kib"),
                "drmVramKiBMedian": median("drm_vram_kib"), "cpuPercentOneCoreMedian": median("cpu_percent_one_core")}}
        (output / "result.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps({"status": report["status"], "report": str(output / "result.json"), "keylessIdle": report["keylessIdle"]}))
        return 0 if valid else 1
    finally:
        if child.returncode is None:
            # Exceptions/cancellation must not abandon the new-session UI or its observed Host.
            for fd in pidfds.values():
                try:
                    signal.pidfd_send_signal(fd, signal.SIGTERM)
                except OSError:
                    pass
            try:
                await asyncio.wait_for(child.wait(), 3)
            except asyncio.TimeoutError:
                for fd in pidfds.values():
                    try:
                        signal.pidfd_send_signal(fd, signal.SIGKILL)
                    except OSError:
                        pass
                await asyncio.wait_for(child.wait(), 3)
        for fd in pidfds.values():
            os.close(fd)
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.remove_signal_handler(sig)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True)
    parser.add_argument("--scale", type=float, choices=[1.0, 1.25], default=1.0)
    parser.add_argument("--seconds", type=int, choices=range(8, 31), default=10)
    parser.add_argument("--screenshot", action='store_true', help='Capture only the new real app Iced render target once')
    args = parser.parse_args()
    raise SystemExit(asyncio.run(run(args)))
