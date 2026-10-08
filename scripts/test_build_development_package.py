import importlib.util
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('package_builder', Path(__file__).with_name('build-development-package.py'))
B = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(B)


class PackageBuilderTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='PUBLIC-development-builder-')
        self.root = Path(self.temp.name).resolve()
        self.assertEqual(self.root.parent, Path(tempfile.gettempdir()).resolve())
        self.addCleanup(self.temp.cleanup)
        self.cache = self.root / 'cache'; self.cache.mkdir()
        self.crate = self.cache / 'PUBLIC-registry/public-crate-1.0.0'; self.crate.mkdir(parents=True)
        (self.crate / 'Cargo.toml').write_text('[package]\nname="public-crate"\nversion="1.0.0"\n')
        (self.crate / 'LICENSE').write_text('PUBLIC fixture license text\n')
        self.payload = self.root / 'payload'; self.payload.mkdir()
        self.metadata = {'packages': [dict(name='public-crate', version='1.0.0', source='registry+PUBLIC',
                                          manifest_path=str(self.crate / 'Cargo.toml'), license='MIT', license_file=None)]}
        self.patch = patch.object(B, 'CARGO_SRC', self.cache); self.patch.start(); self.addCleanup(self.patch.stop)

    def test_only_named_cached_crate_license_texts_are_copied(self):
        result = B.licenses(self.metadata, self.payload)
        self.assertEqual(result['packageCount'], 1)
        self.assertEqual(result['packagesWithCopiedLicenseTexts'], 1)
        self.assertEqual((self.payload / 'licenses/rust/public-crate-1.0.0/LICENSE').read_text(), 'PUBLIC fixture license text\n')
        self.assertNotIn(str(self.root), json.dumps(result))

    def test_arbitrary_manifest_path_or_escaping_name_is_not_admitted(self):
        for field, value in (('manifest_path', str(self.root / 'unrelated/Cargo.toml')), ('name', '../unrelated'), ('version', '../../escape')):
            metadata = json.loads(json.dumps(self.metadata)); metadata['packages'][0][field] = value
            with self.assertRaises((ValueError, OSError)): B.licenses(metadata, self.payload)
        self.assertEqual(list(self.payload.iterdir()), [])

    def test_explicit_license_outside_cached_crate_is_not_copied(self):
        unrelated = self.root / 'PUBLIC-unrelated'; unrelated.write_text('PUBLIC do not copy\n')
        self.metadata['packages'][0]['license_file'] = str(unrelated)
        result = B.licenses(self.metadata, self.payload)
        self.assertNotIn(str(unrelated), json.dumps(result))
        self.assertFalse((self.payload / 'licenses/rust/public-crate-1.0.0/PUBLIC-unrelated').exists())

    def test_empty_or_nonobject_metadata_is_rejected(self):
        for value in (None, [], {}, {'packages': []}, {'packages': [None]}):
            with self.assertRaises(ValueError): B.licenses(value, self.payload)

    def test_snapshot_copy_does_not_follow_link_or_overwrite(self):
        source = self.root / 'PUBLIC-source'; source.write_bytes(b'PUBLIC-one')
        target = self.root / 'PUBLIC-target'; target.write_bytes(b'PUBLIC-preserve')
        with self.assertRaises(FileExistsError): B.copy_new(source, target)
        self.assertEqual(target.read_bytes(), b'PUBLIC-preserve')
        link = self.root / 'PUBLIC-link'; link.symlink_to(source)
        with self.assertRaises(ValueError): B.copy_new(link, self.root / 'PUBLIC-new-target')
        self.assertFalse((self.root / 'PUBLIC-new-target').exists())

    def test_existing_package_output_is_rejected_before_metadata_or_binary_reads(self):
        with patch.object(B, 'BASE', self.root):
            (self.root / 'app/evidence').mkdir(parents=True)
            output = self.root / 'app/evidence/existing'; output.mkdir()
            with self.assertRaises(ValueError): B.build(output, self.root / 'never-read.json')
        self.assertEqual(list(output.iterdir()), [])

    def public_qualification(self):
        app = self.root / 'app'; (app / 'src').mkdir(parents=True)
        source = app / 'src/ui.rs'; source.write_text('PUBLIC source fixture\n')
        digest = hashlib.sha256(source.read_bytes()).hexdigest()
        value = dict(scope='native-control-layout-and-Harness-style-conversation', defaultReleaseSha256='a'*64,
                     parentPaintInspectionComplete=True, checkAllTargetsFeaturePassed=True, formatCheckPassed=True,
                     defaultAppTestsPassed=1, featureAppTestsPassed=1, fixtureExampleTestsPassed=1,
                     publicCases=[dict(passed=True, exitCode=0)], realKeyless=dict(passed=True, exitCode=0),
                     sourceSha256={'src/ui.rs':digest})
        path = self.root / 'PUBLIC-qualification.json'; path.write_text(json.dumps(value))
        return app, source, path, value

    def test_native_qualification_requires_matching_binary_and_current_source(self):
        app, source, path, value = self.public_qualification()
        self.assertEqual(B.qualified_native(path, 'a'*64, app), value)
        with self.assertRaises(ValueError): B.qualified_native(path, 'b'*64, app)
        source.write_text('PUBLIC changed source\n')
        with self.assertRaises(ValueError): B.qualified_native(path, 'a'*64, app)

    def test_qualification_rejects_missing_failed_or_false_integer_evidence(self):
        app, _, path, value = self.public_qualification()
        for field, replacement in [('publicCases', []), ('realKeyless', dict(passed=False, exitCode=0)),
                                   ('parentPaintInspectionComplete', False), ('defaultAppTestsPassed', True),
                                   ('sourceSha256', {}), ('publicCases', [dict(passed=True, exitCode=False)])]:
            changed = dict(value); changed[field] = replacement; path.write_text(json.dumps(changed))
            with self.assertRaises(ValueError): B.qualified_native(path, 'a'*64, app)

    def test_explicit_scoped_formatting_preserves_full_format_false(self):
        app, _, path, value = self.public_qualification()
        value['formatCheckPassed'] = False
        value['scopedFormatCheck'] = dict(passed=True, scope='explicit-app-files', exitCode=0, files=['src/ui.rs'])
        path.write_text(json.dumps(value))
        result = B.qualified_native(path, 'a'*64, app)
        self.assertIs(result['formatCheckPassed'], False)
        self.assertEqual(result['scopedFormatCheck'], value['scopedFormatCheck'])

    def test_false_full_format_requires_bounded_scoped_evidence(self):
        app, _, path, value = self.public_qualification()
        value['formatCheckPassed'] = False
        for scoped in [None, {}, dict(passed=False, scope='explicit-app-files', exitCode=0, files=['src/ui.rs']),
                       dict(passed=True, scope='full-app', files=['src/ui.rs']),
                       dict(passed=True, scope='explicit-app-files', exitCode=0, files=[]),
                       dict(passed=True, scope='explicit-app-files', exitCode=0, files=['src/missing.rs']),
                       dict(passed=True, scope='explicit-app-files', exitCode=0, files=['src/ui.rs']*2),
                       dict(passed=True, scope='explicit-app-files', exitCode=0, files=['src/ui.rs']*513),
                       dict(passed=True, scope='explicit-app-files', exitCode=0, files=[{}]),
                       dict(passed=1, scope='explicit-app-files', exitCode=0, files=['src/ui.rs'])]:
            changed=dict(value); changed['scopedFormatCheck']=scoped; path.write_text(json.dumps(changed))
            with self.assertRaises(ValueError): B.qualified_native(path, 'a'*64, app)
        value['scopedFormatCheck']=dict(passed=True, scope='explicit-app-files', exitCode=0, files=['src/ui.rs'])
        for exit_code in [None, False, True, 1, '0']:
            changed=dict(value); changed['scopedFormatCheck']=dict(value['scopedFormatCheck'],exitCode=exit_code);path.write_text(json.dumps(changed))
            with self.assertRaises(ValueError):B.qualified_native(path,'a'*64,app)
        for full in [None, 0, 1, 'false']:
            changed=dict(value); changed['formatCheckPassed']=full; path.write_text(json.dumps(changed))
            with self.assertRaises(ValueError): B.qualified_native(path, 'a'*64, app)

    def test_scoped_formatting_does_not_bypass_binary_source_or_test_gates(self):
        app, source, path, value=self.public_qualification()
        value['formatCheckPassed']=False
        value['scopedFormatCheck']=dict(passed=True, scope='explicit-app-files', exitCode=0, files=['src/ui.rs'])
        for field, replacement in [('checkAllTargetsFeaturePassed',False),('parentPaintInspectionComplete',False),
                                    ('defaultAppTestsPassed',False),('publicCases',[]),('realKeyless',dict(passed=False,exitCode=0))]:
            changed=dict(value);changed[field]=replacement;path.write_text(json.dumps(changed))
            with self.assertRaises(ValueError):B.qualified_native(path,'a'*64,app)
        path.write_text(json.dumps(value))
        with self.assertRaises(ValueError):B.qualified_native(path,'b'*64,app)
        source.write_text('PUBLIC changed scope source\n')
        with self.assertRaises(ValueError):B.qualified_native(path,'a'*64,app)

    def test_qualification_refuses_escaping_or_symlinked_source_pins(self):
        app, source, path, value = self.public_qualification()
        for name in ['../unrelated', '/absolute', 'src/../ui.rs', 'src//ui.rs', 'src/./ui.rs', 'src/\\\\ui.rs', 'src/\nui.rs']:
            changed = dict(value); changed['sourceSha256'] = dict(value['sourceSha256']); changed['sourceSha256'][name] = 'a'*64
            path.write_text(json.dumps(changed))
            with self.assertRaises(ValueError): B.qualified_native(path, 'a'*64, app)
        path.write_text(json.dumps(value))
        linked_app = self.root / 'linked-app'; linked_app.symlink_to(app, target_is_directory=True)
        with self.assertRaises(ValueError): B.qualified_native(path, 'a'*64, linked_app)
        original = source.read_bytes(); source.unlink(); actual = self.root / 'PUBLIC-target'; actual.write_bytes(original); source.symlink_to(actual)
        with self.assertRaises(ValueError): B.qualified_native(path, 'a'*64, app)

    def test_observed_node_patch_is_optional_not_hardcoded(self):
        app, _, path, value=self.public_qualification()
        self.assertNotIn('systemNodeVersionObserved',B.qualified_native(path,'a'*64,app))
        for version in ['v26.10.0','v26.12.1']:
            changed=dict(value,systemNodeVersionObserved=version);path.write_text(json.dumps(changed))
            self.assertEqual(B.qualified_native(path,'a'*64,app)['systemNodeVersionObserved'],version)
        for version in [True,26,'v25.10.0','v26.10.0\n','26.10.0','v26..0','v26.1.0-custom']:
            changed=dict(value,systemNodeVersionObserved=version);path.write_text(json.dumps(changed))
            with self.assertRaises(ValueError):B.qualified_native(path,'a'*64,app)

    def test_native_source_bundle_includes_examples_fixtures_and_inventory(self):
        base = self.root / 'native'
        for folder in ['app/src', 'app/examples/support', 'app/fixtures', 'core/runtime', 'transport/tests/support']:
            directory = base / folder; directory.mkdir(parents=True); (directory / 'PUBLIC.txt').write_text('PUBLIC body\n')
        (base / 'app/Cargo.toml').write_text('PUBLIC manifest\n')
        B.copy_native_sources(base, self.payload)
        result = B.snapshot_inventory(self.payload)
        self.assertEqual(result['fileCount'], 6)
        for record in result['files']:
            data = (self.payload / record['path']).read_bytes()
            self.assertEqual(record['bytes'], len(data)); self.assertEqual(record['sha256'], hashlib.sha256(data).hexdigest())
        self.assertTrue((self.payload / 'source/native/app/examples/support/PUBLIC.txt').exists())
        self.assertTrue((self.payload / 'source/native/app/fixtures/PUBLIC.txt').exists())

    def test_source_folder_and_module_symlinks_are_refused_before_enumeration(self):
        outside = self.root / 'outside'; outside.mkdir(); (outside / 'PUBLIC.txt').write_text('PUBLIC not copied\n')
        for module, folder in [('app','src'), ('core','runtime'), ('transport','fixtures')]:
            base = self.root / ('case-' + module); (base / module).mkdir(parents=True)
            (base / module / folder).symlink_to(outside, target_is_directory=True)
            with self.assertRaises(ValueError): B.copy_native_sources(base, self.payload)
        base = self.root / 'module-case'; base.mkdir(); (base / 'app').symlink_to(outside, target_is_directory=True)
        with self.assertRaises(ValueError): B.copy_native_sources(base, self.payload)
        self.assertFalse((self.payload / 'source/native').exists())


if __name__ == '__main__': unittest.main()
