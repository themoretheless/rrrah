"""Qualify native original-region inference and original-file pixel admission.

Two crumpled-print crop positives, all twenty source-quadrant/foreign-full-or-
quadrant negatives, full crumpled baseline and exact full-rephoto proof parity.
Explicit crop API only: automatic tiled portfolio/collection integration and
broad negative precision remain unqualified. Tensor/RSS use is measured, not
admitted by the managed payload budget.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time

ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-loftr-regions-frozen-20261010.json'
BINARY=T/'loftr-regions-frozen-qualified-20261010'
MANIFEST=D/'dedup-model-region-snapshot-20261010.json'
RUNTIME=D/'dedup-loftr-native-20261010-runtime-pins.json'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
manifest=json.loads(MANIFEST.read_text())
runtime=json.loads(RUNTIME.read_text())['pins']
def verify():
    assert all(sha(Path(manifest['snapshot'])/p)==h for p,h in manifest['file_hashes'].items())
    assert all(sha(p)==h for p,h in runtime.items())
verify()
assert not OUT.exists() and not BINARY.exists()
shutil.copy2(ROOT/'target/debug/compare_regions',BINARY)
weights=T/'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
a=T/'original-resolution-negative-200201/200300.jpg'
b=T/'original-resolution-strong-all/200302.jpg'
foreign=T/'original-resolution-negative-200201/201400.jpg'
left=T/'original-resolution-negative-200201/209400.jpg'
right=T/'original-resolution-strong-all/209401.jpg'
cases=[('crumpled-source-crop',a,b,'5.12',[1024,0,1024,768],[0,0,800,600],True),
       ('crumpled-both-crops',a,b,'5.12',[1024,0,1024,768],[400,0,400,300],True),
       ('full-crumpled-baseline',a,b,'5.12',[0,0,2048,1536],[0,0,800,600],False),
       ('full-rephoto-parity',left,right,'5.157305482252365',[0,0,2560,1920],[0,0,1024,701],True)]
for sy in range(2):
    for sx in range(2):
        lr=[1024*sx,768*sy,1024,768]
        for index,rr in enumerate([[0,0,2560,1920],*[[1280*tx,960*ty,1280,960] for ty in range(2) for tx in range(2)]]):
            cases.append((f'foreign-{sx}-{sy}-{index}',a,foreign,'1.6',lr,rr,False))
assert len(cases)==24
pins={str(p):sha(p) for p in [BINARY,weights,Path(__file__),MANIFEST,RUNTIME,a,b,foreign,left,right,D/'dedup-model-region-frozen-native-build-20261010.log',D/'dedup-loftr-regions-initial-20261010.json',D/'dedup-loftr-library-f32-measurement-20261010.json']}
r={'status':'running','scope':__doc__,'pins':pins,'results':[],'producer_pid':os.getpid()}
def save():
    temp=OUT.with_suffix('.tmp');temp.write_text(json.dumps(r,indent=2)+'\n');temp.replace(OUT)
save()
try:
    initial=json.loads((D/'dedup-loftr-regions-initial-20261010.json').read_text())
    prior=json.loads((D/'dedup-loftr-library-f32-measurement-20261010.json').read_text())
    for name,source,target,tol,lr,rr,expected in cases:
        row={'case':name,'status':'running','source':str(source),'target':str(target),'left_rectangle':lr,'right_rectangle':rr,'expected_supported':expected};r['results'].append(row);save()
        args=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH=/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib',str(BINARY),str(source),str(target),str(weights),'640',tol,*map(str,lr+rr)]
        started=time.monotonic()
        p=subprocess.Popen(args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['process_pid']=p.pid;save()
        stdout,stderr=p.communicate();row.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-started);save()
        assert p.returncode==0,stderr
        e=json.loads(stdout);row['evidence']=e;row['process_peak_rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1])
        assert e['status']=='ok' and e['supported']==expected
        assert e['managed_used']==0 and e['managed_peak']<=512*1024**2
        if name.startswith('crumpled-'):
            before=next(x['evidence'] for x in initial['results'] if x['case']==name)
            row['exact_initial_evidence_parity']=all(e[k]==v for k,v in before.items() if k not in ('scope','managed_peak'))
            assert row['exact_initial_evidence_parity']
            assert e['pixels']['supported'] and min(e['pixels']['anchors'])>=10
        if name=='full-rephoto-parity':
            before=next(x['evidence'] for x in prior['results'] if x['source']==str(left) and x['max_side']==640)
            row['exact_full_evidence_parity']=all(e[k]==v for k,v in before.items() if k not in ('scope','managed_peak'))
            assert row['exact_full_evidence_parity']
        row['status']='complete';save();print(name,e['supported'],e['geometry_inliers'],flush=True)
    verify()
    assert all(sha(p)==h for p,h in pins.items())
    r['status']='complete_twenty_four_native_region_comparisons'
except Exception as error:
    r.update(status='failed',error=str(error));raise
finally:
    save()
