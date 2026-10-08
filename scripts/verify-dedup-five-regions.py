#!/usr/bin/env python3
"""Indexed five-search retrieval loss and native confirmation parity."""
import argparse,collections,hashlib,json
from pathlib import Path
p=argparse.ArgumentParser()
for name in ('manifest','probe','report','reference','regional_reference','output'):p.add_argument(name,type=Path)
p.add_argument('--checkpoint',action='store_true');a=p.parse_args()
digest=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
captured={v:v.read_bytes() for v in (a.manifest,a.report,a.reference,a.regional_reference)}
m,r,b,g=[json.loads(captured[v]) for v in (a.manifest,a.report,a.reference,a.regional_reference)]
assert r['mode']=='five_regions_collection' and b['mode']=='complementary_gradient_portfolio'
assert r['probe_sha256']==digest(a.probe)
assert b['probe_sha256']==digest(a.probe.parent/'photo-probe-five-lane')
assert g['mode']=='pyramid_region_grid' and g['probe_sha256']==digest(a.probe.parent/'photo-probe-native-region-grid')
assert g['manifest_sha256']==digest(a.manifest) and g['required_pairs']==g['completed_pairs']==len(g['results'])==229
for v in (r,b):
 assert v['manifest_sha256']==digest(a.manifest) and v['required_pairs']==229
 assert v['completed_pairs']==len(v['results'])
 assert v['status_counts']==dict(collections.Counter(row['status'] for row in v['results']))
 assert v['candidates']==sum(row['evidence']['candidate'] for row in v['results'])
assert len(m['positive_pairs'])==229
assert len({pair['query_id'] for pair in m['positive_pairs']})==229
assert b['completed_pairs']==229 and b['status']=='complete_measurement_not_full_library_qualification'
assert r['region_supported_pairs']==sum(row['evidence']['region_support_count']>0 for row in r['results'])
assert r['region_support_total']==sum(row['evidence']['region_support_count'] for row in r['results'])
assert 0<r['completed_pairs']<=229
if not a.checkpoint:assert r['completed_pairs']==229 and r['status']=='complete_measurement_not_full_library_qualification'
lost=[];unretrieved=[];images=set()
for row,old,regional,pair in zip(r['results'],b['results'],g['results'],m['positive_pairs']):
 for v in (row,old):
  assert v['query_id']==pair['query_id'] and v['label']==pair['label']
  assert v['status']=='ok' and v['returncode']==0
 e,be=row['evidence'],old['evidence']
 assert regional['query_id']==pair['query_id'] and regional['label']==pair['label'] and regional['status']=='ok' and regional['returncode']==0
 ge=regional['evidence']
 assert type(e['region_support_count'])is int and e['region_support_count']==sum(region['accepted_region'] for region in e['regions'])
 assert all(type(region['accepted_region'])is bool for region in e['regions'])
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
  for key in ('regions','region_support_count'):assert e[key]==ge[key],(pair['query_id'],key)
  assert e['regional_geometry']==ge['geometry'] and e['regional_whole_candidate']==ge['candidate']
 else:
  assert e['candidate'] is False and e['regions']==[] and e['region_support_count']==0
  assert not any(key in e for key in ('five_accepted_searches','four_accepted_searches','regional_geometry','regional_whole_candidate'))
  unretrieved.append(pair['query_id'])
  if be['candidate'] or ge['region_support_count']>0:lost.append(pair['query_id'])
 for side in ('left','right'):
  image=pair[side];path=Path(image['normalized_path'])
  if path not in images:assert digest(path)==image['normalized_sha256'];images.add(path)
for path,raw in captured.items():assert path.read_bytes()==raw,'Report changed: retry audit.'
out={'verified_pairs':r['completed_pairs'],'required_pairs':229,'candidates':r['candidates'],
 'region_supported_pairs':r['region_supported_pairs'],'retrieval_losses':lost,'not_retrieved':unretrieved,'verified_images':len(images),
 'retrieved_native_fields_equal_except_peak':True,'report_sha256':digest(a.report),'reference_sha256':digest(a.reference),
 'scope':'Two-source native unified indexed five-search and regional confirmation versus both pinned constituents. Not one386-image collection, scale, semantic/burst or full-library qualification.'}
a.output.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({k:out[k] for k in ('verified_pairs','candidates','region_supported_pairs','retrieval_losses')},indent=2))
