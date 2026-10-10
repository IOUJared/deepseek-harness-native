import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch
from contextlib import redirect_stdout
import io
import os
import sys
import tempfile

SPEC = importlib.util.spec_from_file_location('package_release', Path(__file__).with_name('package_release.py'))
RELEASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RELEASE)
MANIFEST = '[package]\nversion = "0.1.0"\n'
SHA = 'a' * 40


class ReleaseMetadataTests(unittest.TestCase):
    def test_versioned_main_pushes_have_distinct_development_tags(self):
        first = RELEASE.release_identity(MANIFEST, SHA, '1')
        second = RELEASE.release_identity(MANIFEST, SHA, '2')
        self.assertEqual(first['releaseTag'], 'v0.1.0-dev.1')
        self.assertNotEqual(first['releaseTag'], second['releaseTag'])
        self.assertEqual(first['commit'], SHA)

    def test_rerun_has_same_version_and_target(self):
        self.assertEqual(RELEASE.release_identity(MANIFEST, SHA, '15'),
                         RELEASE.release_identity(MANIFEST, SHA, '15'))

    def test_cargo_version_change_changes_release_series(self):
        value = RELEASE.release_identity(MANIFEST.replace('0.1.0', '0.2.0'), SHA, '19')
        self.assertEqual(value['releaseTag'], 'v0.2.0-dev.19')

    def test_unusable_versions_commits_and_run_numbers_are_rejected(self):
        for version in ['01.1.0', '0.1', '0.1.0+extra', '0.1.0-dev.1', 'oops']:
            with self.subTest(version=version), self.assertRaises(ValueError):
                RELEASE.release_identity(MANIFEST.replace('0.1.0', version), SHA, '1')
        for commit in ['', 'a' * 7, 'A' * 40, '../../branch']:
            with self.subTest(commit=commit), self.assertRaises(ValueError):
                RELEASE.release_identity(MANIFEST, commit, '1')
        for number in ['', '0', '-1', '01', '1\ntag=bad', '1.2']:
            with self.subTest(number=number), self.assertRaises(ValueError):
                RELEASE.release_identity(MANIFEST, SHA, number)

    def test_calendar_release_uses_manifest_number_not_ci_run_number(self):
        manifest = MANIFEST.replace('0.1.0', '2026.10.10-r1')
        first = RELEASE.release_identity(manifest, SHA, '1')
        second = RELEASE.release_identity(manifest, SHA, '29')
        self.assertEqual(first['releaseTag'], 'v2026.10.10-r1')
        self.assertEqual(first['releaseTag'], second['releaseTag'])
        self.assertIs(first['automaticRelease'], False)
        self.assertEqual(first['packageVersion'], '2026.10.10-r1')

    def test_calendar_ci_does_not_create_assets_or_publishable_tag_output(self):
        with tempfile.TemporaryDirectory(prefix='PUBLIC-calendar-release-') as directory:
            root = Path(directory).resolve(); self.assertEqual(root.parent, Path(tempfile.gettempdir()).resolve())
            (root / 'app').mkdir(); (root / 'app/Cargo.toml').write_text(MANIFEST.replace('0.1.0', '2026.10.10-r1'))
            output = root / 'PUBLIC-output'; tag_output = root / 'PUBLIC-ci-output'
            with patch.object(RELEASE, 'ROOT', root), patch.object(RELEASE, 'command', return_value=SHA) as command, \
                    patch.object(sys, 'argv', ['package_release.py', '--binary', str(root / 'PUBLIC-missing-binary'), '--output', str(output)]), \
                    patch.dict(os.environ, {'GITHUB_SHA': SHA, 'GITHUB_RUN_NUMBER': '1', 'GITHUB_OUTPUT': str(tag_output)}, clear=True), \
                    redirect_stdout(io.StringIO()) as stdout:
                RELEASE.main()
            command.assert_called_once_with('git', 'rev-parse', 'HEAD')
            self.assertIn('checks-only-explicit-calendar-publication', stdout.getvalue())
            self.assertFalse(output.exists()); self.assertFalse(tag_output.exists())

    def test_calendar_invalid_day_or_release_number_is_rejected(self):
        for version in ['2026.2.30-r1', '2026.13.10-r1', '2026.10.10-r0', '2026.10.10-r01']:
            with self.subTest(version=version), self.assertRaises(ValueError):
                RELEASE.release_identity(MANIFEST.replace('0.1.0', version), SHA, '1')

    def test_glibc_requirement_uses_numeric_maximum(self):
        value = RELEASE.elf_requirements('Name: GLIBC_2.9\nName: GLIBC_2.39\nName: GLIBC_2.2',
                                         '0x01 (NEEDED) Shared library: [libc.so.6]\n0x02 (NEEDED) [libm.so.6]')
        self.assertEqual(value['minimumGlibcSymbols'], '2.39')
        self.assertEqual(value['dynamicDependencies'], ['libc.so.6', 'libm.so.6'])

    def test_missing_glibc_metadata_fails(self):
        with self.assertRaises(ValueError):
            RELEASE.elf_requirements('not a GNU ELF executable', '')


if __name__ == '__main__':
    unittest.main()
