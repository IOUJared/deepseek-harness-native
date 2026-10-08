#!/usr/bin/env python3
"""Owned actual native-worker selected-file intake, with independent static stored bytes."""
import argparse
import asyncio
import hashlib
import importlib.util
import json
import os
from pathlib import Path

BASE = Path(__file__).resolve().parents[1]
def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module
FILES = load('native_stage_storage', BASE / 'scripts/qualify-file-upload.py')
EXAMPLE = BASE / 'app/target/debug/examples/file_stage_smoke'
FIXTURES = [('raw', bytes((i * 43 + 19) % 256 for i in range(5003)), 'PUBLIC owned file.bin'),
            ('empty', b'', 'PUBLIC empty.bin'),
            ('ceiling', bytes((i * 13 + 7) % 256 for i in range(4 * 1024 * 1024)), 'PUBLIC ceiling.bin')]
REFUSED = b'PUBLIC refusal fixture must never be stored'


def valid_worker(value):
    try:
        stop = value['stop']
        base = ['permission/preset', 'sandbox/mode', 'approval/policy']
        return (value['status'] == 'passed' and value['version'] == '0.2.1-alpha.1'
                and value['files'] == [{'name': name, 'bytes': len(data)} for _, data, name in FIXTURES]
                and all(type(entry['bytes']) is int for entry in value['files'])
                and type(value['foreignDiscardRefusals']) is int and value['foreignDiscardRefusals'] == 3
                and value['preSnapshotRefused'] is True and value['staleSelectionRefused'] is True
                and type(value['unexpectedWork']) is int and value['unexpectedWork'] == 0
                and type(value['explicitModelPrompts']) is int and value['explicitModelPrompts'] == 0
                and value['snapshots'] == [base, base]
                and value['nativeWorkerExercised'] is True
                and all(value[key] is False for key in ['modelCatalogRequested', 'processWideNetworkTrace', 'uiExercised'])
                and stop['exited'] is True and stop['graceful'] is True and stop['containmentUnknown'] is False
                and type(stop['exitCode']) is int and stop['exitCode'] == 0
                and type(stop['observedDescendantsRemaining']) is int and stop['observedDescendantsRemaining'] == 0)
    except (KeyError, TypeError, ValueError):
        return False


def command(output):
    for name, data in [(name, data) for _, data, name in FIXTURES] + [('PUBLIC refused.bin', REFUSED)]:
        with os.fdopen(os.open(output / 'workspace' / name, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'wb') as stream:
            stream.write(data)
    return [str(EXAMPLE), '--runtime', str((BASE.parent / 'deepseek-harness-linux/apps/cli').resolve()),
            '--expected-version', '0.2.1-alpha.1', '--home', str(output / 'harness'), '--cwd', str(output / 'workspace'),
            '--user-home', str(output / 'user')]


async def run(options):
    result = await FILES.RUNNER.run(options, script=EXAMPLE, report_name='worker.json', report_key='worker',
                                    validator=valid_worker, capture_stdout=True, command_factory=command,
                                    scope='actual headless native worker/owned blocking file intake/Core/public alpha raw HTTP; not chooser, GUI or prompt binding')
    output = Path(options.output).resolve()
    owned = json.loads((output / 'result.json').read_text())
    summary = {'status': 'failed', 'ownedWorkerStatus': owned['status'],
               'scope': 'complete static source/default-provider bytes, modes and aliases; selected header and ticket admission/refusal; no real input/model or generic provider guarantees'}
    try:
        if result or not valid_worker(owned['worker']):
            raise ValueError('owned-native-stage-not-qualified')
        summary['storage'] = FILES.verify_local_cases(output, FIXTURES)
        digest = hashlib.sha256(REFUSED).hexdigest()
        refused = output / 'harness/attachments/v1/file-objects' / digest[:2] / digest
        if os.path.lexists(refused):
            raise ValueError('refused-file-was-published')
        summary['refusedFixtureNotStored'] = True
        summary['status'] = 'passed'
    except (OSError, ValueError, KeyError, TypeError):
        summary['refusal'] = 'invalid-owned-worker-or-storage-evidence'
    FILES.RUNNER.OWNED.private_json(output / 'native-stage-qualification.json', summary)
    print(json.dumps({'status': summary['status'], 'report': str(output / 'native-stage-qualification.json')}))
    return 0 if summary['status'] == 'passed' else 1

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    raise SystemExit(asyncio.run(run(parser.parse_args())))
