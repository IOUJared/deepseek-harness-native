import copy
import hashlib
import importlib.util
import unittest
from pathlib import Path
spec=importlib.util.spec_from_file_location('qualified_prompt',Path(__file__).with_name('qualify-native-file-prompt.py'))
M=importlib.util.module_from_spec(spec);spec.loader.exec_module(M)
def fixture():
    value={'status':'passed','version':'0.2.1-alpha.1','acceptedPrompts':3,'replayRefusals':3,'genericFileRefusals':3,'explicitPrompts':3,'unexpectedWork':0,'explicitModelPrompts':0,'rootRegistered':True,'ordinaryHeaderValidated':True,'nativeWorkerExercised':True,'snapshots':[['permission/preset','sandbox/mode','approval/policy']],'files':[],'phases':[],'host':{},'stop':{'exited':True,'graceful':True,'containmentUnknown':False,'exitCode':0,'observedDescendantsRemaining':0}}
    for key in ['modelCatalogRequested','browserSignInRequested','realCredentialsUsed','processWideNetworkTrace','uiExercised','historyDrivenReceiptRetirementQualified']:value[key]=False
    for n,(_,data,name) in enumerate(M.STAGE.FIXTURES,1):
        value['files'].append({'name':name,'bytes':len(data)})
        phase={'phase':n,'requestId':f'PUBLIC_native_file_prompt_{n}','file':{'attachmentId':'sha256:'+hashlib.sha256(data).hexdigest(),'name':name,'bytes':len(data)},'counts':{**{k:n for k in M.COUNTS},**{k:0 for k in M.ZERO}}}
        for key in ['rootRegistered','durableInboxFileMatched','claimedFileMatched','blockedTurn','zeroAdmittedModelSteps']:phase[key]=True
        value['phases'].append(phase)
    value['host']={'status':'passed','rootDisposed':True,'turnClosed':True,'zeroAdmittedModelSteps':True,'counts':copy.deepcopy(value['phases'][-1]['counts'])}
    return value
class Validator(unittest.TestCase):
    def test_complete_exact_metadata_and_prompts_only(self):
        self.assertTrue(M.valid_worker(fixture()))
        for key in ['acceptedPrompts','replayRefusals','genericFileRefusals','explicitPrompts','unexpectedWork','explicitModelPrompts']:
            value=fixture();value[key]=True;self.assertFalse(M.valid_worker(value))
        for key in ['historyDrivenReceiptRetirementQualified','uiExercised','processWideNetworkTrace']:
            value=fixture();value[key]=True;self.assertFalse(M.valid_worker(value))
    def test_actual_inbox_and_claimed_exact_bytes_digest_and_scope(self):
        for index in range(3):
            for field,new in [('requestId','PUBLIC_wrong'),('phase',True),('durableInboxFileMatched',False),('claimedFileMatched',False),('rootRegistered',False)]:
                value=fixture();value['phases'][index][field]=new;self.assertFalse(M.valid_worker(value))
            for field,new in [('bytes',True),('attachmentId','sha256:'+'0'*64),('name','PUBLIC_wrong')]:
                value=fixture();value['phases'][index]['file'][field]=new;self.assertFalse(M.valid_worker(value))
        value=fixture();value['snapshots'][0].append('user/message');self.assertFalse(M.valid_worker(value))
    def test_zero_model_work_and_graceful_owned_teardown(self):
        for key in M.ZERO:
            value=fixture();value['host']['counts'][key]=1;self.assertFalse(M.valid_worker(value))
            value=fixture();value['phases'][0]['counts'][key]=True;self.assertFalse(M.valid_worker(value))
        for key,new in [('graceful',False),('exited',False),('containmentUnknown',True),('exitCode',True),('observedDescendantsRemaining',1)]:
            value=fixture();value['stop'][key]=new;self.assertFalse(M.valid_worker(value))
        for value in [{},None,{'status':'passed'}]:self.assertFalse(M.valid_worker(value))
if __name__=='__main__':unittest.main()
