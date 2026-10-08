import importlib.util
from pathlib import Path
import unittest
SPEC = importlib.util.spec_from_file_location('agent_qualification', Path(__file__).with_name('qualify-agent-discovery.py'))
Q = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(Q)

def agent():
    return dict(status='passed', phase='complete', fatal=False, providerSubstituted=False,
                discoveryPromptOrResumeRequested=False, accountOrKeyMutationRequested=False, processWideNetworkTrace=False,
                facts={key: True for key in Q.FACTS}, counts={key: 0 for key in Q.COUNTS})

def sdk():
    return dict(status='passed', ready={'version': '0.2.1-alpha.1'}, modelPrompts=0, modelCatalogRequested=False,
                agentDiscovery=dict(unregisteredReturnedNone=True, registeredReturnedTypedId=True, sessionEchoMatched=True,
                    repeatedReadStable=True, alphaSharedBytesObserved=True, agentIdInferred=False),
                coreStopRejectsDiscovery=True, coreStopInvalidatesTransport=True,
                stop=dict(exited=True, graceful=True, exitCode=0, containmentUnknown=False, observedDescendantsRemaining=0),
                snapshot={'eventTypes': []})

class Qualification(unittest.TestCase):
    def test_agent_accepts_complete_scoped_proof_only(self):
        self.assertTrue(Q.valid_agent(agent()))
        for key in Q.FACTS:
            value = agent(); value['facts'][key] = False; self.assertFalse(Q.valid_agent(value))
        self.assertFalse(Q.valid_agent({}))
    def test_agent_rejects_missing_extra_and_boolean_counters(self):
        for key in Q.COUNTS:
            for bad in [True, False, 1, -1]:
                value = agent(); value['counts'][key] = bad; self.assertFalse(Q.valid_agent(value))
        value = agent(); value['counts']['extra'] = 0; self.assertFalse(Q.valid_agent(value))
    def test_sdk_requires_registered_typed_echo_and_owned_shutdown(self):
        self.assertTrue(Q.valid_sdk(sdk()))
        for key in ['unregisteredReturnedNone', 'registeredReturnedTypedId', 'sessionEchoMatched', 'repeatedReadStable', 'alphaSharedBytesObserved']:
            value = sdk(); value['agentDiscovery'][key] = False; self.assertFalse(Q.valid_sdk(value))
        value = sdk(); value['stop']['exitCode'] = 9; self.assertFalse(Q.valid_sdk(value))
        value = sdk(); value['agentDiscovery']['agentIdInferred'] = True; self.assertFalse(Q.valid_sdk(value))
    def test_sdk_rejects_generation_records_and_wrong_runtime(self):
        for key in ['request/header', 'tool/call', 'tool/result', 'assistant/message', 'turn/start', 'step/start', 'step/end']:
            value = sdk(); value['snapshot']['eventTypes'] = [key]; self.assertFalse(Q.valid_sdk(value))
        value = sdk(); value['ready']['version'] = '0.2.0-rc.2'; self.assertFalse(Q.valid_sdk(value))
        value = sdk(); value['modelPrompts'] = False; self.assertFalse(Q.valid_sdk(value))

if __name__ == '__main__': unittest.main()
