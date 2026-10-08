"""Input provenance, exhaustive same-frame swaps and support-predicate audit."""
import hashlib,json,struct
from pathlib import Path
D=Path('docs/research');report=D/'dedup-anchor-real-pixel-negatives.json';a=json.loads(report.read_text());h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for p,digest in a['pins'].items():assert h(p)==digest,p
T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');m=json.loads((T/'prepared.json').read_text());groups={x['filename']:x['group_id'] for x in m['images']['strong']};baseline=json.loads((D/'dedup-combined-domain-release-original-full.json').read_text());oracle=json.loads((D/'dedup-geometric-anchor-rank-bidirectional.json').read_text())['summary'];seen=set();max_agreement=0.;enough=0
for row in a['rows']:
 q=row['borrowed_geometry_query'];other=row['unrelated_query'];key=(q,other);assert key not in seen;seen.add(key);assert groups[q]!=groups[other]
 e=next(x['evidence'] for x in baseline['results'] if x['query']==q)
 model=T/'anchor-rank-native'/f'{q[:-4]}-model.txt';points=T/'anchor-rank-native'/f'{q[:-4]}-points.txt'
 assert list(map(float,model.read_text().split()))==[x for r in e['matrix'] for x in r]
 assert [list(map(float,line.split())) for line in points.read_text().splitlines()]==[[x for p in e['correspondences'][i] for x in p] for i in e['inliers']]
 def dims(name):
  with (T/'all-photometric-local-rank'/f'{name[:-4]}.rgba32').open('rb') as f:f.read(21);return struct.unpack('<II',f.read(8))
 assert dims(q)==dims(other)
 assert [x['filter_radius'] for x in row['evidence']]==[0,3,8]
 for result in row['evidence']:
  assert result['status']=='ok';predicates=[]
  for i,direction in enumerate(['forward','reverse']):
   expected=next(x for x in oracle if x['query']==q and x['filter_radius']==result['filter_radius'] and x['direction']==direction)
   assert result[direction+'_anchors']==len(expected['anchor_indices'])>=10
   v=result['directions'][i];assert v['sites']==expected['sites'] and 0<=v['agreeing_pairs']<=v['informative_pairs']<=8*v['valid_sites']<=8*v['sites']
   agreement=v['agreeing_pairs']/v['informative_pairs'] if v['informative_pairs'] else 0.;predicates.append(v['informative_pairs']>=1000 and v['valid_sites']/v['sites']>=.3 and agreement>=.9)
   if result['filter_radius']==8:max_agreement=max(max_agreement,agreement);enough+=v['informative_pairs']>=1000
  assert result['supported']==all(predicates)
assert len(seen)==a['pairs']==24 and a['false_supports_filter8']==0
out=D/'dedup-anchor-real-pixel-negatives-audit.json';out.write_text(json.dumps({'status':'verified_real_pixel_negative_provenance_and_predicates','pairs':24,'native_filter_cases':72,'filter8_false_supports':0,'filter8_directions_with_sufficient_information':enough,'maximum_filter8_direction_agreement':max_agreement,'pins':{str(p):h(p) for p in [report,Path(__file__)]},'scope':'Pins, source groups, preserved original models/points, same dimensions, anchor counts and native count predicates. Pixel counts are native, not independently recomputed here. Borrowed positive geometry, not end-to-end retrieved hard negatives.'},indent=2)+'\n');print('Verified24 real negatives; max direction agreement',max_agreement,'informative directions',enough)
