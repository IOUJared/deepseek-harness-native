#!/usr/bin/env python3
"""Measure PUBLIC actual-App native controlled-frame workload, not input FPS.

Fresh Rust processes/private XDG directories, exact retained executable, no Host or
worker. /proc CPU/RSS belongs to this process only; observed child census is not an
adversarial ownership proof. Diagnostic model/observer/I/O overhead is retained,
not subtracted from wall latencies to manufacture a product-only measurement.
"""
import argparse
import hashlib
import json
import math
import os
import pathlib
import shutil
import subprocess
import tempfile
import time

ROOT=pathlib.Path(__file__).resolve().parent
TICKS=os.sysconf("SC_CLK_TCK")
PAGE=os.sysconf("SC_PAGE_SIZE")

def private_json(path,value):
    with open(path,"x",opener=lambda p,f:os.open(p,f,0o600)) as output:
        json.dump(value,output,indent=2);output.write("\n")

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def percentile(values,p):
    if not values:return None
    ordered=sorted(values)
    return ordered[max(0,math.ceil(p*len(ordered))-1)]

def proc_sample(pid,start):
    before=time.monotonic_ns()
    try:
        stat=pathlib.Path(f"/proc/{pid}/stat").read_text()
        fields=stat[stat.rfind(")")+2:].split()
        identity=int(fields[19])
        if start is not None and identity!=start:return None
        if fields[0] in {"Z","X"}:return None
        children=set()
        for task in pathlib.Path(f"/proc/{pid}/task").iterdir():
            try:children.update(int(v) for v in (task/"children").read_text().split())
            except (OSError,ValueError):pass
        pss=None
        try:
            for line in pathlib.Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines():
                if line.startswith("Pss:"):pss=int(line.split()[1]);break
        except (OSError,ValueError):pass
        return {"monotonicNs":before,"unixTimestampNs":time.time_ns(),"sampleDurationNs":time.monotonic_ns()-before,"pid":pid,"startTicks":identity,"cpuTicks":int(fields[11])+int(fields[12]),"rssKiB":int(fields[21])*PAGE/1024,"pssKiB":pss,"directChildrenObserved":sorted(children)}
    except (OSError,ValueError,IndexError):return None

def markers(path):
    result=[]
    if path.exists():
        for line in path.read_text().splitlines():
            try:result.append(json.loads(line))
            except ValueError:pass
    return result

def idle_interval(records):
    # Metadata names are fixed by the owned probe; no model payload interpretation.
    start=end=None
    for record in records:
        tag=record.get("stage")
        if tag in {"idle-begin","idle-start","idle-hold-start","idleStart"}:start=record.get("unixTimestampNs")
        if tag in {"idle-end","idle-hold-end","idleEnd"}:end=record.get("unixTimestampNs")
    return (start,end)

def resources(samples,interval=None):
    selected=samples if not interval or None in interval else [s for s in samples if interval[0]<=s["unixTimestampNs"]<=interval[1]]
    cpus=[]
    for a,b in zip(selected,selected[1:]):
        dt=(b["monotonicNs"]-a["monotonicNs"])/1e9
        if dt>0 and b["cpuTicks"]>=a["cpuTicks"]:cpus.append((b["cpuTicks"]-a["cpuTicks"])/TICKS/dt*100)
    first,last=(selected[0],selected[-1]) if selected else (None,None)
    dt=(last["monotonicNs"]-first["monotonicNs"])/1e9 if first else 0
    pss=[s["pssKiB"] for s in selected if s["pssKiB"] is not None]
    return {"samples":len(selected),"observedSeconds":dt,"oneCoreCpuPercent":(last["cpuTicks"]-first["cpuTicks"])/TICKS/dt*100 if dt>0 else None,"oneCoreCpuP95Percent":percentile(cpus,.95),"peakRssMiB":max((s["rssKiB"]/1024 for s in selected),default=None),"medianRssMiB":percentile([s["rssKiB"]/1024 for s in selected],.5),"peakPssMiB":max((v/1024 for v in pss),default=None),"medianPssMiB":percentile([v/1024 for v in pss],.5),"maximumSampleReadMs":max((s["sampleDurationNs"]/1e6 for s in selected),default=None),"ownedDirectChildrenAbsentInSamples":all(not s["directChildrenObserved"] for s in selected)}

def run_case(binary,out,rows,repeat,cycles,idle_ms):
    folder=out/f"rows-{rows}-run-{repeat}";folder.mkdir(mode=0o700)
    env={k:v for k,v in os.environ.items() if k in {"WAYLAND_DISPLAY","XDG_RUNTIME_DIR","DBUS_SESSION_BUS_ADDRESS","LANG","LC_ALL","XDG_SESSION_TYPE","XDG_CURRENT_DESKTOP","XDG_SESSION_DESKTOP"}}
    for name in ["home","config","data","cache"]:(folder/name).mkdir(mode=0o700)
    env.update(PATH="/usr/bin:/bin",HOME=str(folder/"home"),XDG_CONFIG_HOME=str(folder/"config"),XDG_DATA_HOME=str(folder/"data"),XDG_CACHE_HOME=str(folder/"cache"))
    args=[str(binary),"--report",str(folder/"native.json"),"--phase-log",str(folder/"phases.jsonl"),"--rows",str(rows),"--cycles",str(cycles),"--idle-ms",str(idle_ms)]
    started=time.monotonic_ns();samples=[];missing=0;identity=None;timeout=False
    with open(folder/"private-run.log","xb",opener=lambda p,f:os.open(p,f,0o600)) as log:
        process=subprocess.Popen(args,cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
        try:
            while process.poll() is None:
                sample=proc_sample(process.pid,identity)
                if sample:
                    identity=sample["startTicks"];samples.append(sample)
                else:missing+=1
                if time.monotonic_ns()-started>60_000_000_000:
                    timeout=True;process.kill();break
                time.sleep(.05)
        finally:
            code=process.wait(timeout=5)
    wall=(time.monotonic_ns()-started)/1e9
    report=json.loads((folder/"native.json").read_text()) if (folder/"native.json").exists() else {}
    phase_records=markers(folder/"phases.jsonl");interval=idle_interval(phase_records)
    idle=resources(samples,interval) if None not in interval else None
    valid=code==0 and report.get("status")=="passed" and report.get("commandCount")==0 and report.get("windowBackend")=="wayland" and report.get("finalModel",{}).get("keysCount")==rows+12 and report.get("finalModel",{}).get("anchorKey")== (112 if rows==80 else 100+rows//2) and report.get("finalModel",{}).get("anchorIntra")==12 and idle is not None and idle["samples"]>=10
    case={"rowsInitial":rows,"rowsFinal":rows+12,"repeat":repeat,"cycles":cycles,"exitCode":code,"timeout":timeout,"passed":valid,"wallSeconds":wall,"pid":process.pid,"startTicks":identity,"clockTicksPerSecond":TICKS,"samplingTargetSeconds":.05,"missingSamples":missing,"wholeInstrumentedProcess":resources(samples),"idleHold":idle,"idleUnixIntervalNs":interval,"sampleScope":"single owned fresh Rust process, threads included in process ticks; no Host/worker; children census observational, RSS/PSS not GPU VRAM or complete backend memory","startupScope":"fresh process/private profile+cache; global OS/font/GPU caches not cleared","physicalInputQualified":False,"liveHostQualified":False,"nativeWindowResizeQualified":False,"fullAppPerformanceQualified":False,"nativeReport":"native.json","phaseLog":"phases.jsonl"}
    private_json(folder/"samples.json",samples);private_json(folder/"measurement.json",case)
    print(f"rows={rows} run={repeat} passed={valid} wall={wall:.3f}s idle={idle}",flush=True)
    return case

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--label",default="baseline")
    parser.add_argument("--rows",nargs="+",type=int,default=[80,4084])
    parser.add_argument("--repeats",type=int,default=3)
    parser.add_argument("--cycles",type=int,default=4)
    parser.add_argument("--idle-ms",type=int,default=2000)
    options=parser.parse_args()
    if not options.label.replace("-","").isalnum() or any(not 80<=v<=4084 for v in options.rows) or not 1<=options.repeats<=8 or not 1<=options.cycles<=32 or not 1000<=options.idle_ms<=5000:parser.error("invalid bounded workload")
    out=pathlib.Path(tempfile.mkdtemp(prefix=f"native-history-{options.label}-",dir=ROOT/"evidence"));os.chmod(out,0o700)
    print(f"EVIDENCE_ROOT={out}",flush=True)
    frozen=out/"native-history-probe"
    with (ROOT/"target/release/examples/scroll_app_smoke").open("rb") as source,open(frozen,"xb",opener=lambda p,f:os.open(p,f,0o700)) as output:shutil.copyfileobj(source,output)
    source_files=[ROOT/"Cargo.toml",ROOT/"Cargo.lock",ROOT/"examples/scroll_app_smoke.rs",ROOT/"measure_native_history.py",*sorted((ROOT/"src").glob("*.rs"))]
    pins={str(p.relative_to(ROOT)):sha(p) for p in source_files}
    private_json(out/"build-observation.json",{"probeSha256":sha(frozen),"probeBytes":frozen.stat().st_size,"sourceSha256":pins,"fullCompiledSourceClosureQualified":False})
    cases=[run_case(frozen,out,rows,repeat,options.cycles,options.idle_ms) for repeat in range(1,options.repeats+1) for rows in options.rows]
    summary={"scope":"PUBLIC actual native production-App controlled-frame history reflow and instrumented process resources; not wheel scrolling FPS, physical input or integrated Host performance","label":options.label,"passed":all(v["passed"] for v in cases),"cases":cases,"probeSha256":sha(frozen),"probeBytes":frozen.stat().st_size,"sourceUnchangedDuringRuns":pins=={str(p.relative_to(ROOT)):sha(p) for p in source_files},"fullCompiledSourceClosureQualified":False,"fullAppPerformanceQualified":False,"physicalInputQualified":False,"liveHostQualified":False,"nativeWindowResizeQualified":False}
    private_json(out/"qualification.json",summary)
    print(f"QUALIFICATION={out/'qualification.json'}",flush=True)
    return 0 if summary["passed"] else 1

if __name__=="__main__":raise SystemExit(main())
