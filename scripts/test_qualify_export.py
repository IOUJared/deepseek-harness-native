import importlib.util
import io
import json
from pathlib import Path
import struct
import sys
import unittest
from unittest.mock import patch
import zipfile

SPEC = importlib.util.spec_from_file_location('qualified_export', Path(__file__).with_name('qualify-composed-export.py'))
Q = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = Q
SPEC.loader.exec_module(Q)


def header(**changes):
    value = dict(type='session', version=4, id='PUBLIC-session', createdAt=1,
                 isSeeded=False, delegationDepth=0)
    value.update(changes)
    return json.dumps(value).encode() + b'\n'


def archive(root=None, name='session.v4.jsonl', extra=False):
    output = io.BytesIO()
    with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_DEFLATED) as writer:
        writer.writestr(name, header() if root is None else root)
        if extra:
            writer.writestr('files/PUBLIC-unused.bin', b'PUBLIC')
    return output.getvalue()


def inspect(data):
    with patch.object(Q.REG, 'private_regular', return_value=data):
        return Q.inspect_archive(Path('/PUBLIC'), {'privateArchiveFileVerified': {'bytes': len(data)}})


def app():
    value = dict(status='passed', pid=7, applicationScale=1.0, windowBackend='wayland',
                 targetCount=1, zipArtifact=Q.ARCHIVE, modelPrompts=0)
    for key in ('publicFixtureOnly', 'scriptedUiMessages', 'realWorkerReady', 'freshEmptyBaselineObserved',
                'ownedWorkspaceCreated', 'oneOrdinaryPublicSessionCreated', 'selectedBaselineObserved',
                'matchingSavedReceiptObserved', 'rootOnlyExportConfirmed', 'noOverwriteReceiptObserved',
                'privateZipVerified', 'exactBytesUnchanged', 'samePathBytesAndMetadataUnchanged',
                'twoExactReceiptsObserved', 'gracefulStopped', 'actualCloseUiDispatched', 'actualCloseInspectionObserved'):
        value[key] = True
    for key in ('modelCatalogRequested', 'browserSignInRequested', 'apiKeySaveRequested',
                'pluginWritesRequested', 'settingsWritesRequested', 'manualBusinessMessagesForwarded',
                'sessionDeletionRequested', 'stopActivityRequested', 'includeDescendants', 'realCredentialsUsed'):
        value[key] = False
    value['stages'] = [dict(name='save-new-public-archive', matchingExportedReceipt=True,
                           controllerPendingRetired=True, enteredPathCleared=True, saved=True,
                           notCreated=False, savedBytes=200,
                           receipt=dict(epoch=1, generation=1, serial=1, editor=2, attempt=1)),
                       dict(name='same-path-no-overwrite', matchingExportedReceipt=True,
                            controllerPendingRetired=True, enteredPathCleared=True, saved=False,
                            notCreated=True, savedBytes=None,
                            receipt=dict(epoch=1, generation=1, serial=2, editor=4, attempt=2))]
    return value


class ExportQualificationTests(unittest.TestCase):
    def test_actual_content_not_just_zip_prefix_is_required(self):
        self.assertFalse(inspect(b'PK\x03\x04')['passed'])
        self.assertFalse(inspect(archive(root=b''))['passed'])
        self.assertFalse(inspect(archive(root=b'PUBLIC-not-JSON\n'))['passed'])

    def test_public_blank_header_and_zero_events_pass_without_raw_output(self):
        result = inspect(archive())
        self.assertTrue(result['passed'])
        self.assertEqual(result['headerLineCount'], 1)
        self.assertEqual(result['eventRecordCount'], 0)
        self.assertNotIn('PUBLIC-session', json.dumps(result))
        self.assertFalse(result['sessionIdentityAuthenticated'])

    def test_only_fresh_blank_root_entry_profile(self):
        for data in (archive(name='session.v2.jsonl'), archive(extra=True), archive(name='../session.v4.jsonl')):
            self.assertFalse(inspect(data)['passed'])

    def test_current_ordinary_header_profile_not_just_arbitrary_json(self):
        for changes in ({'version': 2}, {'version': True}, {'id': ''}, {'type': 'other'},
                        {'isSeeded': True}, {'origin': {}}, {'parentSession': 'PUBLIC'}, {'delegationDepth': 1}):
            self.assertFalse(inspect(archive(header(**changes)))['passed'])

    def test_jsonl_requires_terminated_utf8_dictionary_records(self):
        for root in (header().rstrip(b'\n'), header() + b'[]\n', header() + b'\xff\n'):
            self.assertFalse(inspect(archive(root))['passed'])

    def test_actual_bounded_expansion_and_jsonl_line_bounds(self):
        self.assertFalse(inspect(archive(b'A' * (Q.MAX_ROOT + 1)))['passed'])
        self.assertFalse(inspect(archive(header() + b'{}\n' * 256))['passed'])
        self.assertFalse(inspect(archive(header() + json.dumps({'PUBLIC': 'x' * 65536}).encode() + b'\n'))['passed'])

    def test_crc_disagreement_is_refused_independently(self):
        data = bytearray(archive())
        central = data.index(b'PK\x01\x02')
        struct.pack_into('<I', data, central + 16, 123)
        self.assertFalse(inspect(bytes(data))['passed'])

    def test_deflate_eof_and_truncation_are_required(self):
        data = bytearray(archive())
        name_len, extra_len = struct.unpack_from('<HH', data, 26)
        start = 30 + name_len + extra_len
        data[start] = 0xff
        self.assertFalse(inspect(bytes(data))['passed'])
        self.assertFalse(inspect(archive()[:-12])['passed'])

    def test_independent_file_receipt_count_match_is_required(self):
        with patch.object(Q.REG, 'private_regular', return_value=archive()):
            self.assertFalse(Q.inspect_archive(Path('/PUBLIC'), {})['passed'])
            self.assertFalse(Q.inspect_archive(Path('/PUBLIC'), {'privateArchiveFileVerified': 'invalid'})['passed'])

    def test_exact_two_receipts_and_negative_outcome_are_required(self):
        self.assertTrue(Q.valid_app(app(), 7, 1.0))
        for field, value in (('epoch', 2), ('generation', 2), ('serial', 1), ('editor', 2), ('attempt', 1)):
            changed = app(); changed['stages'][1]['receipt'][field] = value
            self.assertFalse(Q.valid_app(changed, 7, 1.0))
        changed = app(); changed['stages'][1]['notCreated'] = False
        self.assertFalse(Q.valid_app(changed, 7, 1.0))

    def test_physical_wayland_public_zero_model_scope_is_required(self):
        for field, value in (('modelPrompts', 1), ('realCredentialsUsed', True), ('windowBackend', 'x11'),
                             ('includeDescendants', True), ('exactBytesUnchanged', False), ('gracefulStopped', False)):
            changed = app(); changed[field] = value
            self.assertFalse(Q.valid_app(changed, 7, 1.0))
        self.assertFalse(Q.valid_app(app(), 8, 1.0))
        self.assertFalse(Q.valid_app(app(), 7, 1.25))

    def test_capture_requires_complete_owned_png_and_metadata_match(self):
        value = {'exportReviewCapture': {'saved': True, 'artifact': 'export-review.png', 'physicalSize': [1, 1]},
                 'exportSavedCapture': {'saved': True, 'artifact': 'export-saved.png', 'physicalSize': [1, 1]}}
        with patch.object(Q.REG, 'private_regular', return_value=b'\x89PNG\r\n\x1a\n'):
            self.assertFalse(Q.valid_captures(Path('/PUBLIC'), value))


if __name__ == '__main__':
    unittest.main()
