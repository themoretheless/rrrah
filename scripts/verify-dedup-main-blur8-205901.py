"""Verify the terminal fixed-geometry blur diagnostic's provenance and predicates."""
import hashlib,json,math
from pathlib import Path
b=Path('docs/research');pin=b/'dedup-main-problem-blur8-205901-screen-pins.json';pins=json.loads(pin.read_text());h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert all(h(p)==v for p,v in pins.items())
output=b/'dedup-main-problem-blur8-205901-screen.jsonl';lines=output.read_text().splitlines();assert len(lines)==1;e=json.loads(lines[0]);assert e['status']=='ok' and e['managed_used']==0 and e['managed_peak']<=512*1024*1024
baseline=json.loads((b/'dedup-combined-domain-release-original-full.json').read_text());old=next(r['evidence'] for r in baseline['results'] if r['query']=='205901.jpg')
assert all(e[k]==old[k] for k in ['correspondences','matrix','inliers']);assert [r['domains'] for r in e['regions']]==[r['domains'] for r in old['regions']]
for r in e['regions']:
 p=r['pixels'];accepted=False
 if p is not None:
  assert r['fit_failure'] is None
  for rect,(m,c,t) in zip(r['domains'],p['counts']):assert 0<=m<=c<=t and t==rect[2]*rect[3]
  for gains,offsets in zip(p['gains'],p['offsets']):
   assert all(math.isfinite(v) and .2-1e-9<=v<=5+1e-9 for v in gains)
   assert all(math.isfinite(v) and abs(v)<=.1+1e-9 for v in offsets)
  accepted=all(c>=1000 and c/t>=.3 and m/c>=.9 for m,c,t in p['counts'])
 assert accepted==r['accepted']
assert sum(r['accepted'] for r in e['regions'])==e['region_support_count']
summary={'status':'verified_fixed_geometry_radius8_predicates','baseline_supported_regions':old['region_support_count'],'radius8_supported_regions':e['region_support_count'],'same_geometry':True,'same_region_domains':True,'pins':{**pins,str(output):h(output),str(pin):h(pin),str(Path(__file__)):h(__file__)},'scope':'Unchanged geometry and domains, source hashes/managed caps and predicate math. Does not independently verify native pixel resampling or semantic precision; no default promotion.'}
(b/'dedup-main-problem-blur8-205901-screen-audit.json').write_text(json.dumps(summary,indent=2)+'\n');print(summary['radius8_supported_regions'])
