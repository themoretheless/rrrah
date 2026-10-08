#!/usr/bin/env python3
"""Pinned intermediate-scale direct/collection measurements; refusals are errors."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path
p=argparse.ArgumentParser()
for key in ('manifest','probe','output'):p.add_argument(key,type=Path)
p.add_argument('--negative-query')
a=p.parse_args()
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert not a.output.exists(), 'Preserve existing measurement; choose a fresh output.'
m=json.loads(a.manifest.read_text());pairs=m['positive_pairs'];assert len(pairs)==229
if a.negative_query:
 q=next(v['right'] for v in pairs if v['query_id']==a.negative_query)
 pairs=[{'query_id':q['filename']+'/'+v['filename'],'left':v,'right':q,'label':'different_publisher_origin'} for v in m['images']['original'] if v['group_id']!=q['group_id']];assert len(pairs)==156
pins={str(a.manifest):digest(a.manifest),str(a.probe):digest(a.probe),str(Path(__file__).resolve()):digest(__file__)}
for pair in pairs:
 for side in ('left','right'):
  image=pair[side];assert digest(image['normalized_path'])==image['normalized_sha256'];pins[image['normalized_path']]=image['normalized_sha256']
r={'status':'running','input_hashes':pins,'required_pairs':len(pairs),'negative_query':a.negative_query,'results':[],'scope':'Fixed four-scale recipe, two-source collection vs fresh direct confirmations; no single full collection or broad precision claim.'}
def save():
 tmp=a.output.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(a.output)
save()
for pair in pairs:
 row={'query_id':pair['query_id'],'label':pair['label']};r['results'].append(row)
 for mode in ('direct','collection'):
  run=subprocess.run([str(a.probe.resolve()),mode,pair['left']['normalized_path'],pair['right']['normalized_path']],capture_output=True,text=True)
  row[mode]={'returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr};save()
  if run.returncode:r['status']='native_failed';save();raise SystemExit(run.returncode)
  e=json.loads(run.stdout);row[mode]['evidence']=e
  assert e['status']=='ok' and type(e['candidate']) is bool
  assert type(e['managed_used']) is int and e['managed_used']==0
  assert type(e['managed_peak']) is int and 0<=e['managed_peak']<=67108864
 direct=row['direct']['evidence'];collection=row['collection']['evidence'];assert type(collection['retrieved']) is bool
 if collection['retrieved']:
  assert {k:v for k,v in direct.items() if k not in ('managed_peak','retrieved')}=={k:v for k,v in collection.items() if k not in ('managed_peak','retrieved')}
 else:assert not direct['candidate'] and not collection['candidate']
 assert all(digest(path)==value for path,value in pins.items());save()
r['candidates']=sum(row['direct']['evidence']['candidate'] for row in r['results']);r['status']='complete';save()
if a.negative_query and r['candidates']:raise SystemExit(1)
