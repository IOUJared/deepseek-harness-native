#!/usr/bin/env python3
"""Qualify that the composed alpha Host publishes every projection native's chat reads."""
import argparse
import asyncio
import importlib.util
from pathlib import Path

BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('native_node_qualification', BASE / 'scripts/qualify-lazy-account.py')
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
FACTS = {'registryPresent', 'allNamedKeysPublished', 'blankSessionShapes'}


def valid_projections(report):
    try:
        return (report['status'] == 'passed' and report['phase'] == 'complete' and report['fatal'] is False
                and report['providerSubstituted'] is False and report['modelPromptsRequested'] is False
                and report['catalogRequested'] is False
                and set(report['facts']) == FACTS and all(value is True for value in report['facts'].values())
                and report['missing'] == []
                and 'turnOutline' in report['published'] and 'tokenUsage' in report['published']
                and 'inbox' in report['published']
                and set(report['named']) >= {'turnOutline', 'tokenUsage', 'inbox', 'contextPressure'}
                and all(type(key) is str and key != '' for key in report['published']))
    except (KeyError, TypeError):
        return False


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True)
    options = parser.parse_args()
    raise SystemExit(asyncio.run(RUNNER.run(
        options,
        script=BASE / 'scripts/qualify-client-projections.mjs',
        report_name='projections.json',
        report_key='projections',
        evidence_env='DSH_NATIVE_PROJECTIONS_EVIDENCE',
        validator=valid_projections,
        scope='Real alpha base+web profile: every projection key native chat reads, on one blank session; not native transport, GUI or a model run',
    )))
