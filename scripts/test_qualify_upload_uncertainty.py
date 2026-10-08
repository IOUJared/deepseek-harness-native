import copy
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('upload_uncertainty', Path(__file__).with_name('qualify-upload-uncertainty.py'))
QUAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(QUAL)


def report():
    value = {'status': 'passed', 'phase': 'complete', 'fatal': False, 'testOnlyInstanceBarriers': True,
             'failurePoint': 'none', 'injectionReached': False,
             'facts': dict.fromkeys(QUAL.FACTS, True), 'counts': dict.fromkeys(QUAL.COUNTS, 0),
             'bridge': {'active': 0, 'complete': 3, 'errors': 0, 'methodActive': 0},
             'cleanup': {'handlersJoined': True, 'descriptorsRestored': True, 'observerDisposersRan': True,
                         'ownedHandlesDisposed': True, 'registryReturnedToBaseline': True, 'hostShutdown': True, 'drainTimedOut': False},
             'files': []}
    for key in ['scriptedModelPromptOrCatalog', 'scriptedSignInOrCredentialSave', 'providerSubstituted',
                'processWideNetworkTrace', 'rustSdkExercised', 'uiExercised']:
        value[key] = False
    for case in QUAL.CASES:
        data = QUAL.source(case)
        value['files'].append({'case': case, 'file': {'attachmentId': 'sha256:' + hashlib.sha256(data).hexdigest(),
                                                     'name': f'PUBLIC {case}.bin', 'bytes': len(data)}})
    return value


def failed(point):
    value = report()
    value.update(status='failed', failurePoint=point, injectionReached=True,
                 phase='ack-lost-after-real-staging' if point == 'ack-held' else 'agent-disposed-after-real-save-before-upload-commit')
    for key in ['handlersJoined', 'descriptorsRestored', 'registryReturnedToBaseline']:
        value['facts'][key] = False
    future = ['receiptRetainedAfterAckLoss', 'storedBeforeAgentDisposal'] if point == 'ack-held' else ['disposedBeforeStorePromiseReturned', 'lateDisposalRejectedNoReceipt']
    for key in future:
        value['facts'][key] = False
    if point == 'ack-held':
        value['files'] = value['files'][:1]; value['bridge']['complete'] = 1
    return {'status': 'failed', 'exitCode': 1, 'sourceUnchanged': True, 'identityPinFailures': 0,
            'timedOut': False, 'cancelled': False, 'barriers': value,
            'cleanup': {'rootPidfdPinned': True, 'rootReaped': True, 'observedCleanupComplete': True,
                        'rootWasRunningAtCleanup': False, 'ownedChildFallbackUsed': False, 'killEscalated': False,
                        'cleanupTimedOut': False, 'cleanupProbeOrSignalFailed': False,
                        'remainingBeforeExternalCleanup': [], 'remainingAfterExternalCleanup': []}}


class UploadUncertaintyQualificationTests(unittest.TestCase):
    def test_success_requires_exact_facts_counter_types_and_cleanup(self):
        self.assertTrue(QUAL.valid_barriers(report()))
        for section, key, bad in [('facts', 'serverObservedDisconnect', False), ('facts', 'handlersJoined', 1),
                                  ('counts', 'requestHeaders', True), ('counts', 'unexpectedWake', 1),
                                  ('bridge', 'active', 1), ('bridge', 'complete', 3.0),
                                  ('cleanup', 'descriptorsRestored', False), ('cleanup', 'drainTimedOut', True)]:
            value = report(); value[section][key] = bad
            with self.subTest(section=section, key=key):
                self.assertFalse(QUAL.valid_barriers(value))
        value = report(); del value['facts']['storedBeforeAbort']
        self.assertFalse(QUAL.valid_barriers(value))
        value = report(); value['counts']['unknownEvent'] = 0
        self.assertFalse(QUAL.valid_barriers(value))

    def test_no_provider_swap_or_model_or_unintended_surface_claim(self):
        for key in ['scriptedModelPromptOrCatalog', 'scriptedSignInOrCredentialSave', 'providerSubstituted',
                    'processWideNetworkTrace', 'rustSdkExercised', 'uiExercised']:
            value = report(); value[key] = True
            self.assertFalse(QUAL.valid_barriers(value))
        for key, bad in [('failurePoint', 'ack-held'), ('injectionReached', True), ('fatal', True)]:
            value = report(); value[key] = bad
            self.assertFalse(QUAL.valid_barriers(value))

    def test_only_independent_fixture_metadata_and_order_are_accepted(self):
        for key, bad in [('name', '../escape'), ('bytes', True), ('attachmentId', 'sha256:OTHER')]:
            value = report(); value['files'][0]['file'][key] = bad
            self.assertFalse(QUAL.valid_barriers(value))
        value = report(); value['files'][0]['file']['path'] = '/PUBLIC_PATH'
        self.assertFalse(QUAL.valid_barriers(value))
        value = report(); value['files'].reverse()
        self.assertFalse(QUAL.valid_barriers(value))
        self.assertFalse(QUAL.valid_barriers({}))
        self.assertFalse(QUAL.valid_barriers(None))

    def test_injected_failure_requires_exact_phase_reached_and_no_external_cleanup(self):
        for point in ['ack-held', 'store-held']:
            good = failed(point)
            self.assertTrue(QUAL.valid_expected_failure(good, point))
            self.assertFalse(QUAL.valid_barriers(good['barriers']))
            for section, key, bad in [('root', 'exitCode', 0), ('root', 'identityPinFailures', True),
                                      ('root', 'timedOut', True), ('root', 'sourceUnchanged', False),
                                      ('cleanup', 'rootWasRunningAtCleanup', True), ('cleanup', 'killEscalated', True),
                                      ('cleanup', 'remainingBeforeExternalCleanup', ['PUBLIC_OWNED_PID']),
                                      ('barriers', 'injectionReached', False), ('barriers', 'phase', 'boot')]:
                value = copy.deepcopy(good)
                (value if section == 'root' else value[section])[key] = bad
                self.assertFalse(QUAL.valid_expected_failure(value, point))
            value = copy.deepcopy(good); value['barriers']['cleanup']['handlersJoined'] = False
            self.assertFalse(QUAL.valid_expected_failure(value, point))
        self.assertFalse(QUAL.valid_expected_failure(failed('ack-held'), 'store-held'))
        self.assertFalse(QUAL.valid_expected_failure({}, 'unknown'))

    def test_common_storage_oracle_reads_complete_bytes_modes_and_hardlink_identity(self):
        with tempfile.TemporaryDirectory(prefix='native-upload-uncertainty-unit-') as directory:
            output = Path(directory)
            root = output / 'harness' / 'attachments' / 'v1'
            fixtures = [(case, QUAL.source(case), f'PUBLIC {case}.bin') for case in QUAL.CASES]
            for _case, data, name in fixtures:
                digest = hashlib.sha256(data).hexdigest()
                canonical = root / 'file-objects' / digest[:2] / digest
                alias = root / 'files' / digest[:2] / digest / name
                for path in [canonical.parent, alias.parent]:
                    path.mkdir(parents=True, mode=0o700)
                    current = path
                    while current != output:
                        current.chmod(0o700); current = current.parent
                canonical.write_bytes(data); canonical.chmod(0o400); alias.hardlink_to(canonical)
            self.assertEqual(len(QUAL.FILES.verify_local_cases(output, fixtures)), 3)
            alias.chmod(0o600)
            with self.assertRaises(ValueError):
                QUAL.FILES.verify_local_cases(output, fixtures)

    def test_storage_oracle_rejects_symlink_above_versioned_root(self):
        for link_at in ['harness', 'attachments']:
            with self.subTest(link_at=link_at), tempfile.TemporaryDirectory(prefix='native-upload-ancestor-') as directory:
                parent = Path(directory)
                output = parent / 'owned'; output.mkdir(mode=0o700)
                foreign = parent / 'foreign'; foreign.mkdir(mode=0o700)
                if link_at == 'harness':
                    (output / 'harness').symlink_to(foreign, target_is_directory=True)
                    root = foreign / 'attachments' / 'v1'
                else:
                    (output / 'harness').mkdir(mode=0o700)
                    (output / 'harness' / 'attachments').symlink_to(foreign, target_is_directory=True)
                    root = foreign / 'v1'
                data = QUAL.source('ack-loss'); digest = hashlib.sha256(data).hexdigest()
                canonical = root / 'file-objects' / digest[:2] / digest
                alias = root / 'files' / digest[:2] / digest / 'PUBLIC ack-loss.bin'
                for path in [canonical.parent, alias.parent]:
                    path.mkdir(parents=True, mode=0o700)
                    current = path
                    while current != foreign:
                        current.chmod(0o700); current = current.parent
                canonical.write_bytes(data); canonical.chmod(0o400); alias.hardlink_to(canonical)
                with self.assertRaises(ValueError):
                    QUAL.FILES.verify_local_cases(output, [('ack-loss', data, 'PUBLIC ack-loss.bin')])


if __name__ == '__main__':
    unittest.main()
