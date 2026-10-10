#!/usr/bin/env python3
"""Opt-in real native Wayland drop: private nested compositor, PUBLIC files, zero Host.
Only the new inner socket receives software pointer input. No outer desktop dispatch.
"""
import argparse
import asyncio
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import struct
import subprocess
import tempfile
import time
import zlib

BASE = Path(__file__).resolve().parents[1]
def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, BASE / 'scripts' / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module
SEAT = load('drop_seat', 'qualify-private-native-seat.py')
OWN = SEAT.OWN

def png():
    def chunk(kind, data):
        return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data))
    raw = b''.join(b'\0' + bytes((100, 155, 210)) * 32 for _ in range(24))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!IIBBBBB', 32, 24, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b'')

async def run(args):
    os.umask(0o077)
    output = Path(tempfile.mkdtemp(prefix='PUBLIC-native-drop-', dir=BASE / 'app/evidence')).resolve()
    scratch = Path(tempfile.mkdtemp(prefix='d-', dir='/tmp')).resolve()
    runtime = scratch / 'runtime'
    runtime.mkdir(mode=0o700)
    for name in ('workspace', 'user', 'config', 'cache', 'data', 'state', 'bin', 'files'):
        (output / name).mkdir(mode=0o700)
    images = [output / 'files/PUBLIC-photo space é.png', output / 'files/PUBLIC-second.png']
    for path in images: path.write_bytes(png())
    binaries = {}
    for name, path in [('app', args.fixture), ('source', args.source), ('pointer', args.pointer)]:
        path = Path(path).resolve(strict=True)
        data = SEAT.executable(str(path), hashlib.sha256(path.read_bytes()).hexdigest())
        binaries[name] = SEAT.copy_executable(output / 'bin' / name, data)
    config = output / 'private-hyprland.lua'
    config.write_text('''hl.monitor({output="",mode="1920x1080@60",position="auto",scale=1})
hl.config({general={layout="dwindle",gaps_in=0,gaps_out=0,border_size=1},animations={enabled=false},decoration={rounding=0,blur={enabled=false},shadow={enabled=false}},xwayland={enabled=false},input={kb_layout="us",follow_mouse=1},misc={disable_hyprland_logo=true,disable_splash_rendering=true}})
''')
    outer_runtime = Path(os.environ['XDG_RUNTIME_DIR']).resolve(strict=True)
    outer_display = Path(os.environ['WAYLAND_DISPLAY'])
    if not outer_display.is_absolute(): outer_display = outer_runtime / outer_display
    if not outer_display.is_socket() or outer_display.stat().st_uid != os.getuid():
        raise ValueError('owned-outer-display-required')
    env = {'PATH':'/usr/bin:/bin','LANG':'C.UTF-8','HOME':str(output/'user'),
           'XDG_RUNTIME_DIR':str(runtime),'XDG_CONFIG_HOME':str(output/'config'),
           'XDG_CACHE_HOME':str(output/'cache'),'XDG_DATA_HOME':str(output/'data'),
           'XDG_STATE_HOME':str(output/'state'),'WAYLAND_DISPLAY':str(outer_display),
           'LIBSEAT_BACKEND':'PUBLIC_DISABLED','HYPRLAND_NO_SD_VARS':'1',
           'HYPRLAND_NO_SD_NOTIFY':'1','HYPRLAND_NO_RT':'1',
           'GDK_BACKEND':'wayland','GIO_USE_VFS':'local','GTK_USE_PORTAL':'0',
           'GSETTINGS_BACKEND':'memory','NO_AT_BRIDGE':'1',
           'DBUS_SYSTEM_BUS_ADDRESS':'unix:path='+str(scratch/'no-system-bus'),
           'WINIT_UNIX_BACKEND':'wayland','ICED_BACKEND':'wgpu'}
    resources = []
    result = {'status':'failed','publicOnly':True,'physicalHardwareMouseQualified':False,
              'noOuterInputInjection':True,'noHostWorker':True,'output':str(output),
              'scratchRetained':str(scratch),'mode':args.mode,'sourceKind':args.kind,
              'sourceSha256':{str(p.relative_to(BASE)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [BASE/'app/examples/layout_smoke.rs', BASE/'app/src/attachment_ui.rs', BASE/'app/src/layout_fixture.rs', BASE/'app/vendor/winit/src/platform_impl/linux/wayland/data_device.rs']}}
    stop = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM): loop.add_signal_handler(sig, stop.set)
    deadline = time.monotonic()+75
    async def tick():
        for resource in resources: SEAT.observe(resource)
        if time.monotonic()>=deadline or stop.is_set(): raise ValueError('owned-drop-run-deadline')
        await asyncio.sleep(.05)
    try:
        comp = await SEAT.spawn(['/usr/bin/dbus-run-session','--','/usr/bin/Hyprland','--config',str(config)],env,output,'compositor',resources)
        lock = None
        while lock is None:
            await tick()
            if comp['child'].returncode is not None: raise ValueError('nested-compositor-exited')
            candidates = list((runtime/'hypr').glob('*/hyprland.lock'))
            if len(candidates)==1: lock=candidates[0]
        values=lock.read_text().splitlines()
        pid=int(values[0]); display=values[1]; signature=lock.parent.name
        current=OWN.METRICS.proc_stat(pid)
        if current is None or OWN.identity(current) not in comp['pidfds']: raise ValueError('inner-compositor-not-pinned')
        if display.startswith('/') or '/' in display or not display.startswith('wayland-'): raise ValueError('inner-display-name-refused')
        socket=runtime/display
        while not socket.is_socket(): await tick()
        raw=Path(f'/proc/{pid}/environ').read_bytes()
        bus=next(x.split(b'=',1)[1].decode() for x in raw.split(b'\0') if x.startswith(b'DBUS_SESSION_BUS_ADDRESS='))
        inner={**env,'WAYLAND_DISPLAY':str(socket),'DBUS_SESSION_BUS_ADDRESS':bus,'HYPRLAND_INSTANCE_SIGNATURE':signature}
        def ipc(*args):
            r=subprocess.run(['/usr/bin/hyprctl','-i',signature,*args],env=inner,capture_output=True,text=True,timeout=3)
            if r.returncode:
                result.setdefault('ipcFailures',[]).append({'command':list(args),'stdout':r.stdout,'stderr':r.stderr,'exitCode':r.returncode})
                raise ValueError('private-compositor-ipc-failed')
            return r.stdout
        monitors=json.loads(ipc('-j','monitors'))
        if len(monitors)!=1 or monitors[0]['width']<1180 or monitors[0]['height']<620: raise ValueError('private-monitor-too-small')
        width,height=monitors[0]['width'],monitors[0]['height']
        result['privateMonitor']={k:monitors[0][k] for k in ('name','width','height','scale')}
        app=await SEAT.spawn([str(binaries['app']),'--mode','narrow-welcome','--drop-file',str(images[0]),'--drop-expect','rejected' if args.mode=='multiple' else 'accepted','--screenshot',str(output/'app.png'),'--report',str(output/'app.json')],inner,output,'app',resources)
        while not (output/'drop-ready.json').exists():
            await tick()
            if app['child'].returncode is not None: raise ValueError('drop-fixture-exited-before-ready')
        if args.kind=='thunar':
            source_args=['/usr/bin/thunar',str(output/'files')]
        else:
            source_args=[str(binaries['source']),'--multiple' if args.mode=='multiple' else '--single',str(images[0])]
            if args.mode=='multiple':source_args.append(str(images[1]))
        source=await SEAT.spawn(source_args,{**inner,'WAYLAND_DEBUG':'client'},output,'source',resources)
        clients=[]
        while len(clients)<2:
            await tick();clients=json.loads(ipc('-j','clients'))
            if source['child'].returncode is not None:raise ValueError('native-source-exited')
        target=next(c for c in clients if c['pid']==app['child'].pid)
        src=next(c for c in clients if c['pid']==source['child'].pid)
        for client,x,y,w,h in [(target,10,10,760,592),(src,800,10,360,592)]:
            addr='address:'+client['address']
            selector=json.dumps(addr)
            for expression in [f'hl.dsp.window.float({{action="enable",window={selector}}})',f'hl.dsp.window.resize({{x={w},y={h},relative=false,window={selector}}})',f'hl.dsp.window.move({{x={x},y={y},relative=false,window={selector}}})']:
                ipc('dispatch',expression)
        for _ in range(4): await tick()
        clients=json.loads(ipc('-j','clients'));target=next(c for c in clients if c['pid']==app['child'].pid);src=next(c for c in clients if c['pid']==source['child'].pid)
        result['privateClientRects']=[{k:c[k] for k in ('pid','at','size','class')} for c in (target,src)]
        sx=src['at'][0]+src['size'][0]//2;sy=src['at'][1]+src['size'][1]//2
        if args.kind=='thunar':sx=src['at'][0]+225;sy=src['at'][1]+155
        tx=target['at'][0]+target['size'][0]//2;ty=target['at'][1]+target['size'][1]//2
        owner_fd=comp['pidfds'][OWN.identity(current)]
        pointer=await SEAT.spawn([str(binaries['pointer']),str(scratch),str(socket),str(pid),str(current['start_ticks']),str(owner_fd),str(sx),str(sy),str(tx),str(ty),str(width),str(height)],inner,output,'pointer',resources,(owner_fd,))
        while app['child'].returncode is None:
            await tick()
            if pointer['child'].returncode not in (None,0):raise ValueError('pointer-transport-failed')
        result['app']=json.loads((output/'app.json').read_text())
        result['sourceFilesUnchanged']=all(p.exists() and p.read_bytes()==png() for p in images)
        result['actualNativeDropQualified']=(app['child'].returncode==0 and result['app'].get('fixturePassed') is True and result['app'].get('actualProductionNativeDropQualified') is True and result['sourceFilesUnchanged'])
        result['status']='passed' if result['actualNativeDropQualified'] else 'failed'
    except (OSError,ValueError,KeyError,StopIteration,subprocess.TimeoutExpired) as error:
        result['failureCode']=str(error) if isinstance(error,ValueError) else type(error).__name__
    finally:
        cleanups=[]
        for resource in reversed(resources):
            SEAT.observe(resource)
            cleanup=await OWN.cleanup_owned(resource['child'],resource['pidfds'],resource['pinned'])
            cleanups.append({'name':resource['name'],'exitCode':resource['child'].returncode,'cleanup':cleanup})
            for fd in resource['pidfds'].values():os.close(fd)
        result['processes']=cleanups
        if any(not c['cleanup']['observedCleanupComplete'] for c in cleanups):result['status']='failed'
        OWN.private_json(output/'qualification.json',result)
        for sig in (signal.SIGINT,signal.SIGTERM):loop.remove_signal_handler(sig)
    print(json.dumps({'status':result['status'],'report':str(output/'qualification.json'),'failureCode':result.get('failureCode')}))
    return 0 if result['status']=='passed' else 1

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture',required=True);parser.add_argument('--source',required=True);parser.add_argument('--pointer',required=True)
    parser.add_argument('--mode',choices=('single','multiple'),default='single')
    parser.add_argument('--kind',choices=('gtk','thunar'),default='gtk')
    return asyncio.run(run(parser.parse_args()))
if __name__=='__main__':raise SystemExit(main())
