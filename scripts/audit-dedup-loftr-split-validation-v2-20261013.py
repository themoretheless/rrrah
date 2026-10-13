#!/usr/bin/env python3
import json,hashlib,math
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-loftr-split-validation-v2-plan-20261013.json';O=D/'dedup-loftr-split-validation-20261013.json';plan=json.loads(P.read_text());data=json.loads(O.read_text());assert data['status']=='complete_fixed12_split_validation' and data['completed']==data['required']==12 and data['source_unchanged'] and data['harness_unchanged'];assert data['plan_sha256']==sha(P)
A=Path(plan['source_audit']);a=json.loads(A.read_text());S=Path(a['source']);assert sha(A)==data['source_audit_sha256'] and all(sha(S/p)==h for p,h in a['source_sha256'].items());H=Path(plan['harness']);assert all(sha(H/p)==h for p,h in plan['harness_sha256'].items());assert all(sha(p)==h for p,h in plan['pins'].items());assert sha(H/'Cargo.lock')==data['lock_sha256'];assert sha(data['executable'])==data['executable_sha256'] and sha(data['build_log'])==data['build_log_sha256']
rows=[]
for c,r in zip(plan['cases'],data['results']):
 assert c['label']==r['label'] and sha(c['plan'])==c['plan_sha256'] and sha(r['report'])==r['report_sha256'];case=json.loads(Path(r['report']).read_text());assert case['returncode']==0;v=json.loads(case['stdout']);assert v['managed_used']==0 and v['dimensions']==c['dimensions'];points=v['selected_points'];assert all(math.isfinite(x) for point in points for endpoint in point for x in endpoint);n=len(points);assert n<=5000 and v['split_sizes']==[(n+1)//2,n//2];assert r['terminal']==v['result']['terminal']=='fit_and_validation'
 for side in [0,1]:assert all(0<=point[side][axis]<c['dimensions'][side][axis] for point in points for axis in [0,1])
 m=v['result']['matrix']
 aug=[list(row)+[float(i==j) for j in range(3)] for i,row in enumerate(m)]
 for col in range(3):
  pivot=max(range(col,3),key=lambda i:abs(aug[i][col]));aug[col],aug[pivot]=aug[pivot],aug[col];divisor=aug[col][col];assert divisor!=0 and math.isfinite(divisor);aug[col]=[x/divisor for x in aug[col]]
  for row in range(3):
   if row!=col:
    factor=aug[row][col];aug[row]=[x-factor*y for x,y in zip(aug[row],aug[col])]
 inv=[row[3:] for row in aug]
 def residual(points):
  def apply(mat,xy):
   z=[sum(a*b for a,b in zip(row,[xy[0],xy[1],1.])) for row in mat];assert z[2]!=0 and all(math.isfinite(x) for x in z);return [z[0]/z[2],z[1]/z[2]]
  f=[];b=[]
  for source,target in points:
   q=apply(m,source);r=apply(inv,target);f.append(math.hypot(q[0]-target[0],q[1]-target[1]));b.append(math.hypot(r[0]-source[0],r[1]-source[1]))
  assert all(math.isfinite(x) for x in f+b);return f,b
 train=points[::2];held=points[1::2];f,b=residual(held);reported=v['result']['validation'];fa=[x<=2. for x in f];ba=[x<=c['source_tolerance'] for x in b];assert reported['points']==len(held) and reported['forward_inliers']==sum(fa) and reported['reverse_inliers']==sum(ba) and reported['joint_inliers']==sum(a and b for a,b in zip(fa,ba));assert math.isclose(reported['maximum_forward_error'],max(f),rel_tol=1e-8,abs_tol=1e-8) and math.isclose(reported['maximum_reverse_error'],max(b),rel_tol=1e-8,abs_tol=1e-8);assert reported['separation_checks']==n*(n-1)//2 and reported['projection_checks']==2*len(held)
 f,b=residual(train);indices=[i for i,(x,y) in enumerate(zip(f,b)) if x<=2. and y<=c['source_tolerance']];assert indices==v['result']['fitting_inliers'];assert v['result']['hypotheses']==1000000
 rows.append(dict(label=c['label'],raw_proposals=v['raw_proposals'],selected_points=n,split_sizes=v['split_sizes'],fitting_inliers=len(indices),validation_joint_inliers=reported['joint_inliers'],validation_points=len(held),independent_scalar_residual_check=True,rss_bytes=case['rss_bytes'],elapsed_seconds=case['elapsed_seconds'],case_report_sha256=r['report_sha256']))
# Trace the actual older cascade that produced the known false acceptance.
old_gate=json.loads((D/'dedup-current-decode-gate-20261012.json').read_text());old_source=Path(old_gate['source']);trace={}
for rel in ['crates/rrrah-dedup/src/search_portfolio.rs','crates/rrrah-dedup/src/model_banks.rs']:
 p=old_source/rel;assert sha(p)==old_gate['source_hashes'][rel];trace[rel]=sha(p)
text=(old_source/'crates/rrrah-dedup/src/search_portfolio.rs').read_text();start=text.index('pub fn search_native_bank_recovery_files_with(');block=text[start:];assert 'search_model_banks_coverage_files_with' in block and 'NativeBankRecoveryTerminal::Model(e.comparison)' in block
out=D/'dedup-loftr-split-validation-audit-v2-20261013.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified12_native_full_frame_split_diagnostics_not_acceptance_fix',prior_vector_audit_sha256=sha(D/'dedup-loftr-split-validation-audit-20261013.json'),verifier_correction='The vectorized verifier emitted floating-status warnings despite matching finite results. This separate scalar Gaussian-elimination/projection/hypot verification uses no BLAS and retains all prior artifacts.',plan_sha256=sha(P),producer_report_sha256=sha(O),source_files=len(a['source_sha256']),source_audit_sha256=sha(A),executable_sha256=data['executable_sha256'],manifest_sha256=sha(H/'Cargo.toml'),lock_sha256=data['lock_sha256'],build_log_sha256=data['build_log_sha256'],independent_verifier='Scalar Python float Gaussian elimination, projection and math.hypot; no NumPy or BLAS',rows=rows,original_cascade_source_pins=trace,original_cascade_trace='Frozen executed native/opposite cascade calls model_banks::search_model_banks_coverage_files_with; terminal Model(e.comparison) is bank comparison evidence, not proof of a direct primary full-frame accept.',scope='Actual full-frame provider plus predeclared even/odd train/validation, all12 native executions and saved original-coordinate proposals; independent scalar residual recomputation. Does not reproduce 25-bank aggregation or prove pixel/descriptor/statistical independence, calibrated precision, acceptance fix or preservation of all458 positives.'),indent=2)+'\n');print(out);print([(r['label'],r['fitting_inliers'],r['validation_joint_inliers'],r['validation_points']) for r in rows])
