"""Complete automatic collection search: rotation controls and original-resolution rephoto/foreign trio.

Every unordered pair is required. This does not qualify the full corpus, all
formats, Windows, total inference RSS admission or scalable indexed retrieval.
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
OUT=D/'dedup-portfolio-collection-measurement-20261010.json'
BINARY=T/'loftr-portfolio-collection-qualified-20261010'
MANIFEST=D/'dedup-portfolio-collection-snapshot-20261010.json'
RUNTIME=D/'dedup-loftr-native-20261010-runtime-pins.json'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
s=json.loads(MANIFEST.read_text())
def verify():
    assert all(sha(ROOT/p)==h and sha(Path(s['snapshot'])/p)==h for p,h in s['file_hashes'].items())
    assert all(sha(p)==h for p,h in json.loads(RUNTIME.read_text())['pins'].items())
verify()
assert not OUT.exists() and not BINARY.exists()
shutil.copy2(ROOT/'target/debug/collection_files',BINARY)
weights=T/'learned-matching-models/loftr-outdoor-native-20261010.safetensors'
f=ROOT/'crates/rrrah-dedup/tests/fixtures/rotation'
original=T/'original-resolution-negative-200201/209400.jpg'
rephoto=T/'original-resolution-strong-all/209401.jpg'
foreign=T/'original-resolution-negative-200201/201400.jpg'
cases=[('rotation-controls',320,'2',[f/'base.png',f/'mirrored.png',f/'unrelated.png']),('original-resolution-three-files',640,'5.157305482252365',[original,rephoto,foreign])]
files=[BINARY,MANIFEST,RUNTIME,weights,Path(__file__),D/'dedup-portfolio-collection-native-build-20261010.log']+[p for _,_,_,paths in cases for p in paths]
r=dict(status='running',scope=__doc__,pins={str(p):sha(p) for p in files},results=[],producer_pid=os.getpid())
def save():
    tmp=OUT.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(OUT)
save()
try:
    dyld='/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'
    baseline=json.loads((D/'dedup-loftr-library-f32-measurement-20261010.json').read_text())
    for name,side,tolerance,paths in cases:
        row=dict(case=name,max_side=side,paths=list(map(str,paths)),order='model-first',status='running')
        r['results'].append(row);save()
        args=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+dyld,str(BINARY),str(weights),str(side),tolerance,'model-first',*map(str,paths)]
        started=time.monotonic()
        p=subprocess.Popen(args,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        row['process_pid']=p.pid;save()
        stdout,stderr=p.communicate()
        row.update(returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-started)
        save()
        assert p.returncode==0,stderr
        e=json.loads(stdout);row['evidence']=e
        row['process_peak_rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1])
        assert e['status']=='ok' and e['files']==[0,1,2]
        assert [(p['left'],p['right']) for p in e['pairs']]==[(0,1),(0,2),(1,2)]
        assert [p['evidence']['supported'] for p in e['pairs']]==[True,False,False]
        assert e['managed_used']==0 and e['managed_retained_before_drop']>0 and e['managed_peak']<=512*1024*1024
        if name=='original-resolution-three-files':
            before=next(x['evidence'] for x in baseline['results'] if x['source']==str(original) and x['max_side']==side)
            pair=e['pairs'][0]['evidence']
            row['exact_positive_model_evidence_parity']=all(pair[k]==v for k,v in before.items() if k not in ('scope','status','managed_used','managed_peak'))
            assert row['exact_positive_model_evidence_parity']
        verify()
        row['status']='complete';save();print(name,side,'three complete pairs, one supported',flush=True)
    assert all(sha(p)==h for p,h in r['pins'].items())
    r['status']='complete_two_three_file_collections_six_pairs'
except Exception as error:
    r.update(status='failed',error=str(error));raise
finally:
    save()
