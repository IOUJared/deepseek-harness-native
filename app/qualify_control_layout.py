#!/usr/bin/env python3
"""Run owned PUBLIC/native-keyless captures serially; never desktop capture or model work."""
import hashlib, json, os, pathlib, shutil, subprocess, tempfile
ROOT = pathlib.Path(__file__).resolve().parent
OUT = pathlib.Path(tempfile.mkdtemp(prefix="control-layout-", dir=ROOT / "evidence"))
print(f"EVIDENCE={OUT}", flush=True)
def private_json(path, value):
    with open(path, "x", opener=lambda p,f:os.open(p,f,0o600)) as stream:
        json.dump(value, stream, indent=2); stream.write("\n")
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
# Retain only display/session essentials; never inherit API/provider credentials.
env = {k:v for k,v in os.environ.items() if k in {"WAYLAND_DISPLAY","XDG_RUNTIME_DIR","DBUS_SESSION_BUS_ADDRESS","LANG","LC_ALL","XDG_SESSION_TYPE","XDG_CURRENT_DESKTOP","XDG_SESSION_DESKTOP"}}
env.update(PATH="/usr/bin:/bin", ICED_BACKEND="wgpu", XDG_CONFIG_HOME=str(OUT / "config"), XDG_DATA_HOME=str(OUT / "data"), XDG_CACHE_HOME=str(OUT / "cache"))
for name in ["user-home","config","data","cache"]: (OUT/name).mkdir(mode=0o700)
env["HOME"] = str(OUT / "user-home")
CASES = ["wide-welcome","narrow-welcome","tiny-welcome","menu","navigation","navigation-tiny","collapsed-wide","conversation","long-model","tiny-long-model","warnings","settings-general","settings-codex","information","information-expanded","tool-activity","tool-activity-narrow","tool-activity-tiny","tool-activity-details","chat-density","assistant-details-source","assistant-details-formatted","assistant-details-narrow","assistant-details-fallback","file-path-review","file-path-tiny","file-ready","file-unknown","inbox-canceled","title-controls","title-controls-expanded"]
# Preserve the exact executables observed even when another writer rebuilds target/.
# Source pins are observations, not proof of a full compiler/runtime build closure.
artifacts=OUT/"compiled-snapshot"; artifacts.mkdir(mode=0o700)
def freeze(source, name):
    target=artifacts/name
    with open(source,"rb") as src, open(target,"xb",opener=lambda p,f:os.open(p,f,0o700)) as dst:
        shutil.copyfileobj(src,dst)
    return target
runner=freeze(ROOT/"target/release/examples/layout_smoke","layout_smoke")
app=freeze(ROOT/"target/release/dsh-native-app","dsh-native-app")
sources=[ROOT/"Cargo.toml",ROOT/"Cargo.lock"]+list((ROOT/"src").glob("*.rs"))+[ROOT/"examples/layout_smoke.rs"]
source_pins={str(p.relative_to(ROOT)):sha(p) for p in sources}
results=[]
for mode in CASES:
    folder=OUT/mode; folder.mkdir(mode=0o700)
    command=[str(runner),"--mode",mode,"--screenshot",str(folder/"own-window.png"),"--report",str(folder/"result.json")]
    with open(folder/"private-run.log","xb",opener=lambda p,f:os.open(p,f,0o600)) as log:
        run=subprocess.run(command,cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=20)
    report=json.loads((folder/"result.json").read_text()) if (folder/"result.json").exists() else {}
    passed=run.returncode==0 and report.get("status")=="passed" and report.get("effectsDispatched")==0 and report.get("publicSource") is True
    results.append({"mode":mode,"exitCode":run.returncode,"passed":passed,"imageSha256":sha(folder/"own-window.png") if (folder/"own-window.png").exists() else None})
    print(f"{mode}: exit={run.returncode}, passed={passed}",flush=True)
# Actual production binary against explicitly isolated same-version Alpha; zero credentials/prompts.
folder=OUT/"real-keyless"; folder.mkdir(mode=0o700)
for name in ["native-home","user-home","workspace"]:(folder/name).mkdir(mode=0o700)
command=[str(app),"--runtime",str(ROOT.parents[1]/"deepseek-harness-linux/apps/cli"),"--expected-version","0.2.1-alpha.1","--home",str(folder/"native-home"),"--user-home",str(folder/"user-home"),"--cwd",str(folder/"workspace"),"--scale","1.0","--smoke-new-session","--exit-after-seconds","8","--smoke-report",str(folder/"result.json"),"--smoke-screenshot",str(folder/"own-window.png")]
with open(folder/"private-run.log","xb",opener=lambda p,f:os.open(p,f,0o600)) as log:
    run=subprocess.run(command,cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=30)
report=json.loads((folder/"result.json").read_text()) if (folder/"result.json").exists() else {}
real_pass=run.returncode==0 and report.get("status")=="passed" and report.get("modelPrompts")==0 and report.get("modelCatalogRequested") is False and report.get("windowBackend")=="wayland" and report.get("snapshotFoldValid") is True and report.get("stop",{}).get("graceful") is True and report.get("stop",{}).get("containmentUnknown") is False
print(f"real-keyless: exit={run.returncode}, passed={real_pass}",flush=True)
source_stable=source_pins=={str(p.relative_to(ROOT)):sha(p) for p in sources}
manifest={"scope":"native-control-layout-and-Harness-style-conversation","defaultReleaseSha256":sha(app),"defaultReleaseBytes":app.stat().st_size,"fixtureReleaseSha256":sha(runner),"sourceSha256":source_pins,"sourceUnchangedDuringCaptures":source_stable,"fullCompiledSourceClosureQualified":False,"defaultReleaseArtifact":"compiled-snapshot/dsh-native-app","fixtureReleaseArtifact":"compiled-snapshot/layout_smoke","publicCases":results,"realKeyless":{"exitCode":run.returncode,"passed":real_pass,"imageSha256":sha(folder/"own-window.png") if (folder/"own-window.png").exists() else None},"parentPaintInspectionComplete":False,"physicalInputQualified":False,"realAccountQualified":False,"modelGenerationQualified":False,"compositorFractionalScalingQualified":False,"fullParityQualified":False}
private_json(OUT/"qualification.json",manifest)
print(f"QUALIFICATION={OUT/'qualification.json'}",flush=True)
raise SystemExit(0 if all(x["passed"] for x in results) and real_pass else 1)
