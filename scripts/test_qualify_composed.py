import asyncio
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import AsyncMock, patch

SPEC = importlib.util.spec_from_file_location('composed_qualification', Path(__file__).with_name('qualify-composed-settings.py'))
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


class Child:
    """No wait() method: tests fail if cleanup regresses to an unbounded child wait."""
    def __init__(self, returncode=None):
        self.pid = 4242
        self.returncode = returncode
        self.polls = 0
        self.terminated = 0
        self.killed = 0

    def poll(self):
        self.polls += 1
        return self.returncode

    def terminate(self):
        self.terminated += 1

    def kill(self):
        self.killed += 1


def stat(pid=4242, start=100, parent=None):
    return {'pid': pid, 'start_ticks': start, 'ppid': os.getpid() if parent is None else parent,
            'pgrp': pid, 'cpu_ticks': 0}


class ComposedQualificationTests(unittest.TestCase):
    def test_new_private_report_never_overwrites(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'result.json'
            RUNNER.private_json(path, {'publicFixtureOnly': True})
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            with self.assertRaises(FileExistsError):
                RUNNER.private_json(path, {'replaced': True})
            self.assertEqual(json.loads(path.read_text()), {'publicFixtureOnly': True})

    def test_owned_output_preparation_rejects_existing_and_unrelated_paths(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            (base / 'app/evidence').mkdir(parents=True)
            with patch.object(RUNNER.NATIVE, 'BASE', base):
                output = RUNNER.NATIVE.prepare(base / 'app/evidence/new-composed-run')
                self.assertEqual((output / 'harness').stat().st_mode & 0o777, 0o700)
                with self.assertRaises(ValueError):
                    RUNNER.NATIVE.prepare(output)
                with self.assertRaises(ValueError):
                    RUNNER.NATIVE.prepare(base / 'unrelated')

    def test_root_pin_uses_exact_schema_and_precedes_reaping(self):
        child = Child()
        root = stat()
        with patch.object(RUNNER.METRICS, 'proc_stat', return_value=root), \
                patch.object(RUNNER.os, 'pidfd_open', return_value=81) as opened:
            self.assertEqual(RUNNER.pin_root(child), (root, 81))
            opened.assert_called_once_with(child.pid)
            self.assertEqual(child.polls, 0)

    def test_root_identity_change_rejects_and_closes_pin(self):
        child = Child()
        with patch.object(RUNNER.METRICS, 'proc_stat', side_effect=[stat(), stat(start=101)]), \
                patch.object(RUNNER.os, 'pidfd_open', return_value=81), \
                patch.object(RUNNER.os, 'close') as close:
            with self.assertRaisesRegex(RuntimeError, 'identity-changed'):
                RUNNER.pin_root(child)
            close.assert_called_once_with(81)
            self.assertEqual(child.polls, 0)

    def test_foreign_parent_or_already_reaped_root_never_opens_pidfd(self):
        for child, current in [(Child(), stat(parent=os.getpid() + 1)), (Child(0), stat())]:
            with patch.object(RUNNER.METRICS, 'proc_stat', return_value=current), \
                    patch.object(RUNNER.os, 'pidfd_open') as opened:
                with self.assertRaises(RuntimeError):
                    RUNNER.pin_root(child)
                opened.assert_not_called()

    def test_reused_ancestor_identity_cannot_admit_foreign_descendant(self):
        child = Child()
        root = stat()
        parent = stat(5001, 200, child.pid)
        descendant = stat(7001, 300, 5001)
        current = {child.pid: root, 5001: stat(5001, 201, child.pid), 7001: descendant}
        fds = {RUNNER.identity(root): 81}
        with patch.object(RUNNER.METRICS, 'process_tree', return_value=[root, parent, descendant]), \
                patch.object(RUNNER.METRICS, 'proc_stat', side_effect=current.get), \
                patch.object(RUNNER.os, 'pidfd_open') as opened:
            failures = RUNNER.observe_descendants(child, root, fds, set(fds))
            self.assertEqual(failures, 2)
            opened.assert_not_called()
            self.assertEqual(fds, {RUNNER.identity(root): 81})


class CleanupTests(unittest.IsolatedAsyncioTestCase):
    async def test_exited_GUI_root_still_escalates_TERM_ignoring_owned_descendant(self):
        child = Child(0)
        key = (5001, 200)
        dead = set()
        signals = []

        def send(fd, sig):
            signals.append((fd, sig))
            if sig == signal.SIGKILL:
                dead.add(fd)

        def ready(reads, *_):
            return [fd for fd in reads if fd in dead], [], []

        with patch.object(RUNNER.select, 'select', side_effect=ready), \
                patch.object(RUNNER.signal, 'pidfd_send_signal', side_effect=send):
            result = await asyncio.wait_for(RUNNER.cleanup_owned(child, {key: 81}, True,
                term_seconds=.002, kill_seconds=.002), .3)
        self.assertEqual(signals, [(81, signal.SIGTERM), (81, signal.SIGKILL)])
        self.assertEqual(result['remainingBeforeExternalCleanup'], [key])
        self.assertEqual(result['remainingAfterExternalCleanup'], [])
        self.assertTrue(result['killEscalated'])
        self.assertTrue(result['observedCleanupComplete'])
        self.assertFalse(result['rootWasRunningAtCleanup'])
        self.assertEqual((child.terminated, child.killed), (0, 0))

    async def test_hung_pinned_root_and_wait_are_finite_without_numeric_signalling(self):
        child = Child()
        key = (child.pid, 100)
        with patch.object(RUNNER.select, 'select', return_value=([], [], [])), \
                patch.object(RUNNER.signal, 'pidfd_send_signal') as send:
            result = await asyncio.wait_for(RUNNER.cleanup_owned(child, {key: 81}, True,
                term_seconds=.002, kill_seconds=.002), .3)
        self.assertEqual(send.call_args_list[0].args, (81, signal.SIGTERM))
        self.assertEqual(send.call_args_list[1].args, (81, signal.SIGKILL))
        self.assertTrue(result['cleanupTimedOut'])
        self.assertFalse(result['rootReaped'])
        self.assertFalse(result['observedCleanupComplete'])
        self.assertEqual((child.terminated, child.killed), (0, 0))

    async def test_probe_and_signal_errors_are_fixed_flags_not_false_clean(self):
        with patch.object(RUNNER.select, 'select', side_effect=OSError('private detail')), \
                patch.object(RUNNER.signal, 'pidfd_send_signal', side_effect=PermissionError('private detail')):
            result = await asyncio.wait_for(RUNNER.cleanup_owned(Child(0), {(5001, 200): 81}, True,
                term_seconds=.002, kill_seconds=.002), .3)
        self.assertTrue(result['cleanupProbeOrSignalFailed'])
        self.assertFalse(result['observedCleanupComplete'])
        self.assertNotIn('private detail', json.dumps(result))

    async def test_root_pin_failure_prevents_loop_and_hung_child_cleanup_is_finite(self):
        child = Child()  # Even the owned-resource KILL cannot make this fake exit.
        cleanup_owned = RUNNER.cleanup_owned

        async def short_cleanup(proc, fds, pinned):
            return await cleanup_owned(proc, fds, pinned, term_seconds=.002, kill_seconds=.002)

        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            binary = base / 'app/target/release/examples/settings_composed_smoke'
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b'PUBLIC_MOCK_BINARY_NOT_EXECUTED')
            output = base / 'output'
            output.mkdir()
            with patch.object(RUNNER, 'BASE', base), \
                    patch.object(RUNNER.NATIVE, 'prepare', return_value=output), \
                    patch.object(RUNNER.subprocess, 'Popen', return_value=child), \
                    patch.object(RUNNER.METRICS, 'proc_stat', return_value=stat()), \
                    patch.object(RUNNER.os, 'pidfd_open', side_effect=OSError('private pin detail')), \
                    patch.object(RUNNER, 'observe_descendants') as observe, \
                    patch.object(RUNNER.METRICS, 'native_window', new_callable=AsyncMock) as probe, \
                    patch.object(RUNNER, 'cleanup_owned', side_effect=short_cleanup), \
                    patch.object(RUNNER.signal, 'pidfd_send_signal') as send, \
                    patch('builtins.print'):
                status = await asyncio.wait_for(RUNNER.run(SimpleNamespace(output=output, scale=1.0)), .3)
            result = json.loads((output / 'result.json').read_text())
        self.assertEqual(status, 1)
        self.assertEqual(result['failureCode'], 'root-pidfd-acquisition-failed')
        self.assertTrue(result['forced'])
        self.assertTrue(result['cleanup']['ownedChildFallbackUsed'])
        self.assertTrue(result['cleanup']['cleanupTimedOut'])
        self.assertFalse(result['cleanup']['rootPidfdPinned'])
        self.assertFalse(result['cleanup']['observedCleanupComplete'])
        self.assertEqual((child.terminated, child.killed), (1, 1))
        observe.assert_not_called()
        probe.assert_not_awaited()
        send.assert_not_called()
        self.assertNotIn('private pin detail', json.dumps(result))

    @unittest.skipUnless(hasattr(os, 'pidfd_open') and hasattr(signal, 'pidfd_send_signal'), 'Linux pidfds required')
    async def test_real_headless_exited_root_and_TERM_ignoring_python_descendant(self):
        # Root cannot fork until pinned. On unsuccessful setup, root's own Child resource
        # kills its helper; only the intentional normal exit leaves it for our pidfd cleanup.
        code = '''import signal, subprocess, sys
sys.stdin.readline()
p = subprocess.Popen([sys.executable, '-c', "import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); print('ready',flush=True); time.sleep(60)"], stdout=subprocess.PIPE)
def stop(*args):
    p.kill()
    raise SystemExit(1)
signal.signal(signal.SIGTERM, stop)
p.stdout.readline()
print(p.pid, flush=True)
if sys.stdin.readline().strip() != 'exit-root-only':
    p.kill()
'''
        child = subprocess.Popen([sys.executable, '-c', code], stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        fds = {}
        try:
            try:
                root, fd = RUNNER.pin_root(child)
            except OSError as error:
                self.skipTest(f'pidfd unavailable: {error.errno}')
            fds[RUNNER.identity(root)] = fd
            child.stdin.write(b'start\n')
            child.stdin.flush()
            loop = asyncio.get_running_loop()
            deadline = loop.time() + 5
            raw = b''
            while b'\n' not in raw and loop.time() < deadline:
                if RUNNER.select.select([child.stdout], [], [], 0)[0]:
                    chunk = os.read(child.stdout.fileno(), 128)
                    if not chunk:
                        break
                    raw += chunk
                else:
                    await asyncio.sleep(.01)
            self.assertTrue(raw.strip().isdigit(), 'public helper PID ready message required')
            helper = int(raw)
            failures = RUNNER.observe_descendants(child, root, fds, set(fds))
            self.assertEqual(failures, 0)
            self.assertTrue(any(key[0] == helper for key in fds))
            child.stdin.write(b'exit-root-only\n')
            child.stdin.flush()
            await RUNNER.wait_quiescent(child, {}, 1)
            self.assertEqual(child.returncode, 0)
            result = await asyncio.wait_for(RUNNER.cleanup_owned(child, fds, True,
                term_seconds=.02, kill_seconds=1), 2)
            self.assertTrue(result['killEscalated'])
            self.assertTrue(result['observedCleanupComplete'])
            self.assertEqual(result['remainingAfterExternalCleanup'], [])
        finally:
            await RUNNER.cleanup_owned(child, fds, bool(fds), term_seconds=1, kill_seconds=1)
            for fd in fds.values():
                os.close(fd)
            child.stdin.close()
            child.stdout.close()


if __name__ == '__main__':
    unittest.main()
