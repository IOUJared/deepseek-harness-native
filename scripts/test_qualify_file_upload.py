import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest
import uuid

SPEC = importlib.util.spec_from_file_location('file_upload_qualification', Path(__file__).with_name('qualify-file-upload.py'))
QUAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(QUAL)


def report():
    value = {'status': 'passed', 'versions': ['0.2.1-alpha.1'] * 2, 'explicitModelPrompts': 0,
             'modelCatalogRequested': False, 'processWideNetworkTrace': False, 'uiExercised': False,
             'liveAgentObserved': True, 'missingTargetRefused': True, 'coldBeforeUpload': True, 'resumedAfterUpload': True,
             'stops': [{'exited': True, 'graceful': True, 'exitCode': 0, 'containmentUnknown': False,
                        'observedDescendantsRemaining': 0} for _ in range(2)],
             'postUploadSnapshots': [['permission/preset', 'sandbox/mode', 'approval/policy'],
                                     ['permission/preset', 'sandbox/mode', 'approval/policy', 'session/end-seed']], 'uploads': []}
    for case in QUAL.CASES:
        data = QUAL.source(case)
        value['uploads'].append({'case': case, 'value': {'receiptId': str(uuid.uuid4()),
                                 'file': {'attachmentId': 'sha256:' + hashlib.sha256(data).hexdigest(),
                                          'name': QUAL.NAMES[case], 'bytes': len(data)}}})
    return value


class UploadQualificationTests(unittest.TestCase):
    def test_closed_boolean_stop_snapshot_and_case_requirements(self):
        good = report()
        self.assertTrue(QUAL.valid_sdk(good))
        good['explicitModelPrompts'] = False
        self.assertFalse(QUAL.valid_sdk(good))
        good = report(); good['postUploadSnapshots'][0] = ['step/start']
        self.assertFalse(QUAL.valid_sdk(good))
        good = report(); good['stops'][0]['exitCode'] = False
        self.assertFalse(QUAL.valid_sdk(good))
        good = report(); good['uploads'].reverse()
        self.assertFalse(QUAL.valid_sdk(good))

    def test_exact_fixture_metadata_receipt_uniqueness_and_private_leaf(self):
        for field, bad in [('name', '../escape'), ('bytes', True), ('attachmentId', 'sha256:wrong')]:
            good = report(); good['uploads'][0]['value']['file'][field] = bad
            self.assertFalse(QUAL.valid_sdk(good))
        good = report(); good['uploads'][0]['value']['file']['extra'] = 'PUBLIC_EXTRA'
        self.assertFalse(QUAL.valid_sdk(good))
        good = report(); good['uploads'][1]['value']['receiptId'] = good['uploads'][0]['value']['receiptId']
        self.assertFalse(QUAL.valid_sdk(good))

    def test_independent_private_canonical_alias_bytes_and_modes(self):
        with tempfile.TemporaryDirectory(prefix='native-upload-qualification-') as directory:
            output = Path(directory)
            root = output / 'harness' / 'attachments' / 'v1'
            for case in QUAL.CASES:
                data = QUAL.source(case); digest = hashlib.sha256(data).hexdigest()
                canonical = root / 'file-objects' / digest[:2] / digest
                alias = root / 'files' / digest[:2] / digest / QUAL.NAMES[case]
                for path in [canonical.parent, alias.parent]:
                    path.mkdir(parents=True, mode=0o700, exist_ok=True)
                    current = path
                    while current != output:
                        current.chmod(0o700); current = current.parent
                if not canonical.exists():
                    canonical.write_bytes(data); canonical.chmod(0o400)
                alias.hardlink_to(canonical)
            self.assertEqual(len(QUAL.verify_storage(output, report())), 5)
            canonical = root / 'file-objects' / hashlib.sha256(b'').hexdigest()[:2] / hashlib.sha256(b'').hexdigest()
            canonical.chmod(0o600)
            with self.assertRaises(ValueError):
                QUAL.verify_storage(output, report())

    def test_read_verifier_rejects_content_mismatch_and_symlink(self):
        with tempfile.TemporaryDirectory(prefix='native-upload-negative-') as directory:
            root = Path(directory)
            file = root / 'fixture'; file.write_bytes(b'PUBLIC_OTHER'); file.chmod(0o400)
            with self.assertRaises(ValueError):
                QUAL.verify_file(file, b'PUBLIC_RIGHT')
            link = root / 'link'; link.symlink_to(file)
            with self.assertRaises(OSError):
                QUAL.verify_file(link, b'PUBLIC_OTHER')


if __name__ == '__main__':
    unittest.main()
