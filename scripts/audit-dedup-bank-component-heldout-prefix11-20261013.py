#!/usr/bin/env python3
import json,hashlib,math
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-bank-component-heldout-plan-20261013.json';O=D/'dedup-bank-component-heldout-prefix11-20261013.json';plan=json.loads(P.read_text());data=json.loads(O.read_text())
assert data['status']=='running' and data['completed']==11 and data['required']==12 and data['plan_sha256']==sha(P)
A=Path(plan['source_audit']);a=json.loads(A.read_text());S=Path(a['source']);H=Path(plan['harness'])
assert sha(A)==data['source_audit_sha256'] and all(sha(S/p)==h for p,h in a['source_sha256'].items())
assert all(sha(H/p)==h for p,h in plan['harness_sha256'].items()) and sha(H/'Cargo.lock')==data['lock_sha256']
assert sha(plan['bank_audit'])==plan['bank_audit_sha256'] and sha(data['executable'])==data['executable_sha256'] and sha(data['build_log'])==data['build_log_sha256']
def inverse(m):
 aug=[list(row)+[float(i==j) for j in range(3)] for i,row in enumerate(m)]
 for col in range(3):
  pivot=max(range(col,3),key=lambda i:abs(aug[i][col]));aug[col],aug[pivot]=aug[pivot],aug[col];divisor=aug[col][col];assert divisor!=0 and math.isfinite(divisor);aug[col]=[x/divisor for x in aug[col]]
  for row in range(3):
   if row!=col:
    factor=aug[row][col];aug[row]=[x-factor*y for x,y in zip(aug[row],aug[col])]
 return [row[3:] for row in aug]
def apply(m,xy):
 z=[sum(a*b for a,b in zip(row,[xy[0],xy[1],1.])) for row in m];assert z[2]!=0 and all(math.isfinite(x) for x in z);return [z[0]/z[2],z[1]/z[2]]
rows=[]
for c,r in zip(plan['cases'],data['results']):
 assert c['label']==r['label'] and sha(c['plan'])==c['plan_sha256'] and sha(c['case_report'])==c['case_report_sha256'] and sha(r['report'])==r['report_sha256']
 case=json.loads(Path(r['report']).read_text());assert case['returncode']==0;v=json.loads(case['stdout']);assert v['managed_used']==0
 cp=json.loads(Path(c['plan']).read_text());old=json.loads(json.loads(Path(c['case_report']).read_text())['stdout']);flat=[p for b in old['banks'] for p in b['points']];assignments=old['assignments'];groups={}
 for p,a in zip(flat,assignments):groups.setdefault(a[2],[]).append(p)
 sizes=[sum(len(points) for component,points in groups.items() if component%2==side) for side in (0,1)];assert sizes==v['split_sizes']
 assert v['required_projection_checks']==2*(sizes[0]*cp['trials']+sizes[1])
 e=v['result'];assert e['terminal']=='fit_and_validation',e
 m=e['matrix'];inv=inverse(m);tol=cp['source_tolerance'];support=0;error=0.;joint_points=0;fcount=bcount=jcount=0;mf=mb=0.
 for component,points in groups.items():
  residuals=[]
  for source,target in points:
   q=apply(m,source);b=apply(inv,target);f=math.hypot(q[0]-target[0],q[1]-target[1]);rerr=math.hypot(b[0]-source[0],b[1]-source[1]);assert math.isfinite(f) and math.isfinite(rerr);residuals.append((f,rerr))
  if component%2==0:
   eligible=[(f/2)**2+(b/tol)**2 for f,b in residuals if f<=2 and b<=tol]
   if eligible:support+=1;error+=min(eligible)
  else:
   fcount+=any(f<=2 for f,b in residuals);bcount+=any(b<=tol for f,b in residuals);jcount+=any(f<=2 and b<=tol for f,b in residuals);joint_points+=sum(f<=2 and b<=tol for f,b in residuals);mf=max(mf,max(f for f,b in residuals));mb=max(mb,max(b for f,b in residuals))
 assert support==e['fitting_components'] and math.isclose(error,e['fitting_normalized_squared_error'],rel_tol=1e-7,abs_tol=1e-7)
 held=e['validation'];assert (held['points'],held['components'],held['forward_components'],held['reverse_components'],held['joint_components'],held['joint_points'])==(sizes[1],len(groups)//2,fcount,bcount,jcount,joint_points)
 assert math.isclose(mf,held['maximum_forward_error'],rel_tol=1e-7,abs_tol=1e-7) and math.isclose(mb,held['maximum_reverse_error'],rel_tol=1e-7,abs_tol=1e-7)
 assert e['hypotheses']==cp['trials']
 rows.append(dict(label=c['label'],fitting_components=support,validation_joint_components=jcount,validation_components=len(groups)//2,validation_joint_points=joint_points,split_sizes=sizes,independent_scalar_residual_check=True,elapsed_seconds=case['elapsed_seconds'],rss_bytes=case['rss_bytes'],report_sha256=r['report_sha256']))
out=D/'dedup-bank-component-heldout-prefix11-audit-20261013.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified11_different_file_component_heldout_diagnostics_self_control_still_running',plan_sha256=sha(P),producer_sha256=sha(O),source_audit_sha256=sha(A),rows=rows,scope='Independent scalar Gaussian inverse, every fitting/validation residual, per-component minimum training error and same-member joint validation votes. Minimal training-only component hypothesis; no refinement, pixel/statistical independence, calibrated precision or acceptance change.'),indent=2)+'\n');print(json.dumps(rows,indent=2))
