#!/usr/bin/env python3
"""Own-renderer/public local component fixtures; never a Host or live approval/model test."""
import argparse
import asyncio
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import select
import signal
import time

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("native_qualification", BASE / "scripts/qualify-native-app.py")
NATIVE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(NATIVE)
METRICS = NATIVE.METRICS


def private_json(path, data):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as stream:
        json.dump(data, stream, indent=2)
        stream.write("\n")


async def one(binary, root, mode, scale):
    output = root / mode
    output.mkdir(mode=0o700)
    child = await asyncio.create_subprocess_exec(str(binary), "--output", str(output), "--mode", mode,
        "--scale", str(scale), env=NATIVE.environment(root), stdout=asyncio.subprocess.DEVNULL,
        stderr=asyncio.subprocess.DEVNULL, start_new_session=True)
    identity = METRICS.proc_stat(child.pid)
    fd = os.pidfd_open(child.pid)
    verified = METRICS.proc_stat(child.pid)
    if identity is None or verified is None or identity["start_ticks"] != verified["start_ticks"]:
        os.close(fd)
        raise RuntimeError("owned process identity unavailable")
    facts = None
    observed_children = set()
    forced = False
    deadline = time.monotonic() + 15
    try:
        while child.returncode is None:
            observed_children.update(p["pid"] for p in METRICS.process_tree(child.pid, identity["start_ticks"]) if p["pid"] != child.pid)
            if facts is None:
                candidate = await METRICS.native_window(child.pid)
                if candidate and candidate.get("mapped"):
                    facts = candidate
            if time.monotonic() >= deadline:
                forced = True
                signal.pidfd_send_signal(fd, signal.SIGTERM)
                try:
                    await asyncio.wait_for(child.wait(), 2)
                except asyncio.TimeoutError:
                    signal.pidfd_send_signal(fd, signal.SIGKILL)
                    await child.wait()
                break
            try:
                await asyncio.wait_for(child.wait(), 0.1)
            except asyncio.TimeoutError:
                pass
    finally:
        if child.returncode is None and not select.select([fd], [], [], 0)[0]:
            signal.pidfd_send_signal(fd, signal.SIGKILL)
            await child.wait()
        os.close(fd)
    result_path = output / "result.json"
    data = json.loads(result_path.read_text()) if result_path.is_file() else {"status": "missing-fixture-report"}
    passed = child.returncode == 0 and not forced and not observed_children and data.get("status") == "passed" and facts and facts.get("class") == "ai.deepseek.harness.native.dialog-fixture" and facts.get("xwayland") is False
    return {"mode": mode, "status": "passed" if passed else "failed", "exitCode": child.returncode,
        "forced": forced, "observedChildProcesses": sorted(observed_children), "ownedWindow": facts,
        "fixture": data, "ownRendererFrame": str((output / "own-window.png").relative_to(BASE)),
        "paintInspected": False, "liveHostDecisionQualified": False, "nativeKeyboardPointerInputQualified": False}


async def run(options):
    root = NATIVE.prepare(options.output)
    binary = (BASE / "app/target/release/examples/dialog_smoke").resolve()
    if not binary.is_file():
        raise ValueError("release component fixture not built")
    results = []
    for mode in ("approval", "question", "unsupported", "settings", "settings-confirm"):
        results.append(await one(binary, root, mode, options.scale))
    data = {"status": "passed" if all(r["status"] == "passed" for r in results) else "failed",
        "scope": "actual native component rendering/public local reducer fixtures, not composed Host/live interaction",
        "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "applicationScale": options.scale,
        "backendStarted": False, "realCredentialsUsed": False, "modelPrompts": 0, "replyRpcIssued": False,
        "fixtureResults": results, "desktopSettingsChanged": False}
    private_json(root / "result.json", data)
    print(json.dumps({"status": data["status"], "report": str(root / "result.json")}))
    return 0 if data["status"] == "passed" else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True)
    parser.add_argument("--scale", type=float, choices=[1.0, 1.25], default=1.0)
    raise SystemExit(asyncio.run(run(parser.parse_args())))
