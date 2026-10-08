#!/usr/bin/env python3
"""Owned synthetic Slint scaling qualification; never captures the desktop or changes settings."""
import asyncio
import json
import os
from pathlib import Path
import signal
import time

ROOT = Path(__file__).resolve().parent
BINARY = ROOT / "target/release/dsh-slint-benchmark"
PREFIX = "DSH_BENCH:"


def identity(pid):
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        return (int(fields[2]), int(fields[19]))
    except (OSError, ValueError, IndexError):
        return None


async def native_proof(pid):
    probe = await asyncio.create_subprocess_exec(
        "hyprctl", "-j", "clients", stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.DEVNULL)
    raw, _ = await asyncio.wait_for(probe.communicate(), 3)
    if probe.returncode != 0:
        return None
    for item in json.loads(raw):
        if item.get("pid") == pid:
            return {key: item.get(key) for key in ("pid", "class", "mapped", "xwayland", "size")}
    return None


async def qualify(renderer, run):
    run.mkdir()
    env = dict(os.environ)
    env.pop("DISPLAY", None)
    for key in tuple(env):
        if key.startswith("DEEPSEEK_") or any(part in key for part in ("API_KEY", "AUTH_TOKEN", "ACCESS_TOKEN", "SECRET")):
            env.pop(key)
    for key, name in (("HOME", "home"), ("XDG_CONFIG_HOME", "config"),
                      ("XDG_CACHE_HOME", "cache"), ("XDG_DATA_HOME", "data"),
                      ("XDG_STATE_HOME", "state")):
        directory = run / name
        directory.mkdir()
        env[key] = str(directory)
    env["SLINT_BACKEND"] = f"winit-{renderer}"
    snapshot = run / "synthetic-scale-1.25.png"
    process = await asyncio.create_subprocess_exec(
        str(BINARY), "--bench", "--scale", "1.25", "--snapshot", str(snapshot),
        cwd=run, env=env, start_new_session=True,
        stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE)
    owned_identity = identity(process.pid)
    events = []
    proof = None

    async def read_events():
        nonlocal proof
        while line := await process.stdout.readline():
            value = line.decode(errors="replace").rstrip()
            if value.startswith(PREFIX):
                event = json.loads(value[len(PREFIX):])
                events.append(event)
                if event.get("event") == "settled_geometry":
                    proof = await native_proof(process.pid)

    reader = asyncio.create_task(read_events())
    stderr_reader = asyncio.create_task(process.stderr.read())
    timed_out = False
    try:
        await asyncio.wait_for(process.wait(), 35)
    except asyncio.TimeoutError:
        timed_out = True
        current = identity(process.pid)
        if owned_identity is not None and current == owned_identity and current[0] == process.pid:
            os.killpg(process.pid, signal.SIGTERM)
        await asyncio.wait_for(process.wait(), 5)
    await asyncio.wait_for(reader, 5)
    stderr = (await stderr_reader).decode(errors="replace")
    result = {"renderer": renderer, "application_scale": 1.25,
              "qualification_only_not_timing_baseline": True,
              "pid": process.pid, "exit_code": process.returncode,
              "timed_out": timed_out, "native_window": proof,
              "snapshot": str(snapshot), "events": events}
    (run / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    (run / "stderr.log").write_text(stderr)
    geometry = next((event for event in events if event["event"] == "settled_geometry"), {})
    success = (process.returncode == 0 and not timed_out
               and geometry.get("geometry_matches_shared_workload") is True
               and geometry.get("logical_size") == [1200.0, 800.0]
               and geometry.get("physical_size") == [1500, 1000]
               and proof is not None and proof.get("mapped") is True
               and proof.get("xwayland") is False
               and proof.get("class") == "org.deepseek.NativeBenchmark.Slint"
               and proof.get("size") == [1500, 1000]
               and any(event["event"] == "snapshot_saved" for event in events))
    print(json.dumps({"renderer": renderer, "success": success, "result": str(run / "result.json"),
                      "geometry": geometry, "native_window": proof}), flush=True)
    return success


async def main():
    destination = ROOT / f"qualification-scale-fix-{time.time_ns()}"
    destination.mkdir()
    results = []
    # Sequential qualification avoids one prototype occluding the other's initial mapping.
    for renderer in ("software", "femtovg"):
        results.append(await qualify(renderer, destination / renderer))
    print(json.dumps({"qualification_directory": str(destination), "all_passed": all(results)}), flush=True)
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
