#!/usr/bin/env python3
"""Qualify actual registered-Agent discovery under the existing pidfd-owned Node runner."""
import argparse
import asyncio
import importlib.util
from pathlib import Path

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('native_node_qualification', BASE / 'scripts/qualify-lazy-account.py')
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
FACTS = {'absentBefore', 'actualRegisteredObject', 'matchedActualIdentity', 'unrelatedAbsent',
         'unchangedByRepeatedRead', 'absentAfterDisposal', 'registryReturnedToBaseline'}
COUNTS = {'turnStart', 'turnEnd', 'stepStarts', 'stepEnds', 'requestHeaders', 'toolCalls', 'toolResults', 'assistantMessages', 'unexpectedWake'}

def valid_agent(report):
    try:
        return (report['status'] == 'passed' and report['phase'] == 'complete' and report['fatal'] is False
                and report['providerSubstituted'] is False and report['discoveryPromptOrResumeRequested'] is False
                and report['accountOrKeyMutationRequested'] is False and report['processWideNetworkTrace'] is False
                and set(report['facts']) == FACTS and all(value is True for value in report['facts'].values())
                and set(report['counts']) == COUNTS and all(type(value) is int and value == 0 for value in report['counts'].values()))
    except (KeyError, TypeError, AttributeError):
        return False

SDK = BASE / 'core/target/release/examples/transport_smoke'

def sdk_command(output):
    return [str(SDK), '--runtime', str((BASE.parent / 'deepseek-harness-linux/apps/cli').resolve()),
            '--expected-version', '0.2.1-alpha.1', '--home', str(output / 'harness'),
            '--cwd', str(output / 'workspace'), '--user-home', str(output / 'user')]

def valid_sdk(report):
    try:
        facts = report['agentDiscovery']
        stop = report['stop']
        return (report['status'] == 'passed' and report['ready']['version'] == '0.2.1-alpha.1'
                and type(report['modelPrompts']) is int and report['modelPrompts'] == 0 and report['modelCatalogRequested'] is False
                and all(facts[key] is True for key in ['unregisteredReturnedNone', 'registeredReturnedTypedId',
                    'sessionEchoMatched', 'repeatedReadStable', 'alphaSharedBytesObserved']) and facts['agentIdInferred'] is False
                and report['coreStopRejectsDiscovery'] is True and report['coreStopInvalidatesTransport'] is True
                and stop['exited'] is True and stop['graceful'] is True and type(stop['exitCode']) is int and stop['exitCode'] == 0
                and stop['containmentUnknown'] is False and stop['observedDescendantsRemaining'] == 0
                and not set(report['snapshot']['eventTypes']) & {'request/header', 'tool/call', 'tool/result', 'assistant/message', 'turn/start', 'step/start', 'step/end'})
    except (KeyError, TypeError, AttributeError):
        return False

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--sdk', action='store_true', help='Run the release typed-Core/HTTP/WS example instead of the Node-only registration/disposal fixture')
    options = parser.parse_args()
    if options.sdk:
        result = RUNNER.run(options, script=SDK, report_name='sdk.json', report_key='sdk', validator=valid_sdk,
            command_factory=sdk_command, capture_stdout=True,
            scope='owned actual public alpha profile, typed Rust Core discovery and HTTP/WS; not GUI input or process-wide network trace')
    else:
        result = RUNNER.run(options, script=BASE / 'scripts/qualify-agent-discovery.mjs',
            report_name='agent.json', report_key='agent', evidence_env='DSH_NATIVE_AGENT_EVIDENCE', validator=valid_agent,
            scope='owned actual public alpha Loader, real AgentRegistry/factory and private supervisor discovery; not Rust or GUI, no process-wide network trace')
    raise SystemExit(asyncio.run(result))
