import copy
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('decision_qualification', Path(__file__).with_name('qualify-composed-decisions.py'))
RUN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUN)


class DecisionReports(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.control = self.root / 'fixture/control'
        self.control.mkdir(parents=True)
        self.ready = {'actualAgentId': 'PUBLIC-actual-root', 'rootRegistered': True}
        self.phases = [
            {'phase': 1, 'actualAgentId': 'PUBLIC-actual-root', 'settled': True, 'rootRegistered': True, 'approvalOutcome': 'allowed-once'},
            {'phase': 2, 'actualAgentId': 'PUBLIC-actual-root', 'settled': True, 'expectedAnswerMatched': True, 'answerCount': 3},
            {'phase': 3, 'actualAgentId': 'PUBLIC-actual-root', 'settled': True, 'code': 'ASK_CANCELLED'},
            {'phase': 4, 'actualAgentId': 'PUBLIC-actual-root', 'settled': True, 'code': 'ASK_ABORTED'},
        ]
        self.final = {'disposed': True, 'turnClosed': True, 'approvalAuditPair': True, 'zeroAdmittedModelSteps': True,
                      'counts': {'turnStart': 1, 'turnEnd': 1, 'steps': 0, 'requestHeaders': 0, 'toolCalls': 0,
                                 'toolResults': 0, 'assistantMessages': 0, 'approvalAsked': 1, 'approvalDecided': 1}}
        self.app = {'status': 'passed', 'pid': 4242, 'applicationScale': 1.0, 'nativeAcks': 3, 'hostSettlements': 4, 'windowBackend': 'wayland', 'host': self.final}
        for key in ('publicFixtureOnly', 'scriptedUiMessages', 'realWorkerAndRootReady', 'actualRootAgentCorrelated', 'hostAbortCancelObserved', 'liveHostDecisionQualified', 'actualRosterSucceeded'):
            self.app[key] = True
        for key in ('nativeKeyboardPointerInputQualified', 'realCredentialsUsed', 'modelCatalogRequested', 'browserSignInRequested', 'manualBusinessMessagesForwarded', 'timedUiEnabled', 'outboundPacketTracePerformed'):
            self.app[key] = False
        self.write()

    def private(self, name, value):
        path = self.control / name
        path.write_text(json.dumps(value))
        path.chmod(0o600)

    def write(self):
        self.private('ready.json', self.ready)
        for phase in self.phases:
            self.private(f'phase-{phase["phase"]}.json', phase)
        self.private('complete.json', self.final)

    def valid(self, app=None):
        return RUN.valid_reports(self.root, self.app if app is None else app, 4242, 1.0)

    def test_viewport_preset_requires_matching_logical_bounds(self):
        self.assertFalse(RUN.valid_reports(self.root, self.app, 4242, 1.0, 'minimum'))
        self.app['logicalViewport'] = [608, 448]
        self.assertTrue(RUN.valid_reports(self.root, self.app, 4242, 1.0, 'minimum'))
        self.assertFalse(self.valid())
        self.assertFalse(RUN.valid_reports(self.root, self.app, 4242, 1.0, 'arbitrary'))

    def test_separate_matching_host_settlements_and_native_receipts_are_required(self):
        self.assertTrue(self.valid())
        for key in ('nativeAcks', 'hostSettlements', 'pid', 'applicationScale'):
            changed = copy.deepcopy(self.app)
            changed[key] = 999
            self.assertFalse(self.valid(changed))
        self.phases[1]['settled'] = False
        self.write()
        self.assertFalse(self.valid())

    def test_live_root_identity_and_ui_cancel_vs_host_abort_cannot_be_substituted(self):
        self.phases[2]['actualAgentId'] = 'PUBLIC-foreign-root'
        self.write()
        self.assertFalse(self.valid())
        self.phases[2]['actualAgentId'] = self.ready['actualAgentId']
        self.phases[2]['code'] = 'ASK_ABORTED'
        self.write()
        self.assertFalse(self.valid())
        self.phases[2]['code'] = 'ASK_CANCELLED'
        self.phases[3]['code'] = 'ASK_CANCELLED'
        self.write()
        self.assertFalse(self.valid())

    def test_model_step_tool_or_unclosed_turn_counts_refuse_qualification(self):
        for key in self.final['counts']:
            original = self.final['counts'][key]
            self.final['counts'][key] = original + 1
            self.write()
            self.assertFalse(self.valid(), key)
            self.final['counts'][key] = original
        self.final['disposed'] = False
        self.write()
        self.assertFalse(self.valid())

    def test_captures_require_two_bounded_private_png_headers(self):
        import struct
        for phase in (1, 2):
            path = self.root / f'decision-{phase}.png'
            path.write_bytes(b'\x89PNG\r\n\x1a\n' + struct.pack('>I', 13) + b'IHDR' + struct.pack('>II', 1200, 800))
            path.chmod(0o600)
        self.assertTrue(RUN.valid_captures(self.root))
        (self.root / 'decision-2.png').chmod(0o644)
        self.assertFalse(RUN.valid_captures(self.root))
        (self.root / 'decision-2.png').chmod(0o600)
        (self.root / 'decision-2.png').write_bytes(b'not a PNG header' * 2)
        self.assertFalse(RUN.valid_captures(self.root))

    def test_private_reports_refuse_symlink_nonprivate_oversize_and_wrong_json(self):
        path = self.control / 'phase-1.json'
        path.chmod(0o644)
        with self.assertRaises(ValueError):
            RUN.private_json(path)
        path.chmod(0o600)
        alias = self.control / 'alias.json'
        alias.symlink_to(path)
        with self.assertRaises(ValueError):
            RUN.private_json(alias)
        for content in ('[]', 'invalid', ' ' * 16385):
            path.write_text(content)
            with self.assertRaises(ValueError):
                RUN.private_json(path)


if __name__ == '__main__':
    unittest.main()
