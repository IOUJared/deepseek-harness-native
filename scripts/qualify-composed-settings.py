#!/usr/bin/env python3
"""Real native UI/worker/Core saving of a fixed PUBLIC dummy in a new owned home only."""
import argparse
import asyncio
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import time

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('native_qualification', BASE / 'scripts/qualify-native-app.py')
NATIVE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(NATIVE)
METRICS = NATIVE.METRICS
QUALIFICATION_SECONDS = 40
TERM_SECONDS = 2
KILL_SECONDS = 2
POLL_SECONDS = .02


def private_json(path, data):
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'w') as stream:
        json.dump(data, stream, indent=2)
        stream.write('\n')


def identity(stat):
    # Exact schema from benchmarks/run.py; never infer a starttime field or PID group.
    return stat['pid'], stat['start_ticks']


def root_matches(child, root):
    current = METRICS.proc_stat(child.pid)
    return (child.returncode is None and current is not None
            and identity(current) == identity(root)
            and current['ppid'] == os.getpid())


def pin_root(child):
    """Pin before any poll/reap: Popen's sole owner retains the unreaped direct child."""
    root = METRICS.proc_stat(child.pid)
    if (child.returncode is not None or root is None or root['pid'] != child.pid
            or root['ppid'] != os.getpid()):
        raise RuntimeError('owned-root-identity-unavailable')
    fd = os.pidfd_open(child.pid)
    if not root_matches(child, root):
        os.close(fd)
        raise RuntimeError('owned-root-identity-changed')
    return root, fd


def parent_chain_matches(stat, snapshot, root):
    """Fence every captured parent against PID reuse, then recheck the pinned root."""
    visited = set()
    while stat['pid'] != root['pid']:
        if stat['pid'] in visited:
            return False
        visited.add(stat['pid'])
        current = METRICS.proc_stat(stat['pid'])
        if (current is None or identity(current) != identity(stat)
                or current['ppid'] != stat['ppid']):
            return False
        stat = snapshot.get(stat['ppid'])
        if stat is None:
            return False
    current = METRICS.proc_stat(root['pid'])
    return (identity(stat) == identity(root) and current is not None
            and identity(current) == identity(root) and current['ppid'] == os.getpid())


def observe_descendants(child, root, pidfds, observed):
    snapshot = {stat['pid']: stat for stat in METRICS.process_tree(child.pid, root['start_ticks'])}
    failures = 0
    for stat in snapshot.values():
        key = identity(stat)
        if key in pidfds:
            continue
        if not root_matches(child, root) or not parent_chain_matches(stat, snapshot, root):
            failures += 1
            continue
        fd = None
        try:
            fd = os.pidfd_open(stat['pid'])
            if not root_matches(child, root) or not parent_chain_matches(stat, snapshot, root):
                failures += 1
                continue
            pidfds[key] = fd
            observed.add(key)
            fd = None
        except OSError:
            failures += 1
        finally:
            if fd is not None:
                os.close(fd)
    return failures


def live_pidfds(pidfds):
    live = []
    failed = False
    for key, fd in pidfds.items():
        try:
            ready = select.select([fd], [], [], 0)[0]
        except (OSError, ValueError):
            # Unknown is not exited. Signal authority still comes only from the owned pidfd.
            ready = []
            failed = True
        if not ready:
            live.append(key)
    return live, failed


def signal_pinned(pidfds, keys, sig):
    failed = False
    for key in keys:
        try:
            signal.pidfd_send_signal(pidfds[key], sig)
        except ProcessLookupError:
            pass  # Exited between the liveness probe and the pidfd signal.
        except OSError:
            failed = True
    return failed


def poll_child(child):
    try:
        child.poll()  # WNOHANG; this runner is the only owner allowed to reap its Popen.
        return False
    except OSError:
        return True


async def wait_quiescent(child, pidfds, seconds):
    deadline = time.monotonic() + seconds
    failed = False
    while True:
        failed |= poll_child(child)
        live, probe_failed = live_pidfds(pidfds)
        failed |= probe_failed
        if child.returncode is not None and not live:
            return live, failed
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return live, failed
        await asyncio.sleep(min(POLL_SECONDS, remaining))


async def cleanup_owned(child, pidfds, root_pinned, *, term_seconds=TERM_SECONDS,
                        kill_seconds=KILL_SECONDS):
    """Bounded TERM/KILL and reap for every pinned identity, even after GUI root exit.

    Pin acquisition failure alone uses the directly owned, unreaped Popen resource;
    no asynchronous child watcher or other thread may poll/reap that resource.
    No numeric descendant, process-group, or post-reap PID signalling is used.
    """
    failed = poll_child(child)
    before, probe_failed = live_pidfds(pidfds)
    failed |= probe_failed
    root_was_running = child.returncode is None
    fallback = not root_pinned and root_was_running
    failed |= signal_pinned(pidfds, before, signal.SIGTERM)
    if fallback:
        try:
            child.terminate()  # Popen checks its unreaped direct-child resource before signalling.
        except ProcessLookupError:
            pass
        except OSError:
            failed = True
    after_term, wait_failed = await wait_quiescent(child, pidfds, term_seconds)
    failed |= wait_failed
    escalated = bool(after_term) or child.returncode is None
    failed |= signal_pinned(pidfds, after_term, signal.SIGKILL)
    if fallback and child.returncode is None:
        try:
            child.kill()
        except ProcessLookupError:
            pass
        except OSError:
            failed = True
    after, wait_failed = await wait_quiescent(child, pidfds, kill_seconds)
    failed |= wait_failed
    return {
        'rootPidfdPinned': root_pinned,
        'rootWasRunningAtCleanup': root_was_running,
        'ownedChildFallbackUsed': fallback,
        'remainingBeforeExternalCleanup': before,
        'remainingAfterExternalCleanup': after,
        'killEscalated': escalated,
        'rootReaped': child.returncode is not None,
        'cleanupTimedOut': child.returncode is None or bool(after),
        'cleanupProbeOrSignalFailed': failed,
        # Root acquisition failure cannot qualify the unobserved tree as contained.
        'observedCleanupComplete': root_pinned and child.returncode is not None and not after and not failed,
        'termBudgetSeconds': term_seconds,
        'killBudgetSeconds': kill_seconds,
    }


async def run(options):
    output = NATIVE.prepare(options.output)
    binary = (BASE / 'app/target/release/examples/settings_composed_smoke').resolve()
    if not binary.is_file():
        raise ValueError('release composed fixture not ready')
    before_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
    interrupted = asyncio.Event()
    loop = asyncio.get_running_loop()
    handlers = []
    # Install before spawning so an interrupt during acquisition also requests owned cleanup.
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, interrupted.set)
            handlers.append(sig)
        # No asyncio child watcher: retain unreaped-root authority until pinning,
        # and on pin failure use only this directly owned Popen's bounded poll/TERM/KILL path.
        child = subprocess.Popen([str(binary), '--output', str(output), '--scale', str(options.scale)],
            env=NATIVE.environment(output), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            start_new_session=True)
    except BaseException:
        for sig in handlers:
            loop.remove_signal_handler(sig)
        raise
    pidfds = {}
    facts = None
    forced = False
    observed = set()
    observation_failures = 0
    root_pinned = False
    failure = None
    cleanup = None
    try:
        try:
            root, fd = pin_root(child)
        except (OSError, RuntimeError):
            failure = 'root-pidfd-acquisition-failed'
            forced = True
        else:
            root_pinned = True
            pidfds[identity(root)] = fd
            observed.add(identity(root))
            deadline = time.monotonic() + QUALIFICATION_SECONDS
            while child.poll() is None:
                observation_failures += observe_descendants(child, root, pidfds, observed)
                if facts is None:
                    candidate = await METRICS.native_window(child.pid)
                    if candidate and candidate.get('mapped'):
                        facts = candidate
                if interrupted.is_set() or time.monotonic() >= deadline:
                    forced = True
                    break
                await asyncio.sleep(min(.1, max(0, deadline - time.monotonic())))
    except (OSError, RuntimeError, ValueError):
        failure = 'qualification-observation-failed'
        forced = True
    finally:
        # Shield cleanup against task cancellation, but every wait inside it has a finite budget.
        task = asyncio.create_task(cleanup_owned(child, pidfds, root_pinned))
        try:
            cleanup = await asyncio.shield(task)
        except asyncio.CancelledError:
            cleanup = await task
            failure = 'qualification-cancelled'
            forced = True
        finally:
            for fd in pidfds.values():
                os.close(fd)
            for sig in handlers:
                loop.remove_signal_handler(sig)
    if cleanup['rootWasRunningAtCleanup']:
        forced = True
    report_path = output / 'app.json'
    try:
        app = json.loads(report_path.read_text()) if report_path.is_file() else {'status': 'missing-app-report'}
        if not isinstance(app, dict):
            app = {'status': 'invalid-app-report'}
    except (OSError, ValueError):
        app = {'status': 'invalid-app-report'}
    passed = (child.returncode == 0 and not forced and failure is None
              and cleanup['observedCleanupComplete'] and not cleanup['remainingBeforeExternalCleanup']
              and observation_failures == 0 and app.get('status') == 'passed' and facts
              and facts.get('class') == 'ai.deepseek.harness.native.settings-composed-fixture'
              and facts.get('xwayland') is False and hashlib.sha256(binary.read_bytes()).hexdigest() == before_hash)
    data = {'status': 'passed' if passed else 'failed', 'scope': __doc__, 'binarySha256': before_hash,
            'exitCode': child.returncode, 'forced': forced, 'interrupted': interrupted.is_set(),
            'failureCode': failure, 'ownedWindow': facts, 'observedOwnedProcessCount': len(observed),
            'ownershipObservationFailures': observation_failures,
            'qualificationBudgetSeconds': QUALIFICATION_SECONDS, 'cleanup': cleanup,
            'remainingBeforeExternalCleanup': cleanup['remainingBeforeExternalCleanup'],
            'remainingAfterExternalCleanup': cleanup['remainingAfterExternalCleanup'],
            'app': app, 'paintInspected': False, 'nativeKeyboardPointerInputQualified': False,
            'liveHostDecisionQualified': False, 'desktopSettingsChanged': False,
            'installedRuntimeChanged': False, 'realCredentialsUsed': False}
    private_json(output / 'result.json', data)
    print(json.dumps({'status': data['status'], 'report': str(output / 'result.json')}))
    return 0 if passed else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    parser.add_argument('--scale', type=float, choices=[1.0, 1.25], default=1.0)
    raise SystemExit(asyncio.run(run(parser.parse_args())))
