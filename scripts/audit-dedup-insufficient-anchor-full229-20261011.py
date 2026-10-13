#!/usr/bin/env python3
"""Audit the independent repaired full229 run without combining earlier baseline records."""
from pathlib import Path
import collections
import hashlib
import json
import math
import shutil

ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
REPORT=D/'dedup-insufficient-anchor-full229-20261011.json'
OUT=D/'dedup-insufficient-anchor-full229-audit-20261011.json'
ARCHIVE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/insufficient-anchor-full229-qualified-20261011')

def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def main():
    assert not OUT.exists() and not ARCHIVE.exists()
    r=json.loads(REPORT.read_text())
    assert r['status']=='complete_bounded_229_pair_execution'
    assert r['completed_pairs']==r['required_pairs']==len(r['results'])==229
    assert r['counts']=={'supported':229,'miss':0,'execution_failed':0}
    assert r['all_229_supported'] is True
    assert [row['query'] for row in r['results']]==r['pair_order']
    assert len(set(r['pair_order']))==229
    assert all(sha(p)==h for p,h in r['pins'].items())
    manifest=json.loads((D/'dedup-insufficient-anchor-snapshot-20261011.json').read_text())
    assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
    corpus=json.loads(Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/prepared.json').read_text())
    expected={p['query_id']:p for p in corpus['positive_pairs']}
    assert set(expected)==set(r['pair_order'])
    stages=collections.Counter();minimum_anchors=[];minimum_pairs=[];ratios=[];coverage=[];failures=[]
    for row in r['results']:
        pair=expected[row['query']]
        assert pair['label']=='publisher_origin_copy' and pair['left']['group_id']==pair['right']['group_id']
        assert Path(row['source']).name==pair['left']['filename']==row['original']
        assert Path(row['target']).name==pair['right']['filename']==row['query']
        assert row['status']!='running' and math.isfinite(row['elapsed_seconds']) and row['elapsed_seconds']>0
        assert row['status']=='supported' and row['returncode']==0 and row['supported'] is True
        e=row['evidence'];assert json.loads(row['stdout'])==e
        assert e['status']=='ok' and e['files']==[0,1] and e['source_paths']==[row['source'],row['target']]
        assert e['managed_used']==0 and e['managed_peak']<=512*1024*1024 and len(e['pairs'])==1
        p=e['pairs'][0];assert (p['left'],p['right'])==(0,1)
        proof=p['evidence'];assert proof['supported'];stages[proof['terminal']]+=1
        if proof['terminal']=='model_tiles':
            assert 1<=len(proof['attempts'])<=25
            proof=proof['attempts'][-1]['evidence']
        else:
            assert proof['terminal']=='native' and 1<=proof['native_recipes']<=6
        pixels=proof['pixels'];assert pixels['supported']
        assert len(pixels['anchors'])==len(pixels['directions'])==2
        minimum_anchors+=pixels['anchors']
        assert all(n>=10 for n in pixels['anchors'])
        for direction in pixels['directions']:
            assert direction['sites']>0 and direction['informative_pairs']>=1000
            c=direction['valid_sites']/direction['sites']
            agreement=direction['agreeing_pairs']/direction['informative_pairs']
            assert c>=.3 and agreement>=.9
            minimum_pairs.append(direction['informative_pairs']);coverage.append(c);ratios.append(agreement)
    assert sum(stages.values())==229 and set(stages)<= {'model_tiles','native'}
    assert len({row['source'] for row in r['results']})==157
    assert len({row['target'] for row in r['results']})==229
    ARCHIVE.mkdir();artifacts=[]
    for path in [REPORT,Path(__file__),D/'dedup-insufficient-anchor-full229-20261011.log']:
        target=ARCHIVE/path.name;expected_sha=sha(path);shutil.copy2(path,target);assert sha(target)==expected_sha
        artifacts.append({'source':str(path),'archive':str(target),'sha256':expected_sha})
    audit={'status':'complete_all_229_repaired_positive_records','executed_pairs':229,'counts':r['counts'],'all_229_supported':True,'stages':dict(stages),'originals_scheduled':157,'targets_scheduled':229,'pins_verified':len(r['pins']),'frozen_source_files':len(manifest['file_hashes']),'minimum_supported_anchors':min(minimum_anchors),'minimum_supported_informative_pairs':min(minimum_pairs),'minimum_supported_coverage':min(coverage),'minimum_supported_agreement':min(ratios),'failures':failures,'artifacts':artifacts,'source_manifest':'docs/research/dedup-insufficient-anchor-snapshot-20261011.json','limits':['Frozen insufficient-anchor snapshot only; later native lifecycle and projected portfolio changes are separate','Known publisher-origin positives only; no all-query negative precision or one 386-file indexed collection','Support is regional original-pixel rank evidence, not whole-image/file identity','Every record is a fresh execution from this run, not a merge of baseline and repaired diagnostics','Managed payload budget is not total Torch/RSS admission','Timings are observed shared-host costs, not a comparative performance benchmark']}
    OUT.write_text(json.dumps(audit,indent=2)+'\n');print(audit['status'],audit['counts'])

if __name__=='__main__':main()
