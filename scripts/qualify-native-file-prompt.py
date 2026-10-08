#!/usr/bin/env python3
"""Owned real native Worker/Core file-prompt ACK and durable inbox proof, no admitted model step."""
import argparse
import asyncio
import hashlib
import json
from pathlib import Path
import importlib.util
BASE=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('native_prompt_stage',BASE/'scripts/qualify-native-file-stage.py')
STAGE=importlib.util.module_from_spec(spec);spec.loader.exec_module(STAGE)
EXAMPLE=BASE/'app/target/debug/examples/file_prompt_smoke'
COUNTS=['turnStart','turnEnd','blockedTurns','inboxInserted','inboxClaims','preSteps']
ZERO=['steps','stepEnds','requestHeaders','toolCalls','toolResults','assistantMessages','userMessages']
def counts(value,n):
    return type(value) is dict and set(value)==set(COUNTS+ZERO) and all(type(value[k]) is int and value[k]==n for k in COUNTS) and all(type(value[k]) is int and value[k]==0 for k in ZERO)
def valid_worker(value):
    try:
        stop=value['stop'];host=value['host']
        if not (value['status']=='passed' and value['version']=='0.2.1-alpha.1'
                and all(type(value[k]) is int and value[k]==3 for k in ['acceptedPrompts','replayRefusals','genericFileRefusals','explicitPrompts'])
                and all(type(value[k]) is int and value[k]==0 for k in ['unexpectedWork','explicitModelPrompts'])
                and all(value[k] is True for k in ['rootRegistered','ordinaryHeaderValidated','nativeWorkerExercised'])
                and all(value[k] is False for k in ['modelCatalogRequested','browserSignInRequested','realCredentialsUsed','processWideNetworkTrace','uiExercised','historyDrivenReceiptRetirementQualified'])
                and value['files']==[{'name':name,'bytes':len(data)} for _,data,name in STAGE.FIXTURES]
                and all(type(v['bytes']) is int for v in value['files'])
                and value['snapshots']==[['permission/preset','sandbox/mode','approval/policy']]
                and len(value['phases'])==3 and host['status']=='passed'
                and all(host[k] is True for k in ['rootDisposed','turnClosed','zeroAdmittedModelSteps']) and counts(host['counts'],3)
                and stop['exited'] is True and stop['graceful'] is True and stop['containmentUnknown'] is False
                and type(stop['exitCode']) is int and stop['exitCode']==0
                and type(stop['observedDescendantsRemaining']) is int and stop['observedDescendantsRemaining']==0):return False
        for n,(phase,(_,data,name)) in enumerate(zip(value['phases'],STAGE.FIXTURES),1):
            if not (type(phase['phase']) is int and phase['phase']==n and phase['requestId']==f'PUBLIC_native_file_prompt_{n}'
                    and phase['file']=={'attachmentId':'sha256:'+hashlib.sha256(data).hexdigest(),'name':name,'bytes':len(data)}
                    and type(phase['file']['bytes']) is int and counts(phase['counts'],n)
                    and all(phase[k] is True for k in ['rootRegistered','durableInboxFileMatched','claimedFileMatched','blockedTurn','zeroAdmittedModelSteps'])):return False
        return True
    except (KeyError,TypeError,ValueError):return False

def command(output):
    result=STAGE.command(output);result[0]=str(EXAMPLE);return result

async def run(options):
    result=await STAGE.FILES.RUNNER.run(options,script=EXAMPLE,report_name='worker.json',report_key='worker',validator=valid_worker,capture_stdout=True,command_factory=command,scope='actual native private file receipt-to-prompt, full Alpha public profile plus fixed isolated owned-Agent reject fixture; no model step/user-message history/GUI')
    output=Path(options.output).resolve();owned=json.loads((output/'result.json').read_text())
    summary={'status':'failed','ownedWorkerStatus':owned['status'],'scope':'three real prompt ACKs/durable file refs in inbox/claimed pre-step rejection, conservative one-attempt native tickets; not model generation or history-driven receipt retirement'}
    try:
        if result or not valid_worker(owned['worker']):raise ValueError('native-file-prompt-not-qualified')
        summary['storage']=STAGE.FILES.verify_local_cases(output,STAGE.FIXTURES);summary['status']='passed'
    except (OSError,ValueError,KeyError,TypeError):summary['refusal']='invalid-owned-worker-or-storage-evidence'
    STAGE.FILES.RUNNER.OWNED.private_json(output/'native-prompt-qualification.json',summary)
    print(json.dumps({'status':summary['status'],'report':str(output/'native-prompt-qualification.json')}))
    return 0 if summary['status']=='passed' else 1
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',required=True)
    raise SystemExit(asyncio.run(run(parser.parse_args())))
