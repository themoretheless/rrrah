#!/usr/bin/env python3
"""Bounded two-worker all-query negative measurement; every error stays explicit."""
import argparse
import collections
import concurrent.futures
import fcntl
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

p=argparse.ArgumentParser()
p.add_argument('manifest',type=Path)
p.add_argument('probe',type=Path)
p.add_argument('output',type=Path)
a=p.parse_args()
a.manifest=a.manifest.resolve();a.probe=a.probe.resolve();a.output=a.output.resolve()
m=json.loads(a.manifest.read_text())
queries=[row['query_id'] for row in m['positive_pairs']]
assert len(queries)==len(set(queries))==229
assert all(re.fullmatch(r'[0-9]{6}\.jpg',q) for q in queries)
a.output.mkdir(parents=True,exist_ok=True)
lease=(a.output/'suite.lease').open('a')
fcntl.flock(lease.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
manifest_hash=hashlib.sha256(a.manifest.read_bytes()).hexdigest()
probe_hash=hashlib.sha256(a.probe.read_bytes()).hexdigest()
helper=Path(__file__).with_name('qualify-dedup-copydays.py').resolve()
summary={'manifest_sha256':manifest_hash,'probe_sha256':probe_hash,'required_queries':229,
    'required_pairs':229*156,'max_workers':2,'queries':{},
    'scope':'All229 strong queries against all156 different publisher-origin groups each. Regional supports remain separate from whole candidates. Explicit refusals/process/time errors never count as rejected images. Not semantic/burst or full-library qualification.'}

def validate(report,q):
    r=json.loads(report.read_text())
    assert r['manifest_sha256']==manifest_hash and r['probe_sha256']==probe_hash
    assert r['mode']=='pyramid_region_grid' and r['required_pairs']==156
    assert r['completed_pairs']==len(r['results'])==156
    query=next(pair['right'] for pair in m['positive_pairs'] if pair['query_id']==q)
    originals=[o for o in m['images']['original'] if o['group_id']!=query['group_id']]
    assert len(originals)==156
    for row,original in zip(r['results'],originals):
        assert row['query_id']==q+'/'+original['filename'] and row['label']=='different_publisher_origin'
        assert row['status'] in ['ok','error','process_error','timeout']
        if row['status']=='ok':
            e=row['evidence'];assert row['returncode']==0 and e['status']=='ok'
            assert isinstance(e['candidate'],bool)
            assert type(e['region_support_count']) is int and e['region_support_count']>=0
            assert e['region_support_count']==sum(v['accepted_region'] for v in e['regions'])
            assert e['managed_used_after_drop']==0
        for image in [query,original]:
            assert hashlib.sha256(Path(image['normalized_path']).read_bytes()).hexdigest()==image['normalized_sha256']
    counts=dict(collections.Counter(row['status'] for row in r['results']))
    supports=sum(row.get('evidence',{}).get('region_support_count',0)>0 for row in r['results'])
    candidates=sum(row.get('evidence',{}).get('candidate',False) for row in r['results'])
    assert r['status_counts']==counts and r['region_supported_pairs']==supports and r['candidates']==candidates
    return {'status_counts':counts,'completed_pairs':156,'region_supported_pairs':supports,'whole_candidates':candidates,'report_sha256':hashlib.sha256(report.read_bytes()).hexdigest()}

def run(q):
    report=a.output/(q+'.json')
    if report.exists():
        existing=json.loads(report.read_text())
        if existing.get('completed_pairs')==156:
            return q,{**validate(report,q),'execution':'validated_existing_complete_report'}
    command=[sys.executable,str(helper),str(a.manifest),str(a.probe),str(report),'--pyramid-region-grid','--negative-query',q]
    if report.exists():command.append('--resume')
    with (a.output/(q+'.log')).open('a') as log:
        result=subprocess.run(command,stdout=log,stderr=subprocess.STDOUT)
    return q,{**validate(report,q),'exit_code':result.returncode,'execution':'measured_or_resumed'}

def checkpoint():
    summary['completed_queries']=len(summary['queries'])
    summary['completed_pairs']=sum(v['completed_pairs'] for v in summary['queries'].values())
    summary['region_supported_pairs']=sum(v['region_supported_pairs'] for v in summary['queries'].values())
    summary['whole_candidates']=sum(v['whole_candidates'] for v in summary['queries'].values())
    counts=collections.Counter()
    for v in summary['queries'].values():counts.update(v['status_counts'])
    summary['status_counts']=dict(counts)
    temporary=a.output/'summary.tmp'
    temporary.write_text(json.dumps(summary,indent=2)+'\n')
    temporary.replace(a.output/'summary.json')
checkpoint()
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    jobs={pool.submit(run,q):q for q in queries}
    for job in concurrent.futures.as_completed(jobs):
        q,value=job.result();summary['queries'][q]=value;checkpoint()
        print(q,value['status_counts'],value['region_supported_pairs'],flush=True)
assert summary['completed_queries']==229 and summary['completed_pairs']==229*156
summary['status']='complete_measurement_not_full_library_qualification';checkpoint()
passed=summary['status_counts']=={'ok':229*156} and summary['region_supported_pairs']==summary['whole_candidates']==0
raise SystemExit(0 if passed else 1)
