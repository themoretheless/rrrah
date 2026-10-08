#!/usr/bin/env python3
"""Indexed regional retrieval loss and native confirmation parity."""
import argparse,collections,hashlib,json
from pathlib import Path
p=argparse.ArgumentParser()
for name in ('manifest','probe','report','reference','output'):p.add_argument(name,type=Path)
p.add_argument('--checkpoint',action='store_true');a=p.parse_args()
digest=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
captured={v:v.read_bytes() for v in (a.manifest,a.report,a.reference)}
m,r,b=[json.loads(captured[v]) for v in (a.manifest,a.report,a.reference)]
assert r['mode']=='pyramid_region_grid_collection' and b['mode']=='pyramid_region_grid'
assert r['probe_sha256']==digest(a.probe)
assert b['probe_sha256']==digest(a.probe.parent/'photo-probe-native-region-grid')
for v in (r,b):
 assert v['manifest_sha256']==digest(a.manifest) and v['required_pairs']==229
 assert v['completed_pairs']==len(v['results'])
 assert v['status_counts']==dict(collections.Counter(row['status'] for row in v['results']))
 assert v['candidates']==sum(row['evidence']['candidate'] for row in v['results'])
 assert v['region_supported_pairs']==sum(row['evidence']['region_support_count']>0 for row in v['results'])
 assert v['region_support_total']==sum(row['evidence']['region_support_count'] for row in v['results'])
assert len(m['positive_pairs'])==229
assert len({pair['query_id'] for pair in m['positive_pairs']})==229
assert b['completed_pairs']==229 and b['status']=='complete_measurement_not_full_library_qualification'
assert 0<r['completed_pairs']<=229
if not a.checkpoint:assert r['completed_pairs']==229 and r['status']=='complete_measurement_not_full_library_qualification'
lost=[];unretrieved=[];images=set()
for row,old,pair in zip(r['results'],b['results'],m['positive_pairs']):
 for v in (row,old):
  assert v['query_id']==pair['query_id'] and v['label']==pair['label']
  assert v['status']=='ok' and v['returncode']==0
 e,be=row['evidence'],old['evidence']
 assert e['status']=='ok' and e['managed_used_after_drop']==0
 assert type(e['managed_peak']) is int and 0<=e['managed_peak']<=64*1024*1024
 assert type(e['candidate']) is bool and type(e['retrieved']) is bool
 assert type(e['indexed_features']) is int and 0<=e['indexed_features']<=3000
 assert type(e['descriptor_hits']) is int and 0<=e['descriptor_hits']<=9_000_000
 assert type(e['region_support_count']) is int
 assert e['region_support_count']==sum(region['accepted_region'] for region in e['regions'])
 for region in e['regions']:
  assert type(region['accepted_region']) is bool
  if region['counts'] is None:assert not region['accepted_region'] and region['fit_failure']!='None'
  else:
   accepted=True
   assert len(region['counts'])==len(region['domains'])==2
   for values,domain in zip(region['counts'],region['domains']):
    assert len(values)==3 and all(type(v)is int and v>=0 for v in values)
    matched,compared,source=values;assert matched<=compared<=source==domain[2]*domain[3]
    accepted &= compared>=1000 and compared>=source*.3 and matched>=compared*.9
   assert region['accepted_region']==accepted
 if e['retrieved']:
  for key in ('candidate','geometry','regions','region_support_count'):
   assert e[key]==be[key],(pair['query_id'],key)
 else:
  assert e['candidate'] is False and e['regions']==[] and e['region_support_count']==0
  unretrieved.append(pair['query_id'])
  if be['candidate'] or be['region_support_count']>0:lost.append(pair['query_id'])
 for side in ('left','right'):
  image=pair[side];path=Path(image['normalized_path'])
  if path not in images:assert digest(path)==image['normalized_sha256'];images.add(path)
for path,raw in captured.items():assert path.read_bytes()==raw,'Report changed: retry audit.'
out={'verified_pairs':r['completed_pairs'],'required_pairs':229,'candidates':r['candidates'],
 'retrieval_losses':lost,'not_retrieved':unretrieved,'verified_images':len(images),
 'retrieved_native_fields_equal':True,'report_sha256':digest(a.report),'reference_sha256':digest(a.reference),
 'scope':'Two-source native indexed regional pairs vs pinned direct confirmation. Not one386-image collection, scale, semantic/burst or full-library qualification.'}
a.output.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({k:out[k] for k in ('verified_pairs','candidates','retrieval_losses')},indent=2))
