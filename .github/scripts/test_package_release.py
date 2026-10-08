import importlib.util
from pathlib import Path
import unittest

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
