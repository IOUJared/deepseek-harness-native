import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('dialog_qualification', Path(__file__).with_name('qualify-dialog-components.py'))
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


class DialogQualificationTests(unittest.TestCase):
    def test_report_is_private_and_never_overwrites(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / 'result.json'
            RUNNER.private_json(path, {'scope': 'public local fixture'})
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            with self.assertRaises(FileExistsError):
                RUNNER.private_json(path, {'replaced': True})
            self.assertEqual(json.loads(path.read_text()), {'scope': 'public local fixture'})

    def test_fixture_child_uses_only_owned_home_and_native_display_environment(self):
        ambient = {'DEEPSEEK_API_KEY': 'PUBLIC_SENTINEL', 'HOME': '/real-user-home',
                   'DSH_HOME': '/real-harness', 'NODE_OPTIONS': 'PUBLIC_SENTINEL', 'DISPLAY': ':9',
                   'WAYLAND_DISPLAY': 'wayland-public-test', 'XDG_RUNTIME_DIR': '/public-test-runtime'}
        with patch.dict(RUNNER.os.environ, ambient, clear=True):
            env = RUNNER.NATIVE.environment(Path('/owned-fixture'))
        for forbidden in ('DEEPSEEK_API_KEY', 'DSH_HOME', 'NODE_OPTIONS', 'DISPLAY'):
            self.assertNotIn(forbidden, env)
        self.assertEqual(env['HOME'], '/owned-fixture/user')
        self.assertEqual(env['WINIT_UNIX_BACKEND'], 'wayland')
        self.assertEqual(env['ICED_BACKEND'], 'wgpu')


if __name__ == '__main__':
    unittest.main()
