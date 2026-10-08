#!/usr/bin/env python3
"""Qualify explicit native --node using a private interpreter copy and fresh keyless Host."""
import argparse
import asyncio
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
import sys
import time

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('explicit_node_package_owner', BASE / 'scripts/qualify-development-package.py')
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)
REG = PACKAGE.REG


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def node_image_matches(key, node):
    """Point observation of a caller's already-pinned identity, path and executable inode."""
    before = REG.METRICS.proc_stat(key[0])
    if before is None or REG.identity(before) != key:
        return False
    try:
        image = Path(f'/proc/{key[0]}/exe')
        if os.readlink(image) != str(node):
            return False
        actual, expected = image.stat(), node.stat()
        if not stat.S_ISREG(expected.st_mode) or (actual.st_dev, actual.st_ino) != (expected.st_dev, expected.st_ino):
            return False
    except OSError:
        return False
    after = REG.METRICS.proc_stat(key[0])
    return after is not None and REG.identity(after) == key


async def run(options):
    binary = Path(options.binary).resolve(strict=True)
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError('Native executable unavailable')
    before_binary = sha(binary)
    output = REG.NATIVE.prepare(options.output)
    environment = REG.NATIVE.environment(output)
    environment['PATH'] = '/usr/bin:/bin'
    node = output / 'node private copy · 世界'
    system_node = Path('/usr/bin/node')
    with system_node.open('rb') as source, open(node, 'xb', opener=lambda p, f: os.open(p, f, 0o700)) as target:
        shutil.copyfileobj(source, target)
    node_sha = sha(node)
    if node_sha != sha(system_node):
        raise ValueError('Private Node copy mismatch')
    observation = subprocess.run([str(node), '--version'], env=environment, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, timeout=5, check=False)
    version = PACKAGE.L.parse_node_version(observation)
    runtime = (BASE.parent / 'deepseek-harness-linux/apps/cli').resolve(strict=True)
    command = [str(binary), '--runtime', str(runtime), '--expected-version', '0.2.1-alpha.1',
               '--home', str(output / 'harness'), '--cwd', str(output / 'workspace'),
               '--user-home', str(output / 'user'), '--node', str(node), '--scale', str(options.scale),
               '--smoke-new-session', '--exit-after-seconds', '8', '--smoke-report', str(output / 'app.json'),
               '--smoke-screenshot', str(output / 'own-window.png')]
    interrupted = asyncio.Event()
    loop = asyncio.get_running_loop()
    handlers = []
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, interrupted.set)
            handlers.append(sig)
        child = subprocess.Popen(command, env=environment, stdout=subprocess.DEVNULL,
                                 stderr=subprocess.DEVNULL, start_new_session=True)
    except BaseException:
        for sig in handlers:
            loop.remove_signal_handler(sig)
        raise
    pidfds, observed, node_identities = {}, set(), set()
    root_pinned = forced = False
    facts = failure = None
    observation_failures = 0
    started = time.monotonic()
    try:
        try:
            root, fd = REG.pin_root(child)
        except (OSError, RuntimeError):
            failure, forced = 'root-pin-failed', True
        else:
            root_pinned = True
            pidfds[REG.identity(root)] = fd
            observed.add(REG.identity(root))
            deadline = started + 45
            while child.poll() is None:
                observation_failures += REG.observe_descendants(child, root, pidfds, observed)
                for key in pidfds:
                    if key != REG.identity(root) and node_image_matches(key, node):
                        node_identities.add(key)
                remaining = deadline - time.monotonic()
                if interrupted.is_set() or remaining <= 0:
                    forced = True
                    break
                if facts is None:
                    candidate = await REG.native_window(child.pid, environment, min(REG.WINDOW_PROBE_SECONDS, remaining))
                    if candidate and candidate.get('mapped'):
                        facts = candidate
                await asyncio.sleep(min(.2, max(0, deadline - time.monotonic())))
    except (OSError, RuntimeError, ValueError):
        failure, forced = 'owned-observation-failed', True
    finally:
        task = asyncio.create_task(REG.cleanup_owned(child, pidfds, root_pinned))
        try:
            cleanup = await asyncio.shield(task)
        except asyncio.CancelledError:
            cleanup = await task
            failure, forced = 'qualification-cancelled', True
        finally:
            for fd in pidfds.values():
                os.close(fd)
            for sig in handlers:
                loop.remove_signal_handler(sig)
    forced = forced or cleanup['rootWasRunningAtCleanup']
    app = REG.read_app(output)
    valid = PACKAGE.valid_app(app, child.pid, options.scale)
    frame = PACKAGE.capture_valid(output, app)
    unchanged = sha(binary) == before_binary and sha(node) == node_sha
    passed = bool(valid and frame and unchanged and node_identities and child.returncode == 0
                  and not forced and failure is None and observation_failures == 0
                  and cleanup['observedCleanupComplete'] and not cleanup['remainingBeforeExternalCleanup']
                  and facts and facts.get('pid') == child.pid and facts.get('class') == PACKAGE.APP_ID
                  and facts.get('xwayland') is False)
    report = dict(status='passed' if passed else 'failed', scope=__doc__, nativeSha256=before_binary,
                  privateNodeSha256=node_sha, privateNodeVersionObserved=version,
                  privateNodeCopyMatchesSystemBytes=True, explicitNodeArgContainedSpacesAndUnicode=True,
                  ownedNodeExecutablePathAndInodeMatched=bool(node_identities),
                  observedMatchingNodeIdentityCount=len(node_identities), nodeExecutablePathRedacted=True,
                  binariesUnchanged=unchanged, exitCode=child.returncode, forced=forced, failureCode=failure,
                  ownedWindow=facts, app=app, appEvidenceVerified=valid,
                  completeOwnRendererCaptureVerified=frame, ownershipObservationFailures=observation_failures,
                  observedOwnedProcessCount=len(observed), cleanup=cleanup,
                  elapsedWallSeconds=time.monotonic() - started, paintInspected=False,
                  containmentScope='observed pidfd-pinned identities; executable path/inode checks are point observations, not exec-time binding',
                  nodeVersionPolicyAddedToProduct=False, nodeRuntimeDependenciesBundled=False,
                  archiveRebuilt=False, realCredentialsUsed=False, actualModelCalls=0,
                  physicalNativeInputQualified=False, clipboardTasksExecuted=False,
                  installedRuntimeDataOrDesktopSettingsChanged=False, fullDesktopParity=False)
    REG.private_json(output / 'result.json', report)
    print(json.dumps({'status': report['status'], 'report': str(output / 'result.json')}))
    return 0 if passed else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--scale', type=float, choices=[1.0, 1.25], default=1.0)
    try:
        sys.exit(asyncio.run(run(parser.parse_args())))
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError, PACKAGE.L.Refused):
        print('Explicit-Node qualification refused or failed.', file=sys.stderr)
        sys.exit(1)
