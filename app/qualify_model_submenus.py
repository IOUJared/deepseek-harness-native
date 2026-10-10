#!/usr/bin/env python3
"""Capture PUBLIC native menu panes; no Host, catalog, prompt or desktop screenshot."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
CASES = ["model-popup", "model-submenu", "effort-submenu", "speed-submenu", "output-submenu", "model-submenu-tiny", "effort-submenu-tiny", "model-submenu-empty", "model-popup-long"]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def private_json(path, value):
    with open(path, "x", opener=lambda p, flags: os.open(p, flags, 0o600)) as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")


def run():
    out = Path(tempfile.mkdtemp(prefix="model-submenu-paint-", dir=ROOT / "evidence"))
    os.chmod(out, 0o700)
    print(f"EVIDENCE={out}", flush=True)
    artifacts = out / "compiled-snapshot"
    artifacts.mkdir(mode=0o700)
    for source, name in [(ROOT / "target/release/examples/layout_smoke", "layout_smoke"), (ROOT / "target/release/dsh-native-app", "dsh-native-app")]:
        with open(source, "rb") as src, open(artifacts / name, "xb", opener=lambda p, flags: os.open(p, flags, 0o700)) as dst:
            shutil.copyfileobj(src, dst)
    sources = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "examples/layout_smoke.rs"] + list((ROOT / "src").glob("*.rs"))
    pins = {str(p.relative_to(ROOT)): sha(p) for p in sources}
    env = {k: v for k, v in os.environ.items() if k in {"WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS", "LANG", "LC_ALL", "XDG_SESSION_TYPE", "XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP"}}
    for name in ["home", "config", "data", "cache"]:
        (out / name).mkdir(mode=0o700)
    env.update(PATH="/usr/bin:/bin", ICED_BACKEND="wgpu", HOME=str(out / "home"), XDG_CONFIG_HOME=str(out / "config"), XDG_DATA_HOME=str(out / "data"), XDG_CACHE_HOME=str(out / "cache"))
    results = []
    for mode in CASES:
        folder = out / mode
        folder.mkdir(mode=0o700)
        with open(folder / "run.log", "xb", opener=lambda p, flags: os.open(p, flags, 0o600)) as log:
            run_result = subprocess.run([str(artifacts / "layout_smoke"), "--mode", mode, "--screenshot", str(folder / "own-window.png"), "--report", str(folder / "result.json")], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=25)
        report = json.loads((folder / "result.json").read_text()) if (folder / "result.json").exists() else {}
        passed = run_result.returncode == 0 and report.get("status") == "passed" and report.get("effectsDispatched") == 0 and report.get("publicSource") is True and report.get("fixture", {}).get("modelCatalogRequested", False) is False
        results.append({"mode": mode, "exitCode": run_result.returncode, "passed": passed, "imageSha256": sha(folder / "own-window.png") if (folder / "own-window.png").exists() else None})
        print(f"{mode}: exit={run_result.returncode}, passed={passed}", flush=True)
    private_json(out / "qualification.json", {"scope": "PUBLIC native click-open option panes", "defaultRelease": "compiled-snapshot/dsh-native-app", "defaultReleaseSha256": sha(artifacts / "dsh-native-app"), "defaultReleaseBytes": (artifacts / "dsh-native-app").stat().st_size, "fixtureReleaseSha256": sha(artifacts / "layout_smoke"), "sourceSha256": pins, "sourceUnchangedDuringCaptures": pins == {str(p.relative_to(ROOT)): sha(p) for p in sources}, "cases": results, "parentPaintInspectionComplete": False, "hostStarted": False, "modelCatalogRequested": False, "modelPrompts": 0, "physicalInputQualified": False, "fullCompiledSourceClosureQualified": False})
    return 0 if all(item["passed"] for item in results) else 1


if __name__ == "__main__":
    raise SystemExit(run())
