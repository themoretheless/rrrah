"""Audit all16 fixed-geometry radius8 experiments; no full pixel oracle claim."""
import hashlib,json,math
from pathlib import Path
b=Path('docs/research');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
p=b/'dedup-main-photometric-misses-radius8.json';r=json.loads(p.read_text());assert r['status']=='complete' and r['required_pairs']==len(r['results'])==15
assert all(h(k)==v for k,v in r['input_hashes'].items())
base=b/'dedup-combined-domain-release-original-full.json';old=json.loads(base.read_text());before={v['query']:v['evidence'] for v in old['results']}
single=b/'dedup-main-problem-blur8-205901-screen-audit.json';s=json.loads(single.read_text());assert s['status']=='verified_fixed_geometry_radius8_predicates' and all(h(k)==v for k,v in s['pins'].items())
summary=[]
for row in r['results']:
 assert row['returncode']==0 and json.loads(row['stdout'])==row['evidence'];e=row['evidence'];initial=before[row['query']]
 assert initial['region_support_count']==0 and all(e[k]==initial[k] for k in ['correspondences','matrix','inliers'])
 assert [v['domains'] for v in e['regions']]==[v['domains'] for v in initial['regions']]
 assert e['status']=='ok' and e['managed_used']==0 and e['retained_before_drop']<=e['managed_peak']<=512*1024*1024
 for v in e['regions']:
  pixels=v['pixels'];accepted=False
  if pixels is not None:
   assert v['fit_failure'] is None
   for rect,(m,c,t) in zip(v['domains'],pixels['counts']):assert all(type(n) is int for n in [m,c,t]) and 0<=m<=c<=t and t==rect[2]*rect[3]
   for gains,offsets in zip(pixels['gains'],pixels['offsets']):
    assert all(math.isfinite(x) and .2-1e-9<=x<=5+1e-9 for x in gains)
    assert all(math.isfinite(x) and abs(x)<=.1+1e-9 for x in offsets)
   accepted=all(c>=1000 and c/t>=.3 and m/c>=.9 for m,c,t in pixels['counts'])
  assert v['accepted']==accepted
 assert sum(v['accepted'] for v in e['regions'])==e['region_support_count']
 summary.append({'query':row['query'],'supported_regions':e['region_support_count']})
summary.append({'query':'205901.jpg','supported_regions':s['radius8_supported_regions']});assert len(summary)==16
out={'status':'verified_fixed_geometry_radius8_all_photometric_misses','cases':16,'new_local_supported':sum(bool(v['supported_regions']) for v in summary),'still_unsupported':sum(not v['supported_regions'] for v in summary),'summary':summary,'input_hashes':{str(x):h(x) for x in [p,base,single,Path(__file__)]},'scope':'Same correspondences/model/inliers and region domains for all16; predicate math/source provenance/managed bounds. No independent resampling proof, broad negative precision, whole-copy or indexed collection claim. Radius8 remains explicit diagnostic.'}
(b/'dedup-main-photometric-misses-radius8-audit.json').write_text(json.dumps(out,indent=2)+'\n');print(out['new_local_supported'],out['still_unsupported'])
