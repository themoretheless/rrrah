#!/usr/bin/env python3
import hashlib,json,math,struct
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def apply(h,p):
 v=[sum(h[i][j]*[p[0],p[1],1][j] for j in range(3)) for i in range(3)];assert v[2]!=0;return [v[0]/v[2],v[1]/v[2]]
def inverse(h):
 a,b,c=h[0];d,e,f=h[1];g,i,j=h[2];v=[[e*j-f*i,c*i-b*j,b*f-c*e],[f*g-d*j,a*j-c*g,c*d-a*f],[d*i-e*g,b*g-a*i,a*e-b*d]];det=a*v[0][0]+b*v[1][0]+c*v[2][0];assert det!=0;return [[x/det for x in row] for row in v]
def main():
 p=D/'dedup-joint-consensus-200301-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete_bounded_sampled_consensus_diagnostic';assert len(r['results'])==len(r['plan'])==25;pins={**r['pins'],str(p):sha(p),str(Path(__file__)):sha(__file__)};assert all(sha(k)==v for k,v in pins.items());counts=[]
 for index,(inp,row) in enumerate(zip(r['plan'],r['results'])):
  assert row['window_index']==index and row['status']=='complete' and row['returncode']==0
  v=json.loads(row['stdout']);assert v['diagnostic_min_inliers']==4 and v['trials']==1000000 and v['seed']==173 and v['target_tolerance']==2
  q=json.loads(Path(inp).read_text());assert v['source_tolerance']==q['source_tolerance'];pts=[]
  for point in q['points']:
   endpoints=[[struct.unpack('>d',struct.pack('>Q',b))[0] for b in point[k+'_bits']] for k in ['source','target']];pts.append(endpoints)
  assert v['points']==len(pts)
  best=v['best'];n=0
  if best:
   h=best['transform'];inv=inverse(h);ids=best['joint_inliers'];assert len(set(ids))==len(ids)>=4 and all(0<=i<len(pts) for i in ids)
   for i in ids:
    a,b=pts[i];assert math.dist(apply(h,a),b)<=2+1e-8;assert math.dist(apply(inv,b),a)<=q['source_tolerance']+1e-8
   n=len(ids)
  assert n==row['joint_inliers'];counts.append(n)
 out=D/'dedup-joint-consensus-200301-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_25_sampled_consensus_diagnostics','pins':pins,'joint_counts':counts,'highest_sampled_consensus':max(counts),'scope':'Independent original bidirectional residual checks on every reported inlier. Diagnostic min4; no lowered production acceptance, exhaustive upper bound, pixel support or full corpus claim.'},indent=2)+'\n');print(counts)
if __name__=='__main__':main()
