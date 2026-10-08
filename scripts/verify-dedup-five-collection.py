#!/usr/bin/env python3
"""Indexed five-search retrieval loss and native confirmation parity."""
import argparse,collections,hashlib,json
from pathlib import Path
p=argparse.ArgumentParser()
for name in ('manifest','probe','report','reference','output'):p.add_argument(name,type=Path)
p.add_argument('--checkpoint',action='store_true');a=p.parse_args()
digest=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
captured={v:v.read_bytes() for v in (a.manifest,a.report,a.reference)}
m,r,b=[json.loads(captured[v]) for v in (a.manifest,a.report,a.reference)]
assert r['mode']=='five_search_collection' and b['mode']=='complementary_gradient_portfolio'
assert r['probe_sha256']==digest(a.probe)
assert b['probe_sha256']==digest(a.probe.parent/'photo-probe-five-lane')
for v in (r,b):
 assert v['manifest_sha256']==digest(a.manifest) and v['required_pairs']==229
 assert v['completed_pairs']==len(v['results'])
 assert v['status_counts']==dict(collections.Counter(row['status'] for row in v['results']))
 assert v['candidates']==sum(row['evidence']['candidate'] for row in v['results'])
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
 assert e['status']=='ok' and e['managed_used']==0
 assert type(e['managed_peak']) is int and 0<=e['managed_peak']<=64*1024*1024
 assert type(e['candidate']) is bool and type(e['retrieved']) is bool
 assert type(e['indexed_features']) is int and 0<=e['indexed_features']<=10000
 assert type(e['descriptor_hits']) is int and 0<=e['descriptor_hits']<=34_000_000
 if e['retrieved']:
  for key,value in be.items():
   if key!='managed_peak':assert e[key]==value,(pair['query_id'],key)
  assert len(e['five_accepted_searches'])==5 and all(type(v) is bool for v in e['five_accepted_searches'])
  assert e['candidate']==any(e['five_accepted_searches'])
 else:
  assert e['candidate'] is False
  unretrieved.append(pair['query_id'])
  if be['candidate']:lost.append(pair['query_id'])
 for side in ('left','right'):
  image=pair[side];path=Path(image['normalized_path'])
  if path not in images:assert digest(path)==image['normalized_sha256'];images.add(path)
for path,raw in captured.items():assert path.read_bytes()==raw,'Report changed: retry audit.'
out={'verified_pairs':r['completed_pairs'],'required_pairs':229,'candidates':r['candidates'],
 'retrieval_losses':lost,'not_retrieved':unretrieved,'verified_images':len(images),
 'retrieved_native_fields_equal_except_peak':True,'report_sha256':digest(a.report),'reference_sha256':digest(a.reference),
 'scope':'Two-source native indexed five-search pairs vs pinned direct confirmation. Not one386-image collection, scale, semantic/burst or full-library qualification.'}
a.output.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({k:out[k] for k in ('verified_pairs','candidates','retrieval_losses')},indent=2))
