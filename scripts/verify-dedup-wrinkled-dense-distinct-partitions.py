"""Audit greedy distinct-location selection, every partition and native geometry evidence."""
import argparse,ast,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={str(a.report):h(a.report),str(Path(__file__)):h(__file__)}
r=json.loads(a.report.read_text());assert r['status']=='complete_native_experiment' and all(h(k)==v for k,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
prior=json.loads(pinned('dedup-wrinkled-dense-patches-bounded.json').read_text());audit=json.loads(pinned('dedup-wrinkled-dense-patches-bounded-audit.json').read_text());assert audit['status']=='verified_dense_patch_proposal_arithmetic' and all(h(k)==v for obj in [prior,audit] for k,v in obj['input_hashes'].items())
ordered=sorted(prior['evidence']['matches'],key=lambda v:(v['squared_distance'],v['source'],v['target']));selected=[]
for row in ordered:
 if not any(math.dist(row['source'],old['source'])<=2 or math.dist(row['target'],old['target'])<=2 for old in selected):selected.append(row)
points=[[v['source'],v['target']] for v in selected];assert len(points)==21
assert [[float(v) for v in line.split()] for line in pinned('dedup-wrinkled-dense-distinct-points.txt').read_text().splitlines()]==[s+t for s,t in points]
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');pins[str(helper)]=h(helper);node=next(v for v in ast.parse(helper.read_text()).body if isinstance(v,ast.FunctionDef) and v.name=='verify_model');ns={'math':math};exec(compile(ast.Module(body=[node],type_ignores=[]),str(helper),'exec'),ns)
assert len(r['results'])==3;summary=[]
for experiment,divisions in zip(r['results'],[1,2,4]):
 assert type(experiment['divisions']) is int and experiment['divisions']==divisions and type(experiment['returncode']) is int and experiment['returncode']==0
 e=experiment['evidence'];assert json.loads(experiment['stdout'])==e and e['status']=='ok' and len(e['regions'])==divisions**2
 expected=[]
 for y in range(divisions):
  for x in range(divisions):
   domain=[x*800/divisions,y*600/divisions,800/divisions,600/divisions];ids=[i for i,(_,t) in enumerate(points) if domain[0]<=t[0]<domain[0]+domain[2] and domain[1]<=t[1]<domain[1]+domain[3]];expected.append((domain,ids))
 for row,(domain,ids) in zip(e['regions'],expected):
  assert row['target_domain']==domain and row['point_indices']==ids and all(type(i) is int for i in row['point_indices'])
  if row['matrix'] is None:assert row['inliers']==[]
  else:
   assert len(row['inliers'])>=10 and all(i in ids for i in row['inliers']);ns['verify_model'](row['matrix'],row['inliers'],points)
  if len(ids)<10:assert row['matrix'] is None
 summary.append({'divisions':divisions,'native_models':sum(v['matrix'] is not None for v in e['regions']),'partition_counts':[len(v['point_indices']) for v in e['regions']]})
assert all(h(k)==v for k,v in pins.items()) and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_dense_distinct_partition_evidence','input_hashes':pins,'distinct_pairs':len(points),'summary':summary,'scope':'Independently reconstructed smallest-distance distinct pair selection and all1/2/4 partitions; typed native outcomes and model residuals when present. No exhaustive geometry nonexistence, descriptor oracle, pixel proof or broad precision.'},indent=2)+'\n')
