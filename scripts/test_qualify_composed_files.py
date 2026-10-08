import asyncio
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import struct
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import AsyncMock, patch
import zlib

SPEC = importlib.util.spec_from_file_location('qualified_composed_files', Path(__file__).with_name('qualify-composed-files.py'))
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


def fixture(scale=1.0):
    value = {'status': 'passed', 'version': '0.2.1-alpha.1', 'pid': 4242, 'applicationScale': scale,
             'windowBackend': 'wayland', **M.COUNTERS, 'files': copy.deepcopy(M.EXPECTED_FILES),
             'captures': list(M.CAPTURES), 'requests': [], 'tickets': [], 'phases': [],
             'stop': dict(M.STOP)}
    value.update({key: True for key in M.TRUE_FLAGS})
    value.update({key: False for key in M.FALSE_FLAGS})
    for n, reference in enumerate(M.EXPECTED_REFS, 1):
        request = f'native-4242-{1000 + n}-7-{n}'
        value['requests'].append(request)
        value['tickets'].append({'epoch': 1, 'generation': 7, 'serial': n, 'target': 'PUBLIC_actual_root'})
        phase = {'phase': n, 'requestId': request, 'file': copy.deepcopy(reference),
                 'counts': {**{key: n for key in M.PROMPT.COUNTS}, **{key: 0 for key in M.PROMPT.ZERO}}}
        phase.update({key: True for key in ['rootRegistered', 'durableInboxFileMatched', 'claimedFileMatched',
                                          'blockedTurn', 'zeroAdmittedModelSteps']})
        value['phases'].append(phase)
    value['host'] = {'status': 'passed', 'rootDisposed': True, 'turnClosed': True,
                     'zeroAdmittedModelSteps': True, 'counts': copy.deepcopy(value['phases'][-1]['counts']),
                     'scope': 'actual durable inbox admission; not user-message history retirement'}
    return value


def private_write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    path.chmod(0o600)


def png():
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(b'\0\x19\x17\x24')) + chunk(b'IEND', b''))


def write_evidence(output, app):
    output.chmod(0o700)
    control = output / 'fixture/control'
    ready = {'rootRegistered': True, 'headerVersion': 4, 'ordinary': True,
             'actualAgentId': app['tickets'][0]['target']}
    for name, value in [('ready.json', ready), ('complete.json', app['host']),
                        *[(f'phase-{n}.json', phase) for n, phase in enumerate(app['phases'], 1)]]:
        private_write(control / name, json.dumps(value).encode())
    for name in M.CAPTURES:
        private_write(output / name, png())
    root = output / 'harness/attachments/v1'
    for _, data, name in M.FIXTURES:
        digest = hashlib.sha256(data).hexdigest()
        canonical = root / 'file-objects' / digest[:2] / digest
        alias = root / 'files' / digest[:2] / digest / name
        canonical.parent.mkdir(parents=True, exist_ok=True)
        alias.parent.mkdir(parents=True, exist_ok=True)
        canonical.write_bytes(data)
        canonical.chmod(0o400)
        os.link(canonical, alias)
        for path in [canonical.parent, *canonical.parent.parents, alias.parent, *alias.parent.parents]:
            if path == output or output in path.parents:
                path.chmod(0o700)


class Validator(unittest.TestCase):
    def test_exact_static_report_for_both_scales(self):
        for scale in [1.0, 1.25]:
            self.assertTrue(M.valid_app(fixture(scale), scale))
            self.assertFalse(M.valid_app(fixture(scale), 2.0))
        for value in [None, [], {}, {'status': 'passed'}]:
            self.assertFalse(M.valid_app(value, 1.0))

    def test_bool_counters_wrong_counts_and_claim_flags_refused(self):
        for key, count in M.COUNTERS.items():
            for new in [True, False, float(count), count + 1]:
                with self.subTest(key=key, new=new):
                    value = fixture(); value[key] = new
                    self.assertFalse(M.valid_app(value, 1.0))
        for key in M.TRUE_FLAGS + M.FALSE_FLAGS:
            for new in [0, 1, None, key in M.FALSE_FLAGS]:
                value = fixture(); value[key] = new
                self.assertFalse(M.valid_app(value, 1.0))
        for key, new in [('pid', True), ('pid', 0), ('pid', 4243), ('applicationScale', 1),
                         ('applicationScale', True), ('windowBackend', 'x11'), ('version', '0.2.0-rc.2')]:
            value = fixture(); value[key] = new
            self.assertFalse(M.valid_app(value, 1.0))

    def test_foreign_request_generation_serial_and_target_refused(self):
        for index in range(3):
            for request in ['PUBLIC_native_file_prompt_1', 'native-4243-1001-7-1',
                            f'native-4242-1001-8-{index + 1}', f'native-4242-0-7-{index + 1}',
                            f'native-4242-1001-7-4', 'native-4242-1001-7-1-extra']:
                value = fixture(); value['requests'][index] = request; value['phases'][index]['requestId'] = request
                self.assertFalse(M.valid_app(value, 1.0))
            for key, new in [('epoch', True), ('epoch', 0), ('generation', 8), ('serial', True),
                             ('serial', 4), ('target', 'foreign'), ('target', ''), ('target', '\0')]:
                value = fixture(); value['tickets'][index][key] = new
                self.assertFalse(M.valid_app(value, 1.0))
        value = fixture(); value['requests'][1] = value['requests'][0]
        self.assertFalse(M.valid_app(value, 1.0))
        for key in ['tickets', 'requests', 'phases']:
            value = fixture(); value[key].pop()
            self.assertFalse(M.valid_app(value, 1.0))

    def test_file_only_exact_metadata_digest_and_phase_scope(self):
        for index in range(3):
            for field, new in [('bytes', True), ('bytes', 17), ('name', 'foreign'),
                               ('attachmentId', 'sha256:' + '0' * 64), ('receiptId', 'private-forged')]:
                value = fixture(); value['phases'][index]['file'][field] = new
                self.assertFalse(M.valid_app(value, 1.0))
            for field, new in [('phase', True), ('requestId', 'foreign'), ('claimedFileMatched', False),
                               ('durableInboxFileMatched', False), ('zeroAdmittedModelSteps', 1)]:
                value = fixture(); value['phases'][index][field] = new
                self.assertFalse(M.valid_app(value, 1.0))
        value = fixture(); value['files'][1]['bytes'] = False  # Empty file must be integer zero.
        self.assertFalse(M.valid_app(value, 1.0))
        value = fixture(); value['files'][1]['name'] = 'different empty file'
        self.assertFalse(M.valid_app(value, 1.0))

    def test_zero_work_exact_counts_and_owned_shutdown(self):
        for key in M.PROMPT.ZERO + M.PROMPT.COUNTS:
            for group in ['host', 'phase']:
                value = fixture(); counts = value['host']['counts'] if group == 'host' else value['phases'][0]['counts']
                counts[key] = True
                self.assertFalse(M.valid_app(value, 1.0))
        for key in M.PROMPT.ZERO:
            value = fixture(); value['host']['counts'][key] = 1
            self.assertFalse(M.valid_app(value, 1.0))
        value = fixture(); value['host']['counts']['unknownWork'] = 0
        self.assertFalse(M.valid_app(value, 1.0))
        for key, new in [('exited', False), ('exitCode', True), ('graceful', False),
                         ('containmentUnknown', True), ('observedDescendantsRemaining', False),
                         ('observedDescendantsRemaining', 1)]:
            value = fixture(); value['stop'][key] = new
            self.assertFalse(M.valid_app(value, 1.0))


class IndependentEvidence(unittest.TestCase):
    def test_static_storage_raw_receipts_and_private_png_hashes(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp); app = fixture(); write_evidence(output, app)
            evidence = M.verify_evidence(output, app, 1.0)
            self.assertEqual(len(evidence['storage']), 3)
            self.assertEqual([v['artifact'] for v in evidence['captures']], M.CAPTURES)
            self.assertTrue(all(v['sha256'] == hashlib.sha256(png()).hexdigest() for v in evidence['captures']))
            self.assertFalse(evidence['paintInspected'])
            self.assertFalse(evidence['pngPixelsDecoded'])

    def test_raw_host_mismatches_missing_nonprivate_and_failed_evidence(self):
        for case in ['foreign-root', 'missing-phase', 'changed-phase', 'changed-complete', 'public-ready', 'failed']:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                output = Path(temp); app = fixture(); write_evidence(output, app)
                control = output / 'fixture/control'
                if case == 'foreign-root':
                    ready = M.private_json(control / 'ready.json'); ready['actualAgentId'] = 'foreign'
                    private_write(control / 'ready.json', json.dumps(ready).encode())
                elif case == 'missing-phase': (control / 'phase-2.json').unlink()
                elif case == 'changed-phase':
                    phase = copy.deepcopy(app['phases'][1]); phase['file']['name'] = 'foreign'
                    private_write(control / 'phase-2.json', json.dumps(phase).encode())
                elif case == 'changed-complete': private_write(control / 'complete.json', b'{"status":"failed"}')
                elif case == 'public-ready': (control / 'ready.json').chmod(0o644)
                else: private_write(control / 'failed.json', b'{}')
                with self.assertRaises((OSError, ValueError)): M.verify_evidence(output, app, 1.0)

    def test_capture_missing_mode_symlink_fifo_header_bounds_and_uid(self):
        for case in ['missing', 'public', 'symlink', 'fifo', 'empty', 'header', 'zero-dimension', 'large-dimension', 'oversize', 'uid']:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                output = Path(temp); app = fixture()
                for name in M.CAPTURES: private_write(output / name, png())
                path = output / M.CAPTURES[0]
                if case == 'missing': path.unlink()
                elif case == 'public': path.chmod(0o644)
                elif case == 'symlink': path.unlink(); path.symlink_to(output / M.CAPTURES[1])
                elif case == 'fifo': path.unlink(); os.mkfifo(path, 0o600)
                elif case == 'empty': private_write(path, b'')
                elif case == 'header': private_write(path, b'not a PNG' * 5)
                elif case in ['zero-dimension', 'large-dimension']:
                    data = bytearray(png()); data[16:20] = struct.pack('>I', 0 if case == 'zero-dimension' else 4097)
                    private_write(path, data)
                elif case == 'oversize':
                    with path.open('r+b') as stream: stream.truncate(M.MAX_PNG + 1)
                if case == 'uid':
                    with patch.object(M.os, 'getuid', return_value=os.getuid() + 1):
                        with self.assertRaises(ValueError): M.verify_captures(output, app)
                else:
                    with self.assertRaises((OSError, ValueError)): M.verify_captures(output, app)
        with tempfile.TemporaryDirectory() as temp:
            app = fixture(); app['captures'].reverse()
            with self.assertRaises(ValueError): M.verify_captures(Path(temp), app)

    def test_missing_changed_nonprivate_storage_and_alias_not_hardlinked(self):
        for case in ['missing', 'bytes', 'mode', 'alias']:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                output = Path(temp); app = fixture(); write_evidence(output, app)
                _, data, name = M.FIXTURES[0]; digest = hashlib.sha256(data).hexdigest()
                root = output / 'harness/attachments/v1'
                canonical = root / 'file-objects' / digest[:2] / digest
                alias = root / 'files' / digest[:2] / digest / name
                if case == 'missing': canonical.unlink()
                elif case == 'bytes': canonical.chmod(0o600); canonical.write_bytes(b'x' * len(data)); canonical.chmod(0o400)
                elif case == 'mode': canonical.chmod(0o600)
                else: alias.unlink(); alias.write_bytes(data); alias.chmod(0o400)
                with self.assertRaises((OSError, ValueError)): M.verify_evidence(output, app, 1.0)

    def test_refused_fixture_canonical_storage_is_not_allowed(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp); app = fixture(); write_evidence(output, app)
            digest = hashlib.sha256(M.PROMPT.STAGE.REFUSED).hexdigest()
            path = output / 'harness/attachments/v1/file-objects' / digest[:2] / digest
            path.parent.mkdir(mode=0o700, exist_ok=True)
            path.write_bytes(M.PROMPT.STAGE.REFUSED)
            path.chmod(0o400)
            with self.assertRaisesRegex(ValueError, 'refused-fixture-was-stored'):
                M.verify_evidence(output, app, 1.0)

    def test_command_retains_shared_owned_flags_and_creates_only_fixed_source_files(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp); (output / 'workspace').mkdir()
            command = M.command(output, 1.25)
            self.assertEqual(command[0], str(M.EXAMPLE))
            flags = dict(zip(command[1::2], command[2::2]))
            self.assertEqual(flags['--scale'], '1.25')
            self.assertEqual(flags['--expected-version'], '0.2.1-alpha.1')
            self.assertEqual(flags['--home'], str(output / 'harness'))
            self.assertEqual(flags['--cwd'], str(output / 'workspace'))
            self.assertEqual(flags['--user-home'], str(output / 'user'))
            expected = [(name, data) for _, data, name in M.FIXTURES] + [('PUBLIC refused.bin', M.PROMPT.STAGE.REFUSED)]
            self.assertEqual(sorted(path.name for path in (output / 'workspace').iterdir()), sorted(name for name, _ in expected))
            for name, data in expected:
                path = output / 'workspace' / name
                self.assertEqual(path.read_bytes(), data)
                self.assertEqual(path.stat().st_mode & 0o7777, 0o600)


class SharedRunner(unittest.IsolatedAsyncioTestCase):
    async def test_shared_ownership_runner_arguments_and_independent_summary_gate(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp); app = fixture(1.25); write_evidence(output, app)
            private_write(output / 'result.json', json.dumps({'status': 'passed', 'app': app}).encode())
            mock = AsyncMock(return_value=0)
            with patch.object(M.RUNNER, 'run', mock), patch('builtins.print'):
                self.assertEqual(await M.run(SimpleNamespace(output=output, scale=1.25)), 0)
            call = mock.call_args.kwargs
            self.assertEqual((call['script'], call['report_name'], call['report_key'], call['capture_stdout']),
                             (M.EXAMPLE, 'app.json', 'app', True))
            self.assertTrue(call['validator'](app))
            self.assertFalse(call['validator'](fixture(1.0)))
            summary = M.private_json(output / 'composed-files-qualification.json')
            self.assertEqual(summary['status'], 'passed')
            self.assertFalse(summary['historyDrivenReceiptRetirementQualified'])
            self.assertFalse(summary['nativeKeyboardPointerInputQualified'])

    async def test_invalid_or_missing_storage_and_owned_failure_cannot_pass(self):
        for case in ['missing-storage', 'missing-png', 'owned-failure', 'missing-result']:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                output = Path(temp); app = fixture(); write_evidence(output, app)
                if case != 'missing-result':
                    private_write(output / 'result.json', json.dumps({'status': 'failed' if case == 'owned-failure' else 'passed', 'app': app}).encode())
                if case == 'missing-storage':
                    digest = M.EXPECTED_REFS[0]['attachmentId'][7:]
                    (output / 'harness/attachments/v1/file-objects' / digest[:2] / digest).unlink()
                elif case == 'missing-png': (output / M.CAPTURES[1]).unlink()
                with patch.object(M.RUNNER, 'run', AsyncMock(return_value=1 if case == 'owned-failure' else 0)), patch('builtins.print'):
                    self.assertEqual(await M.run(SimpleNamespace(output=output, scale=1.0)), 1)
                self.assertEqual(M.private_json(output / 'composed-files-qualification.json')['status'], 'failed')


if __name__ == '__main__':
    unittest.main()
