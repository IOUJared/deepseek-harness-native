#!/usr/bin/env python3
"""Finite report/phase guard checks; no process, GUI, crash or disk-durability claims."""
import asyncio
import copy
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import AsyncMock, patch

SPEC = importlib.util.spec_from_file_location('interruption_guards', Path(__file__).with_name('qualify-draft-interruption.py'))
GUARDS = importlib.util.module_from_spec(SPEC)
with patch('subprocess.Popen', side_effect=AssertionError('pure suite must not launch')):
    SPEC.loader.exec_module(GUARDS)


def report(mode='hold', point='before-temp-sync'):
    return {'status': 'passed', 'version': '0.2.1-alpha.1', 'publicOnly': True, 'pid': 4242,
            'sessionId': 'PUBLIC-session', 'mode': mode, 'checkpoint': point, 'promptSerial': 0,
            'actualAppPromptCommands': 0, 'unexpectedWork': 0, 'manualBusinessMessagesForwarded': 0,
            'catalogReads': 1, 'catalogSucceeded': True, 'windowBackend': 'wayland',
            'recordKinds': ['permission/preset', 'sandbox/mode', 'approval/policy'],
            'kernelBlockedFsyncQualified': False, 'powerLossQualified': False, 'physicalInputQualified': False,
            'networkTraceQualified': False, 'stop': copy.deepcopy(GUARDS.STOP), 'draftIoJoined': True,
            'draftsCleanAtExit': True, 'expectedSnapshotDurable': True, 'saveUnfinished': True,
            'stoppedWhileSaveUnfinished': True, 'exitAnywayRefused': True, 'savedOutcomes': 3,
            'guiHeartbeatsWhileBlocked': 3, 'recoveredNeedsReview': True,
            'crashGateLatchedUntilOwnedKill': mode == 'crash'}


def cleanup():
    return {'rootPidfdPinned': True, 'rootWasRunningAtCleanup': False, 'ownedChildFallbackUsed': False,
            'remainingBeforeExternalCleanup': [], 'remainingAfterExternalCleanup': [], 'killEscalated': False,
            'rootReaped': True, 'cleanupTimedOut': False, 'cleanupProbeOrSignalFailed': False,
            'observedCleanupComplete': True, 'termBudgetSeconds': 2, 'killBudgetSeconds': 2}


class InterruptionGuards(unittest.TestCase):
    def test_hold_and_recover_are_distinct(self):
        self.assertTrue(GUARDS.valid_finished(report(), 4242, 'hold', 'before-temp-sync'))
        recovered = report('recover')
        recovered['savedOutcomes'] = 0
        self.assertTrue(GUARDS.valid_finished(recovered, 4242, 'recover', 'before-temp-sync'))
        self.assertFalse(GUARDS.valid_finished(report('unknown'), 4242, 'unknown', 'before-temp-sync'))
        self.assertFalse(GUARDS.valid_finished(report(), 4242, 'recover', 'before-temp-sync'))

    def test_crash_observation_is_not_save_success(self):
        value = report('crash')
        value.update(status='checkpoint-observed', savedOutcomes=1)
        self.assertTrue(GUARDS.valid_observation(value, 4242, 'before-temp-sync'))
        for key, wrong in [('saveUnfinished', False), ('savedOutcomes', 3), ('mode', 'hold'), ('status', 'passed')]:
            changed = copy.deepcopy(value)
            changed[key] = wrong
            self.assertFalse(GUARDS.valid_observation(changed, 4242, 'before-temp-sync'))
        value['errorCode'] = 'failed'
        self.assertFalse(GUARDS.valid_observation(value, 4242, 'before-temp-sync'))

    def test_crash_observation_requires_a_permanent_gate(self):
        value = report('crash')
        value.update(status='checkpoint-observed', savedOutcomes=1)
        for wrong in [False, 1, 0, None, 'true']:
            changed = copy.deepcopy(value)
            changed['crashGateLatchedUntilOwnedKill'] = wrong
            self.assertFalse(GUARDS.valid_observation(changed, 4242, 'before-temp-sync'), wrong)
        del value['crashGateLatchedUntilOwnedKill']
        self.assertFalse(GUARDS.valid_observation(value, 4242, 'before-temp-sync'))

    def test_crash_observation_rejects_nonzero_live_app_intent_counters(self):
        value = report('crash')
        value.update(status='checkpoint-observed', savedOutcomes=1)
        self.assertTrue(GUARDS.valid_observation(value, 4242, 'before-temp-sync'))
        for key in ['promptSerial', 'actualAppPromptCommands']:
            for wrong in [1, 2, True, False, 0.0, None, '0']:
                changed = copy.deepcopy(value)
                changed[key] = wrong
                self.assertFalse(GUARDS.valid_observation(changed, 4242, 'before-temp-sync'), (key, wrong))

    def test_cleanup_modes_do_not_confuse_external_cleanup_with_graceful_stop(self):
        value = cleanup()
        for mode in ['hold', 'recover', 'crash']:
            self.assertTrue(GUARDS.valid_cleanup(value, mode))
        self.assertFalse(GUARDS.valid_cleanup(value, 'unsupported'))
        value['remainingBeforeExternalCleanup'] = [(4243, 7)]
        self.assertFalse(GUARDS.valid_cleanup(value, 'hold'))
        self.assertFalse(GUARDS.valid_cleanup(value, 'recover'))
        self.assertTrue(GUARDS.valid_cleanup(value, 'crash'))
        value['killEscalated'] = True
        self.assertTrue(GUARDS.valid_cleanup(value, 'crash'))
        value['remainingAfterExternalCleanup'] = [(4243, 7)]
        self.assertFalse(GUARDS.valid_cleanup(value, 'crash'))

    def test_cleanup_success_fields_are_required_and_strictly_typed(self):
        value = cleanup()
        flags = ['rootPidfdPinned', 'rootReaped', 'observedCleanupComplete', 'rootWasRunningAtCleanup',
                 'ownedChildFallbackUsed', 'cleanupTimedOut', 'cleanupProbeOrSignalFailed']
        for key in flags:
            changed = copy.deepcopy(value)
            del changed[key]
            self.assertFalse(GUARDS.valid_cleanup(changed, 'crash'), key)
            for wrong in [not value[key], int(value[key]), None, 'false']:
                changed = copy.deepcopy(value)
                changed[key] = wrong
                self.assertFalse(GUARDS.valid_cleanup(changed, 'crash'), (key, wrong))
        for key in ['remainingBeforeExternalCleanup', 'remainingAfterExternalCleanup', 'killEscalated']:
            changed = copy.deepcopy(value)
            del changed[key]
            self.assertFalse(GUARDS.valid_cleanup(changed, 'crash'), key)
            changed = copy.deepcopy(value)
            changed[key] = () if key.startswith('remaining') else 0
            self.assertFalse(GUARDS.valid_cleanup(changed, 'crash'), key)
        value['killEscalated'] = True
        self.assertFalse(GUARDS.valid_cleanup(value, 'hold'))
        self.assertFalse(GUARDS.valid_cleanup(value, 'recover'))
        for wrong in [None, [], True, 'text']:
            self.assertFalse(GUARDS.valid_cleanup(wrong, 'crash'))

    def test_counts_flags_pids_are_strict(self):
        for key in ['promptSerial', 'actualAppPromptCommands', 'unexpectedWork', 'manualBusinessMessagesForwarded', 'catalogReads', 'savedOutcomes', 'guiHeartbeatsWhileBlocked', 'pid']:
            for wrong in [True, False, 0.0, -1, None, '0']:
                changed = report()
                changed[key] = wrong
                self.assertFalse(GUARDS.valid_finished(changed, 4242, 'hold', 'before-temp-sync'), (key, wrong))
        for key in ['draftIoJoined', 'draftsCleanAtExit', 'expectedSnapshotDurable', 'stoppedWhileSaveUnfinished', 'exitAnywayRefused']:
            changed = report()
            changed[key] = 1
            self.assertFalse(GUARDS.valid_finished(changed, 4242, 'hold', 'before-temp-sync'))

    def test_all_stop_fields_are_required_and_typed(self):
        for key in GUARDS.STOP:
            changed = report()
            del changed['stop'][key]
            self.assertFalse(GUARDS.valid_finished(changed, 4242, 'hold', 'before-temp-sync'))
            changed = report()
            changed['stop'][key] = int(GUARDS.STOP[key]) if type(GUARDS.STOP[key]) is bool else False
            self.assertFalse(GUARDS.valid_finished(changed, 4242, 'hold', 'before-temp-sync'))
        changed = report()
        changed['stop']['extra'] = 0
        self.assertFalse(GUARDS.valid_finished(changed, 4242, 'hold', 'before-temp-sync'))

    def test_forbidden_work_and_expanded_claims_refuse(self):
        for kind in GUARDS.FORBIDDEN + ['llm/request', 'session/title-llm-request']:
            changed = report()
            changed['recordKinds'].append(kind)
            self.assertFalse(GUARDS.valid_finished(changed, 4242, 'hold', 'before-temp-sync'))
        for flag in ['kernelBlockedFsyncQualified', 'powerLossQualified', 'physicalInputQualified', 'networkTraceQualified']:
            changed = report()
            changed[flag] = True
            self.assertFalse(GUARDS.valid_finished(changed, 4242, 'hold', 'before-temp-sync'))

    def test_missing_invalid_subject_point_and_container_refuse(self):
        for wrong in [None, [], True, 'text']:
            self.assertFalse(GUARDS.common(wrong, 4242, 'before-temp-sync'))
        for subject in ['', None, True, 'PUBLIC\nunsafe', 'x' * 129]:
            changed = report()
            changed['sessionId'] = subject
            self.assertFalse(GUARDS.valid_finished(changed, 4242, 'hold', 'before-temp-sync'))
        changed = report(point='unsupported')
        self.assertFalse(GUARDS.valid_finished(changed, 4242, 'hold', 'unsupported'))

    def test_expected_document_changes_only_at_successful_rename(self):
        self.assertEqual(len(GUARDS.POINTS), 9)
        for point in GUARDS.POINTS[:5]:
            self.assertEqual(GUARDS.expected(point), GUARDS.OLD)
        for point in GUARDS.POINTS[5:]:
            self.assertEqual(GUARDS.expected(point), GUARDS.NEW)
        self.assertNotEqual(GUARDS.NEW, GUARDS.LATEST)


class InterruptionPhaseGuards(unittest.IsolatedAsyncioTestCase):
    async def test_graceful_phase_rejects_survivors_even_after_successful_external_cleanup(self):
        # Every process, descriptor, filesystem and owned-cleanup operation is mocked.
        for mode in ['hold', 'recover']:
            for survivors in [[], [(4243, 7)]]:
                with self.subTest(mode=mode, survivors=survivors):
                    child = SimpleNamespace(pid=4242, returncode=0)
                    app = report(mode)
                    if mode == 'recover':
                        app['savedOutcomes'] = 0
                    cleaned = cleanup()
                    cleaned['remainingBeforeExternalCleanup'] = survivors
                    root = {'pid': child.pid, 'start_ticks': 6}
                    with patch.object(GUARDS.OWNED.NATIVE, 'environment', return_value={}), \
                         patch.object(GUARDS.os, 'open', side_effect=[10, 11]), \
                         patch.object(GUARDS.os, 'close'), \
                         patch.object(GUARDS.subprocess, 'Popen', return_value=child) as spawn, \
                         patch.object(GUARDS.OWNED, 'pin_root', return_value=(root, 12)), \
                         patch.object(GUARDS.OWNED, 'cleanup_owned', new_callable=AsyncMock, return_value=cleaned) as reap, \
                         patch.object(GUARDS, 'private_json', return_value=app), \
                         patch.object(GUARDS.OWNED, 'private_json') as write_report:
                        result = await GUARDS.phase(Path('/PUBLIC-mocked'), Path('/mocked-binary'), Path('/mocked-runtime'),
                                                    mode, 'before-temp-sync', asyncio.Event())
                    self.assertEqual(result['status'], 'failed' if survivors else 'passed')
                    self.assertEqual(result['exitCode'], 0)
                    self.assertTrue(result['cleanup']['observedCleanupComplete'])
                    spawn.assert_called_once()
                    reap.assert_awaited_once()
                    write_report.assert_called_once()


if __name__ == '__main__':
    unittest.main()
