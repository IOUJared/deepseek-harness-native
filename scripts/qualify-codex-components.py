#!/usr/bin/env python3
"""Public Codex own-renderer fixtures; no Host, OpenAI login, browser or model requests."""
import argparse
import asyncio
import hashlib
import importlib.util
from pathlib import Path

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("codex_components", BASE / "scripts/qualify-dialog-components.py")
COMPONENTS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPONENTS)
MODES = ("settings-codex-connect", "settings-codex-ready", "settings-codex-callback", "settings-codex-enable")

async def run(options):
    root = COMPONENTS.NATIVE.prepare(options.output)
    binary = (BASE / "app/target/release/examples/dialog_smoke").resolve()
    if not binary.is_file():
        raise ValueError("release component fixture not built")
    results = [await COMPONENTS.one(binary, root, mode, options.scale) for mode in MODES]
    passed = all(result["status"] == "passed" and result["fixture"].get("fixturePassed") is True
        and result["fixture"].get("browserOpened") is False
        and result["fixture"].get("credentialWriteIssued") is False for result in results)
    report = {"status": "passed" if passed else "failed", "scope": "public native Codex settings rendering/local reducer only",
        "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "applicationScale": options.scale,
        "backendStarted": False, "browserOpened": False, "realCredentialsUsed": False,
        "modelPrompts": 0, "rpcIssued": False, "desktopSettingsChanged": False, "fixtureResults": results,
        "realOpenAiLoginQualified": False, "nativeKeyboardPointerInputQualified": False}
    COMPONENTS.private_json(root / "result.json", report)
    print(f'{report["status"]}: {root / "result.json"}')
    return 0 if passed else 1

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True)
    parser.add_argument("--scale", type=float, choices=[1.0, 1.25], default=1.0)
    raise SystemExit(asyncio.run(run(parser.parse_args())))
