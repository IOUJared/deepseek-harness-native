"""Headless registry-runner guards. Fake app/process reports only; never launches GUI/Core/Host."""
import asyncio
import binascii
import copy
import importlib.util
import json
import os
from pathlib import Path
import struct
import tempfile
import unittest
import zlib
from types import SimpleNamespace
from unittest.mock import AsyncMock, Mock, patch

SPEC = importlib.util.spec_from_file_location('registry_qualification_tests', Path(__file__).with_name('qualify-composed-registry.py'))
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


def app_report(pid=4242):
    data = {'status': 'passed', 'pid': pid, 'applicationScale': 1.0, 'windowBackend': 'wayland', 'modelPrompts': 0,
            'publicSessionIds': ['session-PUBLIC-FIRST', 'session-PUBLIC-SECOND'],
            'stop': {'exited': True, 'exitCode': 0, 'graceful': True,
                     'containmentUnknown': False, 'observedDescendantsRemaining': 0}}
    for key in ('publicFixtureOnly', 'scriptedUiMessages', 'realWorkerReady', 'freshEmptyBaselineObserved',
                'ownedWorkspaceCreated', 'twoDistinctPublicSessionsCreated', 'ownedWorkspaceFeedMembershipObserved',
                'actualArchivedViewEntered', 'restoredRecentViewVerified', 'restoreDidNotRepin',
                'archiveAndUnpinCoupledFeedVerified', 'allMatchingAcksAndIndependentFeedsVerified',
                'filesRetainedNotDeletionRollback'):
        data[key] = True
    for key in ('realCredentialsUsed', 'modelCatalogRequested', 'browserSignInRequested', 'apiKeySaveRequested',
                'stopActivityRequested', 'sessionDeletionRequested', 'forkOrRenameRequested',
                'manualBusinessMessagesForwarded', 'rpcSetsInstalled', 'nativeKeyboardPointerInputQualified',
                'liveHostDecisionQualified'):
        data[key] = False
    data['stages'] = []
    for index, (name, operation, target, pins, pin, archive) in enumerate(RUNNER.STAGES):
        data['stages'].append({'name': name, 'operation': operation, 'targetSlot': target, 'pinOrderSlots': list(pins),
            'matchingSubmissionAcknowledged': True, 'requiredFeedIncrementsObserved': True,
            'authoritativeRegistryVerified': True, 'actualNavigationVerified': True,
            'bothSessionsAndAccountingRetained': True, 'pinIncrementObserved': pin,
            'archiveIncrementObserved': archive, 'ticket': {'epoch': 1, 'generation': index + 3, 'serial': index + 5}})
    for key, _ in RUNNER.CAPTURES:
        data[key] = {'saved': True, 'physicalSize': [16, 16], 'manualInputProof': False}
    return data


PNG_SIGNATURE = b'\x89PNG\r\n\x1a\n'


def png_chunk(kind, payload=b''):
    return (struct.pack('>I', len(payload)) + kind + payload
            + struct.pack('>I', binascii.crc32(kind + payload) & 0xffffffff))


def png_ihdr(width=16, height=16, profile=(8, 6, 0, 0, 0)):
    return png_chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, *profile))


def tiny_png(width=16, height=16, rows=None):
    if rows is None:
        rows = (b'\x00' + b'\x19\x17\x24\xff' * width) * height
    return (PNG_SIGNATURE + png_ihdr(width, height)
            + png_chunk(b'IDAT', zlib.compress(rows)) + png_chunk(b'IEND'))


def captures(output):
    for _, name in RUNNER.CAPTURES:
        fd = os.open(output / name, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, 'wb') as stream:
            stream.write(tiny_png())


class Child:
    """Fake Popen resource; no wait() so unbounded child waiting fails immediately."""
    pid = 4242
    def __init__(self):
        self.returncode = None
        self.polls = 0
    def poll(self):
        self.polls += 1
        return self.returncode


CLEAN = {'rootWasRunningAtCleanup': False, 'observedCleanupComplete': True,
         'remainingBeforeExternalCleanup': [], 'remainingAfterExternalCleanup': []}


class RegistryRunnerGuards(unittest.TestCase):
    def test_ownership_helpers_are_imported_not_reimplemented(self):
        for name in ('pin_root', 'identity', 'observe_descendants', 'cleanup_owned', 'private_json'):
            self.assertIs(getattr(RUNNER, name), getattr(RUNNER.OWNER, name))
        self.assertIs(RUNNER.NATIVE, RUNNER.OWNER.NATIVE)

    def test_expected_ack_feed_order_and_restore_report_is_accepted(self):
        self.assertTrue(RUNNER.valid_app(app_report(), 4242, 1.0))

    def test_ack_only_or_missing_archive_unpin_coupling_cannot_pass(self):
        for index, field in [(0, 'pinIncrementObserved'), (4, 'pinIncrementObserved'),
                             (4, 'archiveIncrementObserved'), (5, 'archiveIncrementObserved'),
                             (1, 'matchingSubmissionAcknowledged'), (5, 'authoritativeRegistryVerified')]:
            data = app_report()
            data['stages'][index][field] = False
            self.assertFalse(RUNNER.valid_app(data, 4242, 1.0))

    def test_exact_host_order_not_set_membership_is_required(self):
        for index in (1, 3):
            data = app_report()
            data['stages'][index]['pinOrderSlots'].reverse()
            self.assertFalse(RUNNER.valid_app(data, 4242, 1.0))
        data = app_report()
        data['stages'][5]['pinOrderSlots'] = [2, 1]
        self.assertFalse(RUNNER.valid_app(data, 4242, 1.0))

    def test_wrong_root_scale_epoch_or_replayed_ticket_cannot_pass(self):
        for key, value in [('pid', 99), ('applicationScale', 1.25), ('windowBackend', 'x11')]:
            data = app_report()
            data[key] = value
            self.assertFalse(RUNNER.valid_app(data, 4242, 1.0))
        for key, value in [('epoch', 2), ('serial', 5), ('generation', True)]:
            data = app_report()
            data['stages'][1]['ticket'][key] = value
            self.assertFalse(RUNNER.valid_app(data, 4242, 1.0))

    def test_forbidden_authority_or_false_native_input_claim_cannot_pass(self):
        for key in ('realCredentialsUsed', 'modelCatalogRequested', 'apiKeySaveRequested',
                    'stopActivityRequested', 'sessionDeletionRequested', 'forkOrRenameRequested',
                    'rpcSetsInstalled', 'manualBusinessMessagesForwarded', 'nativeKeyboardPointerInputQualified'):
            data = app_report()
            data[key] = True
            self.assertFalse(RUNNER.valid_app(data, 4242, 1.0))

    def test_private_report_is_bounded_and_never_follows_symlink(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            RUNNER.private_json(output / 'app.json', app_report())
            self.assertEqual(RUNNER.read_app(output)['status'], 'passed')
            with self.assertRaises(FileExistsError):
                RUNNER.private_json(output / 'app.json', {'status': 'replaced'})
            (output / 'other').mkdir()
            (output / 'other/app.json').symlink_to(output / 'app.json')
            self.assertNotEqual(RUNNER.read_app(output / 'other')['status'], 'passed')
            (output / 'large').mkdir()
            with (output / 'large/app.json').open('wb') as stream:
                stream.write(b'X' * (RUNNER.MAX_REPORT_BYTES + 1))
            (output / 'large/app.json').chmod(0o600)
            self.assertNotEqual(RUNNER.read_app(output / 'large')['status'], 'passed')

    def test_complete_rgba_png_accepts_filters_split_idat_and_ancillary_chunks(self):
        rows = b''.join(bytes([filter_tag]) + b'\x19\x17\x24\xff' for filter_tag in range(5))
        compressed = zlib.compress(rows)
        image = (PNG_SIGNATURE + png_ihdr(1, 5) + png_chunk(b'tEXt', b'Public\x00fixture')
                 + png_chunk(b'PLTE', b'\x19\x17\x24')
                 + b''.join(png_chunk(b'IDAT', bytes([byte])) for byte in compressed)
                 + png_chunk(b'tEXt', b'Public\x00complete') + png_chunk(b'IEND'))
        self.assertEqual(RUNNER.png_dimensions(image), (1, 5))
        self.assertEqual(RUNNER.png_dimensions(tiny_png()), (16, 16))

    def test_truncated_prefix_and_crc_corruption_cannot_verify_capture(self):
        image = tiny_png()
        for cut in range(len(image)):
            self.assertIsNone(RUNNER.png_dimensions(image[:cut]))
        for index in (16, 29, len(image) - 1):
            damaged = bytearray(image)
            damaged[index] ^= 1
            self.assertIsNone(RUNNER.png_dimensions(damaged))
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            captures(output)
            # The previously accepted 24-byte prefix is not a complete PNG.
            (output / 'archive-review.png').write_bytes(image[:24])
            self.assertFalse(RUNNER.valid_captures(output, app_report()))

    def test_png_requires_ihdr_idat_iend_order_and_exact_end(self):
        header, pixels, end = png_ihdr(), png_chunk(b'IDAT', zlib.compress(b'\x00' * 1040)), png_chunk(b'IEND')
        for parts in [header + end, header + pixels, pixels + header + end,
                      header + header + pixels + end, header + pixels + end + b'extra',
                      header + pixels + png_chunk(b'IEND', b'extra'),
                      header + png_chunk(b'ABCD') + pixels + end,
                      header + pixels + png_chunk(b'PLTE', b'\x00\x00\x00') + end]:
            self.assertIsNone(RUNNER.png_dimensions(PNG_SIGNATURE + parts))
        compressed = zlib.compress(b'\x00' * 1040)
        split = len(compressed) // 2
        image = (PNG_SIGNATURE + header + png_chunk(b'IDAT', compressed[:split])
                 + png_chunk(b'tEXt', b'Public\x00interrupted')
                 + png_chunk(b'IDAT', compressed[split:]) + end)
        self.assertIsNone(RUNNER.png_dimensions(image))

    def test_png_rejects_unsupported_profiles_and_dimension_allocation_bounds(self):
        for width, height, profile in [(0, 1, (8, 6, 0, 0, 0)), (1, 0, (8, 6, 0, 0, 0)),
                                      (0xffffffff, 0xffffffff, (8, 6, 0, 0, 0)),
                                      (1, 1, (16, 6, 0, 0, 0)), (1, 1, (8, 2, 0, 0, 0)),
                                      (1, 1, (8, 6, 1, 0, 0)), (1, 1, (8, 6, 0, 1, 0)),
                                      (1, 1, (8, 6, 0, 0, 1))]:
            image = PNG_SIGNATURE + png_ihdr(width, height, profile) + png_chunk(b'IEND')
            with patch.object(RUNNER.zlib, 'decompressobj') as inflate:
                self.assertIsNone(RUNNER.png_dimensions(image))
                inflate.assert_not_called()
        image = tiny_png(1, 1)
        with patch.object(RUNNER, 'MAX_PNG_BYTES', len(image) - 1):
            self.assertIsNone(RUNNER.png_dimensions(image))
        with patch.object(RUNNER, 'MAX_PNG_BYTES', len(image)):
            self.assertEqual(RUNNER.png_dimensions(image), (1, 1))

    def test_png_inflation_requires_exact_rows_filters_eof_and_no_extra_stream(self):
        for rows in (b'\x00' * 4, b'\x00' * 6, b'\x05' + b'\x00' * 4):
            self.assertIsNone(RUNNER.png_dimensions(tiny_png(1, 1, rows)))
        compressed = zlib.compress(b'\x00' * 5)
        for payload in (b'not-zlib', compressed[:-1], compressed + b'extra',
                        compressed + zlib.compress(b'\x00' * 5)):
            image = PNG_SIGNATURE + png_ihdr(1, 1) + png_chunk(b'IDAT', payload) + png_chunk(b'IEND')
            self.assertIsNone(RUNNER.png_dimensions(image))

    def test_png_decompression_bomb_is_stopped_with_explicit_output_budget(self):
        payload = zlib.compress(b'\x00' * 1_000_000)
        image = PNG_SIGNATURE + png_ihdr(1, 1) + png_chunk(b'IDAT', payload) + png_chunk(b'IEND')
        self.assertIsNone(RUNNER.png_dimensions(image))
        calls = []
        real_factory = zlib.decompressobj
        class BudgetedInflater:
            def __init__(self):
                self.inner = real_factory()
            def decompress(self, chunk, maximum):
                calls.append(maximum)
                return self.inner.decompress(chunk, maximum)
            def __getattr__(self, name):
                return getattr(self.inner, name)
            def flush(self, *args):
                raise AssertionError('unbounded inflater flush is forbidden')
        with patch.object(RUNNER.zlib, 'decompressobj', BudgetedInflater):
            self.assertIsNone(RUNNER.png_dimensions(image))
        self.assertEqual(calls, [6])

    def test_both_private_own_renderer_captures_required(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            captures(output)
            data = app_report()
            self.assertTrue(RUNNER.valid_captures(output, data))
            data['archiveReviewCapture']['physicalSize'] = [17, 16]
            self.assertFalse(RUNNER.valid_captures(output, data))
            data = app_report()
            (output / 'restore-review.png').chmod(0o644)
            self.assertFalse(RUNNER.valid_captures(output, data))

    def test_new_output_and_clean_environment_reuse_existing_guard(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            (base / 'app/evidence').mkdir(parents=True)
            with patch.object(RUNNER.NATIVE, 'BASE', base):
                output = RUNNER.NATIVE.prepare(base / 'app/evidence/registry')
                self.assertEqual(output.stat().st_mode & 0o777, 0o700)
                with self.assertRaises(ValueError):
                    RUNNER.NATIVE.prepare(output)
                with self.assertRaises(ValueError):
                    RUNNER.NATIVE.prepare(base / 'foreign')
            with patch.dict(os.environ, {'DEEPSEEK_API_KEY': 'PUBLIC_FORBIDDEN', 'ANTHROPIC_API_KEY': 'PUBLIC_FORBIDDEN'}):
                env = RUNNER.NATIVE.environment(output)
                self.assertNotIn('DEEPSEEK_API_KEY', env)
                self.assertNotIn('ANTHROPIC_API_KEY', env)
                self.assertEqual(env['HOME'], str(output / 'user'))

    def test_run_uses_owned_pin_before_poll_and_imported_cleanup(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            binary = output / 'app/target/release/examples/registry_composed_smoke'
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b'PUBLIC NONEXECUTABLE FIXTURE')
            RUNNER.private_json(output / 'app.json', app_report())
            captures(output)
            child = Child()
            def pin(resource):
                self.assertIs(resource, child)
                self.assertEqual(child.polls, 0)
                return {'pid': child.pid, 'start_ticks': 100}, 81
            async def window(*args):
                child.returncode = 0
                return {'pid': child.pid, 'mapped': True, 'class': RUNNER.APP_ID, 'xwayland': False}
            cleanup = AsyncMock(return_value=copy.deepcopy(CLEAN))
            actual_close = RUNNER.os.close
            def close_owned(fd):
                if fd != 81:
                    actual_close(fd)
            with patch.object(RUNNER, 'BASE', output), \
                    patch.object(RUNNER.NATIVE, 'prepare', return_value=output), \
                    patch.object(RUNNER, 'pin_root', side_effect=pin), \
                    patch.object(RUNNER, 'observe_descendants', return_value=0), \
                    patch.object(RUNNER, 'native_window', side_effect=window), \
                    patch.object(RUNNER, 'cleanup_owned', cleanup), \
                    patch.object(RUNNER.subprocess, 'Popen', return_value=child) as spawn, \
                    patch.object(RUNNER.os, 'close', side_effect=close_owned) as close:
                self.assertEqual(asyncio.run(RUNNER.run(SimpleNamespace(output=str(output), scale=1.0))), 0)
            cleanup.assert_awaited_once_with(child, {(4242, 100): 81}, True)
            close.assert_any_call(81)
            self.assertEqual(sum(call.args == (81,) for call in close.call_args_list), 1)
            self.assertEqual(spawn.call_args.args[0][-4:], ['--output', str(output), '--scale', '1.0'])
            report = json.loads((output / 'result.json').read_text())
            self.assertFalse(report['hardTotalWallClockBoundClaimed'])
            self.assertFalse(report['paintInspected'])

    def test_owned_probe_filters_only_fixture_pid_and_cleans_its_pidfd(self):
        probe = Child()
        probe.stdout = SimpleNamespace(fileno=lambda: 777, close=Mock())
        def exited():
            probe.returncode = 0
            return 0
        probe.poll = exited
        raw = json.dumps([{'pid': 99, 'class': RUNNER.APP_ID, 'mapped': True, 'xwayland': False},
                          {'pid': 888, 'title': 'PUBLIC OTHER WINDOW MUST NOT ENTER REPORT'}]).encode()
        cleanup = AsyncMock(return_value=copy.deepcopy(CLEAN))
        actual_close = RUNNER.os.close
        def close_owned(fd):
            if fd != 81:
                actual_close(fd)
        with patch.object(RUNNER.subprocess, 'Popen', return_value=probe) as spawn, \
                patch.object(RUNNER, 'pin_root', return_value=({'pid': 4242, 'start_ticks': 100}, 81)), \
                patch.object(RUNNER, 'observe_descendants', return_value=0), \
                patch.object(RUNNER.os, 'set_blocking'), \
                patch.object(RUNNER.os, 'read', side_effect=[raw, b'']), \
                patch.object(RUNNER, 'cleanup_owned', cleanup), \
                patch.object(RUNNER.os, 'close', side_effect=close_owned):
            facts = asyncio.run(RUNNER.native_window(99, {'PATH': '/PUBLIC'}, .1))
        self.assertEqual(facts['pid'], 99)
        self.assertNotIn('title', facts)
        cleanup.assert_awaited_once_with(probe, {(4242, 100): 81}, True)
        probe.stdout.close.assert_called_once()
        self.assertEqual(spawn.call_args.args[0], ['hyprctl', '-j', 'clients'])

    def test_probe_cancellation_still_runs_owned_bounded_cleanup(self):
        probe = Child()
        probe.stdout = SimpleNamespace(fileno=lambda: 777, close=Mock())
        cleanup = AsyncMock(return_value=copy.deepcopy(CLEAN))
        actual_close = RUNNER.os.close
        def close_owned(fd):
            if fd != 81:
                actual_close(fd)
        with patch.object(RUNNER.subprocess, 'Popen', return_value=probe), \
                patch.object(RUNNER, 'pin_root', return_value=({'pid': 4242, 'start_ticks': 100}, 81)), \
                patch.object(RUNNER, 'observe_descendants', return_value=0), \
                patch.object(RUNNER.os, 'set_blocking'), \
                patch.object(RUNNER.os, 'read', side_effect=asyncio.CancelledError), \
                patch.object(RUNNER, 'cleanup_owned', cleanup), \
                patch.object(RUNNER.os, 'close', side_effect=close_owned):
            with self.assertRaises(asyncio.CancelledError):
                asyncio.run(RUNNER.native_window(99, {}, .1))
        cleanup.assert_awaited_once_with(probe, {(4242, 100): 81}, True)
        probe.stdout.close.assert_called_once()

    def test_pin_failure_still_uses_imported_popen_cleanup_and_cannot_pass(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp)
            binary = output / 'app/target/release/examples/registry_composed_smoke'
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b'PUBLIC NONEXECUTABLE FIXTURE')
            child = Child()
            async def cleanup(resource, pidfds, pinned):
                self.assertIs(resource, child)
                self.assertFalse(pinned)
                child.returncode = -15
                return {**CLEAN, 'rootWasRunningAtCleanup': True, 'observedCleanupComplete': False}
            with patch.object(RUNNER, 'BASE', output), \
                    patch.object(RUNNER.NATIVE, 'prepare', return_value=output), \
                    patch.object(RUNNER, 'pin_root', side_effect=RuntimeError('PUBLIC pin failure')), \
                    patch.object(RUNNER, 'cleanup_owned', side_effect=cleanup) as cleaned, \
                    patch.object(RUNNER.subprocess, 'Popen', return_value=child):
                self.assertEqual(asyncio.run(RUNNER.run(SimpleNamespace(output=str(output), scale=1.0))), 1)
            cleaned.assert_awaited_once()
            self.assertEqual(json.loads((output / 'result.json').read_text())['failureCode'], 'root-pidfd-acquisition-failed')


if __name__ == '__main__':
    unittest.main()
