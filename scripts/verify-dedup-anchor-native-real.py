"""Verify immutable native inputs and exact independent anchor pixel counts."""
import hashlib,json
from pathlib import Path
D=Path('docs/research');native=D/'dedup-anchor-rank-native-real.json';oracle=D/'dedup-geometric-anchor-rank-bidirectional.json';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
a=json.loads(native.read_text());b=json.loads(oracle.read_text())
for report in [a,b]:
 for p,digest in report['pins'].items():assert h(p)==digest,p
assert len(a['rows'])==12 and len(b['summary'])==24
seen=set()
for row in a['rows']:
 key=(row['query'],row['filter_radius']);assert key not in seen;seen.add(key);assert row['status']=='ok'
 predicates=[]
 for i,direction in enumerate(['forward','reverse']):
  expected=next(x for x in b['summary'] if (x['query'],x['filter_radius'],x['direction'])==(*key,direction))
  assert row[direction+'_anchors']==len(expected['anchor_indices'])>=10
  for k,v in row['directions'][i].items():assert v==expected[k],(key,direction,k)
  e=row['directions'][i];predicates.append(e['informative_pairs']>=1000 and e['valid_sites']/e['sites']>=.3 and e['agreeing_pairs']/e['informative_pairs']>=.9)
 assert row['supported']==all(predicates)
assert sum(x['supported'] for x in a['rows'] if x['filter_radius']==8)==4
out=D/'dedup-anchor-rank-native-real-audit.json';out.write_text(json.dumps({'status':'verified_native_anchor_exact_independent_pixel_counts','cases':4,'direction_filter_counts':24,'pins':{str(p):h(p) for p in [native,oracle,Path(__file__)]},'scope':'Immutable inputs and exact count comparison to independent NumPy oracle, including final support predicate. Shared normalized decoding and prior global model/inliers; no independent candidate retrieval, hard negatives or automatic identity proof.'},indent=2)+'\n');print('Verified24 independent direction/filter count sets')
