import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('native_qualification', Path(__file__).with_name('qualify-native-app.py'))
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


class QualificationTests(unittest.TestCase):
    def test_private_new_output_and_directory_modes(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            (base / 'app/evidence').mkdir(parents=True)
            with patch.object(RUNNER, 'BASE', base):
                output = RUNNER.prepare(base / 'app/evidence/owned-run')
                self.assertEqual(output.stat().st_mode & 0o777, 0o700)
                self.assertEqual((output / 'harness').stat().st_mode & 0o777, 0o700)
                with self.assertRaises(ValueError):
                    RUNNER.prepare(output)
                with self.assertRaises(ValueError):
                    RUNNER.prepare(base / 'unrelated')

    def test_child_environment_excludes_ambient_secrets_and_x11(self):
        fake = {'HOME': '/original-home', 'WAYLAND_DISPLAY': 'wayland-test',
                'XDG_RUNTIME_DIR': '/runtime-test', 'DISPLAY': ':9', 'NODE_OPTIONS': 'unsafe',
                'DEEPSEEK_API_KEY': 'SECRET_SENTINEL', 'DSH_HOME': '/original-harness'}
        with patch.dict(RUNNER.os.environ, fake, clear=True):
            env = RUNNER.environment(Path('/isolated'))
        for key in ('DISPLAY', 'NODE_OPTIONS', 'DEEPSEEK_API_KEY', 'DSH_HOME'):
            self.assertNotIn(key, env)
        self.assertEqual(env['HOME'], '/isolated/user')
        self.assertEqual(env['WAYLAND_DISPLAY'], 'wayland-test')
        self.assertEqual(env['ICED_BACKEND'], 'wgpu')
        self.assertEqual(env['WINIT_UNIX_BACKEND'], 'wayland')


if __name__ == '__main__':
    unittest.main()
