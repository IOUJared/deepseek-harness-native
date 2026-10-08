#!/usr/bin/env python3
"""Measure only owned benchmark process trees; no compositor/settings changes."""
import argparse
import asyncio
import json
import math
import os
from pathlib import Path
import signal
import time

PREFIX = "DSH_BENCH:"
HZ = os.sysconf("SC_CLK_TCK")


def proc_stat(pid):
    try:
        value = Path(f"/proc/{pid}/stat").read_text()
        fields = value[value.rindex(")") + 2:].split()
        return {"pid": pid, "ppid": int(fields[1]), "pgrp": int(fields[2]),
                "cpu_ticks": int(fields[11]) + int(fields[12]), "start_ticks": int(fields[19])}
    except (OSError, ValueError, IndexError):
        return None


def process_tree(root_pid, root_start):
    root = proc_stat(root_pid)
    if root is None or root["start_ticks"] != root_start:
        return []
    processes = {}
    for entry in Path("/proc").iterdir():
        if entry.name.isdigit():
            stat = proc_stat(int(entry.name))
            if stat is not None:
                processes[stat["pid"]] = stat
    owned = {root_pid}
    while True:
        descendants = {pid for pid, stat in processes.items() if stat["ppid"] in owned}
        expanded = owned | descendants
        if expanded == owned:
            break
        owned = expanded
    return [processes[pid] for pid in owned if pid in processes]


def drm_clients(pid):
    clients = {}
    try:
        entries = list(Path(f"/proc/{pid}/fdinfo").iterdir())
    except OSError:
        return clients
    for entry in entries:
        try:
            fields = dict(line.split(":", 1) for line in entry.read_text().splitlines() if ":" in line)
            identifier = fields.get("drm-client-id", "").strip()
            if not identifier:
                continue
            key = (fields.get("drm-driver", "").strip(), fields.get("drm-pdev", "").strip(), identifier)
            value = fields.get("drm-memory-vram", "").split()
            if len(value) == 2 and value[0].isdigit() and value[1] == "KiB":
                clients[key] = int(value[0])
        except (OSError, ValueError):
            continue
    return clients


def sample_tree(root_pid, root_start):
    processes = process_tree(root_pid, root_start)
    rss = 0
    pss = 0
    pss_complete = bool(processes)
    identities = {}
    gpu_clients = {}
    for stat in processes:
        pid = stat["pid"]
        current_identity = proc_stat(pid)
        if current_identity is None or current_identity["start_ticks"] != stat["start_ticks"]:
            pss_complete = False
            continue
        identities[f"{pid}:{stat['start_ticks']}"] = stat["cpu_ticks"]
        gpu_clients.update(drm_clients(pid))
        try:
            status = Path(f"/proc/{pid}/status").read_text()
            rss += next((int(line.split()[1]) for line in status.splitlines()
                         if line.startswith("VmRSS:")), 0)
        except (OSError, ValueError):
            pass
        try:
            rollup = Path(f"/proc/{pid}/smaps_rollup").read_text()
            pss += next((int(line.split()[1]) for line in rollup.splitlines()
                         if line.startswith("Pss:")), 0)
        except (OSError, ValueError):
            pss_complete = False
    return {"at": time.monotonic(), "system_loadavg": list(os.getloadavg()), "process_count": len(processes),
            "rss_kib": rss, "pss_kib": pss if pss_complete else None,
            "drm_vram_kib": sum(gpu_clients.values()) if gpu_clients else None, "ticks": identities}


def cpu_percent(previous, current):
    elapsed = current["at"] - previous["at"]
    if elapsed <= 0:
        return None
    # A disappearance prevents complete accounting; do not silently call it zero CPU.
    if not previous["ticks"].keys() <= current["ticks"].keys():
        return None
    delta = sum(value - previous["ticks"].get(key, value)
                for key, value in current["ticks"].items())
    return max(0, delta) / HZ / elapsed * 100


def group_is_owned(pid, start):
    current = proc_stat(pid)
    return current is not None and current["start_ticks"] == start and current["pgrp"] == pid


async def native_window(pid):
    """Inspect only this test PID's compositor facts; never log other window data."""
    try:
        probe = await asyncio.create_subprocess_exec("hyprctl", "-j", "clients",
            stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL)
        raw, _ = await asyncio.wait_for(probe.communicate(), 2)
        if probe.returncode != 0 or len(raw) > 1024 * 1024:
            return None
        entries = json.loads(raw)
        for entry in entries:
            if isinstance(entry, dict) and entry.get("pid") == pid:
                return {key: entry.get(key) for key in ("pid", "class", "xwayland", "mapped", "size")}
    except asyncio.TimeoutError:
        probe.kill()
        await probe.wait()
    except (OSError, ValueError, TypeError):
        pass
    return None


def geometry_errors(events, facts, scale):
    """Require settled geometry and independent native mapping before ranking runs."""
    errors = []
    geometry = next((event for event in events if event.get("event") == "settled_geometry"), None)
    if geometry is None:
        return ["no settled geometry observation"]
    logical = geometry.get("logical_size")
    if not isinstance(logical, list) or len(logical) != 2 or any(
            not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(value) or abs(value - expected) > 1
            for value, expected in zip(logical or [], (1200, 800))):
        errors.append("settled UI geometry is not the matched 1200x800 workload")
    effective = geometry.get("effective_scale")
    if not isinstance(effective, (int, float)) or isinstance(effective, bool) or not abs(effective - scale) < 0.01:
        errors.append("settled scale differs from requested application scale on native scale 1")
    if not facts or facts.get("mapped") is not True or facts.get("xwayland") is not False:
        errors.append("own test window not independently verified as mapped native Wayland")
    else:
        if geometry.get("app_id") and facts.get("class") != geometry["app_id"]:
            errors.append("own test window app_id differs from compositor class")
        expected_size = [round(1200 * scale), round(800 * scale)]
        if facts.get("size") != expected_size:
            errors.append("own compositor geometry differs from matched UI at native scale 1")
        physical = geometry.get("physical_size", geometry.get("physical_size_at_observation"))
        if physical != expected_size:
            errors.append("settled physical viewport differs from matched UI at native scale 1")
    return errors


async def one(binary, output, label, scale, renderer_env):
    run_dir = output / f"{label}-scale-{scale:g}"
    run_dir.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ)
    # This modifies only the child environment. It never changes the live compositor.
    env.pop("DISPLAY", None)
    for key in list(env):
        if key.startswith("DEEPSEEK_") or any(part in key for part in ("API_KEY", "AUTH_TOKEN", "ACCESS_TOKEN", "SECRET")):
            env.pop(key)
    for key, suffix in (("XDG_CONFIG_HOME", "config"), ("XDG_CACHE_HOME", "cache"), ("XDG_DATA_HOME", "data")):
        target = run_dir / suffix
        target.mkdir()
        env[key] = str(target)
    env.update(renderer_env)
    started = time.monotonic()
    process = await asyncio.create_subprocess_exec(str(binary), "--bench", "--scale", str(scale),
        env=env, cwd=run_dir, start_new_session=True,
        stdin=asyncio.subprocess.DEVNULL, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE,
        limit=65536)
    stat = proc_stat(process.pid)
    if stat is None:
        await process.wait()
        raise RuntimeError(f"{label}: process exited before its identity could be recorded")
    root_start = stat["start_ticks"]
    samples = []
    events = []
    errors = []
    phase = "startup"
    wayland_facts = None
    stop_sampling = asyncio.Event()

    async def stdout_reader():
        nonlocal phase, wayland_facts
        while line := await process.stdout.readline():
            decoded = line.decode("utf-8", errors="replace").rstrip()
            if not decoded.startswith(PREFIX):
                continue
            try:
                value = json.loads(decoded[len(PREFIX):])
            except json.JSONDecodeError:
                errors.append("invalid benchmark JSON")
                continue
            if not isinstance(value, dict) or not isinstance(value.get("event"), str):
                errors.append("invalid benchmark event")
                continue
            value["runner_elapsed_ms"] = (time.monotonic() - started) * 1000
            events.append(value)
            kind = value["event"]
            if kind == "ready":
                wayland_facts = await native_window(process.pid)
            elif kind == "settled_geometry":
                # Early software readiness can precede the first mapped Wayland surface.
                wayland_facts = await native_window(process.pid)
            if kind == "idle_start":
                phase = "final_idle" if value.get("phase") == "final_idle" else "initial_idle"
            elif kind == "scroll_start":
                phase = "scroll"
            elif kind == "stream_start":
                phase = "stream"
            elif kind == "stream_end":
                phase = "final_idle"

    async def stderr_reader():
        buffer = bytearray()
        while chunk := await process.stderr.read(4096):
            buffer.extend(chunk)
            del buffer[:-32768]
        return bytes(buffer).decode("utf-8", errors="replace")

    async def sampler():
        previous = None
        while not stop_sampling.is_set():
            current = sample_tree(process.pid, root_start)
            current["phase"] = phase
            current["elapsed_ms"] = (current["at"] - started) * 1000
            current["cpu_percent_one_core"] = None if previous is None else cpu_percent(previous, current)
            samples.append({key: value for key, value in current.items() if key != "ticks"})
            previous = current
            try:
                await asyncio.wait_for(stop_sampling.wait(), 0.5)
            except asyncio.TimeoutError:
                pass

    tasks = [asyncio.create_task(stdout_reader()), asyncio.create_task(stderr_reader()), asyncio.create_task(sampler())]
    timeout = False
    try:
        await asyncio.wait_for(process.wait(), 40)
    except asyncio.TimeoutError:
        timeout = True
        errors.append("bounded benchmark exceeded 40 seconds")
        if group_is_owned(process.pid, root_start):
            os.killpg(process.pid, signal.SIGTERM)
        try:
            await asyncio.wait_for(process.wait(), 5)
        except asyncio.TimeoutError:
            if group_is_owned(process.pid, root_start):
                os.killpg(process.pid, signal.SIGKILL)
            await process.wait()
    finally:
        stop_sampling.set()
    # A child can retain pipes after root exit; bound collection rather than treating EOF as exit.
    try:
        results = await asyncio.wait_for(asyncio.gather(*tasks), 3)
        stderr = results[1]
    except asyncio.TimeoutError:
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        stderr = "Output collection exceeded its deadline; see errors."
        errors.append("benchmark descendant retained an output pipe")
    (run_dir / "stderr.log").write_text(stderr)
    ready = next((event for event in events if event["event"] == "ready"), None)
    finished = any(event["event"] == "finished" for event in events)
    if ready is None:
        errors.append("no ready event")
    if not finished:
        errors.append("no finished event")
    if process.returncode != 0:
        errors.append(f"exit status {process.returncode}")
    qualification = geometry_errors(events, wayland_facts, scale)
    errors.extend(qualification)
    report = {"label": label, "binary": str(binary), "binary_bytes": binary.stat().st_size,
        "frontend_prototype_only": True, "scale_requested": scale,
        "wayland_display_present": bool(env.get("WAYLAND_DISPLAY")), "display_removed": "DISPLAY" not in env,
        "native_wayland_verified": bool(wayland_facts and wayland_facts.get("xwayland") is False and wayland_facts.get("mapped") is True),
        "native_window_facts": wayland_facts, "geometry_qualified": not qualification,
        "compositor_fractional_scaling_verified": False,
        "ready": ready, "duration_seconds": time.monotonic() - started, "exit_code": process.returncode,
        "timeout": timeout, "errors": errors, "events": events, "samples": samples}
    # Never infer actual backend or first presentation from environment/ready naming alone.
    (run_dir / "result.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"label": label, "errors": errors, "result": str(run_dir / "result.json")}), flush=True)
    return report


async def main(args):
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=False)
    reports = []
    for spec in args.app:
        label, path = spec.split("=", 1)
        if not label or any(ch not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_" for ch in label):
            raise ValueError("app label must be a simple identifier")
        binary = Path(path).resolve(strict=True)
        if not binary.is_file() or not os.access(binary, os.X_OK):
            raise ValueError(f"not an executable: {binary}")
        renderer_env = {}
        if label.startswith("slint-software"):
            renderer_env["SLINT_BACKEND"] = "winit-software"
        elif label.startswith("slint-gpu"):
            renderer_env["SLINT_BACKEND"] = "winit-femtovg"
        for scale in args.scale:
            reports.append(await one(binary, output, label, scale, renderer_env))
    (output / "results.json").write_text(json.dumps(reports, indent=2) + "\n")
    return 1 if any(report["errors"] for report in reports) else 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", action="append", required=True, help="label=/absolute/path/to/release/executable")
    parser.add_argument("--scale", type=float, action="append", help="application UI scale, not compositor setting")
    parser.add_argument("--output", required=True, help="new result directory; refuses replacement")
    arguments = parser.parse_args()
    arguments.scale = arguments.scale or [1.0]
    if any(not 0.5 <= scale <= 3 for scale in arguments.scale):
        parser.error("scale must be between 0.5 and 3")
    raise SystemExit(asyncio.run(main(arguments)))
