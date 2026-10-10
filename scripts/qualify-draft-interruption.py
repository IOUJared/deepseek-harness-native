#!/usr/bin/env python3
"""Owned native GUI process SIGKILL and unfinished Store checkpoint campaign; no kernel crash, power loss or physical input claim."""
import argparse
import asyncio
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import time

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('interruption_restart', BASE / 'scripts/qualify-draft-restart.py')
RESTART = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RESTART)
OWNED = RESTART.OWNED
OLD = 'PUBLIC committed draft\nΚαλημέρα · 日本語 · 🦀'
NEW = OLD + '\nPUBLIC interrupted replacement · مرحبا'
LATEST = NEW + '\nPUBLIC newest text while actual save is unfinished · café'
POINTS = ['before-write', 'after-write', 'before-temp-sync', 'after-temp-sync', 'before-rename',
          'after-rename', 'before-dir-sync', 'after-dir-sync', 'before-ack']
STOP = RESTART.STOP
FORBIDDEN = ['step/start', 'step/end', 'request/header', 'tool/call', 'tool/result', 'assistant/message', 'user/message']


def private_json(path, limit=65536):
    return json.loads(RESTART.private_bytes(path, limit), object_pairs_hook=RESTART.unique_object)


def expected(point):
    return OLD if point in POINTS[:5] else NEW


def valid_stop(value):
    return (type(value) is dict and set(value) == set(STOP) and value == STOP
            and all(type(value[k]) is bool for k in ['exited', 'graceful', 'containmentUnknown'])
            and all(type(value[k]) is int for k in ['exitCode', 'observedDescendantsRemaining']))


def valid_cleanup(cleanup, mode):
    """Graceful phases cannot borrow the crash phase's external descendant cleanup."""
    if mode not in ('hold', 'recover', 'crash') or type(cleanup) is not dict:
        return False
    before, after = cleanup.get('remainingBeforeExternalCleanup'), cleanup.get('remainingAfterExternalCleanup')
    return (all(cleanup.get(key) is True for key in ['rootPidfdPinned', 'rootReaped', 'observedCleanupComplete'])
            and all(cleanup.get(key) is False for key in ['rootWasRunningAtCleanup', 'ownedChildFallbackUsed',
                                                        'cleanupTimedOut', 'cleanupProbeOrSignalFailed'])
            and type(cleanup.get('killEscalated')) is bool
            and type(before) is list and type(after) is list and not after
            and (mode == 'crash' or (not before and cleanup['killEscalated'] is False)))


def common(app, pid, point):
    if type(app) is not dict or point not in POINTS:
        return False
    kinds = app.get('recordKinds')
    return (type(app) is dict and app.get('publicOnly') is True and app.get('version') == '0.2.1-alpha.1'
            and type(pid) is int and type(app.get('pid')) is int and app['pid'] == pid and 0 < pid < (1 << 31)
            and RESTART.session_id(app.get('sessionId')) and app.get('checkpoint') == point
            and all(type(app.get(key)) is int and app[key] == 0 for key in ['promptSerial', 'actualAppPromptCommands', 'unexpectedWork', 'manualBusinessMessagesForwarded'])
            and type(app.get('catalogReads')) is int and app['catalogReads'] == 1 and app.get('catalogSucceeded') is True
            and app.get('windowBackend') == 'wayland' and type(kinds) is list and len(kinds) > 0
            and all(type(k) is str and 'llm' not in k and k not in FORBIDDEN for k in kinds)
            and all(app.get(key) is False for key in ['kernelBlockedFsyncQualified', 'powerLossQualified', 'physicalInputQualified', 'networkTraceQualified']))


def valid_finished(app, pid, mode, point):
    if mode not in ['hold', 'recover'] or not common(app, pid, point):
        return False
    if not (app.get('status') == 'passed' and app.get('mode') == mode and 'errorCode' not in app
            and valid_stop(app.get('stop'))
            and all(app.get(k) is True for k in ['draftIoJoined', 'draftsCleanAtExit', 'expectedSnapshotDurable'])):
        return False
    if mode == 'hold':
        return (app.get('saveUnfinished') is True and app.get('stoppedWhileSaveUnfinished') is True
                and app.get('exitAnywayRefused') is True and type(app.get('savedOutcomes')) is int and app['savedOutcomes'] == 3
                and type(app.get('guiHeartbeatsWhileBlocked')) is int and app['guiHeartbeatsWhileBlocked'] >= 3)
    return (app.get('recoveredNeedsReview') is True and type(app.get('savedOutcomes')) is int and app['savedOutcomes'] == 0)


def valid_observation(app, pid, point):
    return (common(app, pid, point) and app.get('status') == 'checkpoint-observed'
            and 'errorCode' not in app and app.get('mode') == 'crash' and app.get('saveUnfinished') is True
            and app.get('crashGateLatchedUntilOwnedKill') is True
            and type(app.get('savedOutcomes')) is int and app['savedOutcomes'] == 1
            and type(app.get('sessionId')) is str and 0 < len(app['sessionId'].encode()) <= 128)


async def phase(output, binary, runtime, mode, point, cancelled):
    args = [str(binary), '--campaign-mode', mode, '--checkpoint', point, '--runtime', str(runtime),
            '--expected-version', '0.2.1-alpha.1', '--home', str(output / 'harness'), '--user-home', str(output / 'user'),
            '--cwd', str(output / 'workspace'), '--scale', '1.0']
    env = OWNED.NATIVE.environment(output)
    env['PATH'] = '/usr/bin:/bin'
    fds = [os.open(output / f'{mode}-{suffix}.log', os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600) for suffix in ['stdout', 'stderr']]
    try:
        child = subprocess.Popen(args, env=env, cwd=output / 'workspace', stdin=subprocess.DEVNULL,
                                 stdout=fds[0], stderr=fds[1], start_new_session=True)
    finally:
        for fd in fds:
            os.close(fd)
    report = {'status': 'failed', 'mode': mode, 'checkpoint': point, 'pid': child.pid,
              'identityPinFailures': 0, 'timedOut': False, 'intendedSignal': None, 'signal': None}
    pidfds, observed, pinned, cleanup = {}, set(), False, None
    deadline = time.monotonic() + 70
    try:
        root, fd = OWNED.pin_root(child)
        pidfds[OWNED.identity(root)] = fd
        observed.add(OWNED.identity(root))
        pinned = True
        while child.returncode is None:
            report['identityPinFailures'] += OWNED.observe_descendants(child, root, pidfds, observed)
            if OWNED.poll_child(child):
                raise ValueError('owned-child-poll-failed')
            if cancelled.is_set() or time.monotonic() > deadline:
                report['timedOut'] = not cancelled.is_set()
                break
            marker = output / 'crash-observed.json'
            if mode == 'crash' and report['intendedSignal'] is None and os.path.lexists(marker):
                app = private_json(marker)
                if not valid_observation(app, child.pid, point):
                    raise ValueError('checkpoint-observation-invalid')
                report['observation'] = app
                report['intendedSignal'] = 9
                # Identity-pinned owned native PID only; never a name, user process or numeric group.
                signal.pidfd_send_signal(fd, signal.SIGKILL)
            if child.returncode is None:
                await asyncio.sleep(.02)
        cleanup = await OWNED.cleanup_owned(child, pidfds, pinned)
        report.update(exitCode=child.returncode, signal=-child.returncode if child.returncode is not None and child.returncode < 0 else None,
                      cleanup=cleanup, observedIdentities=[list(i) for i in sorted(observed)])
        safe = (not cancelled.is_set() and not report['timedOut'] and report['identityPinFailures'] == 0
                and valid_cleanup(cleanup, mode))
        if mode == 'crash':
            if safe and report['intendedSignal'] == 9 and report['signal'] == 9 and child.returncode == -9:
                report['status'] = 'passed'
                OWNED.private_json(output / 'termination.json', {'version': 1, 'pid': child.pid, 'signal': 9,
                                  'nativeReaped': cleanup['rootReaped'], 'ownedCleanupComplete': cleanup['observedCleanupComplete']})
        else:
            app = private_json(output / f'{mode}-app.json')
            report['app'] = app
            if safe and child.returncode == 0 and valid_finished(app, child.pid, mode, point):
                report['status'] = 'passed'
    except (OSError, ValueError, TypeError, KeyError, RuntimeError) as error:
        report['errorType'] = type(error).__name__
    finally:
        if cleanup is None:
            cleanup = await OWNED.cleanup_owned(child, pidfds, pinned)
            report.update(exitCode=child.returncode, cleanup=cleanup)
        for fd in pidfds.values():
            os.close(fd)
        OWNED.private_json(output / f'{mode}-owned.json', report)
    return report


async def run(options):
    os.umask(0o077)
    requested = Path(options.output).absolute()
    if not requested.name.startswith('PUBLIC-draft-interruption-'):
        raise ValueError('explicit PUBLIC interruption root required')
    output = OWNED.NATIVE.prepare(requested).resolve()
    source = Path(options.binary).resolve()
    binary = output / 'draft-interruption-snapshot'
    data = source.read_bytes()
    with binary.open('xb') as f:
        f.write(data)
    binary.chmod(0o700)
    digest = hashlib.sha256(data).hexdigest()
    paths = [Path(__file__), BASE / 'scripts/test_qualify_draft_interruption.py',
             BASE / 'app/examples/draft_interruption_smoke.rs', BASE / 'app/src/draft_store.rs']
    before = RESTART.sources() | {str(p): RESTART.digest(p) for p in paths}
    summary = {'status': 'failed', 'scope': __doc__, 'cases': [], 'artifactSha256': digest, 'sourceClosureComplete': False,
               'powerLossQualified': False, 'kernelBlockedFsyncQualified': False, 'physicalInputQualified': False, 'networkTraceQualified': False}
    cancelled = asyncio.Event()
    loop = asyncio.get_running_loop()
    for sig in [signal.SIGINT, signal.SIGTERM]:
        loop.add_signal_handler(sig, cancelled.set)
    runtime = BASE.parent / 'deepseek-harness-linux/apps/cli'
    try:
        hold = OWNED.NATIVE.prepare(output / 'PUBLIC-draft-interruption-hold').resolve()
        result = await phase(hold, binary, runtime, 'hold', 'before-temp-sync', cancelled)
        summary['cases'].append({'checkpoint': 'unfinished-save-close', 'phases': [result], 'status': result['status']})
        if result['status'] != 'passed':
            raise ValueError('unfinished-save-close-failed')
        for point in POINTS:
            if cancelled.is_set():
                raise ValueError('cancelled')
            case = OWNED.NATIVE.prepare(output / ('PUBLIC-draft-interruption-' + point)).resolve()
            first = await phase(case, binary, runtime, 'crash', point, cancelled)
            item = {'checkpoint': point, 'status': 'failed', 'phases': [first], 'expected': 'old' if point in POINTS[:5] else 'new'}
            summary['cases'].append(item)
            if first['status'] != 'passed':
                raise ValueError('owned-crash-not-qualified')
            doc = private_json(case / 'harness/native-drafts.v1.json', 2 * 1024 * 1024)
            subject = first['observation']['sessionId']
            if (type(doc) is not dict or set(doc) != {'version', 'drafts'} or type(doc['version']) is not int
                    or doc['version'] != 1 or doc['drafts'] != {subject: expected(point)}):
                raise ValueError('post-crash-document-mismatch')
            second = await phase(case, binary, runtime, 'recover', point, cancelled)
            item['phases'].append(second)
            if second['status'] != 'passed' or first['pid'] == second['pid'] or second['app']['sessionId'] != subject:
                raise ValueError('cold-crash-recovery-not-qualified')
            item['status'] = 'passed'
        summary['status'] = 'passed'
    except (OSError, ValueError, TypeError, KeyError, RuntimeError) as error:
        summary['errorType'] = type(error).__name__
    finally:
        after = RESTART.sources() | {str(p): RESTART.digest(p) for p in paths}
        summary.update(sourceSha256Before=before, sourceSha256After=after, sourceUnchanged=before == after,
                       artifactUnchanged=hashlib.sha256(binary.read_bytes()).hexdigest() == digest)
        if not summary['sourceUnchanged'] or not summary['artifactUnchanged']:
            summary['status'] = 'failed'
        for sig in [signal.SIGINT, signal.SIGTERM]:
            loop.remove_signal_handler(sig)
        OWNED.private_json(output / 'draft-interruption-qualification.json', summary)
    print(json.dumps({'status': summary['status'], 'report': str(output / 'draft-interruption-qualification.json')}))
    return 0 if summary['status'] == 'passed' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    parser.add_argument('--binary', default=str(BASE / 'app/target/debug/examples/draft_interruption_smoke'))
    raise SystemExit(asyncio.run(run(parser.parse_args())))
