#!/usr/bin/env python3
import hashlib,json,math
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def apply(h,p):
 w=sum(h[2][i]*v for i,v in enumerate([*p,1.]));assert math.isfinite(w) and w!=0.;return [sum(h[j][i]*v for i,v in enumerate([*p,1.]))/w for j in range(2)]
def inverse(h):
 a,b,c=h[0];d,e,f=h[1];g,i,j=h[2];return [[e*j-f*i,c*i-b*j,b*f-c*e],[f*g-d*j,a*j-c*g,c*d-a*f],[d*i-e*g,b*g-a*i,a*e-b*d]]
def joint(points,h,source_tol):
 inv=inverse(h);return sum(math.dist(apply(h,p['source']),p['target'])<=2. and math.dist(apply(inv,p['target']),p['source'])<=source_tol for p in points)
def main():
 out=D/'dedup-native-direction-trace-204100-audit-20261011.json';assert not out.exists();rp=D/'dedup-native-direction-trace-204100-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_bounded_native_direction_trace' and len(r['results'])==2;assert all(sha(p)==h for p,h in r['pins'].items());m=json.loads(Path(r['source_manifest']).read_text());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());ap=D/'dedup-loftr-publisher-positives-ordering-prefix-0200-audit-20261011.json';a=json.loads(ap.read_text());b=json.loads(Path(a['captured_report']).read_text());fwd=next(b['results'][i] for i,p in enumerate(b['plan'][:200]) if p['group_id']==204100 and p['direction']=='forward');prior=json.loads(fwd['stdout'])['pairs'][0]['evidence'];values=[];pins={str(rp):sha(rp),str(ap):sha(ap),str(Path(__file__)):sha(Path(__file__))};observations=[]
 for index,row in enumerate(r['results']):
  assert row['phase']==['forward','0'][index] and row['status']=='observed' and row['returncode']==0;assert sha(row['log'])==row['log_sha256'] and Path(row['log']).read_text()==row['stdout']+row['stderr'];pins[row['log']]=row['log_sha256'];v=json.loads(row['stdout']);values.append(v);assert v['managed_used']==0;assert v['source_tolerance']==(8. if index==0 else .5);g=v['geometry'];assert g
  admitted=[i for i,p in enumerate(v['correspondences']) if math.dist(apply(g['transform'],p['source']),p['target'])<=2.];assert set(admitted)==set(g['inliers']);observations.append({'phase':row['phase'],'points':len(v['correspondences']),'target_only_inliers':len(admitted),'joint_admissible':joint(v['correspondences'],g['transform'],v['source_tolerance']),'anchor_result':v['anchor_result']})
 f=values[0];assert f['geometry']['transform']==prior['transform'] and f['anchor_result']==prior['pixels'];assert f['swapped_actual_correspondence_anchor_result']=={'error':'Rank(InsufficientGeometry)'};swapped=[{'source':p['target'],'target':p['source']} for p in f['correspondences']];inverse_joint=joint(swapped,f['inverse_transform'],.5);assert inverse_joint<10;assert values[1]['anchor_result']=={'error':'Rank(InsufficientGeometry)'} and observations[1]['joint_admissible']<10;out.write_text(json.dumps({'status':'verified_bounded_204100_direction_trace','pins':pins,'observations':observations,'opposite_inverse_joint_admissible':inverse_joint,'scope':'Actual213 opposite proposals and211 direct reverse proposals with target-only selected transforms. Strict recorded reverse thresholds admit fewer than10 joint witnesses for both. Does not prove no alternative transform could support, or qualify a new fitter.'},indent=2)+'\n');print(json.dumps(observations));print('Inverse joint:',inverse_joint)
if __name__=='__main__':main()
