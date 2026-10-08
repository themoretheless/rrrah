#!/usr/bin/env python3
"""Compare indexed two-source proposals/confirmation with pinned direct evidence."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
p=argparse.ArgumentParser()
for name in ('manifest','probe','report','reference','output'):p.add_argument(name,type=Path)
p.add_argument('--checkpoint',action='store_true')
a=p.parse_args();digest=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
captures={path:path.read_bytes() for path in (a.manifest,a.report,a.reference)}
m,r,b=[json.loads(captures[path]) for path in (a.manifest,a.report,a.reference)]
interpolated=r['mode']=='gradient_pyramid_collection_interpolated'
assert r['mode'] in ('gradient_pyramid_collection_fixed','gradient_pyramid_collection_interpolated')
assert b['mode']==('gradient_pyramid_public_files_interpolated' if interpolated else 'gradient_pyramid_public_files')
assert r['negative_query'] is b['negative_query'] is None
assert r['probe_sha256']==digest(a.probe)
assert b['probe_sha256']==digest(a.probe.parent/('photo-probe-gradient-interpolated-file' if interpolated else 'photo-probe-gradient-file'))
for v in (r,b):
 assert v['manifest_sha256']==digest(a.manifest)
 assert v['required_pairs']==229 and v['completed_pairs']==len(v['results'])
 assert v['status_counts']==dict(collections.Counter(row['status'] for row in v['results']))
 assert v['candidates']==sum(row['status']=='ok' and row['evidence']['candidate'] for row in v['results'])
assert b['measurement_complete'] and b['completed_pairs']==229
assert 0<r['completed_pairs']<=229
if not a.checkpoint:assert r['measurement_complete'] and r['completed_pairs']==229
lost=[];not_retrieved=[];images=set()
for row,old,pair in zip(r['results'],b['results'],m['positive_pairs']):
 for v in (row,old):
  assert v['query_id']==pair['query_id'] and v['label']==pair['label']
  assert v['status']=='ok' and v['returncode']==0
 e,be=row['evidence'],old['evidence']
 assert e['status']=='ok' and e['managed_used_after_drop']==0
 assert type(e['managed_peak']) is int and 0<=e['managed_peak']<=64*1024*1024
 assert type(e['retrieved']) is bool and type(e['candidate']) is bool
 assert type(e['indexed_features']) is int and 0<=e['indexed_features']<=3000
 assert type(e['descriptor_hits']) is int and 0<=e['descriptor_hits']<=9_000_000
 if e['retrieved']:
  for key in ('candidate','correspondences','inliers','geometry','pixels'):assert e[key]==be[key],(pair['query_id'],key)
 else:
  not_retrieved.append(pair['query_id'])
  assert e['candidate'] is False and e['geometry'] is None and e['pixels'] is None
  assert e['correspondences']==e['inliers']==0
  if be['candidate']:lost.append(pair['query_id'])
 if e['pixels'] is not None:
  assert e['pixels']['status']=='ok'
  counts=e['pixels']['counts'];assert len(counts)==2
  for n,c,s in counts:
   assert all(type(v) is int for v in (n,c,s)) and 0<=n<=c<=s and s>0
  assert e['candidate']==all(c>=1000 and c>=s*.3 and n>=c*.9 for n,c,s in counts)
 for side in ('left','right'):
  image=pair[side];path=Path(image['normalized_path'])
  if path not in images:assert digest(path)==image['normalized_sha256'];images.add(path)
for path,raw in captures.items():assert path.read_bytes()==raw,'Changed report: retry audit.'
out={'verified_pairs':r['completed_pairs'],'required_pairs':229,'candidates':r['candidates'],
 'not_retrieved':not_retrieved,'retrieval_losses':lost,'verified_images':len(images),
 'retrieved_native_fields_equal':True,'report_sha256':digest(a.report),'reference_sha256':digest(a.reference),
 'scope':'Fixed two-source indexed runs vs pinned direct native evidence. Not full386-image collection, scale, five-family union or broad coverage.'}
a.output.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({k:out[k] for k in ('verified_pairs','candidates','retrieval_losses')},indent=2))
