#!/usr/bin/env python3
"""Own actual alpha HTTP uncertainty barriers; independently verify retained bytes/modes."""
import argparse
import asyncio
import hashlib
import importlib.util
import json
from pathlib import Path

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('uncertainty_storage', BASE / 'scripts/qualify-file-upload.py')
FILES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FILES)
CASES = ['ack-loss', 'scope', 'late-disposal']
FACTS = {'authenticatedRawRoute', 'storedBeforeAbort', 'stagedBeforeAbort', 'clientAbortedBeforeAck',
         'serverObservedDisconnect', 'receiptRetainedAfterAckLoss', 'foreignResolveRefused', 'foreignBindingRefused',
         'foreignRetirementHarmless', 'rollbackRetainedUnbound', 'rollbackRestoredPrior', 'committedRetirementExact',
         'oldSessionDisposalRetiredReceipt', 'sameIdReplacementDifferentSession', 'sameIdReplacementRefusedOldReceipt',
         'storedBeforeAgentDisposal', 'disposedBeforeStorePromiseReturned', 'lateDisposalRejectedNoReceipt',
         'handlersJoined', 'descriptorsRestored', 'registryReturnedToBaseline'}
COUNTS = {'turnStarts', 'turnEnds', 'stepStarts', 'stepEnds', 'requestHeaders', 'toolCalls', 'toolResults',
          'assistantMessages', 'userMessages', 'unexpectedWake'}


def source(case):
    count, step = {'ack-loss': (5003, 43), 'scope': (6117, 71), 'late-disposal': (7109, 29)}[case]
    return bytes((index * step + 19) % 256 for index in range(count))


def valid_observation(value, cases):
    if (value['fatal'] is not False or value['testOnlyInstanceBarriers'] is not True
            or any(value[key] is not False for key in ['scriptedModelPromptOrCatalog', 'scriptedSignInOrCredentialSave',
                                                     'providerSubstituted', 'processWideNetworkTrace', 'rustSdkExercised', 'uiExercised'])
            or set(value['facts']) != FACTS or any(type(fact) is not bool for fact in value['facts'].values())
            or set(value['counts']) != COUNTS or any(type(number) is not int or number != 0 for number in value['counts'].values())
            or set(value['bridge']) != {'active', 'complete', 'errors', 'methodActive'}
            or any(type(number) is not int for number in value['bridge'].values())
            or value['bridge'] != {'active': 0, 'complete': len(cases), 'errors': 0, 'methodActive': 0}
            or set(value['cleanup']) != {'handlersJoined', 'descriptorsRestored', 'observerDisposersRan', 'ownedHandlesDisposed', 'registryReturnedToBaseline', 'hostShutdown', 'drainTimedOut'}
            or any(flag is not (key != 'drainTimedOut') for key, flag in value['cleanup'].items())
            or [entry['case'] for entry in value['files']] != cases):
        return False
    for entry in value['files']:
        data, file = source(entry['case']), entry['file']
        if (set(entry) != {'case', 'file'} or set(file) != {'attachmentId', 'name', 'bytes'}
                or file['attachmentId'] != 'sha256:' + hashlib.sha256(data).hexdigest()
                or file['name'] != f"PUBLIC {entry['case']}.bin"
                or type(file['bytes']) is not int or file['bytes'] != len(data)):
            return False
    return True


def valid_barriers(value):
    try:
        return (valid_observation(value, CASES) and value['status'] == 'passed' and value['phase'] == 'complete'
                and value['failurePoint'] == 'none' and value['injectionReached'] is False
                and all(fact is True for fact in value['facts'].values()))
    except (KeyError, TypeError, AttributeError, ValueError):
        return False


def valid_expected_failure(owned, point):
    try:
        cases, phase, required = {
            'ack-held': (CASES[:1], 'ack-lost-after-real-staging', ['storedBeforeAbort', 'stagedBeforeAbort', 'clientAbortedBeforeAck', 'serverObservedDisconnect']),
            'store-held': (CASES, 'agent-disposed-after-real-save-before-upload-commit', ['storedBeforeAgentDisposal', 'sameIdReplacementRefusedOldReceipt']),
        }[point]
        value, cleanup = owned['barriers'], owned['cleanup']
        return (owned['status'] == 'failed' and type(owned['exitCode']) is int and owned['exitCode'] == 1
                and owned['sourceUnchanged'] is True and type(owned['identityPinFailures']) is int and owned['identityPinFailures'] == 0
                and owned['timedOut'] is False and owned['cancelled'] is False
                and cleanup['rootPidfdPinned'] is True and cleanup['rootReaped'] is True and cleanup['observedCleanupComplete'] is True
                and all(cleanup[key] is False for key in ['rootWasRunningAtCleanup', 'ownedChildFallbackUsed', 'killEscalated', 'cleanupTimedOut', 'cleanupProbeOrSignalFailed'])
                and cleanup['remainingBeforeExternalCleanup'] == [] and cleanup['remainingAfterExternalCleanup'] == []
                and valid_observation(value, cases) and value['status'] == 'failed' and value['phase'] == phase
                and value['failurePoint'] == point and value['injectionReached'] is True
                and all(value['facts'][key] is True for key in required)
                and all(value['facts'][key] is False for key in (['receiptRetainedAfterAckLoss', 'storedBeforeAgentDisposal'] if point == 'ack-held'
                                                                else ['disposedBeforeStorePromiseReturned', 'lateDisposalRejectedNoReceipt']))
                and not valid_barriers(value))
    except (KeyError, TypeError, AttributeError, ValueError):
        return False


async def qualify(options):
    script = BASE / 'scripts/qualify-upload-uncertainty.mjs'
    result = await FILES.RUNNER.run(options, script=script, report_name='barriers.json', report_key='barriers', validator=valid_barriers,
                                   evidence_env='DSH_NATIVE_UPLOAD_UNCERTAINTY_EVIDENCE',
                                   command_factory=lambda _output: ['/usr/bin/node', str(script), options.fail_at],
                                   scope='actual public alpha HTTP/Agent/storage with test-only post-real-operation barriers; no prompt or GUI')
    output = Path(options.output).resolve()
    owned = json.loads((output / 'result.json').read_text())
    report = {'status': 'failed', 'ownedHostStatus': owned['status'], 'failurePoint': options.fail_at, 'storage': [],
              'scope': 'independent complete private default-provider bytes and joined handler/descriptor/Agent cleanup; injected failures must be refused, not counted as successful uploads'}
    cases = CASES
    try:
        if options.fail_at == 'none':
            if result != 0:
                raise ValueError('owned-host-barriers-failed')
        else:
            if result != 1 or not valid_expected_failure(owned, options.fail_at):
                raise ValueError('expected-owned-failure-or-cleanup-not-established')
            cases = CASES[:1] if options.fail_at == 'ack-held' else CASES
            report['expectedFailureRefusedAndCleaned'] = True
        report['storage'] = FILES.verify_local_cases(output, [(case, source(case), f'PUBLIC {case}.bin') for case in cases])
        report['status'] = 'passed'
    except (OSError, ValueError):
        report['error'] = 'owned-host-or-independent-storage-verification-failed'
    FILES.RUNNER.OWNED.private_json(output / 'uncertainty-qualification.json', report)
    print(json.dumps({'status': report['status'], 'report': str(output / 'uncertainty-qualification.json')}))
    return 0 if report['status'] == 'passed' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    parser.add_argument('--fail-at', choices=['none', 'ack-held', 'store-held'], default='none')
    raise SystemExit(asyncio.run(qualify(parser.parse_args())))
