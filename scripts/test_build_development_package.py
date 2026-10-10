import importlib.util
import hashlib
import io
import json
import os
from contextlib import redirect_stdout
from types import SimpleNamespace
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

    def native_tree(self, name, omit=None):
        base = self.root / name
        for module in B.NATIVE_MODULES:
            root = base / module; root.mkdir(parents=True)
            for filename in ('Cargo.toml', 'Cargo.lock'):
                if omit != (module, filename):
                    text = '# PUBLIC ' + name + ' ' + filename + '\n'
                    if filename == 'Cargo.toml': text += '[package]\nversion="0.1.0"\n'
                    (root / filename).write_text(text)
            if omit != (module, 'src'):
                (root / 'src').mkdir()
                (root / 'src/ui.rs').write_text('PUBLIC ' + name + ' ' + module + ' source\n')
        (base / 'core/LICENSE').write_text('PUBLIC ' + name + ' license\n')
        return base

    def build_inputs(self):
        live = self.native_tree('PUBLIC-live')
        frozen = self.native_tree('PUBLIC-frozen')
        binary = self.root / 'PUBLIC-selected-native'; binary.write_bytes(b'PUBLIC frozen binary\n')
        default = live / 'app/target/release/dsh-native-app'; default.parent.mkdir(parents=True)
        default.write_bytes(b'PUBLIC live binary\n')
        (live / 'app/evidence').mkdir()
        (live / 'scripts').mkdir(); (live / 'scripts/development-launcher.py').write_text('# PUBLIC live launcher\n')
        (live / 'docs').mkdir(); (live / 'docs/NATIVE-DEVELOPMENT-PACKAGE.md').write_text('# PUBLIC live instructions\n')
        metadata = self.root / 'PUBLIC-metadata.json'; metadata.write_text(json.dumps(self.metadata))
        return live, frozen, binary, metadata

    def synthetic_build(self, live, metadata, **kwargs):
        # Fixed PUBLIC files only: readelf is mocked, no launcher/runtime is executed.
        output = live / 'app/evidence/PUBLIC-new-package'
        with patch.object(B, 'BASE', live), patch.object(B.subprocess, 'run',
                return_value=SimpleNamespace(stdout=b'Name: GLIBC_2.34\n')) as elf, redirect_stdout(io.StringIO()):
            result = B.build(output, metadata, **kwargs)
        elf.assert_called_once()
        self.assertEqual(elf.call_args.args[0][:2], ['/usr/bin/readelf', '--version-info'])
        payload = output / result['archive'].removesuffix('.tar.gz')
        return result, payload

    def test_native_pair_defaults_and_explicit_inputs(self):
        live, frozen, binary, _ = self.build_inputs()
        with patch.object(B, 'BASE', live):
            self.assertEqual(B.native_inputs(), (live, live / 'app/target/release/dsh-native-app'))
            self.assertEqual(B.native_inputs(frozen, binary), (frozen, binary))
            for kwargs in [dict(source_root=frozen), dict(native_binary=binary)]:
                with self.assertRaises(ValueError): B.native_inputs(**kwargs)
                with self.assertRaises(ValueError): B.build(live / 'app/evidence/PUBLIC-new', 'never-read', **kwargs)
                self.assertFalse((live / 'app/evidence/PUBLIC-new').exists())

    def test_nonabsolute_noncanonical_parent_control_missing_and_nonregular_inputs_refused(self):
        _, frozen, binary, _ = self.build_inputs()
        bad_roots = ['relative', str(frozen) + '/..', str(frozen) + '/.', str(frozen) + '/',
                     str(frozen) + '\n', str(self.root / 'PUBLIC-missing'), binary]
        bad_bins = ['relative', str(binary.parent) + '/../' + binary.name, str(binary) + '\x7f',
                    str(self.root / 'PUBLIC-missing-binary'), frozen]
        for root in bad_roots:
            with self.assertRaises((ValueError, OSError)): B.native_inputs(root, binary)
        for value in bad_bins:
            with self.assertRaises((ValueError, OSError)): B.native_inputs(frozen, value)
        fifo = self.root / 'PUBLIC-fifo'; os.mkfifo(fifo)
        with self.assertRaises(ValueError): B.native_inputs(frozen, fifo)

    def test_foreign_native_inputs_are_refused(self):
        _, frozen, binary, _ = self.build_inputs()
        with patch.object(B.os, 'getuid', return_value=os.getuid() + 1):
            with self.assertRaises(ValueError): B.native_path(frozen, True)
            with self.assertRaises(ValueError): B.native_path(binary, False)

    def test_root_binary_parent_and_required_module_symlinks_refused(self):
        _, frozen, binary, _ = self.build_inputs()
        root_link = self.root / 'PUBLIC-root-link'; root_link.symlink_to(frozen, target_is_directory=True)
        binary_link = self.root / 'PUBLIC-binary-link'; binary_link.symlink_to(binary)
        parent_link = self.root / 'PUBLIC-parent-link'; parent_link.symlink_to(self.root, target_is_directory=True)
        for root, value in [(root_link, binary), (frozen, binary_link), (frozen, parent_link / binary.name)]:
            with self.assertRaises(ValueError): B.native_inputs(root, value)
        module_case = self.root / 'PUBLIC-module-link'; module_case.mkdir()
        (module_case / 'app').symlink_to(frozen / 'app', target_is_directory=True)
        with self.assertRaises(ValueError): B.native_inputs(module_case, binary)

    def test_all_three_module_manifests_locks_and_sources_required(self):
        binary = self.root / 'PUBLIC-native'; binary.write_bytes(b'PUBLIC binary')
        for module in B.NATIVE_MODULES:
            for field in ('Cargo.toml', 'Cargo.lock', 'src'):
                root = self.native_tree('PUBLIC-missing-' + module + '-' + field, omit=(module, field))
                with self.assertRaises((ValueError, OSError)): B.native_inputs(root, binary)

    def test_custom_sources_and_binary_not_live_checkout_are_packaged(self):
        live, frozen, binary, metadata = self.build_inputs()
        result, payload = self.synthetic_build(live, metadata, source_root=frozen, native_binary=binary)
        self.assertEqual(result['nativeSha256'], hashlib.sha256(binary.read_bytes()).hexdigest())
        self.assertEqual((payload / 'bin/dsh-native-app').read_bytes(), binary.read_bytes())
        for module in B.NATIVE_MODULES:
            for relative in ['Cargo.toml', 'Cargo.lock', 'src/ui.rs']:
                packaged = (payload / 'source/native' / module / relative).read_bytes()
                self.assertEqual(packaged, (frozen / module / relative).read_bytes())
                self.assertNotEqual(packaged, (live / module / relative).read_bytes())
        self.assertEqual((payload / 'launch.py').read_bytes(), (live / 'scripts/development-launcher.py').read_bytes())
        self.assertEqual((payload / 'LICENSE').read_bytes(), (live / 'core/LICENSE').read_bytes())
        self.assertIn('PUBLIC live instructions', (payload / 'README.md').read_text())
        status = json.loads((payload / 'evidence/native-ui-qualification-status.json').read_text())
        self.assertEqual(status['status'], 'unqualified')
        self.assertIs(status['relocatedStartupQualified'], False)
        self.assertIs(result['nativeUiQualificationIncluded'], False)
        self.assertIs(json.loads((payload / 'development-manifest.json').read_text())['nativeUiQualificationIncluded'], False)
        self.assertFalse((payload / 'evidence/native-ui-qualification.json').exists())

    def test_build_without_pair_keeps_original_binary_and_source_defaults(self):
        live, _, _, metadata = self.build_inputs()
        result, payload = self.synthetic_build(live, metadata)
        self.assertEqual((payload / 'bin/dsh-native-app').read_bytes(), (live / 'app/target/release/dsh-native-app').read_bytes())
        self.assertEqual((payload / 'source/native/app/src/ui.rs').read_bytes(), (live / 'app/src/ui.rs').read_bytes())
        self.assertIs(result['runtimeBundled'], False)
        self.assertIs(result['nodeBundled'], False)

    def test_private_selected_source_is_refused_before_creating_output(self):
        live, frozen, binary, metadata = self.build_inputs()
        (frozen / 'app/src/.env.PUBLIC').write_text('PUBLIC private filename, not credentials\n')
        with patch.object(B, 'BASE', live), self.assertRaises(ValueError):
            B.build(live / 'app/evidence/PUBLIC-new-package', metadata, source_root=frozen, native_binary=binary)
        self.assertFalse((live / 'app/evidence/PUBLIC-new-package').exists())

    def test_selected_source_changed_after_individual_copy_is_refused(self):
        live, frozen, binary, metadata = self.build_inputs()
        changed = frozen / 'app/src/ui.rs'; original = B.copy_new
        def copy_and_change(source, target, *args, **kwargs):
            original(source, target, *args, **kwargs)
            if source == changed: changed.write_text('PUBLIC changed after per-file copy check\n')
        with patch.object(B, 'copy_new', side_effect=copy_and_change), self.assertRaisesRegex(ValueError, 'sources changed'):
            self.synthetic_build(live, metadata, source_root=frozen, native_binary=binary)
        self.assertEqual(list((live / 'app/evidence/PUBLIC-new-package').glob('*.tar.gz')), [])

    def test_copied_source_mismatch_is_refused_even_if_input_is_restored(self):
        live, frozen, binary, metadata = self.build_inputs()
        changed = frozen / 'app/src/ui.rs'; saved = changed.read_bytes(); original = B.copy_new
        def copy_changed_then_restore(source, target, *args, **kwargs):
            if source == changed: changed.write_bytes(b'PUBLIC transient changed source\n')
            try: original(source, target, *args, **kwargs)
            finally:
                if source == changed: changed.write_bytes(saved)
        with patch.object(B, 'copy_new', side_effect=copy_changed_then_restore), self.assertRaisesRegex(ValueError, 'sources changed'):
            self.synthetic_build(live, metadata, source_root=frozen, native_binary=binary)
        self.assertEqual(changed.read_bytes(), saved)

    def test_source_inventory_bounds_share_the_copy_selection(self):
        _, frozen, _, _ = self.build_inputs()
        for bound in ['MAX_NATIVE_SOURCE_FILES', 'MAX_NATIVE_SOURCE_ENTRIES', 'MAX_NATIVE_SOURCE_BYTES']:
            with patch.object(B, bound, 1), self.assertRaisesRegex(ValueError, 'exceeded bound'):
                B.native_source_inventory(frozen)

    def test_output_cannot_recurse_into_selected_sources(self):
        frozen = self.native_tree('app/evidence/PUBLIC-frozen')
        binary = self.root / 'PUBLIC-native'; binary.write_bytes(b'PUBLIC binary')
        output = frozen / 'app/src/PUBLIC-output'
        with patch.object(B, 'BASE', self.root), self.assertRaisesRegex(ValueError, 'overlap'):
            B.build(output, 'never-read', source_root=frozen, native_binary=binary)
        self.assertFalse(output.exists())

    def test_selected_binary_changed_after_copy_is_refused(self):
        live, frozen, binary, metadata = self.build_inputs(); original = B.copy_new
        def copy_and_change(source, target, *args, **kwargs):
            original(source, target, *args, **kwargs)
            if source == binary: binary.write_bytes(b'PUBLIC changed selected binary\n')
        with patch.object(B, 'copy_new', side_effect=copy_and_change), self.assertRaisesRegex(ValueError, 'binary changed'):
            self.synthetic_build(live, metadata, source_root=frozen, native_binary=binary)

    def test_selected_qualification_uses_selected_app_root_and_packaged_root(self):
        live, frozen, binary, metadata = self.build_inputs()
        _, _, report, value = self.public_qualification()
        value['defaultReleaseSha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
        value['sourceSha256'] = {'src/ui.rs': hashlib.sha256((frozen / 'app/src/ui.rs').read_bytes()).hexdigest()}
        report.write_text(json.dumps(value))
        with patch.object(B, 'qualified_native', wraps=B.qualified_native) as gate:
            result, payload = self.synthetic_build(live, metadata, qualification_path=report,
                                                    source_root=frozen, native_binary=binary)
        self.assertEqual([call.args[2] for call in gate.call_args_list], [frozen / 'app', payload / 'source/native/app'])
        self.assertIs(result['nativeUiQualificationIncluded'], True)


    def test_calendar_version_comes_from_selected_not_live_manifest(self):
        live, frozen, binary, metadata = self.build_inputs()
        manifest = frozen / 'app/Cargo.toml'
        manifest.write_text(manifest.read_text().replace('0.1.0', '2026.10.10-r1'))
        result, payload = self.synthetic_build(live, metadata, source_root=frozen, native_binary=binary)
        self.assertIn('development-2026.10.10-r1-', result['archive'])
        self.assertEqual(json.loads((payload / 'development-manifest.json').read_text())['packageVersion'], '2026.10.10-r1')

    def test_unsafe_package_version_is_refused_before_output_creation(self):
        live, frozen, binary, metadata = self.build_inputs()
        manifest = frozen / 'app/Cargo.toml'
        manifest.write_text(manifest.read_text().replace('0.1.0', '../PUBLIC-escape'))
        output = live / 'app/evidence/PUBLIC-invalid-version'
        with patch.object(B.subprocess, 'run', side_effect=AssertionError('readelf reached before version refusal')) as elf:
            with patch.object(B, 'BASE', live), self.assertRaisesRegex(ValueError, 'version'):
                B.build(output, metadata, source_root=frozen, native_binary=binary)
            elf.assert_not_called()
        self.assertFalse(output.exists())

    def vendor_tree(self, frozen):
        vendor = frozen / 'app/vendor'
        for name, text in {
            'README.md': 'PUBLIC patched dependency notice\n',
            'WINIT-UPSTREAM.json': '{"PUBLIC": "upstream pins"}\n',
            'winit/Cargo.toml': '[package]\nname="winit"\nversion="0.30.13"\n',
            'winit/.cargo_vcs_info.json': '{"PUBLIC": "upstream identity"}\n',
            'winit/LICENSE': 'PUBLIC Apache license fixture\n',
            'winit/docs/res/ATTRIBUTION.md': 'PUBLIC asset attribution fixture\n',
            'winit/src/lib.rs': '// PUBLIC selected dependency source\n',
        }.items():
            path = vendor / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_text(text)
        return vendor

    def test_vendor_selected_sources_not_live_dependency_are_copied_and_hashed(self):
        live, frozen, binary, metadata = self.build_inputs()
        vendor = self.vendor_tree(frozen); self.vendor_tree(live)
        (live / 'app/vendor/winit/src/lib.rs').write_text('// PUBLIC different live dependency\n')
        result, payload = self.synthetic_build(live, metadata, source_root=frozen, native_binary=binary)
        inventory = B.native_source_inventory(frozen)
        self.assertEqual(json.loads((payload / 'source-snapshot.json').read_text()), inventory)
        records = {r['path']: r for r in inventory['files']}
        for path in vendor.rglob('*'):
            if not path.is_file(): continue
            relative = Path('source/native') / path.relative_to(frozen)
            self.assertEqual((payload / relative).read_bytes(), path.read_bytes())
            self.assertEqual(records[relative.as_posix()]['sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(result['nativeSourceSnapshotFiles'], inventory['fileCount'])
        for module in ['core', 'transport']:
            path = frozen / module / 'vendor/PUBLIC.txt'; path.parent.mkdir(); path.write_text('PUBLIC outside app vendor policy\n')
        self.assertEqual(B.native_source_inventory(frozen), inventory)

    def test_vendor_private_filename_refused_before_package_output(self):
        live, _, binary, metadata = self.build_inputs()
        for index, name in enumerate(['.env.PUBLIC', '.credentials.yaml', '.npmrc', 'PUBLIC\nname']):
            frozen = self.native_tree('PUBLIC-vendor-private-' + str(index)); vendor = self.vendor_tree(frozen)
            (vendor / 'winit' / name).write_text('PUBLIC forbidden filename, no secret\n')
            output = live / 'app/evidence' / ('PUBLIC-private-' + str(index))
            with patch.object(B.subprocess, 'run', side_effect=AssertionError('readelf reached before vendor refusal')) as elf:
                with patch.object(B, 'BASE', live), self.assertRaisesRegex(ValueError, 'Private'):
                    B.build(output, metadata, source_root=frozen, native_binary=binary)
                elf.assert_not_called()
            self.assertFalse(output.exists())

    def test_vendor_fifo_and_broken_symlink_are_refused_without_reading(self):
        for kind in ['fifo', 'broken']:
            frozen = self.native_tree('PUBLIC-vendor-special-' + kind); vendor = self.vendor_tree(frozen)
            path = vendor / 'winit/PUBLIC-unreadable'
            if kind == 'fifo': os.mkfifo(path)
            else: path.symlink_to(self.root / 'PUBLIC-nonexistent')
            with self.assertRaisesRegex(ValueError, 'unexpected|symlink'): B.native_source_inventory(frozen)

    def test_vendor_parent_subdirectory_and_file_symlinks_are_refused(self):
        for level in ['parent', 'directory', 'file']:
            frozen = self.native_tree('PUBLIC-vendor-link-' + level)
            outside = self.root / ('PUBLIC-outside-' + level); outside.mkdir()
            (outside / 'PUBLIC.txt').write_text('PUBLIC not copied\n')
            vendor = frozen / 'app/vendor'
            if level == 'parent': vendor.symlink_to(outside, target_is_directory=True)
            else:
                self.vendor_tree(frozen)
                target = vendor / ('other' if level == 'directory' else 'PUBLIC-link')
                target.symlink_to(outside if level == 'directory' else outside / 'PUBLIC.txt', target_is_directory=level == 'directory')
            with self.assertRaisesRegex(ValueError, 'symlink'): B.native_source_inventory(frozen)

    def test_vendor_entries_and_bytes_count_toward_shared_limits(self):
        _, frozen, _, _ = self.build_inputs()
        before = B.native_source_inventory(frozen); self.vendor_tree(frozen)
        limits = {'MAX_NATIVE_SOURCE_FILES': before['fileCount'],
                  'MAX_NATIVE_SOURCE_BYTES': sum(r['bytes'] for r in before['files']),
                  'MAX_NATIVE_SOURCE_ENTRIES': 4}
        for name, limit in limits.items():
            with patch.object(B, name, limit), self.assertRaisesRegex(ValueError, 'exceeded bound'):
                B.native_source_inventory(frozen)

    def test_package_output_inside_vendor_is_refused_before_creation(self):
        frozen = self.native_tree('app/evidence/PUBLIC-vendor-overlap'); vendor = self.vendor_tree(frozen)
        binary = self.root / 'PUBLIC-native'; binary.write_bytes(b'PUBLIC binary')
        output = vendor / 'winit/PUBLIC-output'
        with patch.object(B.subprocess, 'run', side_effect=AssertionError('readelf reached before overlap refusal')) as elf:
            with patch.object(B, 'BASE', self.root), self.assertRaisesRegex(ValueError, 'overlap'):
                B.build(output, 'never-read', source_root=frozen, native_binary=binary)
            elf.assert_not_called()
        self.assertFalse(output.exists())

    def test_vendor_changed_after_copy_is_rejected_before_archive(self):
        live, frozen, binary, metadata = self.build_inputs(); vendor = self.vendor_tree(frozen)
        changed = vendor / 'winit/src/lib.rs'; original = B.copy_new
        def copy_and_change(source, target, *args, **kwargs):
            original(source, target, *args, **kwargs)
            if source == changed: changed.write_text('// PUBLIC vendor changed after copying\n')
        with patch.object(B, 'copy_new', side_effect=copy_and_change), self.assertRaisesRegex(ValueError, 'sources changed'):
            self.synthetic_build(live, metadata, source_root=frozen, native_binary=binary)
        self.assertEqual(list((live / 'app/evidence/PUBLIC-new-package').glob('*.tar.gz')), [])


if __name__ == '__main__': unittest.main()
