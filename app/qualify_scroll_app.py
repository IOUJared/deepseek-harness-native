#!/usr/bin/env python3
"""Own PUBLIC production-App/native-widget probe only; no Host or business executor.
Run after serial offline/locked feature example build. Freeze exact observed binary.
Source pins are observations, not a reproducible complete compiler/runtime closure.
"""
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import tempfile

ROOT=pathlib.Path(__file__).resolve().parent
OUT=pathlib.Path(tempfile.mkdtemp(prefix="scroll-app-",dir=ROOT/"evidence"))
os.chmod(OUT,0o700)
print(f"EVIDENCE_ROOT={OUT}",flush=True)
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def private_json(path,value):
    with open(path,"x",opener=lambda p,f:os.open(p,f,0o600)) as output:
        json.dump(value,output,indent=2);output.write("\n")
binary=ROOT/"target/release/examples/scroll_app_smoke"
frozen=OUT/"scroll-app-probe"
with binary.open("rb") as source,open(frozen,"xb",opener=lambda p,f:os.open(p,f,0o700)) as output:
    shutil.copyfileobj(source,output)
sources=[ROOT/"Cargo.toml",ROOT/"Cargo.lock",ROOT/"examples/scroll_app_smoke.rs",*sorted((ROOT/"src").glob("*.rs"))]
pins={str(p.relative_to(ROOT)):sha(p) for p in sources}
private_json(OUT/"build-observation.json",{
    "scope":"PUBLIC actual production App/native widget integration, no Host/business executor",
    "probeSha256":sha(frozen),"probeBytes":frozen.stat().st_size,
    "sourceSha256":pins,"fullCompiledSourceClosureQualified":False,
})
env={k:v for k,v in os.environ.items() if k in {"WAYLAND_DISPLAY","XDG_RUNTIME_DIR","DBUS_SESSION_BUS_ADDRESS","LANG","LC_ALL","XDG_SESSION_TYPE","XDG_CURRENT_DESKTOP","XDG_SESSION_DESKTOP"}}
for name in ["home","config","data","cache"]:(OUT/name).mkdir(mode=0o700)
env.update(PATH="/usr/bin:/bin",HOME=str(OUT/"home"),XDG_CONFIG_HOME=str(OUT/"config"),XDG_DATA_HOME=str(OUT/"data"),XDG_CACHE_HOME=str(OUT/"cache"))
with open(OUT/"private-run.log","xb",opener=lambda p,f:os.open(p,f,0o600)) as output:
    run=subprocess.run([str(frozen),"--report",str(OUT/"result.json"),"--screenshot",str(OUT/"own-window.png")],cwd=ROOT,env=env,stdout=output,stderr=subprocess.STDOUT,timeout=30)
report=json.loads((OUT/"result.json").read_text()) if (OUT/"result.json").exists() else {}
passed=run.returncode==0 and report.get("status")=="passed" and report.get("windowBackend")=="wayland" and report.get("callbackCounts")==[1,1,1,0,1] and report.get("commandCount")==0 and report.get("effectsDispatched")==0 and report.get("zeroBusinessEffects") is True and report.get("hostStarted") is False and report.get("workerStarted") is False
private_json(OUT/"qualification.json",{
    "scope":"PUBLIC actual production App view/update/receive/restore + native Scrolled feedback and independently observed raw slot bounds",
    "passed":passed,"exitCode":run.returncode,"probeArtifact":"scroll-app-probe","probeSha256":sha(frozen),"probeBytes":frozen.stat().st_size,
    "sourceSha256":pins,"sourceUnchangedDuringRun":pins=={str(p.relative_to(ROOT)):sha(p) for p in sources},
    "ownRendererPngSha256":sha(OUT/"own-window.png") if (OUT/"own-window.png").exists() else None,
    "parentPaintInspectionComplete":False,"publicProductionAppWidgetCouplingQualified":passed,
    "fullCompiledSourceClosureQualified":False,"physicalInputQualified":False,"liveHostQualified":False,"liveGeneratedTextQualified":False,"integratedPerformanceQualified":False,"fullParityQualified":False,
})
print(f"QUALIFICATION={OUT/'qualification.json'}",flush=True)
print(f"[exit code: {run.returncode}]",flush=True)
raise SystemExit(0 if passed else 1)
