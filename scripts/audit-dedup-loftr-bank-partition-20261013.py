#!/usr/bin/env python3
import json, hashlib, math
from pathlib import Path
R=Path(__file__).resolve().parents[1]; D=R/'docs/research'
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-loftr-bank-partition-plan-20261013.json'; O=D/'dedup-loftr-bank-partition-20261013.json'
plan=json.loads(P.read_text()); data=json.loads(O.read_text())
assert data['status']=='complete_fixed12_bank_partition' and data['completed']==data['required']==12
assert data['source_unchanged'] and data['harness_unchanged'] and sha(P)==data['plan_sha256']
A=Path(plan['source_audit']); a=json.loads(A.read_text()); S=Path(a['source']); H=Path(plan['harness'])
assert sha(A)==data['source_audit_sha256']
assert all(sha(S/p)==h for p,h in a['source_sha256'].items())
assert all(sha(H/p)==h for p,h in plan['harness_sha256'].items())
assert all(sha(p)==h for p,h in plan['pins'].items())
assert sha(H/'Cargo.lock')==data['lock_sha256'] and sha(data['executable'])==data['executable_sha256']
assert sha(data['build_log'])==data['build_log_sha256']
rows=[]
for c,r in zip(plan['cases'],data['results']):
 assert c['label']==r['label'] and sha(c['plan'])==c['plan_sha256'] and sha(r['report'])==r['report_sha256']
 case=json.loads(Path(r['report']).read_text()); assert case['returncode']==0
 v=json.loads(case['stdout']); assert v['managed_used']==0 and len(v['banks'])==25
 points=[]; origin=[]
 for bi,b in enumerate(v['banks']):
  assert b['bank']==bi
  for pi,p in enumerate(b['points']):
   assert all(math.isfinite(x) for xy in p for x in xy)
   assert all(0<=p[side][axis]<v['dimensions'][side][axis] for side in (0,1) for axis in (0,1))
   points.append(p); origin.append([bi,pi])
 n=len(points); assert n==v['selected_points']
 # Independent spatial-grid adjacency and graph flood fill, rather than all-pair union-find.
 adjacency=[set() for _ in points]
 for side in (0,1):
  grid={}
  for i,p in enumerate(points):
   x,y=p[side]; cell=(math.floor(x/2),math.floor(y/2))
   for dx in (-1,0,1):
    for dy in (-1,0,1):
     for j in grid.get((cell[0]+dx,cell[1]+dy),[]):
      q=points[j][side]
      if math.hypot(x-q[0],y-q[1])<=2:
       adjacency[i].add(j); adjacency[j].add(i)
   grid.setdefault(cell,[]).append(i)
 labels=[None]*n; sizes=[]
 for i in range(n):
  if labels[i] is not None: continue
  label=len(sizes); stack=[i]; labels[i]=label; size=0
  while stack:
   j=stack.pop(); size+=1
   for k in adjacency[j]:
    if labels[k] is None: labels[k]=label; stack.append(k)
  sizes.append(size)
 expected=[origin[i]+[labels[i],labels[i]%2] for i in range(n)]
 assert expected==v['assignments']
 s=v['summary']; assert s['components']==len(sizes) and s['component_sizes']==sizes
 assert s['fitting_points']==sum(size for i,size in enumerate(sizes) if i%2==0)
 assert s['validation_points']==n-s['fitting_points'] and s['separation_checks']==n*(n-1)//2
 assert s['parent_steps']<=s['maximum_parent_steps']
 ids=s['original_model_witness_components']; assert len(ids)==s['original_model_witnesses']
 assert len(set(ids))==s['original_model_unique_witness_components'] and all(0<=i<len(sizes) for i in ids)
 cp=json.loads(Path(c['plan']).read_text())
 rows.append(dict(label=c['label'],selected_points=n,components=len(sizes),largest_component=max(sizes,default=0),fitting_points=s['fitting_points'],validation_points=s['validation_points'],old_transform_witnesses=len(ids),old_transform_witness_components=len(set(ids)),expected_original_witnesses=cp['expected_original_witnesses'],original_witness_count_matches=cp['expected_original_witnesses']==len(ids) if cp['expected_original_witnesses'] is not None else None,independent_graph_replay=True,report_sha256=r['report_sha256']))
out=D/'dedup-loftr-bank-partition-audit-20261013.json'; assert not out.exists()
out.write_text(json.dumps(dict(status='verified12_bank_component_diagnostics_not_acceptance_fix',plan_sha256=sha(P),report_sha256=sha(O),source_audit_sha256=sha(A),rows=rows,scope='Independent spatial-grid adjacency and flood-fill replay of all coordinates and assignments. Old transform witness counts are observed provider results, not independently recomputed residuals. No new fitting, pixel independence, acceptance fix or full458 preservation claim.'),indent=2)+'\n')
print(json.dumps(rows,indent=2))
