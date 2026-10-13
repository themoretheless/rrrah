#!/usr/bin/env python3
import hashlib,json,math,struct
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def apply(h,p):
 q=[sum(row[i]*v for i,v in enumerate([*p,1.])) for row in h];assert all(math.isfinite(x) for x in q) and q[2]!=0.;return [q[0]/q[2],q[1]/q[2]]
def inverse(h):
 a,b,c=h[0];d,e,f=h[1];g,i,j=h[2];return [[e*j-f*i,c*i-b*j,b*f-c*e],[f*g-d*j,a*j-c*g,c*d-a*f],[d*i-e*g,b*g-a*i,a*e-b*d]]
def main():
 out=D/'dedup-joint-geometry-sampling-audit-20261011.json';assert not out.exists();rp=D/'dedup-joint-geometry-sampling-patched-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_bounded_joint_sampling_controls' and len(r['results'])==5;assert all(sha(p)==h for p,h in r['pins'].items());mp=D/'dedup-joint-geometry-sampling-patched-harness-20261011.json';m=json.loads(mp.read_text());assert m['registry_dependency_versions_in_parent_lock'];assert all(sha(Path(m['harness'])/f)==h for f,h in m['files'].items());parent=json.loads(Path(m['parent_manifest']).read_text());assert sha(m['parent_manifest'])==m['parent_sha256'];assert all(sha(Path(parent['snapshot'])/f)==h for f,h in parent['file_hashes'].items());oldp=D/'dedup-joint-projective-native-20261011.json';old=json.loads(oldp.read_text());calibration=json.loads(old['results'][0]['stdout']);pins={str(p):sha(p) for p in [rp,mp,oldp,Path(__file__)]};results=[]
 for plan,row in zip(r['plan'],r['results']):
  assert plan['case']==row['case'] and row['returncode']==0;assert sha(row['log'])==row['log_sha256'];assert Path(row['log']).read_text()==row['stdout']+row['stderr'];pins[row['log']]=row['log_sha256'];v=json.loads(row['stdout']);inp=json.loads(Path(plan['input']).read_text());points=[]
  for p in inp['points']:
   q={}
   for key in ['source','target']:
    q[key]=[struct.unpack('>d',struct.pack('>Q',bits))[0] for bits in p[key+'_bits']];assert q[key]==p[key]
   points.append(q)
  assert v['source_tolerance']==.5 and v['target_tolerance']==2 and v['trials']==plan['trials'] and v['seed']==plan['seed']==305441741;assert v['maximum_projection_checks']==2*len(points)*(plan['trials']+1);assert v['managed_used']==0 and not v['alias_pairs'];best=v['best'];count=0
  if best:
   H=best['transform'];inv=inverse(H);joint=[];score=0.
   for i,p in enumerate(points):
    f=math.dist(apply(H,p['source']),p['target']);b=math.dist(apply(inv,p['target']),p['source'])
    if f<=2 and b<=.5:joint.append(i);score+=f*f/4+b*b/.25
   assert joint==best['joint_inliers'] and len(joint)>=10;assert math.isclose(score,best['normalized_bidirectional_error'],rel_tol=1e-9,abs_tol=1e-9);count=len(joint)
  pixels=v['anchor_result']
  if row['status']=='supported':
   assert best and pixels['supported'] is True and min(pixels['anchors'])>=10
   for d in pixels['directions']:assert d['informative_pairs']>=1000 and d['valid_sites']/d['sites']>=.3 and d['agreeing_pairs']/d['informative_pairs']>=.9
  else:assert row['status']=='inconclusive' and (best is None or pixels=={'error':'Rank(InsufficientGeometry)'})
  if plan['case']=='204100_calibration':assert best==calibration['best'] and pixels==calibration['anchor_result']
  results.append({'case':plan['case'],'trials':plan['trials'],'joint_inliers':count,'anchors':pixels.get('anchors') if pixels else None,'status':row['status'],'elapsed_seconds':row['elapsed_seconds']})
 assert [x['status'] for x in results]==['supported','inconclusive','inconclusive','supported','inconclusive'];out.write_text(json.dumps({'status':'verified_bounded_sampling_recovers_207700','pins':pins,'results':results,'scope':'Exact-bit native point sets, production joint fitter, strict source.5/target2 and original pixels independently checked. Calibration exactly matches old transform/error/pixels with parent registry dependency versions. Larger trial budget recovers opposite207700; direct geometry alone still insufficient. Not file extraction/pipeline high-trial integration, all-seed coverage, full corpus or RSS cap.'},indent=2)+'\n');print(json.dumps(results))
if __name__=='__main__':main()
