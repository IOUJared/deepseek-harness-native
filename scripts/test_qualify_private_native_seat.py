"""Headless guard tests only: no compositor, native app, libei sender or backend launch.
New /tmp PUBLIC private manifest/socket scratch is intentionally retained; no deletes.
"""
import contextlib
import copy
import importlib.util
import io
import os
from pathlib import Path
import socket
import stat
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('private_native_seat_guard', Path(__file__).with_name('qualify-private-native-seat.py'))
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


def ready():
    return {'version': 1, 'publicOnly': True, 'phase': 'keyboard-ready', 'pid': 123456,
            'nativeWayland': True, 'waylandDisplay': 'gamescope-0', 'composerEmpty': True,
            'promptCommands': 0, 'windowWidth': 1024, 'windowHeight': 768,
            'pointerFocusX': 750, 'pointerFocusY': 680}


def result():
    return {'version': 1, 'publicOnly': True, 'phase': 'complete', 'pid': 123456,
            'actualProductionEditor': True, 'actualWaylandKeyboardEvents': True,
            'body': RUNNER.TEXT, 'keyPressed': len(RUNNER.TEXT), 'promptCommands': 0,
            'manualSendAllowed': False, 'stop': RUNNER.STOP.copy()}


class SchemaTests(unittest.TestCase):
    def test_ready_exact_public_schema(self):
        self.assertTrue(RUNNER.valid_ready(ready()))

    def test_result_requires_actual_exact_native_observation(self):
        self.assertTrue(RUNNER.valid_result(result(), 123456))


def schema_case(kind, key, value):
    def check(self):
        item = ready() if kind == 'ready' else result()
        item[key] = copy.deepcopy(value)
        accepted = RUNNER.valid_ready(item) if kind == 'ready' else RUNNER.valid_result(item, 123456)
        self.assertFalse(accepted)
    return check


# Original 22 source-plane checks are retained as individually named durable tests.
CASES = [('ready', 'version', True), ('ready', 'publicOnly', False), ('ready', 'pid', True),
         ('ready', 'nativeWayland', False), ('ready', 'waylandDisplay', '/run/user/1000/wayland-1'),
         ('ready', 'composerEmpty', False), ('ready', 'promptCommands', False), ('ready', 'promptCommands', 1),
         ('ready', 'pointerFocusX', 1024), ('ready', 'pointerFocusY', -1), ('ready', 'windowWidth', 5000),
         ('result', 'version', True), ('result', 'pid', True), ('result', 'actualProductionEditor', False),
         ('result', 'actualWaylandKeyboardEvents', False), ('result', 'body', 'public forged'),
         ('result', 'keyPressed', 0), ('result', 'promptCommands', 1), ('result', 'manualSendAllowed', True),
         ('result', 'stop', {'exited': True})]
for index, (kind, key, value) in enumerate(CASES):
    setattr(SchemaTests, f'test_refuse_{kind}_{key}_{index:02}', schema_case(kind, key, value))


class ShortRuntimeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.output = Path(tempfile.mkdtemp(prefix='PUBLIC-native-seat-test-evidence-', dir='/tmp')).resolve()
        os.chmod(cls.output, 0o700)
        cls.scratch, cls.runtime, cls.manifest = RUNNER.prepare_short_runtime(cls.output)

    def test_manifest_exact_fields_modes_and_runtime_identity(self):
        recorded = RUNNER.ART.private_json(self.output / 'runtime-manifest.json')
        self.assertEqual(recorded, self.manifest)
        self.assertEqual(set(recorded), {'version', 'publicOnly', 'runtimePath', 'runtimeStamp', 'scratchPath', 'evidenceRoot'})
        self.assertTrue(RUNNER.valid_runtime_manifest(recorded, self.output))
        self.assertEqual(stat.S_IMODE((self.output / 'runtime-manifest.json').stat().st_mode), 0o600)
        for directory in (self.output, self.scratch, self.runtime):
            self.assertEqual(stat.S_IMODE(directory.lstat().st_mode), 0o700)
            self.assertEqual(directory.lstat().st_uid, os.getuid())

    def test_manifest_refuses_each_changed_identity_and_schema_field(self):
        changes = [('version', True), ('publicOnly', False), ('scratchPath', '/tmp'),
                   ('runtimePath', str(self.output)), ('evidenceRoot', '/tmp'),
                   ('runtimeStamp', {'device': 0, 'inode': 0, 'uid': os.getuid()})]
        for key, value in changes:
            with self.subTest(key=key):
                changed = copy.deepcopy(self.manifest)
                changed[key] = value
                self.assertFalse(RUNNER.valid_runtime_manifest(changed, self.output))
        for field in ('device', 'inode', 'uid'):
            changed = copy.deepcopy(self.manifest)
            changed['runtimeStamp'][field] = True
            self.assertFalse(RUNNER.valid_runtime_manifest(changed, self.output))
        changed = copy.deepcopy(self.manifest)
        changed['unexpected'] = 'public'
        self.assertFalse(RUNNER.valid_runtime_manifest(changed, self.output))

    def test_runtime_directory_permissions_are_rechecked(self):
        os.chmod(self.runtime, 0o755)
        try:
            self.assertFalse(RUNNER.valid_runtime_manifest(self.manifest, self.output))
        finally:
            os.chmod(self.runtime, 0o700)
        self.assertTrue(RUNNER.valid_runtime_manifest(self.manifest, self.output))

    def test_actual_linux_unix_socket_binds_short_owned_path(self):
        path = RUNNER.private_socket_path(self.runtime, 'gamescope-127')
        self.assertLess(len(os.fsencode(path)), 108)
        old_umask = os.umask(0o077)
        try:
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as owned_socket:
                owned_socket.bind(str(path))
                info = path.lstat()
                self.assertTrue(stat.S_ISSOCK(info.st_mode))
                self.assertEqual(info.st_uid, os.getuid())
                self.assertEqual(str(path.parent), self.manifest['runtimePath'])
        finally:
            os.umask(old_umask)
        # Keep the socket pathname and its0700 parent; never remove active runtime trees.

    def test_long_and_non_ascii_socket_paths_use_byte_length(self):
        for parent in (Path('/tmp') / ('x' * 108), Path('/tmp') / ('é' * 54)):
            with self.assertRaisesRegex(ValueError, 'path-too-long'):
                RUNNER.private_socket_path(parent, 'gamescope-0')

    def test_socket_names_never_allow_ambient_display_or_traversal(self):
        for display in ('wayland-1', '/run/user/1000/wayland-1', '../gamescope-0', 'gamescope-1000', None):
            with self.subTest(display=display), self.assertRaises(ValueError):
                RUNNER.private_socket_path(self.runtime, display)

    def test_runtime_manifest_never_overwrites(self):
        with self.assertRaises(FileExistsError):
            RUNNER.OWN.private_json(self.output / 'runtime-manifest.json', {'publicOnly': False})
        self.assertEqual(RUNNER.ART.private_json(self.output / 'runtime-manifest.json'), self.manifest)

    def test_default_plan_has_no_launch_read_or_scratch_creation(self):
        argv = ['private-seat-plan', '--fixture', '/PUBLIC-not-read', '--fixture-sha256', '0' * 64,
                '--sender', '/PUBLIC-not-read', '--sender-sha256', '0' * 64,
                '--runtime', '/PUBLIC-not-read', '--output', '/PUBLIC-not-created']
        with (patch('sys.argv', argv), patch.object(RUNNER, 'executable') as read,
              patch.object(RUNNER.subprocess, 'Popen') as launch,
              patch.object(RUNNER.tempfile, 'mkdtemp') as create,
              contextlib.redirect_stdout(io.StringIO()) as stream):
            self.assertEqual(RUNNER.main(), 0)
            read.assert_not_called()
            launch.assert_not_called()
            create.assert_not_called()
        self.assertIn('planned-not-launched', stream.getvalue())


if __name__ == '__main__':
    unittest.main()
