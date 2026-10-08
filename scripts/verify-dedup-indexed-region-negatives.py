#!/usr/bin/env python3
"""A fixed indexed-regional negative query; no broad precision inference."""
import argparse,collections,hashlib,json
from pathlib import Path
p=argparse.ArgumentParser()
for key in ('manifest','probe','report','query','output'):p.add_argument(key,type=str)
p.add_argument('--checkpoint',action='store_true');a=p.parse_args()
manifest,probe,report,output=map(Path,(a.manifest,a.probe,a.report,a.output))
raw=report.read_bytes();manifest_raw=manifest.read_bytes();r=json.loads(raw);m=json.loads(manifest_raw)
h=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
assert r['mode']=='pyramid_region_grid_collection' and r['probe_sha256']==h(probe)
assert r['manifest_sha256']==hashlib.sha256(manifest_raw).hexdigest()
query=next(pair['right'] for pair in m['positive_pairs'] if pair['query_id']==a.query)
originals=[o for o in m['images']['original'] if o['group_id']!=query['group_id']]
assert len(originals)==r['required_pairs']==156
assert 0<r['completed_pairs']==len(r['results'])<=156
if not a.checkpoint:assert r['completed_pairs']==156 and r['status']=='complete_measurement_not_full_library_qualification'
images=set();retrieved=0
for row,original in zip(r['results'],originals):
 assert row['query_id']==query['filename']+'/'+original['filename'] and row['label']=='different_publisher_origin'
 assert row['status']=='ok' and row['returncode']==0
 e=row['evidence'];assert e['status']=='ok' and e['candidate'] is False and type(e['retrieved'])is bool
 assert e['managed_used_after_drop']==0 and type(e['managed_peak'])is int and 0<=e['managed_peak']<=64*1024*1024
 assert type(e['indexed_features'])is int and 0<=e['indexed_features']<=3000
 assert type(e['descriptor_hits'])is int and 0<=e['descriptor_hits']<=9_000_000
 assert e['region_support_count']==0 and all(region['accepted_region'] is False for region in e['regions'])
 for region in e['regions']:
  if region['counts'] is None:assert region['fit_failure']!='None'
  else:
   assert region['fit_failure']=='None' and len(region['counts'])==len(region['domains'])==2
   accepted=True
   for values,domain in zip(region['counts'],region['domains']):
    assert len(values)==3 and all(type(v)is int and v>=0 for v in values)
    assert len(domain)==4 and all(type(v)is int and v>=0 for v in domain) and domain[2]>0 and domain[3]>0
    matched,compared,source=values;assert matched<=compared<=source==domain[2]*domain[3]
    accepted &= compared>=1000 and compared>=source*.3 and matched>=compared*.9
   assert accepted is False
 if not e['retrieved']:assert e['regions']==[] and e['geometry'] is None
 retrieved+=e['retrieved']
 for image in (original,query):
  path=Path(image['normalized_path'])
  if path not in images:assert h(path)==image['normalized_sha256'];images.add(path)
assert r['status_counts']==dict(collections.Counter(row['status'] for row in r['results']))
assert r['candidates']==r['region_supported_pairs']==r['region_support_total']==0
assert report.read_bytes()==raw and manifest.read_bytes()==manifest_raw,'Report changed: retry audit.'
out={'verified_pairs':r['completed_pairs'],'required_pairs':156,'query_id':a.query,'retrieved_pairs':retrieved,'verified_images':len(images),'whole_candidates':0,'region_supported_pairs':0,'report_sha256':hashlib.sha256(raw).hexdigest(),'scope':'One fixed query against different publisher-origin originals, indexed regional API. No errors count as rejection. Not burst/semantic, all-query precision, full collection scale or full-library qualification.'}
output.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
