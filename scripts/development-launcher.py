#!/usr/bin/python3
"""Start a private development build; never install, migrate, or adopt another profile."""
import argparse
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import stat
import subprocess
import sys

VERSION = '0.2.1-alpha.1'
APP = 'ai.deepseek.harness.native.development'
MAX_BINARY = 64 * 1024 * 1024


class Refused(Exception):
    pass


def private_read(path, limit):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_size > limit:
            raise Refused('Unexpected package file')
        with os.fdopen(fd, 'rb', closefd=False) as stream:
            data = stream.read(limit + 1)
        if len(data) > limit or len(data) != info.st_size:
            raise Refused('Package file changed or exceeded its bound')
        return data
    finally:
        os.close(fd)


def binary_digest(path):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        if not stat.S_ISREG(before.st_mode) or not 0 < before.st_size <= MAX_BINARY:
            raise Refused('Unexpected native binary')
        digest = hashlib.sha256()
        count = 0
        while chunk := os.read(fd, 1024 * 1024):
            count += len(chunk)
            if count > MAX_BINARY:
                raise Refused('Native binary exceeded its bound')
            digest.update(chunk)
        after = os.fstat(fd)
        if (count != before.st_size or (before.st_dev, before.st_ino, before.st_size,
                before.st_mtime_ns, before.st_ctime_ns) != (after.st_dev, after.st_ino,
                after.st_size, after.st_mtime_ns, after.st_ctime_ns)):
            raise Refused('Native binary changed during validation')
        return digest.hexdigest()
    finally:
        os.close(fd)


def absolute(value):
    if (not isinstance(value, str) or len(value.encode('utf-8')) > 4096
            or any(ord(c) < 32 or ord(c) == 127 for c in value)):
        raise Refused('Invalid absolute directory')
    path = Path(value)
    if not path.is_absolute() or '..' in path.parts:
        raise Refused('Explicit absolute directory required')
    return path


def overlaps(a, b):
    return a == b or a in b.parents or b in a.parents


def package(root):
    value = json.loads(private_read(root / 'development-manifest.json', 65536))
    if (not isinstance(value, dict) or value.get('application') != APP or value.get('runtimeVersion') != VERSION
            or sys.platform != 'linux' or value.get('architecture') != 'x86_64' or platform.machine() != 'x86_64'
            or value.get('runtimeMode') != 'external-explicit'):
        raise Refused('Unsupported development package')
    minimum = value.get('minimumGlibc')
    actual = os.confstr('CS_GNU_LIBC_VERSION')
    if (not isinstance(minimum, str) or not actual or not actual.startswith('glibc ')
            or tuple(map(int, actual.split()[1].split('.'))) < tuple(map(int, minimum.split('.')))):
        raise Refused('System glibc is older than this development build')
    digest = value.get('nativeSha256')
    if (not isinstance(digest, str) or len(digest) != 64
            or any(c not in '0123456789abcdef' for c in digest)):
        raise Refused('Invalid package identity')
    binary = root / 'bin/dsh-native-app'
    if binary_digest(binary) != digest:
        raise Refused('Native binary does not match package manifest')
    if not os.access(binary, os.X_OK):
        raise Refused('Native binary is not executable')
    return value, binary


def environment(user, inherited, display=False):
    result = {'HOME': str(user), 'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8'}
    keys = ('LANG', 'LC_ALL', 'LC_CTYPE', 'XDG_RUNTIME_DIR', 'WAYLAND_DISPLAY',
            'DBUS_SESSION_BUS_ADDRESS', 'XDG_SESSION_TYPE', 'XDG_CURRENT_DESKTOP',
            'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME')
    for key in keys:
        value = inherited.get(key)
        if value is not None:
            if len(value.encode('utf-8')) > 4096 or any(ord(c) < 32 or ord(c) == 127 for c in value):
                raise Refused('Invalid display or locale environment')
            result[key] = value
    # Preserve only this isolation exclusion; never use it to choose native storage.
    if 'DSH_HOME' in inherited:
        result['DSH_HOME'] = str(absolute(inherited['DSH_HOME']))
    if display and (not result.get('WAYLAND_DISPLAY') or not result.get('XDG_RUNTIME_DIR')):
        raise Refused('This development package requires the current Wayland session')
    return result


def marker_for(manifest):
    return {'application': APP, 'layout': 1, 'runtimeVersion': VERSION,
            'nativeSha256': manifest['nativeSha256']}


def inspect_state(data, marker):
    try:
        info = data.lstat()
    except FileNotFoundError:
        if not data.parent.is_dir():
            raise Refused('Native data parent must already exist')
        return 'new'
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o700):
        raise Refused('Existing native data directory is not private and owned')
    path = data / '.native-development.json'
    info = path.lstat()
    if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o600
            or json.loads(private_read(path, 1024)) != marker):
        raise Refused('Refusing to adopt or migrate an unknown native data directory')
    host = data / 'host'
    try:
        host_info = host.lstat()
    except FileNotFoundError:
        host_info = None
    if host_info is not None and (not stat.S_ISDIR(host_info.st_mode)
                                 or host_info.st_uid != os.getuid()
                                 or stat.S_IMODE(host_info.st_mode) != 0o700):
        raise Refused('Native Host directory is not private and owned')
    return 'owned'


def acquire_state(data, marker):
    status = inspect_state(data, marker)
    if status == 'new':
        data.mkdir(mode=0o700)
    directory = os.open(data, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    lock = None
    try:
        info = os.fstat(directory)
        if info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
            raise Refused('Native data directory ownership changed')
        if status == 'new':
            fd = os.open('.native-development.json', os.O_WRONLY | os.O_CREAT | os.O_EXCL
                         | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600, dir_fd=directory)
            try:
                os.fchmod(fd, 0o600)
                body = (json.dumps(marker, sort_keys=True) + '\n').encode()
                with os.fdopen(fd, 'wb', closefd=False) as stream:
                    if stream.write(body) != len(body):
                        raise Refused('Native marker write incomplete')
                    stream.flush()
                os.fsync(fd)
            finally:
                os.close(fd)
            os.fsync(directory)
        # Read the held directory, not a replaced pathname, before taking its lock.
        fd = os.open('.native-development.json', os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC,
                     dir_fd=directory)
        try:
            info = os.fstat(fd)
            if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid()
                    or stat.S_IMODE(info.st_mode) != 0o600 or info.st_size > 1024):
                raise Refused('Native marker ownership changed')
            body = os.read(fd, 1025)
            if len(body) > 1024 or json.loads(body) != marker:
                raise Refused('Native marker changed')
        finally:
            os.close(fd)
        lock = os.open('.native-development.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW
                       | os.O_CLOEXEC, 0o600, dir_fd=directory)
        info = os.fstat(lock)
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid()
                or stat.S_IMODE(info.st_mode) != 0o600):
            raise Refused('Native lock is not private and owned')
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise Refused('This native data directory is already in use') from None
        try:
            os.mkdir('host', 0o700, dir_fd=directory)
        except FileExistsError:
            pass
        host = os.open('host', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                       dir_fd=directory)
        try:
            info = os.fstat(host)
            if info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
                raise Refused('Native Host directory ownership changed')
        finally:
            os.close(host)
        current = data.lstat()
        pinned = os.fstat(directory)
        if (not stat.S_ISDIR(current.st_mode)
                or (current.st_dev, current.st_ino) != (pinned.st_dev, pinned.st_ino)):
            raise Refused('Native data pathname changed during acquisition')
        inspect_state(data, marker)
        os.fsync(directory)
        os.set_inheritable(lock, True)
        return lock
    except BaseException:
        if lock is not None:
            os.close(lock)
        raise
    finally:
        os.close(directory)


def parse_node_version(result):
    """Accept a successful bounded standard Node-26 version line; never report stderr."""
    if (type(result.returncode) is not int or result.returncode != 0
            or not isinstance(result.stdout, bytes) or len(result.stdout) > 32
            or not isinstance(result.stderr, bytes) or len(result.stderr) > 1024
            or re.fullmatch(rb'v26\.[0-9]{1,4}\.[0-9]{1,4}\n?', result.stdout) is None):
        raise Refused('This development build requires Node version 26')
    return result.stdout.removesuffix(b'\n').decode('ascii')


def selected_node(value=None):
    """Default is fixed; explicit paths resolve to executable regular files, not PATH."""
    if value is None:
        return Path('/usr/bin/node')
    node = absolute(value).resolve(strict=True)
    # A clean alias may resolve to controls/non-UTF8 the native argv parser rejects.
    absolute(str(node))
    info = node.stat()
    if not stat.S_ISREG(info.st_mode) or not os.access(node, os.X_OK):
        raise Refused('Selected Node is not an executable regular file')
    return node


def probe_node(env, cwd=None, *, node=None):
    return parse_node_version(subprocess.run([str(node) if node is not None else '/usr/bin/node', '--version'], env=env,
                                            cwd=cwd, capture_output=True, timeout=5, check=False))


def plan(args, root, inherited, *, observations=None):
    manifest, binary = package(root)
    user = absolute(args.user_home or inherited.get('HOME', '')).resolve(strict=True)
    workspace = absolute(args.workspace).resolve(strict=True)
    if not user.is_dir() or not workspace.is_dir():
        raise Refused('User home and workspace must be existing directories')
    if args.runtime is None:
        raise Refused('This package requires an explicit isolated alpha --runtime')
    runtime = absolute(args.runtime).resolve(strict=True)
    runtime_manifest = json.loads(private_read(runtime / 'package.json', 65536))
    if (not isinstance(runtime_manifest, dict) or runtime_manifest.get('name') != '@deepseek-ai/dsh'
            or runtime_manifest.get('version') != VERSION):
        raise Refused('Only the explicit isolated alpha CLI runtime is supported')
    if not (runtime / 'lib/bin.js').is_file():
        raise Refused('Runtime CLI is not built')
    data_raw = absolute(args.data_home) if args.data_home else user / '.local/share' / ('dsh-native-dev-' + manifest['nativeSha256'][:12])
    if data_raw.is_symlink():
        raise Refused('Native data directory must not be a symlink')
    data = data_raw.resolve(strict=False)
    forbidden = [user, user / '.dsh', root, runtime]
    if 'DSH_HOME' in inherited:
        forbidden.append(absolute(inherited['DSH_HOME']).resolve(strict=False))
    if (data == Path('/') or data == user or user in data.parents and data == user / '.local/share'
            or user in data.parents and data == user / '.local/share/dsh'
            or any(overlaps(data, path) for path in forbidden[1:])
            or data in user.parents):
        raise Refused('Native data overlaps protected installation or storage')
    if not math.isfinite(args.scale) or not 0.75 <= args.scale <= 2.0:
        raise Refused('Scale must be between 0.75 and 2.0')
    marker = marker_for(manifest)
    status = inspect_state(data, marker)
    env = environment(user, inherited, display=not args.check)
    node_value = getattr(args, 'node', None)
    node = selected_node(node_value)
    probe_options = {} if node_value is None else {'node': node}
    node_version = probe_node(environment(user, inherited), cwd=root, **probe_options)
    if observations is not None:
        observations.update(nodeVersionObserved=node_version, nodeMajorAccepted=26,
                            nodeSelectionMode='system-default' if node_value is None else 'explicit-absolute')
        if node_value is None:
            observations['systemNodeVersionObserved'] = node_version
    argv = [str(binary), '--runtime', str(runtime), '--expected-version', VERSION,
            '--home', str(data / 'host'), '--user-home', str(user), '--cwd', str(workspace),
            '--scale', str(args.scale)]
    if node_value is not None:
        argv += ['--node', str(node)]
    if args.qualify_evidence is not None:
        if args.check or not args.data_home or not args.user_home or status != 'new':
            raise Refused('Qualification requires explicit fresh data and user homes, not --check')
        evidence = absolute(args.qualify_evidence).resolve(strict=True)
        info = evidence.stat()
        private_dirs = (evidence, workspace, user)
        if (any(not p.is_dir() or p.stat().st_uid != os.getuid()
                or stat.S_IMODE(p.stat().st_mode) != 0o700 for p in private_dirs)
                or overlaps(evidence, data) or any(evidence.iterdir())
                or any(workspace.iterdir()) or any(user.iterdir())):
            raise Refused('Qualification requires separate owned private empty evidence, workspace and user directories')
        argv += ['--smoke-new-session', '--exit-after-seconds', '8',
                 '--smoke-evidence-root', str(evidence), '--smoke-report', str(evidence / 'app.json'),
                 '--smoke-screenshot', str(evidence / 'own-window.png')]
    return argv, env, data, marker, status


def main(argv=None, root=None):
    parser = argparse.ArgumentParser(allow_abbrev=False, description='Private native development launcher; no installer or profile migration.')
    parser.add_argument('--workspace', required=True, help='Existing absolute workspace directory')
    parser.add_argument('--runtime', help='Explicit isolated built alpha apps/cli; required by external-runtime package')
    parser.add_argument('--user-home', help='Existing absolute user home; defaults only to HOME')
    parser.add_argument('--data-home', help='New or marked private container; parent must exist; never installed DSH data')
    parser.add_argument('--node', help='Explicit absolute Node-26 executable; omission uses /usr/bin/node, never PATH')
    parser.add_argument('--scale', type=float, default=1.0)
    parser.add_argument('--check', action='store_true', help='Validate prerequisites only; no data, GUI, or Host creation')
    parser.add_argument('--qualify-evidence', help='Opt-in PUBLIC blank-session eight-second own-window smoke; empty owned 0700 evidence/user/workspace and fresh explicit data required')
    tokens = list(sys.argv[1:] if argv is None else argv)
    names = [token.split('=', 1)[0] for token in tokens if token.startswith('--')]
    if len(names) != len(set(names)):
        parser.error('Duplicate launcher option')
    args = parser.parse_args(tokens)
    root = Path(__file__).resolve().parent if root is None else Path(root).resolve(strict=True)
    lock = None
    try:
        observations = {}
        native_argv, env, data, marker, status = plan(args, root, os.environ, observations=observations)
        if args.check:
            print(json.dumps({'status': 'prerequisites-valid', 'runtimeVersion': VERSION, 'nativeDataStatus': status,
                              'runtimeDependencyResolutionValidated': False, 'runtimeStartupValidated': False,
                              'guiStarted': False, 'hostStarted': False, 'nativeDataCreated': False,
                              'runtimeMode': package(root)[0]['runtimeMode'], **observations}))
            return 0
        lock = acquire_state(data, marker)
        os.execve(native_argv[0], native_argv, env)
    except (Refused, OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError):
        print('Native development launch refused: invalid package, runtime, private data, display or Node prerequisites. No installation or migration was attempted.', file=sys.stderr)
        return 2
    finally:
        if lock is not None:
            os.close(lock)


if __name__ == '__main__':
    raise SystemExit(main())
