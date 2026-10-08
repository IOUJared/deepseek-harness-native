#!/usr/bin/env python3
"""Owned actual native UI/worker/Gateway/registered-root decisions; no physical-input or outbound-trace proof."""
import argparse
import asyncio
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import time

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('decisions_owned_runner', BASE / 'scripts/qualify-composed-settings.py')
OWN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(OWN)
APP_ID = 'ai.deepseek.harness.native.decisions-composed-fixture'


def private_json(path):
    from stat import S_ISREG
    info = path.lstat()
    if not S_ISREG(info.st_mode) or info.st_mode & 0o7777 != 0o600 or info.st_uid != os.getuid() or info.st_size > 16384:
        raise ValueError('private-report-required')
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW), 'rb') as stream:
        opened = os.fstat(stream.fileno())
        if (opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino):
            raise ValueError('report-changed')
        data = stream.read(16385)
    if len(data) > 16384:
        raise ValueError('report-oversize')
    value = json.loads(data)
    if not isinstance(value, dict):
        raise ValueError('report-object-required')
    return value


def valid_reports(output, app, pid, scale, viewport='full'):
    if viewport not in ('full', 'minimum'):
        return False
    if app.get('logicalViewport') != ([608, 448] if viewport == 'minimum' else None):
        return False
    if any(type(app.get(key)) is not int for key in ('pid', 'nativeAcks', 'hostSettlements')):
        return False
    if (app.get('status') != 'passed' or app.get('pid') != pid or app.get('applicationScale') != scale
            or app.get('nativeAcks') != 3 or app.get('hostSettlements') != 4 or app.get('windowBackend') != 'wayland'):
        return False
    for flag in ('publicFixtureOnly', 'scriptedUiMessages', 'realWorkerAndRootReady', 'actualRootAgentCorrelated', 'hostAbortCancelObserved', 'liveHostDecisionQualified', 'actualRosterSucceeded'):
        if app.get(flag) is not True:
            return False
    for flag in ('nativeKeyboardPointerInputQualified', 'realCredentialsUsed', 'modelCatalogRequested', 'browserSignInRequested', 'manualBusinessMessagesForwarded', 'timedUiEnabled', 'outboundPacketTracePerformed'):
        if app.get(flag) is not False:
            return False
    control = output / 'fixture/control'
    ready = private_json(control / 'ready.json')
    if ready.get('rootRegistered') is not True or not isinstance(ready.get('actualAgentId'), str) or not ready['actualAgentId']:
        return False
    for ordinal in range(1, 5):
        phase = private_json(control / f'phase-{ordinal}.json')
        if type(phase.get('phase')) is not int or phase.get('phase') != ordinal or phase.get('settled') is not True or phase.get('actualAgentId') != ready['actualAgentId']:
            return False
        if ordinal == 1 and (phase.get('rootRegistered') is not True or phase.get('approvalOutcome') != 'allowed-once'):
            return False
        if ordinal == 2 and (phase.get('expectedAnswerMatched') is not True or phase.get('answerCount') != 3):
            return False
        if ordinal in (3, 4) and phase.get('code') != {3: 'ASK_CANCELLED', 4: 'ASK_ABORTED'}[ordinal]:
            return False
    final = private_json(control / 'complete.json')
    if app.get('host') != final or any(final.get(flag) is not True for flag in ('disposed', 'turnClosed', 'approvalAuditPair', 'zeroAdmittedModelSteps')):
        return False
    expected = {'turnStart': 1, 'turnEnd': 1, 'steps': 0, 'requestHeaders': 0, 'toolCalls': 0,
                'toolResults': 0, 'assistantMessages': 0, 'approvalAsked': 1, 'approvalDecided': 1}
    return final.get('counts') == expected


def valid_captures(output):
    import struct
    from stat import S_ISREG
    for phase in (1, 2):
        path = output / f'decision-{phase}.png'
        info = path.lstat()
        if not S_ISREG(info.st_mode) or info.st_mode & 0o7777 != 0o600 or info.st_uid != os.getuid() or not 24 <= info.st_size <= 32 * 1024 * 1024:
            return False
        with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW), 'rb') as stream:
            opened = os.fstat(stream.fileno())
            if (opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino):
                return False
            header = stream.read(24)
        if header[:8] != b'\x89PNG\r\n\x1a\n' or header[12:16] != b'IHDR':
            return False
        if any(not 1 <= dimension <= 4096 for dimension in struct.unpack('>II', header[16:24])):
            return False
    return True


async def run(options):
    output = OWN.NATIVE.prepare(options.output)
    binary = (BASE / 'app/target/release/examples/decisions_composed_smoke').resolve()
    if not binary.is_file():
        raise ValueError('release-fixture-not-ready')
    before_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
    interrupted = asyncio.Event()
    loop = asyncio.get_running_loop()
    handlers = []
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, interrupted.set)
            handlers.append(sig)
        child = subprocess.Popen([str(binary), '--output', str(output), '--scale', str(options.scale), '--viewport', options.viewport],
            env=OWN.NATIVE.environment(output), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    except BaseException:
        for sig in handlers:
            loop.remove_signal_handler(sig)
        raise
    pidfds, observed = {}, set()
    facts, failure, cleanup = None, None, None
    forced, root_pinned, observation_failures = False, False, 0
    try:
        try:
            root, fd = OWN.pin_root(child)
        except (OSError, RuntimeError):
            failure, forced = 'root-pidfd-acquisition-failed', True
        else:
            root_pinned = True
            pidfds[OWN.identity(root)] = fd
            observed.add(OWN.identity(root))
            deadline = time.monotonic() + OWN.QUALIFICATION_SECONDS
            while child.poll() is None:
                observation_failures += OWN.observe_descendants(child, root, pidfds, observed)
                if facts is None:
                    candidate = await OWN.METRICS.native_window(child.pid)
                    if candidate and candidate.get('mapped'):
                        facts = candidate
                if interrupted.is_set() or time.monotonic() >= deadline:
                    forced = True
                    break
                await asyncio.sleep(min(.1, max(0, deadline - time.monotonic())))
    except (OSError, RuntimeError, ValueError):
        failure, forced = 'qualification-observation-failed', True
    finally:
        task = asyncio.create_task(OWN.cleanup_owned(child, pidfds, root_pinned))
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
    forced |= cleanup['rootWasRunningAtCleanup']
    try:
        app = private_json(output / 'app.json')
        verified = valid_reports(output, app, child.pid, options.scale, options.viewport) and valid_captures(output)
    except (OSError, ValueError):
        app, verified = {'status': 'invalid-or-missing-report'}, False
    passed = (child.returncode == 0 and not forced and failure is None and verified and facts
              and cleanup['observedCleanupComplete'] and not cleanup['remainingBeforeExternalCleanup']
              and observation_failures == 0 and facts.get('class') == APP_ID and facts.get('xwayland') is False
              and hashlib.sha256(binary.read_bytes()).hexdigest() == before_hash)
    data = {'status': 'passed' if passed else 'failed', 'scope': __doc__, 'binarySha256': before_hash,
            'exitCode': child.returncode, 'forced': forced, 'interrupted': interrupted.is_set(),
            'failureCode': failure, 'ownedWindow': facts, 'observedOwnedProcessCount': len(observed),
            'ownershipObservationFailures': observation_failures, 'cleanup': cleanup, 'app': app,
            'separateHostReceiptsVerified': verified, 'paintInspected': False, 'outboundPacketTracePerformed': False,
            'nativeKeyboardPointerInputQualified': False, 'desktopSettingsChanged': False,
            'installedRuntimeChanged': False, 'realCredentialsUsed': False}
    OWN.private_json(output / 'result.json', data)
    print(json.dumps({'status': data['status'], 'report': str(output / 'result.json')}))
    return 0 if passed else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    parser.add_argument('--scale', type=float, choices=[1.0, 1.25], default=1.0)
    parser.add_argument('--viewport', choices=['full', 'minimum'], default='full', help='Fixed 608x448 logical parent constraints, not desktop/window reconfiguration')
    raise SystemExit(asyncio.run(run(parser.parse_args())))
