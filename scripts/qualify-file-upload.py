#!/usr/bin/env python3
"""Owned real SDK/raw-HTTP upload, with independent local-provider byte/mode verification."""
import argparse
import asyncio
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import uuid

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('upload_owned_runner', BASE / 'scripts/qualify-lazy-account.py')
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
SDK = BASE / 'core/target/release/examples/file_upload_smoke'
CASES = ['raw', 'empty', 'ceiling', 'alias', 'cold']
NAMES = {'raw': 'PUBLIC report é__.bin', 'empty': 'file', 'ceiling': 'PUBLIC ceiling.bin',
         'alias': 'PUBLIC alias.bin', 'cold': 'PUBLIC cold.bin'}


def source(case):
    if case in {'raw', 'alias'}:
        return bytes(i * 17 % 256 for i in range(4097))
    if case == 'ceiling':
        return bytes((i * 13 + 7) % 256 for i in range(4 * 1024 * 1024))
    if case == 'empty':
        return b''
    if case == 'cold':
        return bytes([80, 0, 255, 128, 10])
    raise ValueError('unknown-fixture-case')


def valid_sdk(report):
    try:
        if (report['status'] != 'passed' or report['versions'] != ['0.2.1-alpha.1'] * 2
                or type(report['explicitModelPrompts']) is not int or report['explicitModelPrompts'] != 0
                or any(report[key] is not False for key in ['modelCatalogRequested', 'processWideNetworkTrace', 'uiExercised'])
                or any(report[key] is not True for key in ['liveAgentObserved', 'missingTargetRefused', 'coldBeforeUpload', 'resumedAfterUpload'])
                or len(report['stops']) != 2 or len(report['postUploadSnapshots']) != 2
                or [entry['case'] for entry in report['uploads']] != CASES):
            return False
        for stop in report['stops']:
            if (stop['exited'] is not True or stop['graceful'] is not True or type(stop['exitCode']) is not int
                    or stop['exitCode'] != 0 or stop['containmentUnknown'] is not False
                    or type(stop['observedDescendantsRemaining']) is not int or stop['observedDescendantsRemaining'] != 0):
                return False
        base = ['permission/preset', 'sandbox/mode', 'approval/policy']
        if report['postUploadSnapshots'] != [base, base + ['session/end-seed']]:
            return False
        receipts = set()
        for entry in report['uploads']:
            value, case = entry['value'], entry['case']
            receipt = uuid.UUID(value['receiptId'])
            if receipt.version != 4 or str(receipt) != value['receiptId'] or value['receiptId'] in receipts:
                return False
            receipts.add(value['receiptId'])
            data = source(case)
            file = value['file']
            if (set(value) != {'receiptId', 'file'} or set(file) != {'attachmentId', 'name', 'bytes'}
                    or file['name'] != NAMES[case] or type(file['bytes']) is not int or file['bytes'] != len(data)
                    or file['attachmentId'] != 'sha256:' + hashlib.sha256(data).hexdigest()):
                return False
        return True
    except (KeyError, TypeError, AttributeError, ValueError):
        return False


def verify_file(path, data):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o400 or info.st_size != len(data):
            raise ValueError('file-size-or-mode')
        offset = 0
        while chunk := os.read(fd, 65536):
            if chunk != data[offset:offset + len(chunk)]:
                raise ValueError('independent-byte-mismatch')
            offset += len(chunk)
        if offset != len(data):
            raise ValueError('incomplete-independent-read')
        return info.st_dev, info.st_ino
    finally:
        os.close(fd)


def verify_storage(output, report):
    if not valid_sdk(report):
        raise ValueError('invalid-sdk-report')
    return verify_local_cases(output, [(case, source(case), NAMES[case]) for case in CASES])


def verify_local_cases(output, fixtures):
    """Read only caller-supplied static fixture paths, never returned upload paths."""
    root = output / 'harness' / 'attachments' / 'v1'
    verified = []
    for case, data, name in fixtures:
        digest = hashlib.sha256(data).hexdigest()
        canonical = root / 'file-objects' / digest[:2] / digest
        alias = root / 'files' / digest[:2] / digest / name
        # Only static fixture-derived paths are opened, never remote receipt paths.
        for path in [output, output / 'harness', output / 'harness' / 'attachments', root,
                     *canonical.relative_to(root).parents, *alias.relative_to(root).parents]:
            directory = path if path.is_absolute() else root / path
            info = directory.lstat()
            if not stat.S_ISDIR(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o700:
                raise ValueError('private-directory-mode-or-link')
        canonical_identity = verify_file(canonical, data)
        if verify_file(alias, data) != canonical_identity:
            raise ValueError('alias-not-hardlinked-to-canonical')
        verified.append({'case': case, 'sha256': digest, 'bytes': len(data), 'rawBytesVerified': True,
                         'canonicalAndAliasMode': '0400', 'privateDirectories': True, 'sameInode': True})
    return verified


def sdk_command(output):
    return [str(SDK), '--runtime', str((BASE.parent / 'deepseek-harness-linux/apps/cli').resolve()),
            '--expected-version', '0.2.1-alpha.1', '--home', str(output / 'harness'),
            '--cwd', str(output / 'workspace'), '--user-home', str(output / 'user')]


async def qualify(options):
    result = await RUNNER.run(options, script=SDK, report_name='sdk.json', report_key='sdk',
                             validator=valid_sdk, command_factory=sdk_command, capture_stdout=True,
                             scope='owned two-epoch public alpha composition and Rust SDK/raw HTTP; post-upload snapshots only, no process-wide trace or GUI')
    output = Path(options.output).resolve()
    owned = json.loads((output / 'result.json').read_text())
    report = {'status': 'failed', 'ownedSdkStatus': owned['status'], 'storage': [],
              'scope': 'independent private default LocalAttachmentStore bytes, names, deduplication, modes; not other providers, receipt prompt binding, model/image validation, GUI or complete rollback'}
    try:
        if result != 0:
            raise ValueError('owned-sdk-failed')
        report['storage'] = verify_storage(output, owned['sdk'])
        report['status'] = 'passed'
    except (OSError, ValueError):
        report['error'] = 'owned-sdk-or-independent-storage-verification-failed'
    RUNNER.OWNED.private_json(output / 'upload-qualification.json', report)
    print(json.dumps({'status': report['status'], 'report': str(output / 'upload-qualification.json')}))
    return 0 if report['status'] == 'passed' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    raise SystemExit(asyncio.run(qualify(parser.parse_args())))
