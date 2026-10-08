import argparse
import asyncio
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('qualify_package', Path(__file__).with_name('qualify-development-package.py'))
Q = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(Q)


def valid():
    return dict(status='passed', pid=12, applicationId=Q.APP_ID, applicationScale=1.0,
                windowBackend='wayland', modelPrompts=0, modelCatalogRequested=False, coreReady=True,
                workspaceOpened=True, sessionCreated=True, workspaceCount=1, sessionCount=1,
                snapshotFoldValid=True, snapshotCursor=3, screenshotRequested=True, screenshotSaved=True,
                screenshotPhysicalSize=[1, 1], stopError=False, deadlineReached=True,
                stop=dict(exited=True, graceful=True, containmentUnknown=False, observedDescendantsRemaining=0, exitCode=0))


class PackageQualificationGuards(unittest.TestCase):
    def test_package_uses_shared_strict_observed_node_parser(self):
        for raw in [b'v26.10.0\n',b'v26.12.1\n']:
            with patch.object(Q.L.subprocess,'run',return_value=Q.L.subprocess.CompletedProcess([],0,raw,b'')):
                self.assertEqual(Q.L.probe_node({'PATH':'/usr/bin:/bin'}),raw.removesuffix(b'\n').decode('ascii'))
        with patch.object(Q.L.subprocess,'run',return_value=Q.L.subprocess.CompletedProcess([],0,b'v26.not-a-version',b'')):
            with self.assertRaises(Q.L.Refused):Q.L.probe_node({'PATH':'/usr/bin:/bin'})

    def test_bad_node_observation_prevents_qualification_child_start(self):
        with tempfile.TemporaryDirectory(prefix='PUBLIC-node-qualification-') as folder:
            root=Path(folder).resolve();self.assertEqual(root.parent,Path(tempfile.gettempdir()).resolve())
            options=argparse.Namespace(package=str(root),output=str(root),scale=1.0)
            env={'PATH':'/usr/bin:/bin','HOME':str(root)}
            with patch.object(Q.L,'package',return_value=({},root/'PUBLIC-binary')),patch.object(Q.L,'binary_digest',return_value='a'*64),patch.object(Q.REG.NATIVE,'prepare',return_value=root),patch.object(Q.REG.NATIVE,'environment',return_value=env),patch.object(Q.L,'probe_node',side_effect=Q.L.Refused('public refusal')) as probe,patch.object(Q.subprocess,'Popen') as child:
                with self.assertRaises(Q.L.Refused):asyncio.run(Q.run(options))
                probe.assert_called_once_with(env);child.assert_not_called()
            self.assertFalse((root/'native-container').exists())

    def test_bad_explicit_node_version_prevents_launch_and_uses_selected_probe(self):
        with tempfile.TemporaryDirectory(prefix='PUBLIC-explicit-node-package-') as folder:
            root = Path(folder).resolve(); self.assertEqual(root.parent, Path(tempfile.gettempdir()).resolve())
            node = root / 'PUBLIC private node · 世界'; node.write_bytes(b'PUBLIC fake'); node.chmod(0o700)
            options = argparse.Namespace(package=str(root), output=str(root), scale=1.0, node=str(node))
            env = dict(PATH='/usr/bin:/bin', HOME=str(root))
            with patch.object(Q.L, 'package', return_value=({}, root/'PUBLIC-binary')), patch.object(Q.L, 'binary_digest', return_value='a'*64), patch.object(Q.REG.NATIVE, 'prepare', return_value=root), patch.object(Q.REG.NATIVE, 'environment', return_value=env), patch.object(Q.L, 'probe_node', side_effect=Q.L.Refused('PUBLIC refusal')) as probe, patch.object(Q.subprocess, 'Popen') as child:
                with self.assertRaises(Q.L.Refused): asyncio.run(Q.run(options))
                probe.assert_called_once_with(env, node=node); child.assert_not_called()
            self.assertFalse((root/'native-container').exists())

    def test_missing_explicit_node_never_uses_system_probe_or_starts_child(self):
        with tempfile.TemporaryDirectory(prefix='PUBLIC-missing-node-package-') as folder:
            root = Path(folder).resolve(); self.assertEqual(root.parent, Path(tempfile.gettempdir()).resolve())
            options = argparse.Namespace(package=str(root), output=str(root), scale=1.0, node=str(root/'PUBLIC missing'))
            with patch.object(Q.L, 'package', return_value=({}, root/'PUBLIC-binary')), patch.object(Q.L, 'binary_digest', return_value='a'*64), patch.object(Q.REG.NATIVE, 'prepare', return_value=root), patch.object(Q.REG.NATIVE, 'environment', return_value={'HOME':str(root)}), patch.object(Q.L, 'probe_node') as probe, patch.object(Q.subprocess, 'Popen') as child:
                with self.assertRaises(OSError): asyncio.run(Q.run(options))
                probe.assert_not_called(); child.assert_not_called()
            self.assertFalse((root/'native-container').exists())

    def test_matching_actual_entrypoint_and_live_fold_are_required(self):
        self.assertTrue(Q.valid_app(valid(), 12, 1.0))
        self.assertFalse(Q.valid_app(valid(), 13, 1.0))
        self.assertFalse(Q.valid_app(valid(), 12, 1.25))
        self.assertFalse(Q.valid_app(None, 12, 1.0))
        for field, value in (('coreReady', False), ('sessionCount', 0), ('workspaceCount', 2),
                             ('snapshotFoldValid', False), ('snapshotCursor', True), ('applicationId', 'foreign'),
                             ('screenshotSaved', False), ('windowBackend', 'x11')):
            changed = valid(); changed[field] = value
            self.assertFalse(Q.valid_app(changed, 12, 1.0))

    def test_no_model_and_clean_owned_stop_are_required(self):
        for field, value in (('modelPrompts', 1), ('modelCatalogRequested', True), ('deadlineReached', False), ('stopError', True)):
            changed = valid(); changed[field] = value
            self.assertFalse(Q.valid_app(changed, 12, 1.0))
        for field, value in (('graceful', False), ('exitCode', 1), ('containmentUnknown', True), ('observedDescendantsRemaining', 1)):
            changed = valid(); changed['stop'][field] = value
            self.assertFalse(Q.valid_app(changed, 12, 1.0))

    def test_missing_private_capture_is_invalid_without_parsing(self):
        with patch.object(Q.REG, 'private_regular', return_value=None) as private_regular, patch.object(Q.REG, 'png_dimensions') as png_dimensions:
            self.assertFalse(Q.capture_valid(Path('/PUBLIC'), valid()))
            private_regular.assert_called_once_with(Path('/PUBLIC/own-window.png'), 16 * 1024 * 1024)
            png_dimensions.assert_not_called()

    def test_rejected_private_capture_is_invalid_without_parsing(self):
        for rejection in ('wrong-mode', 'oversize'):
            with self.subTest(rejection=rejection):
                with patch.object(Q.REG, 'private_regular', return_value=None), patch.object(Q.REG, 'png_dimensions') as png_dimensions:
                    self.assertFalse(Q.capture_valid(Path('/PUBLIC'), valid()))
                    png_dimensions.assert_not_called()

    def test_complete_private_capture_must_match_app_report(self):
        with patch.object(Q.REG, 'private_regular', return_value=b'\x89PNG\r\n\x1a\n'):
            self.assertFalse(Q.capture_valid(Path('/PUBLIC'), valid()))
        with patch.object(Q.REG, 'private_regular', return_value=b'PUBLIC'), patch.object(Q.REG, 'png_dimensions', return_value=(1, 1)):
            self.assertTrue(Q.capture_valid(Path('/PUBLIC'), valid()))
            changed = valid(); changed['screenshotPhysicalSize'] = [2, 1]
            self.assertFalse(Q.capture_valid(Path('/PUBLIC'), changed))

    def test_ownership_and_bounded_cleanup_are_imported_not_reimplemented(self):
        self.assertTrue(callable(Q.REG.pin_root)); self.assertTrue(callable(Q.REG.observe_descendants))
        self.assertTrue(callable(Q.REG.cleanup_owned)); self.assertTrue(callable(Q.REG.native_window))


if __name__ == '__main__': unittest.main()
