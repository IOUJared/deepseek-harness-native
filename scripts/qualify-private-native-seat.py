#!/usr/bin/env python3
"""Reviewed opt-in PRIVATE headless Gamescope seat; exact PUBLIC App result required.
No default launch/build/install. No desktop socket, released profile, account or clipboard access.
"""
import argparse
import asyncio
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import tempfile
import time

BASE = Path(__file__).resolve().parents[1]
TEXT = 'public native keyboard check'
STOP = {'exited': True, 'graceful': True, 'containmentUnknown': False,
        'observedDescendantsRemaining': 0, 'exitCode': 0}


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, BASE / 'scripts' / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


OWN = load('private_seat_owned', 'qualify-composed-settings.py')
ART = load('private_seat_artifacts', 'qualify-draft-restart.py')


def executable(value, digest):
    path = Path(value)
    if not path.is_absolute() or path != path.resolve() or not re.fullmatch(r'[0-9a-f]{64}', digest):
        raise ValueError('explicit-canonical-executable-and-sha256-required')
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or not os.access(path, os.X_OK):
        raise ValueError('regular-executable-required')
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC), 'rb') as stream:
        opened = os.fstat(stream.fileno())
        if (before.st_dev, before.st_ino) != (opened.st_dev, opened.st_ino):
            raise ValueError('executable-changed')
        data = stream.read(128 * 1024 * 1024 + 1)
    if len(data) > 128 * 1024 * 1024 or hashlib.sha256(data).hexdigest() != digest:
        raise ValueError('executable-sha256-mismatch')
    return data


def copy_executable(path, data):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o700), 'wb') as stream:
        stream.write(data)
    return path


def natural(value, minimum=0, maximum=(1 << 31) - 1):
    return type(value) is int and minimum <= value <= maximum


def valid_ready(value):
    return (type(value) is dict and type(value.get('version')) is int and value['version'] == 1 and value.get('publicOnly') is True
            and value.get('phase') == 'keyboard-ready' and value.get('nativeWayland') is True
            and value.get('composerEmpty') is True and type(value.get('promptCommands')) is int and value['promptCommands'] == 0
            and natural(value.get('pid'), 1)
            and type(value.get('waylandDisplay')) is str
            and re.fullmatch(r'gamescope-[0-9]{1,3}', value['waylandDisplay']) is not None
            and all(natural(value.get(key), 200, 4096) for key in ('windowWidth', 'windowHeight'))
            and natural(value.get('pointerFocusX'), 1, value['windowWidth'] - 1)
            and natural(value.get('pointerFocusY'), 1, value['windowHeight'] - 1))


def valid_result(value, pid):
    stop = value.get('stop') if type(value) is dict else None
    return (type(value) is dict and type(value.get('version')) is int and value['version'] == 1 and value.get('publicOnly') is True
            and value.get('phase') == 'complete' and natural(value.get('pid'), 1) and value['pid'] == pid
            and value.get('actualProductionEditor') is True and value.get('actualWaylandKeyboardEvents') is True
            and value.get('body') == TEXT and natural(value.get('keyPressed'), len(TEXT))
            and type(value.get('promptCommands')) is int and value['promptCommands'] == 0
            and value.get('manualSendAllowed') is False
            and type(stop) is dict and set(stop) == set(STOP)
            and all(type(stop[key]) is type(expected) and stop[key] == expected for key, expected in STOP.items()))


def private_socket_path(runtime, display):
    if type(display) is not str or re.fullmatch(r'gamescope-[0-9]{1,3}', display) is None:
        raise ValueError('private-socket-name-refused')
    path = Path(runtime) / (display + '-ei')
    # Linux sockaddr_un.sun_path[108] includes the terminating NUL.
    if not path.is_absolute() or len(os.fsencode(path)) >= 108:
        raise ValueError('private-socket-path-too-long')
    return path


def valid_runtime_manifest(value, output):
    if (type(value) is not dict or set(value) != {'version', 'publicOnly', 'runtimePath', 'runtimeStamp', 'scratchPath', 'evidenceRoot'}
            or type(value.get('version')) is not int or value['version'] != 1 or value.get('publicOnly') is not True
            or any(type(value.get(key)) is not str for key in ('runtimePath', 'scratchPath', 'evidenceRoot'))):
        return False
    stamp = value['runtimeStamp']
    if (type(stamp) is not dict or set(stamp) != {'device', 'inode', 'uid'}
            or any(type(stamp[key]) is not int or stamp[key] < 0 for key in stamp)):
        return False
    scratch, runtime, evidence = (Path(value[key]) for key in ('scratchPath', 'runtimePath', 'evidenceRoot'))
    try:
        if (any(not path.is_absolute() or path != path.resolve() for path in (scratch, runtime, evidence))
                or scratch.parent != Path('/tmp').resolve() or not scratch.name.startswith('PUBLIC-native-seat-')
                or runtime != scratch / 'runtime' or evidence != Path(output).resolve()):
            return False
        for path in (scratch, runtime, evidence):
            info = path.lstat()
            if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
                return False
        info = runtime.lstat()
        if stamp != {'device': info.st_dev, 'inode': info.st_ino, 'uid': info.st_uid}:
            return False
        private_socket_path(runtime, 'gamescope-127')
    except (OSError, ValueError, RuntimeError):
        return False
    return True


def prepare_short_runtime(output):
    # Separate owned scratch avoids long evidence-root paths exceeding sun_path.
    # Retained intentionally: never remove a socket tree while a child may live.
    scratch = Path(tempfile.mkdtemp(prefix='PUBLIC-native-seat-', dir='/tmp')).resolve()
    os.chmod(scratch, 0o700)
    runtime = scratch / 'runtime'
    runtime.mkdir(mode=0o700)
    info = runtime.lstat()
    manifest = {'version': 1, 'publicOnly': True, 'runtimePath': str(runtime),
                'runtimeStamp': {'device': info.st_dev, 'inode': info.st_ino, 'uid': info.st_uid},
                'scratchPath': str(scratch), 'evidenceRoot': str(output.resolve())}
    if not valid_runtime_manifest(manifest, output):
        raise ValueError('new-short-runtime-refused')
    OWN.private_json(output / 'runtime-manifest.json', manifest)
    return scratch, runtime, manifest


async def spawn(argv, env, output, name, resources, pass_fds=()):
    logs = [os.open(output / f'{name}-{kind}.log', os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            for kind in ('stdout', 'stderr')]
    try:
        child = subprocess.Popen(argv, env=env, cwd=output / 'workspace', stdin=subprocess.DEVNULL,
                                 stdout=logs[0], stderr=logs[1], pass_fds=pass_fds, start_new_session=True)
    finally:
        for fd in logs:
            os.close(fd)
    resource = {'name': name, 'child': child, 'pidfds': {}, 'observed': set(), 'pinned': False, 'pinFailures': 0}
    resources.append(resource)  # Record before pinning so acquisition failure still receives owned cleanup.
    root, fd = OWN.pin_root(child)
    resource.update(root=root, pinned=True)
    resource['pidfds'][OWN.identity(root)] = fd
    resource['observed'].add(OWN.identity(root))
    return resource


def observe(resource):
    child = resource['child']
    if child.returncode is None and resource['pinned']:
        resource['pinFailures'] += OWN.observe_descendants(child, resource['root'], resource['pidfds'], resource['observed'])
    if OWN.poll_child(child):
        raise ValueError('owned-child-poll-failed')


async def run(args):
    fixture = executable(args.fixture, args.fixture_sha256)
    sender = executable(args.sender, args.sender_sha256)
    runtime = Path(args.runtime)
    if not runtime.is_absolute() or runtime != runtime.resolve():
        raise ValueError('canonical-alpha-runtime-required')
    manifest = json.loads((runtime / 'package.json').read_text())
    if manifest.get('name') != '@deepseek-ai/dsh' or manifest.get('version') != '0.2.1-alpha.1':
        raise ValueError('same-version-full-alpha-profile-required')
    requested = Path(args.output).absolute()
    if not requested.name.startswith('PUBLIC-native-seat-'):
        raise ValueError('new-PUBLIC-native-seat-output-required')
    os.umask(0o077)
    output = OWN.NATIVE.prepare(requested).resolve()
    scratch, seat_runtime, runtime_manifest = prepare_short_runtime(output)
    (output / 'bin').mkdir(mode=0o700)
    app = copy_executable(output / 'bin/native-input-fixture', fixture)
    keyboard = copy_executable(output / 'bin/native-seat-keyboard', sender)
    env = {'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8', 'HOME': str(output / 'user'),
           'XDG_RUNTIME_DIR': str(seat_runtime), 'XDG_CONFIG_HOME': str(output / 'config'),
           'XDG_CACHE_HOME': str(output / 'cache'), 'XDG_DATA_HOME': str(output / 'data'),
           'XDG_STATE_HOME': str(output / 'state'), 'WINIT_UNIX_BACKEND': 'wayland', 'ICED_BACKEND': 'wgpu'}
    command = ['/usr/bin/gamescope', '--backend', 'headless', '--expose-wayland',
               '-W', '1024', '-H', '768', '-w', '1024', '-h', '768', '--', str(app),
               '--input-evidence-root', str(output), '--runtime', str(runtime),
               '--expected-version', '0.2.1-alpha.1', '--home', str(output / 'harness'),
               '--user-home', str(output / 'user'), '--cwd', str(output / 'workspace')]
    resources, summary = [], {'status': 'failed', 'expectedText': TEXT, 'applicationInputQualified': False,
                             'physicalImeQualified': False, 'desktopClipboardQualified': False,
                             'sourceClosureComplete': False, 'fixtureSha256': args.fixture_sha256,
                             'senderSha256': args.sender_sha256, 'scope': __doc__,
                             'runtimeManifest': runtime_manifest, 'scratchRetained': True}
    interrupted = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, interrupted.set)
    deadline, sender_process, ready = time.monotonic() + args.seconds, None, None
    try:
        recorded = ART.private_json(output / 'runtime-manifest.json')
        if recorded != runtime_manifest or not valid_runtime_manifest(recorded, output):
            raise ValueError('runtime-manifest-changed-before-launch')
        compositor = await spawn(command, env, output, 'gamescope', resources)
        while time.monotonic() < deadline and not interrupted.is_set():
            for resource in resources:
                observe(resource)
            if compositor['child'].returncode is not None:
                break
            if sender_process is None and (output / 'input-ready.json').exists():
                ready = ART.private_json(output / 'input-ready.json')
                if not valid_ready(ready):
                    raise ValueError('native-ready-schema-refused')
                current = OWN.METRICS.proc_stat(ready['pid'])
                if current is None or OWN.identity(current) not in compositor['pidfds']:
                    raise ValueError('native-pid-not-owned-pinned-descendant')
                live, failed = OWN.live_pidfds(compositor['pidfds'])
                if failed or OWN.identity(current) not in live:
                    raise ValueError('native-pid-not-live')
                actual, snapshot = os.stat(f'/proc/{ready["pid"]}/exe'), app.stat()
                if (actual.st_dev, actual.st_ino) != (snapshot.st_dev, snapshot.st_ino):
                    raise ValueError('native-executable-not-pinned-snapshot')
                recorded = ART.private_json(output / 'runtime-manifest.json')
                if recorded != runtime_manifest or not valid_runtime_manifest(recorded, output):
                    raise ValueError('runtime-manifest-changed-before-input')
                socket = private_socket_path(seat_runtime, ready['waylandDisplay'])
                if not socket.exists():
                    await asyncio.sleep(.02)
                    continue
                root = compositor['root']
                root_fd = compositor['pidfds'][OWN.identity(root)]
                argv = [str(keyboard), str(scratch), str(socket), str(root['pid']), str(root['start_ticks']), str(root_fd)]
                argv += [str(ready[key]) for key in ('pointerFocusX', 'pointerFocusY', 'windowWidth', 'windowHeight')]
                sender_process = await spawn(argv, env, output, 'sender', resources, (root_fd,))
            if sender_process is not None and sender_process['child'].returncode is not None:
                if sender_process['child'].returncode != 0:
                    raise ValueError('ei-sender-failed-not-app-success')
            await asyncio.sleep(.02)
        summary.update(timedOut=time.monotonic() >= deadline, interrupted=interrupted.is_set(), ready=ready)
        if (sender_process is not None and sender_process['child'].returncode == 0
                and compositor['child'].returncode == 0 and not summary['timedOut'] and not summary['interrupted']):
            result = ART.private_json(output / 'input-result.json')
            summary['app'] = result
            summary['applicationInputQualified'] = valid_result(result, ready['pid'])
            if not summary['applicationInputQualified']:
                summary['failureCode'] = 'actual-native-body-or-keyboard-observation-mismatch'
    except (OSError, ValueError, RuntimeError, KeyError, TypeError) as error:
        summary.update(failureCode=str(error) if isinstance(error, ValueError) else 'owned-runner-error', errorType=type(error).__name__)
    finally:
        cleanups = []
        for resource in reversed(resources):
            cleanup = await OWN.cleanup_owned(resource['child'], resource['pidfds'], resource['pinned'])
            cleanups.append({'name': resource['name'], 'pid': resource['child'].pid, 'exitCode': resource['child'].returncode,
                             'identityPinFailures': resource['pinFailures'], 'cleanup': cleanup,
                             'observedIdentities': [list(key) for key in sorted(resource['observed'])]})
            for fd in resource['pidfds'].values():
                os.close(fd)
        summary['processes'] = cleanups
        if (summary['applicationInputQualified'] and cleanups
                and all(item['exitCode'] == 0 and item['identityPinFailures'] == 0
                        and item['cleanup']['observedCleanupComplete']
                        and not item['cleanup']['rootWasRunningAtCleanup']
                        and not item['cleanup']['remainingBeforeExternalCleanup'] for item in cleanups)):
            summary['status'] = 'passed'
        OWN.private_json(output / 'seat-result.json', summary)
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.remove_signal_handler(sig)
    print(json.dumps({'status': summary['status'], 'report': str(output / 'seat-result.json')}))
    return 0 if summary['status'] == 'passed' else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture', required=True, help='explicit dedicated native-input release snapshot, never main app')
    parser.add_argument('--fixture-sha256', required=True)
    parser.add_argument('--sender', required=True, help='explicit compiled native-seat-keyboard snapshot')
    parser.add_argument('--sender-sha256', required=True)
    parser.add_argument('--runtime', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--seconds', type=int, choices=range(10, 91), default=45)
    parser.add_argument('--launch-reviewed', action='store_true', help='only after review/authorization; omission prints plan without launching or writing')
    args = parser.parse_args()
    if not args.launch_reviewed:
        print(json.dumps({'status': 'planned-not-launched', 'requires': 'reviewed dedicated fixture and explicit sha256 snapshots',
                          'expectedText': TEXT, 'noEnter': True, 'physicalImeQualified': False}))
        return 0
    return asyncio.run(run(args))


if __name__ == '__main__':
    raise SystemExit(main())
