#!/usr/bin/env python3
"""Own-renderer PUBLIC actual-App detail snapshots, no Host or clipboard executor."""
import hashlib,json,os,pathlib,shutil,subprocess,tempfile
ROOT=pathlib.Path(__file__).resolve().parent
OUT=pathlib.Path(tempfile.mkdtemp(prefix='formatted-details-paint-',dir=ROOT/'evidence'));os.chmod(OUT,0o700)
print(f'EVIDENCE_ROOT={OUT}',flush=True)
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def private_json(path,value):
    with open(path,'x',opener=lambda p,f:os.open(p,f,0o600)) as output:json.dump(value,output,indent=2);output.write('\n')
frozen=OUT/'layout-probe'
with (ROOT/'target/release/examples/layout_smoke').open('rb') as source,open(frozen,'xb',opener=lambda p,f:os.open(p,f,0o700)) as output:shutil.copyfileobj(source,output)
sources=[ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'examples/layout_smoke.rs',ROOT/'qualify_formatted_details.py',*sorted((ROOT/'src').glob('*.rs'))]
pins={str(p.relative_to(ROOT)):sha(p) for p in sources}
env={k:v for k,v in os.environ.items() if k in {'WAYLAND_DISPLAY','XDG_RUNTIME_DIR','DBUS_SESSION_BUS_ADDRESS','LANG','LC_ALL','XDG_SESSION_TYPE','XDG_CURRENT_DESKTOP','XDG_SESSION_DESKTOP'}}
for name in ['home','config','data','cache']:(OUT/name).mkdir(mode=0o700)
env.update(PATH='/usr/bin:/bin',HOME=str(OUT/'home'),XDG_CONFIG_HOME=str(OUT/'config'),XDG_DATA_HOME=str(OUT/'data'),XDG_CACHE_HOME=str(OUT/'cache'))
cases=[]
for mode in ['assistant-details-source','assistant-details-formatted','assistant-details-narrow','assistant-details-fallback']:
    folder=OUT/mode;folder.mkdir(mode=0o700)
    with open(folder/'private-run.log','xb',opener=lambda p,f:os.open(p,f,0o600)) as log:
        run=subprocess.run([str(frozen),'--mode',mode,'--screenshot',str(folder/'own-window.png'),'--report',str(folder/'native.json')],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=20)
    report=json.loads((folder/'native.json').read_text()) if (folder/'native.json').exists() else {}
    expected_mode='formatted' if mode in {'assistant-details-formatted','assistant-details-narrow'} else 'source'
    passed=run.returncode==0 and report.get('status')=='passed' and report.get('effectsDispatched')==0 and report.get('capturedCommandCount')==0 and report.get('detailVisible') is True and report.get('detailMode')==expected_mode and report.get('detailFormattedAvailable')==(mode!='assistant-details-fallback') and report.get('windowBackend')=='wayland'
    cases.append({'mode':mode,'exitCode':run.returncode,'passed':passed,'sourceBytes':report.get('detailSourceBytes'),'imageSha256':sha(folder/'own-window.png') if (folder/'own-window.png').exists() else None})
    print(f'{mode}: passed={passed} exit={run.returncode}',flush=True)
summary={'scope':'PUBLIC bounded native basic-formatted frozen assistant display Details; actual App view and scripted mode update; exact native control bounds and own-renderer capture','passed':all(v['passed'] for v in cases),'cases':cases,'probeSha256':sha(frozen),'probeBytes':frozen.stat().st_size,'sourceSha256':pins,'sourceUnchangedDuringCaptures':pins=={str(p.relative_to(ROOT)):sha(p) for p in sources},'parentPaintInspectionComplete':False,'hostStarted':False,'businessExecutorStarted':False,'clipboardExecutorStarted':False,'desktopCapture':False,'physicalInputQualified':False,'compositorFractionalScalingQualified':False,'fullMarkdownQualified':False,'fullCompiledSourceClosureQualified':False,'fullParityQualified':False}
private_json(OUT/'qualification.json',summary)
print(f'QUALIFICATION={OUT/"qualification.json"}',flush=True)
raise SystemExit(0 if summary['passed'] else 1)
