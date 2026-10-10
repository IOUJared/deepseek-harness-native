#!/usr/bin/env python3
"""Pure report/private-artifact regressions; no Cargo, Host, GUI or process spawning."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

SCRIPT = Path(__file__).with_name('qualify-draft-restart.py')
SPEC = importlib.util.spec_from_file_location('draft_restart_validator', SCRIPT)
DRIVER = importlib.util.module_from_spec(SPEC)
with mock.patch('subprocess.Popen', side_effect=AssertionError('pure suite must not spawn')):
    SPEC.loader.exec_module(DRIVER)


def app(mode='write', pid=7001):
    result = {key: True for key in ['publicOnly', 'actualAppExercised', 'actualDraftTasks', 'wholeProfileUnchanged',
              'joinedOutcomeNotBlockedSyscall', 'draftRecoveryLoaded', 'draftIoJoined', 'draftsCleanAtExit',
              'latestSnapshotDurable', 'modelCatalogSucceeded', 'inspectionObserved', 'completePUBLICSnapshotObserved',
              'gracefulStopped', 'stoppedWhileBusy', 'exitAnywayBlockedWhileBusy', 'heldOutcomeReleased']}
    result.update({key: False for key in ['backendMetricsAvailable', 'networkTraceQualified', 'physicalInputQualified',
                   'crashDurabilityQualified', 'driverPromptResumeModelSigninAllowed']})
    result.update({key: 0 for key in ['actualAppPromptCommands', 'promptSerial', 'unexpectedWork', 'manualBusinessMessagesForwarded']})
    result.update(status='passed', mode=mode, pid=pid, sessionId='session-PUBLIC-validator', version='0.2.1-alpha.1',
                  windowBackend='wayland', modelCatalogReads=1, stop=copy.deepcopy(DRIVER.STOP), savedOutcomes=2,
                  heldRevision=1, latestSavedRevision=2, observedPUBLICRecordKinds=['permission/preset', 'sandbox/mode', 'approval/policy'])
    if mode == 'recover':
        result.update(recoveredNeedsReview=True, coldExplicitSelectOnly=True, coldRosterContainedSubject=True,
                      sendAllowed=False, savedOutcomes=0)
    return result


class Reports(unittest.TestCase):
    def test_write_and_recover_accept_real_report_types(self):
        self.assertTrue(DRIVER.valid(app(), 'write', 7001))
        self.assertTrue(DRIVER.valid(app('recover'), 'recover', 7001))

    def test_malformed_root_or_mode_refused_without_throwing(self):
        for value in [None, [], 1, True, 'PUBLIC', {}, {'status': 'passed'}]:
            with self.subTest(value=value):
                self.assertFalse(DRIVER.valid(value, 'write', 7001))
        self.assertFalse(DRIVER.valid(app(), 'unknown', 7001))
        self.assertFalse(DRIVER.valid(app(), 'recover', 7001))

    def test_all_stop_fields_require_exact_type_and_value(self):
        for key, expected in DRIVER.STOP.items():
            wrong_values = [None, '0', [], 1, 0] if type(expected) is bool else [None, '0', [], False, 0.0, -1, 1]
            for value in wrong_values:
                with self.subTest(key=key, value=value):
                    candidate = app()
                    candidate['stop'][key] = value
                    self.assertFalse(DRIVER.valid(candidate, 'write', 7001))
        for value in [None, [], {}, {**DRIVER.STOP, 'extra': 0}]:
            candidate = app()
            candidate['stop'] = value
            self.assertFalse(DRIVER.valid(candidate, 'write', 7001))

    def test_counts_refuse_bool_float_negative_and_wrong_total(self):
        for key in ['actualAppPromptCommands', 'promptSerial', 'unexpectedWork', 'manualBusinessMessagesForwarded', 'modelCatalogReads', 'savedOutcomes']:
            for value in [False, True, 0.0, 1.0, -1, '0', None, (1 << 64), 3]:
                with self.subTest(key=key, value=value):
                    candidate = app()
                    candidate[key] = value
                    self.assertFalse(DRIVER.valid(candidate, 'write', 7001))
        candidate = app('recover')
        candidate['savedOutcomes'] = 1
        self.assertFalse(DRIVER.valid(candidate, 'recover', 7001))

    def test_pid_and_revision_are_bounded_integer_identities(self):
        for value in [True, False, 7001.0, -1, 0, '7001', None, 1 << 31]:
            with self.subTest(value=value):
                candidate = app()
                candidate['pid'] = value
                self.assertFalse(DRIVER.valid(candidate, 'write', 7001))
                self.assertFalse(DRIVER.valid(app(), 'write', value))
        for key in ['heldRevision', 'latestSavedRevision']:
            for value in [True, 1.0, -1, 0, '1', None, 1 << 64]:
                candidate = app()
                candidate[key] = value
                self.assertFalse(DRIVER.valid(candidate, 'write', 7001))
        for latest in [1, 0]:
            candidate = app()
            candidate['latestSavedRevision'] = latest
            self.assertFalse(DRIVER.valid(candidate, 'write', 7001))

    def test_required_flags_cannot_be_missing_or_integer(self):
        required = [key for key, value in app().items() if type(value) is bool]
        for key in required:
            for replacement in [None, int(app()[key])]:
                candidate = app()
                candidate[key] = replacement
                self.assertFalse(DRIVER.valid(candidate, 'write', 7001), key)
            candidate = app()
            del candidate[key]
            self.assertFalse(DRIVER.valid(candidate, 'write', 7001), key)
        candidate = app('recover')
        candidate['sendAllowed'] = 0
        self.assertFalse(DRIVER.valid(candidate, 'recover', 7001))

    def test_forbidden_record_kinds_empty_or_malformed_records_refused(self):
        for kind in ['step/start', 'step/end', 'request/header', 'tool/call', 'tool/result', 'assistant/message', 'user/message', 'llm/request', 'session/title-llm-request']:
            candidate = app()
            candidate['observedPUBLICRecordKinds'].append(kind)
            self.assertFalse(DRIVER.valid(candidate, 'write', 7001), kind)
        for kinds in [[], None, {}, 'permission/preset', [1], [None]]:
            candidate = app()
            candidate['observedPUBLICRecordKinds'] = kinds
            self.assertFalse(DRIVER.valid(candidate, 'write', 7001))
        candidate = app()
        candidate['errorCode'] = 'PUBLIC prior failure'
        self.assertFalse(DRIVER.valid(candidate, 'write', 7001))

    def test_invalid_subject_refused(self):
        for value in [None, '', 'PUBLIC subject', 'PUBLIC\x00', 'PUBLIC\x7f', 'x' * 129, '\ud800', True]:
            candidate = app()
            candidate['sessionId'] = value
            self.assertFalse(DRIVER.valid(candidate, 'write', 7001))

    def test_cold_restart_correlates_exact_private_document(self):
        first, second = app(), app('recover', 7002)
        manifest = {'version': 1, 'publicOnly': True, 'firstPid': 7001, 'sessionId': first['sessionId']}
        document = {'version': 1, 'drafts': {first['sessionId']: DRIVER.LATEST}}
        self.assertTrue(DRIVER.valid_restart(first, second, manifest, document))
        for corrupted in [None, [], {}, {**document, 'version': True}, {**document, 'version': 1.0},
                          {**document, 'extra': 0}, {'version': 1, 'drafts': {}},
                          {'version': 1, 'drafts': {first['sessionId']: DRIVER.LATEST + 'PUBLIC corrupted'}},
                          {'version': 1, 'drafts': {first['sessionId']: DRIVER.LATEST, 'session-PUBLIC-other': 'PUBLIC'}}]:
            self.assertFalse(DRIVER.valid_restart(first, second, manifest, corrupted))
        for key, value in [('firstPid', True), ('firstPid', 7001.0), ('firstPid', 7002), ('sessionId', 'session-PUBLIC-other'), ('version', True), ('publicOnly', 1)]:
            self.assertFalse(DRIVER.valid_restart(first, second, {**manifest, key: value}, document))
        self.assertFalse(DRIVER.valid_restart(first, app('recover', 7001), manifest, document))
        self.assertFalse(DRIVER.valid_restart(None, second, manifest, document))


class PrivateArtifacts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # PUBLIC finite test artifacts are retained; no cleanup/delete path or account access.
        cls.root = Path(tempfile.mkdtemp(prefix='PUBLIC-draft-validator-')).resolve()
        assert cls.root.is_relative_to(Path(tempfile.gettempdir()).resolve())

    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix='case-', dir=self.root))

    def artifact(self, data=b'{"PUBLIC":true}', name='PUBLIC.json', mode=0o600):
        path = self.directory / name
        with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode), 'wb') as stream:
            stream.write(data)
        path.chmod(mode)
        return path

    def test_bounded_private_json_and_duplicate_keys(self):
        self.assertEqual(DRIVER.private_json(self.artifact()), {'PUBLIC': True})
        for serial, data in enumerate([b'{', b'[]', b'null', b'{"version":1,"version":2}',
                                       b'{"drafts":{"PUBLIC":"one","PUBLIC":"two"}}']):
            with self.subTest(data=data), self.assertRaises(ValueError):
                DRIVER.private_json(self.artifact(data, name=f'PUBLIC-{serial}.json'))

    def test_wrong_mode_empty_or_oversize_file_refused(self):
        for serial, mode in enumerate([0o644, 0o400, 0o700]):
            with self.assertRaises(ValueError):
                DRIVER.private_bytes(self.artifact(name=f'PUBLIC-mode-{serial}', mode=mode))
        with self.assertRaises(ValueError):
            DRIVER.private_bytes(self.artifact(b'', name='PUBLIC-empty'))
        with self.assertRaises(ValueError):
            DRIVER.private_bytes(self.artifact(b'x' * 65537, name='PUBLIC-big'))

    def test_final_symlink_fifo_directory_and_hardlink_refused(self):
        target = self.artifact()
        link = self.directory / 'PUBLIC-link'
        link.symlink_to(target)
        fifo = self.directory / 'PUBLIC-fifo'
        os.mkfifo(fifo, 0o600)
        for path in [link, fifo, self.directory]:
            with self.subTest(path=path.name), self.assertRaises(ValueError):
                DRIVER.private_bytes(path)
        hardlink = self.directory / 'PUBLIC-hardlink'
        os.link(target, hardlink)
        with self.assertRaises(ValueError):
            DRIVER.private_bytes(hardlink)

    def test_opened_inode_mode_or_uid_change_refused(self):
        path = self.artifact()
        info = path.stat()
        for key, value in [('st_ino', info.st_ino + 1), ('st_mode', info.st_mode ^ 0o044), ('st_uid', info.st_uid + 1)]:
            opened = mock.Mock(**{field: getattr(info, field) for field in ['st_dev', 'st_ino', 'st_mode', 'st_uid', 'st_nlink', 'st_size']})
            setattr(opened, key, value)
            with self.subTest(key=key), mock.patch.object(DRIVER.os, 'fstat', return_value=opened), self.assertRaises(ValueError):
                DRIVER.private_bytes(path)


if __name__ == '__main__':
    unittest.main()
