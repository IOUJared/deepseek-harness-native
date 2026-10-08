#!/usr/bin/env python3
"""Compare retained native workload reports without rerunning or rewriting either."""
import json
import pathlib
import statistics
import sys

ROOT=pathlib.Path(__file__).resolve().parent
FIELDS=['nativeAnchorRowBounds','nativeCurrent','nativeAnchorRelativeTop','actualApplicationFrameBounds','currentWidgetMatches','previousWidgetMatches']
MODEL=['anchorKey','anchorIntra','anchorHeight','offset','viewport','totalHeight','keysCount','followBottom','active']

def load(path):return json.loads(path.read_text())
def summarize(folder,rows):
    cases=[v for v in load(folder/'qualification.json')['cases'] if v['rowsInitial']==rows]
    reports=[load(folder/f'rows-{rows}-run-{case["repeat"]}'/'native.json') for case in cases]
    samples=[[v['timing']['operationToAppliedNs']/1e6 for v in report['steps'] if v['phase']==1] for report in reports]
    warm=[v for run in samples for v in run[1:]]
    return {'freshProcesses':len(cases),'reflowsPerProcess':len(samples[0]),'allReflowIssueToReceiptMs':samples,'firstNarrowMedianMs':statistics.median(v[0] for v in samples),'subsequentReflowSamples':len(warm),'subsequentReflowMedianMs':statistics.median(warm),'subsequentReflowMaximumMs':max(warm),'nativeParagraphShapes':[r['fixtureMetrics']['nativeParagraphShapes'] for r in reports],'shapeConstructorTotalMs':[r['fixtureMetrics']['nativeParagraphShapeNs']/1e6 for r in reports],'productionUpdateTotalMs':[r['fixtureMetrics']['productionUpdate']['totalNs']/1e6 for r in reports],'productionViewTotalMs':[r['fixtureMetrics']['productionView']['totalNs']/1e6 for r in reports],'idleRssMiB':[v['idleHold']['medianRssMiB'] for v in cases],'idlePssMiB':[v['idleHold']['medianPssMiB'] for v in cases],'idleCpuPercent':[v['idleHold']['oneCoreCpuPercent'] for v in cases]}

def compare(before,after):
    a=load(before/'qualification.json');b=load(after/'qualification.json')
    assert a['passed'] and b['passed'] and a['sourceUnchangedDuringRuns'] and b['sourceUnchangedDuringRuns']
    cases_a={(v['rowsInitial'],v['repeat']):v for v in a['cases']};cases_b={(v['rowsInitial'],v['repeat']):v for v in b['cases']};assert cases_a.keys()==cases_b.keys()
    for (rows,repeat),case in cases_a.items():
        assert case['cycles']==cases_b[rows,repeat]['cycles']
        x=load(before/f'rows-{rows}-run-{repeat}'/'native.json');y=load(after/f'rows-{rows}-run-{repeat}'/'native.json');assert len(x['steps'])==len(y['steps'])
        for p,q in zip(x['steps'],y['steps']):
            assert p['passed'] and q['passed']
            for field in FIELDS:assert p.get(field)==q.get(field),(rows,repeat,field)
            for field in MODEL:assert p['model'][field]==q['model'][field],(rows,repeat,field)
    result={'scope':'paired PUBLIC actual native App controlled two-width reflow; observed issue-to-widget-receipt wall time, NOT presented-frame/wheel-input latency; diagnostics are included, not subtracted','exactNativeGeometryAndModelParity':True,'baselineQualification':str((before/'qualification.json').relative_to(ROOT)),'optimizedQualification':str((after/'qualification.json').relative_to(ROOT)),'baselineProbeSha256':a['probeSha256'],'optimizedProbeSha256':b['probeSha256'],'fullAppPerformanceQualified':False,'physicalInputQualified':False,'liveHostQualified':False,'nativeWindowResizeQualified':False,'rows':{}}
    for rows in sorted({k[0] for k in cases_a}):
        old=summarize(before,rows);new=summarize(after,rows)
        result['rows'][str(rows)]={'before':old,'after':new,'subsequentReflowObservedSpeedRatio':old['subsequentReflowMedianMs']/new['subsequentReflowMedianMs']}
    return result

if __name__=='__main__':
    if len(sys.argv)!=4:raise SystemExit('usage: compare_native_history.py BASELINE_FOLDER OPTIMIZED_FOLDER NEW_OUTPUT_JSON')
    result=compare(pathlib.Path(sys.argv[1]).resolve(),pathlib.Path(sys.argv[2]).resolve())
    with open(sys.argv[3],'x') as output:json.dump(result,output,indent=2);output.write('\n')
    print('Exact native geometry/model parity and paired raw timing summaries retained.')
