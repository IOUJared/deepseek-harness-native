import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


def load_script(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


V = load_script('verify_archive', 'verify-development-archive.py')
B = load_script('package_builder_fixture', 'build-development-package.py')


class ArchiveGuards(unittest.TestCase):
    def member(self, name='PUBLIC-package/source/PUBLIC.rs', kind=tarfile.REGTYPE, size=12):
        value = tarfile.TarInfo(name); value.type = kind
        value.mode = 0o755 if kind == tarfile.DIRTYPE else 0o644; value.size = size
        return value

    def root(self):
        return self.member('PUBLIC-package', tarfile.DIRTYPE, 0)

    def test_only_bounded_expected_root_regular_files_and_directories(self):
        self.assertEqual(V.validate_members([self.root(), self.member()], 'PUBLIC-package'), 12)
        self.assertEqual(V.validate_members([self.root()], 'PUBLIC-package'), 0)

    def test_unsafe_paths_wrong_roots_duplicates_and_links_refuse(self):
        for name in ['/PUBLIC-package/a', 'PUBLIC-package/../a', 'PUBLIC-package//a', 'PUBLIC-package/./a', 'OTHER/a', 'PUBLIC-package/a\\b', 'PUBLIC-package/a\nb']:
            with self.subTest(name=name), self.assertRaises(ValueError):
                V.validate_members([self.root(), self.member(name)], 'PUBLIC-package')
        with self.assertRaises(ValueError):
            V.validate_members([self.root(), self.member(), self.member()], 'PUBLIC-package')
        for kind in [tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE, tarfile.CHRTYPE, tarfile.BLKTYPE]:
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                V.validate_members([self.root(), self.member(kind=kind)], 'PUBLIC-package')

    def test_member_count_expanded_size_and_permissions_are_bounded(self):
        for members in [[], [self.root()]*2049, [self.root(), self.member(size=64*1024*1024+1)], [self.root(), self.member(size=-1)]]:
            with self.assertRaises(ValueError): V.validate_members(members, 'PUBLIC-package')
        value = self.member(); value.mode = 0o777
        with self.assertRaises(ValueError): V.validate_members([self.root(), value], 'PUBLIC-package')

    def test_root_directory_is_required_and_cannot_be_a_file(self):
        for members in [[self.member()], [self.member('PUBLIC-package')]]:
            with self.assertRaises(ValueError): V.validate_members(members, 'PUBLIC-package')

    def test_directories_require_755_and_zero_size(self):
        for mode, size in [(0o644, 0), (0o755, 1), (0o755, -1)]:
            root = self.root(); root.mode = mode; root.size = size
            with self.subTest(mode=mode, size=size), self.assertRaises(ValueError):
                V.validate_members([root], 'PUBLIC-package')


class VerifyEndToEnd(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='PUBLIC-archive-verifier-')
        self.base = Path(self.temp.name).resolve()
        self.assertEqual(self.base.parent, Path(tempfile.gettempdir()).resolve())
        self.addCleanup(self.temp.cleanup)
        for directory in ['app/evidence', 'app/target/release', 'app/src', 'core', 'scripts']:
            (self.base / directory).mkdir(parents=True, exist_ok=True)
        (self.base / 'app/target/release/dsh-native-app').write_bytes(b'PUBLIC fixture binary, never executed\n')
        (self.base / 'app/src/ui.rs').write_bytes(b'PUBLIC source one\n')
        (self.base / 'app/src/other.rs').write_bytes(b'PUBLIC source two\n')
        (self.base / 'core/LICENSE').write_bytes(b'PUBLIC fixture license\n')
        (self.base / 'scripts/development-launcher.py').write_bytes(b'#!/usr/bin/python3\n# PUBLIC never executed\n')
        metadata = self.base / 'PUBLIC-metadata.json'
        metadata.write_text(json.dumps({'packages': [dict(name='PUBLIC-fixture', version='0.0.0', source=None)]}))
        self.evidence = self.base / 'app/evidence'
        self.output = self.evidence / 'built'
        for module in (V, B):
            base_patch = patch.object(module, 'BASE', self.base)
            base_patch.start(); self.addCleanup(base_patch.stop)
        original_read = B.L.private_read

        def fixture_read(path, limit):
            if path == self.base / 'docs/NATIVE-DEVELOPMENT-PACKAGE.md':
                return b'PUBLIC fixture instructions\n'
            return original_read(path, limit)

        # Exercise the real builder without Cargo, network, ELF commands, or GUI.
        with patch.object(B.L, 'private_read', side_effect=fixture_read), \
                patch.object(B.subprocess, 'run', return_value=SimpleNamespace(stdout=b'Name: GLIBC_2.34\n')), \
                contextlib.redirect_stdout(io.StringIO()):
            built = B.build(self.output, metadata)
        self.archive = self.output / built['archive']
        self.payload = self.output / built['archive'].removesuffix('.tar.gz')
        self.inventory_path = self.payload / 'source-snapshot.json'
        self.inventory = json.loads(self.inventory_path.read_text())
        self.counter = 0

    def destination(self):
        self.counter += 1
        return self.evidence / ('extracted-' + str(self.counter))

    def repack(self, change=None):
        self.counter += 1
        archive = self.output / ('PUBLIC-repacked-' + str(self.counter) + '.tar.gz')
        with tarfile.open(archive, 'w:gz') as handle:
            for path in [self.payload] + sorted(self.payload.rglob('*')):
                info = handle.gettarinfo(str(path), arcname=path.relative_to(self.output).as_posix())
                info.mode = 0o755 if path.is_dir() or os.access(path, os.X_OK) else 0o644
                if change is not None and change(info) is False:
                    continue
                if path.is_file():
                    with path.open('rb') as reader: handle.addfile(info, reader)
                else: handle.addfile(info)
        return archive

    def inventory_case(self, inventory):
        self.inventory_path.write_text(json.dumps(inventory))
        return self.repack()

    def test_builder_archive_passes_preserving_report_fields_and_actual_modes(self):
        destination = self.destination()
        report = V.verify(self.archive, destination, self.payload)
        self.assertEqual(report['status'], 'passed')
        self.assertEqual(report['nativeSourceFiles'], self.inventory['fileCount'])
        self.assertTrue(report['completeGeneratedPayloadAndExtractedHashesMatch'])
        self.assertTrue(report['sourceInventoryComplete'])
        self.assertTrue(report['noLinksSpecialFilesDuplicateOrUnsafeMembers'])
        self.assertFalse(report['signatureOrAdversarialSourceProtectionClaimed'])
        self.assertEqual(json.loads((destination / 'extraction-result.json').read_text()), report)
        self.assertEqual(stat.S_IMODE((destination / 'extraction-result.json').stat().st_mode), 0o600)
        extracted = Path(report['extractedPackage'])
        self.assertEqual(V.tree(extracted), V.tree(self.payload))
        self.assertEqual(stat.S_IMODE(self.payload.stat().st_mode), 0o700)
        self.assertEqual(V.snapshot(extracted), V.snapshot(self.payload, normalize_modes=True))
        for name in ['launch.py', 'bin/dsh-native-app']:
            self.assertEqual(stat.S_IMODE((extracted / name).stat().st_mode), 0o755)

    def test_duplicate_inventory_record_cannot_hide_behind_set_equality(self):
        value = json.loads(json.dumps(self.inventory))
        value['files'].append(dict(value['files'][0])); value['fileCount'] += 1
        with self.assertRaises(ValueError): V.verify(self.inventory_case(value), self.destination(), self.payload)

    def test_equal_count_duplicate_and_missing_inventory_identity_is_rejected(self):
        value = json.loads(json.dumps(self.inventory))
        value['files'][1] = dict(value['files'][0])
        with self.assertRaises(ValueError): V.verify(self.inventory_case(value), self.destination(), self.payload)

    def test_file_count_requires_exact_integer(self):
        for count in [True, float(self.inventory['fileCount']), str(self.inventory['fileCount']), -1, None]:
            value = json.loads(json.dumps(self.inventory)); value['fileCount'] = count
            with self.subTest(count=count), self.assertRaises(ValueError):
                V.verify(self.inventory_case(value), self.destination(), self.payload)

    def test_record_bytes_require_exact_nonnegative_integer_and_match(self):
        size = self.inventory['files'][0]['bytes']
        for count in [True, float(size), str(size), -1, size + 1, None]:
            value = json.loads(json.dumps(self.inventory)); value['files'][0]['bytes'] = count
            with self.subTest(count=count), self.assertRaises(ValueError):
                V.verify(self.inventory_case(value), self.destination(), self.payload)

    def test_boolean_bytes_cannot_impersonate_one_byte_source(self):
        record = self.inventory['files'][0]
        (self.payload / record['path']).write_bytes(b'x')
        record['bytes'] = True
        record['sha256'] = V.hashlib.sha256(b'x').hexdigest()
        with self.assertRaises(ValueError):
            V.verify(self.inventory_case(self.inventory), self.destination(), self.payload)

    def test_noncanonical_wrong_prefix_and_unknown_inventory_paths_are_rejected(self):
        original = self.inventory['files'][0]['path']
        for name in [original.replace('/', '//', 1), original.replace('/', '/./', 1),
                     'source/native/../outside', '/'+original, 'source/native',
                     'source/native/app/src/missing.rs', 'launch.py', 'source/native/a\\b',
                     'source/native/a\nb', None, {}]:
            value = json.loads(json.dumps(self.inventory)); value['files'][0]['path'] = name
            with self.subTest(name=name), self.assertRaises(ValueError):
                V.verify(self.inventory_case(value), self.destination(), self.payload)

    def test_inventory_shapes_and_digest_mismatches_are_rejected(self):
        values = [None, [], {}, dict(fileCount=self.inventory['fileCount'], files={})]
        for record in [None, [], {}, dict(self.inventory['files'][0], sha256='0'*64)]:
            value = json.loads(json.dumps(self.inventory)); value['files'][0] = record; values.append(value)
        for value in values:
            with self.subTest(value=value), self.assertRaises(ValueError):
                V.verify(self.inventory_case(value), self.destination(), self.payload)

    def test_omitted_and_extra_source_inventory_records_are_rejected(self):
        for keep in [False, True]:
            value = json.loads(json.dumps(self.inventory))
            if keep:
                value['files'].append(dict(value['files'][0], path='source/native/app/src/missing.rs'))
            else:
                value['files'] = value['files'][:-1]
            value['fileCount'] = len(value['files'])
            with self.assertRaises(ValueError): V.verify(self.inventory_case(value), self.destination(), self.payload)

    def test_tar_modes_must_match_builder_normalized_staged_files(self):
        for relative, mode in [('launch.py', 0o644), ('bin/dsh-native-app', 0o644),
                               ('source/native/app/src/ui.rs', 0o755)]:
            def change(info):
                if info.name == self.payload.name + '/' + relative: info.mode = mode
            destination = self.destination()
            with self.subTest(relative=relative), self.assertRaises(ValueError):
                V.verify(self.repack(change), destination, self.payload)
            self.assertFalse(destination.exists())

    def test_staged_entry_points_must_be_executable_even_when_tar_matches(self):
        for relative in ['launch.py', 'bin/dsh-native-app']:
            path = self.payload / relative; path.chmod(0o644)
            with self.subTest(relative=relative), self.assertRaises(ValueError):
                V.verify(self.repack(), self.destination(), self.payload)
            path.chmod(0o755)

    def test_builder_execute_criterion_normalizes_nonstandard_staged_mode(self):
        source = self.payload / 'source/native/app/src/ui.rs'; source.chmod(0o744)
        destination = self.destination()
        V.verify(self.repack(), destination, self.payload)
        self.assertEqual(stat.S_IMODE((destination / self.payload.name / source.relative_to(self.payload)).stat().st_mode), 0o755)

    def test_extracted_permissions_are_normalized_under_restrictive_umask(self):
        previous = os.umask(0o077)
        self.addCleanup(os.umask, previous)
        destination = self.destination()
        report = V.verify(self.archive, destination, self.payload)
        self.assertEqual(V.snapshot(Path(report['extractedPackage'])),
                         V.snapshot(self.payload, normalize_modes=True))

    def test_actual_extracted_execute_loss_or_unnormalized_permissions_are_rejected(self):
        original_extract = tarfile.TarFile.extractall
        for relative, mode in [('launch.py', 0o644), ('bin/dsh-native-app', 0o744),
                               ('source/native/app/src/ui.rs', 0o600), ('source', 0o700), ('', 0o700)]:
            def altered_extract(handle, destination, *args, **kwargs):
                original_extract(handle, destination, *args, **kwargs)
                (Path(destination) / self.payload.name / relative).chmod(mode)
            destination = self.destination()
            with self.subTest(relative=relative, mode=mode), \
                    patch.object(tarfile.TarFile, 'extractall', altered_extract), self.assertRaises(ValueError):
                V.verify(self.archive, destination, self.payload)
            self.assertFalse((destination / 'extraction-result.json').exists())

    def test_missing_root_wrong_directory_mode_and_nonzero_directory_size_are_rejected(self):
        for action in ['omit', 'mode', 'size']:
            def change(info):
                if info.name == self.payload.name:
                    if action == 'omit': return False
                    if action == 'mode': info.mode = 0o644
                    if action == 'size': info.size = 1
            destination = self.destination()
            with self.subTest(action=action), self.assertRaises(ValueError):
                V.verify(self.repack(change), destination, self.payload)
            self.assertFalse(destination.exists())

    def test_changed_payload_bytes_fail_before_extraction(self):
        (self.payload / 'source/native/app/src/ui.rs').write_bytes(b'PUBLIC changed after packaging\n')
        destination = self.destination()
        with self.assertRaises(ValueError): V.verify(self.archive, destination, self.payload)
        self.assertFalse(destination.exists())

    def test_expected_snapshot_must_be_a_directory(self):
        with self.assertRaises(ValueError):
            V.verify(self.archive, self.destination(), self.inventory_path)

    def test_expected_package_requires_entry_points_not_only_matching_hashes(self):
        empty_payload = self.output / 'PUBLIC-missing-entry-points'
        empty_payload.mkdir()
        with self.assertRaises(ValueError):
            V.verify(self.archive, self.destination(), empty_payload)

    def test_existing_or_outside_owned_evidence_destination_is_rejected(self):
        for destination in [self.output, self.base / 'outside']:
            with self.assertRaises(ValueError): V.verify(self.archive, destination, self.payload)


if __name__ == '__main__': unittest.main()
