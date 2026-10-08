#!/usr/bin/env python3
"""Scripted actual native App/worker/Core reversible registry fixture in a NEW private home only."""
import argparse
import asyncio
import binascii
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import stat
import struct
import subprocess
import sys
import time
import zlib

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('registry_owned_cleanup', BASE / 'scripts/qualify-composed-settings.py')
OWNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(OWNER)
# Reuse the repaired implementation, including identity-fenced root acquisition,
# every pinned descendant's finite TERM/KILL path and unreaped-Popen fallback.
NATIVE = OWNER.NATIVE
METRICS = OWNER.METRICS
pin_root = OWNER.pin_root
identity = OWNER.identity
observe_descendants = OWNER.observe_descendants
cleanup_owned = OWNER.cleanup_owned
private_json = OWNER.private_json
APP_ID = 'ai.deepseek.harness.native.registry-composed-fixture'
QUALIFICATION_SECONDS = 45
WINDOW_PROBE_SECONDS = 1
MAX_REPORT_BYTES = 65_536
MAX_PNG_BYTES = 64 * 1024 * 1024
STAGES = (
    ('pin-first', 'Pin', 1, [1], True, False),
    ('pin-second', 'Pin', 2, [2, 1], True, False),
    ('unpin-first', 'Unpin', 1, [2], True, False),
    ('repin-first', 'Pin', 1, [1, 2], True, False),
    ('archive-first', 'Archive', 1, [2], True, True),
    ('restore-first', 'Restore', 1, [2], False, True),
)
CAPTURES = (
    ('archiveReviewCapture', 'archive-review.png'),
    ('archivedRestoreReviewCapture', 'restore-review.png'),
)


def private_regular(path, maximum):
    """Read only an owned non-symlink 0600 regular file with bounded allocation."""
    try:
        info = path.lstat()
        if (not stat.S_ISREG(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o600
                or info.st_size > maximum):
            return None
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        with os.fdopen(fd, 'rb') as stream:
            opened = os.fstat(stream.fileno())
            if ((opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino)
                    or not stat.S_ISREG(opened.st_mode) or stat.S_IMODE(opened.st_mode) != 0o600):
                return None
            data = stream.read(maximum + 1)
        return data if len(data) <= maximum else None
    except OSError:
        return None


def read_app(output):
    data = private_regular(output / 'app.json', MAX_REPORT_BYTES)
    if data is None:
        return {'status': 'missing-or-invalid-private-app-report'}
    try:
        app = json.loads(data)
    except (ValueError, UnicodeError):
        return {'status': 'invalid-app-report'}
    return app if isinstance(app, dict) else {'status': 'invalid-app-report'}


def valid_app(app, child_pid, scale):
    """Receipts alone cannot pass: every planned state requires independent follow evidence."""
    if (not isinstance(app, dict) or app.get('status') != 'passed'
            or app.get('pid') != child_pid or app.get('applicationScale') != scale
            or app.get('windowBackend') != 'wayland' or app.get('modelPrompts') != 0):
        return False
    required = ('publicFixtureOnly', 'scriptedUiMessages', 'realWorkerReady',
                'freshEmptyBaselineObserved', 'ownedWorkspaceCreated',
                'twoDistinctPublicSessionsCreated', 'ownedWorkspaceFeedMembershipObserved',
                'actualArchivedViewEntered', 'restoredRecentViewVerified', 'restoreDidNotRepin',
                'archiveAndUnpinCoupledFeedVerified', 'allMatchingAcksAndIndependentFeedsVerified',
                'filesRetainedNotDeletionRollback')
    forbidden = ('realCredentialsUsed', 'modelCatalogRequested', 'browserSignInRequested',
                 'apiKeySaveRequested', 'stopActivityRequested', 'sessionDeletionRequested',
                 'forkOrRenameRequested', 'manualBusinessMessagesForwarded', 'rpcSetsInstalled',
                 'nativeKeyboardPointerInputQualified', 'liveHostDecisionQualified')
    if any(app.get(key) is not True for key in required) or any(app.get(key) is not False for key in forbidden):
        return False
    ids = app.get('publicSessionIds')
    if (not isinstance(ids, list) or len(ids) != 2 or any(not isinstance(value, str)
            or not 1 <= len(value) <= 4096 for value in ids) or ids[0] == ids[1]):
        return False
    stages = app.get('stages')
    if not isinstance(stages, list) or len(stages) != len(STAGES):
        return False
    previous_serial = 0
    for observed, (name, operation, target, pins, needs_pin, needs_archive) in zip(stages, STAGES):
        if (not isinstance(observed, dict) or observed.get('name') != name
                or observed.get('operation') != operation or observed.get('targetSlot') != target
                or observed.get('pinOrderSlots') != pins):
            return False
        for key in ('matchingSubmissionAcknowledged', 'requiredFeedIncrementsObserved',
                    'authoritativeRegistryVerified', 'actualNavigationVerified',
                    'bothSessionsAndAccountingRetained'):
            if observed.get(key) is not True:
                return False
        if needs_pin and observed.get('pinIncrementObserved') is not True:
            return False
        if needs_archive and observed.get('archiveIncrementObserved') is not True:
            return False
        ticket = observed.get('ticket')
        if (not isinstance(ticket, dict) or ticket.get('epoch') != 1
                or type(ticket.get('generation')) is not int or ticket['generation'] <= 0
                or type(ticket.get('serial')) is not int or ticket['serial'] <= previous_serial):
            return False
        previous_serial = ticket['serial']
    stop = app.get('stop')
    return (isinstance(stop, dict) and stop.get('exited') is True and stop.get('exitCode') == 0
            and stop.get('graceful') is True and stop.get('containmentUnknown') is False
            and stop.get('observedDescendantsRemaining') == 0)


def png_dimensions(data):
    """Validate bounded RGBA8/noninterlaced writer-profile PNGs, not decode all PNG formats.

    CRCs, chunk order and one complete zlib stream are required. Inflated rows
    must have exact lengths and valid filter tags; pixels are not reconstructed.
    Ancillary chunks are CRC-checked but their application semantics are ignored.
    """
    if (len(data) > MAX_PNG_BYTES or len(data) < 33
            or data[:8] != b'\x89PNG\r\n\x1a\n'):
        return None
    view = memoryview(data)
    position = 8
    dimensions = None
    inflater = None
    expected = inflated = stride = 0
    idat_seen = idat_ended = palette_seen = False
    while position < len(data):
        if len(data) - position < 12:
            return None
        length = struct.unpack_from('>I', data, position)[0]
        kind = data[position + 4:position + 8]
        end = position + 12 + length
        if (end > len(data) or any(not (65 <= c <= 90 or 97 <= c <= 122) for c in kind)
                or kind[2] & 32):
            return None
        payload = view[position + 8:position + 8 + length]
        crc = binascii.crc32(payload, binascii.crc32(kind)) & 0xffffffff
        if struct.unpack_from('>I', data, end - 4)[0] != crc:
            return None
        if dimensions is None:
            if kind != b'IHDR' or length != 13:
                return None
            width, height, depth, color, compression, filtering, interlace = struct.unpack('>IIBBBBB', payload)
            stride = width * 4 + 1
            expected = stride * height
            if (not width or not height or expected > MAX_PNG_BYTES
                    or (depth, color, compression, filtering, interlace) != (8, 6, 0, 0, 0)):
                return None
            dimensions = (width, height)
            inflater = zlib.decompressobj()
        elif kind == b'IHDR':
            return None
        elif kind == b'PLTE':
            if palette_seen or idat_seen or not length or length % 3 or length > 768:
                return None
            palette_seen = True
        elif kind == b'IDAT':
            if idat_ended:
                return None
            idat_seen = True
            try:
                # Never use unbounded decompress/flush. One extra output byte detects
                # excess expansion without allocating the complete decompression bomb.
                rows = inflater.decompress(payload, expected - inflated + 1)
            except zlib.error:
                return None
            if len(rows) > expected - inflated or inflater.unused_data or inflater.unconsumed_tail:
                return None
            first_filter = (-inflated) % stride
            if any(rows[index] > 4 for index in range(first_filter, len(rows), stride)):
                return None
            inflated += len(rows)
        elif kind == b'IEND':
            if (length or end != len(data) or not idat_seen or inflated != expected
                    or not inflater.eof or inflater.unused_data or inflater.unconsumed_tail):
                return None
            return dimensions
        else:
            if not kind[0] & 32:  # Unknown critical chunks change image interpretation.
                return None
            if idat_seen:
                idat_ended = True
        position = end
    return None


def valid_captures(output, app):
    for key, filename in CAPTURES:
        report = app.get(key)
        data = private_regular(output / filename, MAX_PNG_BYTES)
        if (not isinstance(report, dict) or report.get('saved') is not True
                or report.get('manualInputProof') is not False or data is None):
            return False
        dimensions = png_dimensions(data)
        if dimensions is None or report.get('physicalSize') != list(dimensions):
            return False
    return True


async def native_window(pid, environment, seconds):
    """Bound an independently owned read-only compositor probe, including its cleanup."""
    probe = subprocess.Popen(['hyprctl', '-j', 'clients'], env=environment,
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    fds = {}
    pinned = False
    observed = set()
    observation_failures = 0
    data = bytearray()
    valid = False
    cleanup = None
    try:
        root, fd = pin_root(probe)
        pinned = True
        fds[identity(root)] = fd
        os.set_blocking(probe.stdout.fileno(), False)
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            observation_failures += observe_descendants(probe, root, fds, observed)
            try:
                part = os.read(probe.stdout.fileno(), 65_536)
            except BlockingIOError:
                part = None
            if part is not None:
                if len(data) + len(part) > 1024 * 1024:
                    break
                data.extend(part)
                if not part and probe.poll() is not None:
                    valid = probe.returncode == 0
                    break
            await asyncio.sleep(min(.02, max(0, deadline - time.monotonic())))
    finally:
        task = asyncio.create_task(cleanup_owned(probe, fds, pinned))
        try:
            cleanup = await asyncio.shield(task)
        except asyncio.CancelledError:
            cleanup = await task
            raise
        finally:
            probe.stdout.close()
            for fd in fds.values():
                os.close(fd)
        if not cleanup['observedCleanupComplete'] or observation_failures:
            raise RuntimeError('owned-window-probe-cleanup-incomplete')
    if not valid:
        return None
    try:
        entries = json.loads(data)
    except (ValueError, UnicodeError):
        return None
    if not isinstance(entries, list):
        return None
    for entry in entries:
        if isinstance(entry, dict) and entry.get('pid') == pid:
            return {key: entry.get(key) for key in ('pid', 'class', 'xwayland', 'mapped', 'size')}
    return None


async def run(options):
    started = time.monotonic()
    if options.scale not in (1.0, 1.25):
        raise ValueError('invalid-scale')
    output = NATIVE.prepare(options.output)
    binary = (BASE / 'app/target/release/examples/registry_composed_smoke').resolve()
    if not binary.is_file():
        raise ValueError('release-registry-composed-fixture-not-ready')
    before_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
    interrupted = asyncio.Event()
    loop = asyncio.get_running_loop()
    handlers = []
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, interrupted.set)
            handlers.append(sig)
        # No asyncio child watcher may reap the resource before imported pin_root.
        child = subprocess.Popen([str(binary), '--output', str(output), '--scale', str(options.scale)],
            env=NATIVE.environment(output), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            start_new_session=True)
    except BaseException:
        for sig in handlers:
            loop.remove_signal_handler(sig)
        raise
    pidfds, observed = {}, set()
    facts = None
    forced = root_pinned = False
    failure = None
    observation_failures = window_probe_attempts = 0
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
                remaining = deadline - time.monotonic()
                if interrupted.is_set() or remaining <= 0:
                    forced = True
                    break
                if facts is None:
                    window_probe_attempts += 1
                    candidate = await native_window(child.pid, NATIVE.environment(output),
                        min(WINDOW_PROBE_SECONDS, remaining))
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
        # The imported implementation owns finite TERM/KILL, all observed pidfds,
        # nonblocking reap and pin-failure fallback. Do not duplicate that logic.
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
    app = read_app(output)
    app_valid = valid_app(app, child.pid, options.scale)
    captures_valid = valid_captures(output, app)
    binary_unchanged = hashlib.sha256(binary.read_bytes()).hexdigest() == before_hash
    passed = (child.returncode == 0 and not forced and failure is None
              and cleanup['observedCleanupComplete'] and not cleanup['remainingBeforeExternalCleanup']
              and observation_failures == 0 and app_valid and captures_valid and facts
              and facts.get('class') == APP_ID and facts.get('xwayland') is False
              and binary_unchanged)
    data = {'status': 'passed' if passed else 'failed', 'scope': __doc__, 'binarySha256': before_hash,
            'binaryUnchanged': binary_unchanged, 'exitCode': child.returncode, 'forced': forced,
            'interrupted': interrupted.is_set(), 'failureCode': failure, 'ownedWindow': facts,
            'observedOwnedProcessCount': len(observed), 'ownershipObservationFailures': observation_failures,
            'windowProbeAttempts': window_probe_attempts, 'qualificationBudgetSeconds': QUALIFICATION_SECONDS,
            'fixtureShutdownRequestSeconds': 35, 'cleanup': cleanup,
            'elapsedWallSeconds': time.monotonic() - started,
            'wallClockIncludesPreparationObservationCleanup': True,
            'hardTotalWallClockBoundClaimed': False, 'containmentScope': 'observed pinned identities only',
            'remainingBeforeExternalCleanup': cleanup['remainingBeforeExternalCleanup'],
            'remainingAfterExternalCleanup': cleanup['remainingAfterExternalCleanup'],
            'app': app, 'appEvidenceVerified': app_valid, 'privateOwnRendererCapturesVerified': captures_valid,
            'paintInspected': False, 'nativeKeyboardPointerInputQualified': False,
            'liveHostDecisionQualified': False, 'desktopSettingsChanged': False,
            'installedRuntimeChanged': False, 'realCredentialsUsed': False,
            'mutationsReversible': True, 'filesRetainedNotDeletionRollback': True}
    private_json(output / 'result.json', data)
    print(json.dumps({'status': data['status'], 'report': str(output / 'result.json')}))
    return 0 if passed else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    parser.add_argument('--scale', type=float, choices=[1.0, 1.25], default=1.0)
    try:
        status = asyncio.run(run(parser.parse_args()))
    except (OSError, RuntimeError, ValueError):
        print('composed-registry-runner-failed', file=sys.stderr)
        status = 1
    raise SystemExit(status)
