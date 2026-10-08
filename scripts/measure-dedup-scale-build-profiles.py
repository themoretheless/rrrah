#!/usr/bin/env python3
"""Alternate same-source binaries; require exact outputs before timing claims."""
import argparse
import hashlib
import json
import resource
import statistics
import subprocess
import time
from pathlib import Path
p=argparse.ArgumentParser()
for key in ('snapshot','debug','release','cases','output'):p.add_argument(key,type=Path)
a=p.parse_args();assert not a.output.exists()
digest=lambda path:hashlib.sha256(Path(path).read_bytes()).hexdigest()
manifest=a.snapshot/'validation-snapshot.json';source=json.loads(manifest.read_text())
def verify_source():
 for path,value in source['files'].items():assert digest(a.snapshot/path)==value,path
verify_source();cases=json.loads(a.cases.read_text());assert len({v['case'] for v in cases})==len(cases)>0
pins={str(path):digest(path) for path in (a.debug,a.release,a.cases,manifest,Path(__file__).resolve())}
for case in cases:
 for side in ('left','right'):pins[case[side]]=digest(case[side])
r={'status':'running','input_hashes':pins,'samples':[],'scope':'Alternating profiles on identical immutable sources and fixed two-source cases under concurrent background load. CPU/wall times only; not full-collection scale or general throughput.'}
def save():a.output.write_text(json.dumps(r,indent=2)+'\n')
save();reference={}
for case in cases:
 for mode in ('direct','collection'):
  for repeat in range(6):
   for name in (('debug','release') if repeat%2==0 else ('release','debug')):
    before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic()
    run=subprocess.run([str(getattr(a,name).resolve()),mode,case['left'],case['right']],capture_output=True,text=True)
    elapsed=time.monotonic()-start;after=resource.getrusage(resource.RUSAGE_CHILDREN)
    row={'case':case['case'],'mode':mode,'profile':name,'repeat':repeat,'warmup':repeat==0,'returncode':run.returncode,'wall_seconds':elapsed,'cpu_seconds':after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime,'stdout':run.stdout,'stderr':run.stderr};r['samples'].append(row);save()
    if run.returncode:r['status']='native_failed';save();raise SystemExit(run.returncode)
    e=json.loads(run.stdout);assert e['status']=='ok' and e['managed_used']==0
    canonical=json.dumps(e,sort_keys=True,allow_nan=False);key=(case['case'],mode)
    if key in reference and reference[key]!=canonical:r['status']='output_parity_failed';save();raise SystemExit(1)
    reference[key]=canonical
    assert all(digest(path)==value for path,value in pins.items())
r['medians']=[]
for case in cases:
 for mode in ('direct','collection'):
  values={name:{metric:statistics.median(v[metric] for v in r['samples'] if v['case']==case['case'] and v['mode']==mode and v['profile']==name and not v['warmup']) for metric in ('cpu_seconds','wall_seconds')} for name in ('debug','release')}
  r['medians'].append({'case':case['case'],'mode':mode,'profiles':values})
verify_source();r['status']='verified_output_parity_and_profile_measurements';save()
