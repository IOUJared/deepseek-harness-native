#!/usr/bin/env python3
"""Qualify that the composed alpha Host publishes the `turnOutline` projection to clients."""
import argparse
import asyncio
import importlib.util
from pathlib import Path

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('native_node_qualification', BASE / 'scripts/qualify-lazy-account.py')
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
FACTS = {'registryPresent', 'keyPublished', 'blankSessionIsEmpty', 'keysAreClientVisible'}


def valid_outline(report):
    try:
        return (report['status'] == 'passed' and report['phase'] == 'complete' and report['fatal'] is False
                and report['providerSubstituted'] is False and report['modelPromptsRequested'] is False
                and report['catalogRequested'] is False
                and set(report['facts']) == FACTS and all(value is True for value in report['facts'].values())
                and 'turnOutline' in report['keys']
                and all(type(key) is str and key != '' for key in report['keys']))
    except (KeyError, TypeError):
        return False


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    options = parser.parse_args()
    raise SystemExit(asyncio.run(RUNNER.run(
        options,
        script=BASE / 'scripts/qualify-turn-outline.mjs',
        report_name='turn-outline.json',
        report_key='turnOutline',
        evidence_env='DSH_NATIVE_TURNOUTLINE_EVIDENCE',
        validator=valid_outline,
        scope='Real alpha base+web profile: turnOutline registration and one blank session snapshot; not native transport, GUI or a model run',
    )))
