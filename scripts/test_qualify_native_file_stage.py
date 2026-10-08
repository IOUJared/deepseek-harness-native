import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('native_file_stage_test', Path(__file__).with_name('qualify-native-file-stage.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

def good():
    base = ['permission/preset', 'sandbox/mode', 'approval/policy']
    return {'status':'passed', 'version':'0.2.1-alpha.1',
            'files':[{'name':name,'bytes':len(data)} for _,data,name in module.FIXTURES],
            'foreignDiscardRefusals':3,'preSnapshotRefused':True,'staleSelectionRefused':True,
            'unexpectedWork':0,'explicitModelPrompts':0,'snapshots':[base,base],
            'nativeWorkerExercised':True,'modelCatalogRequested':False,'processWideNetworkTrace':False,'uiExercised':False,
            'stop':{'exited':True,'graceful':True,'containmentUnknown':False,'exitCode':0,'observedDescendantsRemaining':0}}

class Evidence(unittest.TestCase):
    def test_exact_validated_worker_subject_and_private_metadata(self):
        self.assertTrue(module.valid_worker(good()))
        for key in good():
            value=good();del value[key];self.assertFalse(module.valid_worker(value),key)
        for key in ['preSnapshotRefused','staleSelectionRefused','nativeWorkerExercised']:
            value=good();value[key]=1;self.assertFalse(module.valid_worker(value),key)
        for key in ['explicitModelPrompts','unexpectedWork','foreignDiscardRefusals']:
            value=good();value[key]=False;self.assertFalse(module.valid_worker(value),key)
    def test_no_scope_substitution_or_incomplete_owned_stop(self):
        for key in ['modelCatalogRequested','processWideNetworkTrace','uiExercised']:
            value=good();value[key]=True;self.assertFalse(module.valid_worker(value),key)
        for key in ['exited','graceful']:
            value=good();value['stop'][key]=False;self.assertFalse(module.valid_worker(value),key)
        for key,value_ in [('containmentUnknown',True),('exitCode',False),('exitCode',1),('observedDescendantsRemaining',1)]:
            value=good();value['stop'][key]=value_;self.assertFalse(module.valid_worker(value),key)
        value=good();value['snapshots'][1]=['user/message'];self.assertFalse(module.valid_worker(value))
    def test_static_fixture_order_lengths_and_names_not_adopted_from_receipts(self):
        for change in ['order','length','boolean','name','extra-receipt']:
            value=good()
            if change=='order':value['files'].reverse()
            elif change=='length':value['files'][0]['bytes']+=1
            elif change=='boolean':value['files'][1]['bytes']=False
            elif change=='name':value['files'][0]['name']='../../arbitrary'
            else:value['files'][0]['receiptId']='PUBLIC_unknown'
            self.assertFalse(module.valid_worker(value),change)
        self.assertEqual(len(module.FIXTURES),3)
        self.assertEqual([len(data) for _,data,_ in module.FIXTURES],[5003,0,4*1024*1024])

if __name__=='__main__':unittest.main()
