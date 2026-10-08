#!/usr/bin/env python3
"""Owned scripted native UI/worker/isolated Host export; public blank session only, no physical-input proof."""
import argparse
import asyncio
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys
import time
import zipfile
import zlib
import struct
import binascii

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('export_owned_registry', BASE / 'scripts/qualify-composed-registry.py')
REG = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REG)
APP_ID = 'ai.deepseek.harness.native.export-composed-fixture'
ARCHIVE = 'PUBLIC-session-export.zip'
MAX_ZIP = 4 * 1024 * 1024
MAX_ROOT = 1024 * 1024
CAPTURES = (('exportReviewCapture', 'export-review.png'), ('exportSavedCapture', 'export-saved.png'))


def binary_digest(path):
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or before.st_size > 64 * 1024 * 1024:
        raise ValueError('invalid-fixture-binary')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    digest, count = hashlib.sha256(), 0
    with os.fdopen(fd, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise ValueError('fixture-binary-changed')
        while chunk := stream.read(65536):
            count += len(chunk)
            if count > 64 * 1024 * 1024:
                raise ValueError('fixture-binary-oversize')
            digest.update(chunk)
    return digest.hexdigest()


def valid_app(app, pid, scale):
    if (app.get('status') != 'passed' or app.get('pid') != pid
            or app.get('applicationScale') != scale or app.get('windowBackend') != 'wayland'
            or app.get('targetCount') != 1 or app.get('zipArtifact') != ARCHIVE):
        return False
    required = ('publicFixtureOnly', 'scriptedUiMessages', 'realWorkerReady',
                'freshEmptyBaselineObserved', 'ownedWorkspaceCreated', 'oneOrdinaryPublicSessionCreated',
                'selectedBaselineObserved', 'matchingSavedReceiptObserved', 'rootOnlyExportConfirmed',
                'noOverwriteReceiptObserved', 'privateZipVerified', 'exactBytesUnchanged',
                'samePathBytesAndMetadataUnchanged', 'twoExactReceiptsObserved', 'gracefulStopped',
                'actualCloseUiDispatched', 'actualCloseInspectionObserved')
    if any(app.get(key) is not True for key in required):
        return False
    absent = ('modelCatalogRequested', 'browserSignInRequested', 'apiKeySaveRequested',
              'pluginWritesRequested', 'settingsWritesRequested', 'manualBusinessMessagesForwarded',
              'sessionDeletionRequested', 'stopActivityRequested', 'includeDescendants', 'realCredentialsUsed')
    if app.get('modelPrompts') != 0 or any(app.get(key) is not False for key in absent):
        return False
    stages = app.get('stages')
    if not isinstance(stages, list) or len(stages) != 2:
        return False
    first, second = stages
    for stage in stages:
        if (not isinstance(stage, dict) or any(stage.get(key) is not True for key in
                ('matchingExportedReceipt', 'controllerPendingRetired', 'enteredPathCleared'))):
            return False
        ticket = stage.get('receipt')
        if not isinstance(ticket, dict) or any(type(ticket.get(key)) is not int or ticket[key] < 1
                for key in ('epoch', 'generation', 'serial', 'editor', 'attempt')):
            return False
    a, b = first['receipt'], second['receipt']
    return (first.get('name') == 'save-new-public-archive' and first.get('saved') is True
            and first.get('notCreated') is False and type(first.get('savedBytes')) is int
            and 0 < first['savedBytes'] <= MAX_ZIP and second.get('name') == 'same-path-no-overwrite'
            and second.get('saved') is False and second.get('notCreated') is True
            and second.get('savedBytes') is None and a['epoch'] == b['epoch']
            and a['generation'] == b['generation'] and b['serial'] > a['serial']
            and b['editor'] > a['editor'] and b['attempt'] > a['attempt'])


def valid_captures(output, app):
    for key, filename in CAPTURES:
        capture = app.get(key)
        data = REG.private_regular(output / filename, REG.MAX_PNG_BYTES)
        dimensions = REG.png_dimensions(data) if data is not None else None
        if (not isinstance(capture, dict) or capture.get('saved') is not True
                or capture.get('artifact') != filename or dimensions is None
                or list(dimensions) != capture.get('physicalSize')):
            return False
    return True


def inspect_archive(output, app):
    """Bounded independent inflate+CRC+UTF8/object-JSONL check, never extraction or secret output."""
    data = REG.private_regular(output / ARCHIVE, MAX_ZIP)
    metadata = app.get('privateArchiveFileVerified')
    expected = metadata.get('bytes') if isinstance(metadata, dict) else None
    if data is None or len(data) != expected or not data:
        return {'passed': False, 'failureCode': 'private-archive-count'}
    try:
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            entries = archive.infolist()
            # Fresh blank session must have no referenced attachment; this is not rich-file qualification.
            if len(entries) != 1 or entries[0].filename != 'session.v4.jsonl':
                return {'passed': False, 'failureCode': 'blank-root-entry-profile'}
            info = entries[0]
            if info.flag_bits & 1 or info.is_dir() or not 0 <= info.file_size <= MAX_ROOT:
                return {'passed': False, 'failureCode': 'root-metadata-bound'}
            offset = info.header_offset
            if data[offset:offset + 4] != b'PK\x03\x04' or offset + 30 > len(data):
                return {'passed': False, 'failureCode': 'root-local-header'}
            name_len, extra_len = struct.unpack_from('<HH', data, offset + 26)
            start = offset + 30 + name_len + extra_len
            end = start + info.compress_size
            if (extra_len != 0 or end > len(data)
                    or data[offset + 30:offset + 30 + name_len] != b'session.v4.jsonl'):
                return {'passed': False, 'failureCode': 'root-local-data-bound'}
            compressed = data[start:end]
            if info.compress_type == zipfile.ZIP_DEFLATED:
                decoder = zlib.decompressobj(-15)
                root = decoder.decompress(compressed, MAX_ROOT + 1)
                if len(root) > MAX_ROOT or decoder.unconsumed_tail or decoder.unused_data or not decoder.eof:
                    return {'passed': False, 'failureCode': 'root-inflate-bound-or-eof'}
            elif info.compress_type == zipfile.ZIP_STORED:
                root = compressed
            else:
                return {'passed': False, 'failureCode': 'root-compression-method'}
            if (len(root) > MAX_ROOT or len(root) != info.file_size
                    or (binascii.crc32(root) & 0xffffffff) != info.CRC):
                return {'passed': False, 'failureCode': 'root-inflate-count-or-crc'}
            lines = root.decode('utf-8').splitlines()
            if not root.endswith(b'\n') or not lines or len(lines) > 256 or any(len(line.encode('utf-8')) > 65536 for line in lines):
                return {'passed': False, 'failureCode': 'root-jsonl-bound'}
            records = [json.loads(line) for line in lines]
            if any(not isinstance(record, dict) for record in records):
                return {'passed': False, 'failureCode': 'root-jsonl-object-profile'}
            header = records[0]
            if (header.get('type') != 'session' or type(header.get('version')) is not int
                    or header['version'] != 4 or not isinstance(header.get('id'), str)
                    or not 0 < len(header['id'].encode('utf-8')) <= 512
                    or type(header.get('createdAt')) is not int or header['createdAt'] < 0
                    or header.get('isSeeded') is not False or type(header.get('delegationDepth')) is not int
                    or header['delegationDepth'] != 0
                    or 'origin' in header or 'parentSession' in header):
                return {'passed': False, 'failureCode': 'ordinary-v4-header-profile'}
    except (OSError, ValueError, UnicodeError, RuntimeError, zipfile.BadZipFile, zlib.error, struct.error):
        return {'passed': False, 'failureCode': 'root-zip-crc-or-jsonl-invalid'}
    return {'passed': True, 'archiveBytes': len(data), 'archiveSha256': hashlib.sha256(data).hexdigest(),
            'entryCount': 1, 'rootUncompressedBytes': len(root), 'jsonLineCount': len(lines), 'headerLineCount': 1, 'eventRecordCount': len(lines) - 1,
            'crcAndBoundedInflateChecked': True, 'utf8ObjectJsonlChecked': True,
            'sessionIdentityAuthenticated': False, 'sessionFormatSemanticsFullyChecked': False,
            'richReferencedAttachmentsQualified': False, 'extracted': False, 'rawContentReported': False}


async def run(options):
    started = time.monotonic()
    output = REG.NATIVE.prepare(options.output)
    binary = (BASE / 'app/target/release/examples/export_composed_smoke').resolve()
    before_hash = binary_digest(binary)
    interrupted = asyncio.Event()
    loop = asyncio.get_running_loop()
    handlers = []
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, interrupted.set)
            handlers.append(sig)
        child = subprocess.Popen([str(binary), '--output', str(output), '--scale', str(options.scale)],
                env=REG.NATIVE.environment(output), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                start_new_session=True)
    except BaseException:
        for sig in handlers:
            loop.remove_signal_handler(sig)
        raise
    pidfds, observed = {}, set()
    facts, failure = None, None
    forced = root_pinned = False
    observation_failures = window_probe_attempts = 0
    try:
        try:
            root, fd = REG.pin_root(child)
        except (OSError, RuntimeError):
            failure, forced = 'root-pidfd-acquisition-failed', True
        else:
            root_pinned = True
            pidfds[REG.identity(root)] = fd
            observed.add(REG.identity(root))
            deadline = time.monotonic() + REG.QUALIFICATION_SECONDS
            while child.poll() is None:
                observation_failures += REG.observe_descendants(child, root, pidfds, observed)
                remaining = deadline - time.monotonic()
                if interrupted.is_set() or remaining <= 0:
                    forced = True
                    break
                if facts is None:
                    window_probe_attempts += 1
                    candidate = await REG.native_window(child.pid, REG.NATIVE.environment(output),
                            min(REG.WINDOW_PROBE_SECONDS, remaining))
                    if candidate and candidate.get('mapped'):
                        facts = candidate
                if interrupted.is_set() or time.monotonic() >= deadline:
                    forced = True
                    break
                await asyncio.sleep(min(.1, max(0, deadline - time.monotonic())))
    except (OSError, RuntimeError, ValueError):
        failure, forced = 'qualification-observation-failed', True
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
    app_valid = valid_app(app, child.pid, options.scale)
    captures_valid = valid_captures(output, app)
    archive = inspect_archive(output, app)
    unchanged = binary_digest(binary) == before_hash
    passed = (child.returncode == 0 and not forced and failure is None and app_valid and captures_valid
              and archive['passed'] and unchanged and cleanup['observedCleanupComplete']
              and not cleanup['remainingBeforeExternalCleanup'] and observation_failures == 0
              and facts and facts.get('class') == APP_ID and facts.get('xwayland') is False)
    report = {'status': 'passed' if passed else 'failed', 'scope': __doc__, 'binarySha256': before_hash,
              'binaryUnchanged': unchanged, 'exitCode': child.returncode, 'forced': forced,
              'failureCode': failure, 'interrupted': interrupted.is_set(), 'ownedWindow': facts,
              'ownershipObservationFailures': observation_failures, 'observedOwnedProcessCount': len(observed),
              'windowProbeAttempts': window_probe_attempts, 'qualificationBudgetSeconds': REG.QUALIFICATION_SECONDS,
              'fixtureShutdownRequestSeconds': 35, 'cleanup': cleanup,
              'elapsedWallSeconds': time.monotonic() - started, 'hardTotalWallClockBoundClaimed': False,
              'containmentScope': 'observed pinned identities only', 'app': app,
              'appEvidenceVerified': app_valid, 'privateOwnRendererCapturesVerified': captures_valid,
              'independentArchiveInspection': archive, 'paintInspected': False,
              'nativeKeyboardPointerInputQualified': False, 'realCredentialsUsed': False,
              'desktopSettingsChanged': False, 'installedRuntimeChanged': False,
              'partialFailureOrUnhealthyFilesystemLiveQualified': False, 'fullDesktopParity': False}
    REG.private_json(output / 'result.json', report)
    print(json.dumps({'status': report['status'], 'report': str(output / 'result.json')}))
    return 0 if passed else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    parser.add_argument('--scale', type=float, choices=[1.0, 1.25], default=1.0)
    try:
        sys.exit(asyncio.run(run(parser.parse_args())))
    except (OSError, RuntimeError, ValueError):
        print('composed-export-runner-failed', file=sys.stderr)
        sys.exit(1)
