#!/usr/bin/env python3
"""Owned scripted actual App/Worker/file upload/private PromptFile binding; no physical input or model history proof."""
import argparse
import asyncio
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import struct

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('composed_files_prompt', BASE / 'scripts/qualify-native-file-prompt.py')
PROMPT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROMPT)
FIXTURES = PROMPT.STAGE.FIXTURES
RUNNER = PROMPT.STAGE.FILES.RUNNER
EXAMPLE = BASE / 'app/target/debug/examples/file_composed_smoke'
CAPTURES = ['file-review.png', 'file-ready-1.png', 'file-ready-2.png', 'file-records.png']
MAX_PNG = 32 * 1024 * 1024
COUNTERS = {'nativeAcks': 3, 'actualFileStages': 3, 'hostSettlements': 3, 'explicitPrompts': 3,
            'actualAppPromptCommands': 3, 'currentTicketProbeRefusals': 6, 'staleUploadRefusals': 3,
            'duplicateSendRefusals': 6, 'staleRemoveRefusals': 2, 'staleAckRefusals': 2,
            'unexpectedWork': 0, 'explicitModelPrompts': 0, 'manualBusinessMessagesForwarded': 0, 'recordsDisclosureRoundTrips': 1}
TRUE_FLAGS = ['actualAppExercised', 'nativeWorkerExercised', 'scriptedUiMessages', 'actualRosterSucceeded',
              'ordinaryHeaderValidated', 'actualRootAgentCorrelated', 'publicFixtureOnly', 'quietProjectionObserved', 'retainedRecordsRevealed']
FALSE_FLAGS = ['modelCatalogRequested', 'browserSignInRequested', 'realCredentialsUsed', 'processWideNetworkTrace',
               'nativeKeyboardPointerInputQualified', 'historyDrivenReceiptRetirementQualified']
EXPECTED_FILES = [{'name': name, 'bytes': len(data)} for _, data, name in FIXTURES]
EXPECTED_REFS = [{'attachmentId': 'sha256:' + hashlib.sha256(data).hexdigest(), 'name': name, 'bytes': len(data)}
                 for _, data, name in FIXTURES]
STOP = {'exited': True, 'exitCode': 0, 'graceful': True, 'containmentUnknown': False,
        'observedDescendantsRemaining': 0}


def valid_app(value, scale):
    """Validate the driver report, not its independent on-disk storage/capture/Host evidence."""
    try:
        if not (type(value) is dict and type(scale) is float and scale in (1.0, 1.25)
                and value['status'] == 'passed' and value['version'] == '0.2.1-alpha.1'
                and type(value['pid']) is int and value['pid'] > 0
                and type(value['applicationScale']) is float and value['applicationScale'] == scale
                and value['windowBackend'] == 'wayland'
                and all(type(value[k]) is int and value[k] == n for k, n in COUNTERS.items())
                and all(value[k] is True for k in TRUE_FLAGS)
                and all(value[k] is False for k in FALSE_FLAGS)
                and type(value['files']) is list and value['files'] == EXPECTED_FILES
                and all(type(file['bytes']) is int for file in value['files'])
                and value['captures'] == CAPTURES
                and type(value['requests']) is list and len(value['requests']) == 3
                and type(value['tickets']) is list and len(value['tickets']) == 3
                and type(value['phases']) is list and len(value['phases']) == 3
                and type(value['stop']) is dict and value['stop'] == STOP
                and all(type(value['stop'][k]) is int for k in ['exitCode', 'observedDescendantsRemaining'])
                and all(type(value['stop'][k]) is bool for k in ['exited', 'graceful', 'containmentUnknown'])):
            return False
        first = value['tickets'][0]
        for n, (ticket, request, phase, reference) in enumerate(zip(
                value['tickets'], value['requests'], value['phases'], EXPECTED_REFS), 1):
            if not (type(ticket) is dict and set(ticket) == {'epoch', 'generation', 'serial', 'target'}
                    and all(type(ticket[k]) is int and ticket[k] > 0 for k in ['epoch', 'generation', 'serial'])
                    and ticket['serial'] == n and ticket['epoch'] == first['epoch']
                    and ticket['generation'] == first['generation'] and ticket['target'] == first['target']
                    and type(ticket['target']) is str and 0 < len(ticket['target'].encode('utf-8')) <= 512
                    and '\0' not in ticket['target'] and type(request) is str):
                return False
            match = re.fullmatch(r'native-([1-9][0-9]*)-([1-9][0-9]*)-([1-9][0-9]*)-([1-3])', request)
            if not (match and int(match[1]) == value['pid'] and int(match[3]) == ticket['generation']
                    and int(match[4]) == n and type(phase) is dict
                    and type(phase['phase']) is int and phase['phase'] == n and phase['requestId'] == request
                    and phase['file'] == reference and type(phase['file']['bytes']) is int
                    and PROMPT.counts(phase['counts'], n)
                    and all(phase[k] is True for k in ['rootRegistered', 'durableInboxFileMatched',
                        'claimedFileMatched', 'blockedTurn', 'zeroAdmittedModelSteps'])):
                return False
        host = value['host']
        return (len(set(value['requests'])) == 3 and type(host) is dict and host['status'] == 'passed'
                and all(host[k] is True for k in ['rootDisposed', 'turnClosed', 'zeroAdmittedModelSteps'])
                and PROMPT.counts(host['counts'], 3))
    except (KeyError, TypeError, ValueError, AttributeError, UnicodeError):
        return False


def private_bytes(path, limit):
    """Read a bounded owner-private regular artifact, without following a final symlink or blocking on a FIFO."""
    info = path.lstat()
    if (not stat.S_ISREG(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_uid != os.getuid() or not 0 < info.st_size <= limit):
        raise ValueError('private-regular-artifact-required')
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC), 'rb') as stream:
        opened = os.fstat(stream.fileno())
        if ((opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino)
                or not stat.S_ISREG(opened.st_mode) or stat.S_IMODE(opened.st_mode) != 0o600
                or opened.st_uid != os.getuid() or opened.st_size != info.st_size):
            raise ValueError('private-artifact-changed')
        data = stream.read(limit + 1)
    if len(data) != info.st_size or len(data) > limit:
        raise ValueError('private-artifact-size-changed')
    return data


def private_json(path, limit=16384):
    value = json.loads(private_bytes(path, limit))
    if type(value) is not dict:
        raise ValueError('private-report-object-required')
    return value


def verify_captures(output, app):
    if app.get('captures') != CAPTURES:
        raise ValueError('capture-list-mismatch')
    captures = []
    for name in CAPTURES:
        data = private_bytes(output / name, MAX_PNG)
        if (len(data) < 33 or data[:8] != b'\x89PNG\r\n\x1a\n'
                or data[8:16] != b'\x00\x00\x00\x0dIHDR'):
            raise ValueError('capture-png-header-invalid')
        size = struct.unpack('>II', data[16:24])
        if any(not 1 <= dimension <= 4096 for dimension in size):
            raise ValueError('capture-dimensions-invalid')
        captures.append({'artifact': name, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(),
                         'physicalSize': list(size), 'privateMode': '0600'})
    return captures


def verify_evidence(output, app, scale):
    """Cross-check raw Host receipts and independently open only fixed storage/capture paths."""
    if not valid_app(app, scale):
        raise ValueError('composed-app-report-invalid')
    control = output / 'fixture/control'
    ready = private_json(control / 'ready.json')
    if not (ready.get('rootRegistered') is True and type(ready.get('headerVersion')) is int
            and ready['headerVersion'] == 4 and ready.get('ordinary') is True
            and ready.get('actualAgentId') == app['tickets'][0]['target']):
        raise ValueError('actual-root-ticket-correlation-invalid')
    for n, phase in enumerate(app['phases'], 1):
        if private_json(control / f'phase-{n}.json') != phase:
            raise ValueError('raw-host-phase-mismatch')
    if private_json(control / 'complete.json') != app['host'] or os.path.lexists(control / 'failed.json'):
        raise ValueError('raw-host-completion-invalid')
    storage = PROMPT.STAGE.FILES.verify_local_cases(output, FIXTURES)
    digest = hashlib.sha256(PROMPT.STAGE.REFUSED).hexdigest()
    if os.path.lexists(output / 'harness/attachments/v1/file-objects' / digest[:2] / digest):
        raise ValueError('refused-fixture-was-stored')
    return {'storage': storage, 'captures': verify_captures(output, app),
            'separateHostReceiptsVerified': True, 'actualRootTicketCorrelationVerified': True,
            'refusedFixtureNotStored': True, 'paintInspected': False,
            'pngHeaderAndDimensionsChecked': True, 'pngPixelsDecoded': False}


def command(output, scale=1.0):
    result = PROMPT.STAGE.command(output)
    result[0] = str(EXAMPLE)
    return result + ['--scale', str(scale)]


async def run(options):
    scale = options.scale
    result = await RUNNER.run(options, script=EXAMPLE, report_name='app.json', report_key='app',
        validator=lambda value: valid_app(value, scale), capture_stdout=True,
        command_factory=lambda output: command(output, scale),
        scope='actual App/Worker raw upload/private PromptFile binding/full Alpha Host plus isolated reject fixture and scripted native window; not physical input or model/user-message history')
    output = Path(options.output).resolve()
    summary = {'status': 'failed', 'scope': __doc__, 'paintInspected': False,
               'nativeKeyboardPointerInputQualified': False, 'historyDrivenReceiptRetirementQualified': False}
    try:
        owned = private_json(output / 'result.json', 65536)
        summary['ownedAppStatus'] = owned['status']
        if result or owned['status'] != 'passed':
            raise ValueError('owned-app-not-qualified')
        summary.update(verify_evidence(output, owned['app'], scale))
        summary['status'] = 'passed'
    except (OSError, ValueError, KeyError, TypeError):
        summary['refusal'] = 'invalid-owned-app-or-independent-file-evidence'
    path = output / 'composed-files-qualification.json'
    RUNNER.OWNED.private_json(path, summary)
    print(json.dumps({'status': summary['status'], 'report': str(path)}))
    return 0 if summary['status'] == 'passed' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    parser.add_argument('--scale', type=float, choices=[1.0, 1.25], default=1.0)
    raise SystemExit(asyncio.run(run(parser.parse_args())))
