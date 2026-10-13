"""Initial native explicit-region API diagnostic; full frozen-source qualification follows.

The two recovered crumpled-print crops, corresponding foreign controls, and
full-image rephoto parity are all required. This is not automatic crop search.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-loftr-regions-initial-20261010.json'
BINARY=T/'loftr-regions-initial-qualified-20261010'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
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
       ('foreign-full',a,foreign,'1.6',[1024,0,1024,768],[0,0,2560,1920],False),
       ('foreign-both-crops',a,foreign,'1.6',[1024,0,1024,768],[1280,0,1280,960],False),
       ('full-rephoto-parity',left,right,'5.157305482252365',[0,0,2560,1920],[0,0,1024,701],True)]
sources=[ROOT/'crates/rrrah-dedup/src'/f for f in ['model_input.rs','model_input_file.rs','model_compare.rs','model_loftr.rs','proposals.rs']]
pins={str(p):sha(p) for p in [BINARY,weights,Path(__file__),*sources,a,b,foreign,left,right,D/'dedup-model-region-native-build-20261010.log']}
runtime=json.loads((D/'dedup-loftr-native-20261010-runtime-pins.json').read_text())['pins']
assert all(sha(p)==h for p,h in runtime.items())
r={'status':'running','scope':__doc__,'pins':pins,'results':[],'producer_pid':os.getpid()}
def save():
    temp=OUT.with_suffix('.tmp');temp.write_text(json.dumps(r,indent=2)+'\n');temp.replace(OUT)
save()
try:
    for name,source,target,tol,lr,rr,expected in cases:
        row={'case':name,'status':'running','source':str(source),'target':str(target),'left_rectangle':lr,'right_rectangle':rr,'expected_supported':expected};r['results'].append(row);save()
        args=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH=/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib',str(BINARY),str(source),str(target),str(weights),'640',tol,*map(str,lr+rr)]
        p=subprocess.Popen(args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['process_pid']=p.pid;save()
        stdout,stderr=p.communicate();row.update(returncode=p.returncode,stdout=stdout,stderr=stderr);save()
        assert p.returncode==0,stderr
        e=json.loads(stdout);row['evidence']=e;row['process_peak_rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1])
        row['matches_expected_support']=e['supported']==expected
        assert e['managed_used']==0
        if name=='full-rephoto-parity':
            prior=json.loads((D/'dedup-loftr-library-f32-measurement-20261010.json').read_text())
            baseline=next(x['evidence'] for x in prior['results'] if x['source']==str(left) and x['max_side']==640)
            row['exact_full_evidence_parity']=all(e[k]==v for k,v in baseline.items() if k not in ('scope','managed_peak'))
        row['status']='complete';save();print(name,e['supported'],e['geometry_inliers'],flush=True)
    assert all(sha(p)==h for p,h in pins.items()) and all(sha(p)==h for p,h in runtime.items())
    r['status']='complete_five_native_region_comparisons'
except Exception as error:
    r.update(status='failed',error=str(error));raise
finally:
    save()
