#!/usr/bin/env python3
import hashlib,json,math
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def apply(h,p):
 w=h[2][0]*p[0]+h[2][1]*p[1]+h[2][2];assert math.isfinite(w) and abs(w)>1e-12
 return [(h[j][0]*p[0]+h[j][1]*p[1]+h[j][2])/w for j in range(2)]
def inverse(h):
 a,b,c=h[0];d,e,f=h[1];g,i,j=h[2];v=[[e*j-f*i,c*i-b*j,b*f-c*e],[f*g-d*j,a*j-c*g,c*d-a*f],[d*i-e*g,b*g-a*i,a*e-b*d]];det=a*v[0][0]+b*v[1][0]+c*v[2][0];assert abs(det)>1e-12
 return [[x/det for x in row] for row in v]
def pixel_proof(p):
 assert p['supported'] is True and all(n>=10 for n in p['anchors']) and len(p['directions'])==2
 for v in p['directions']:assert v['informative_pairs']>=1000 and v['sites']>0 and v['valid_sites']/v['sites']>=.3 and v['agreeing_pairs']/v['informative_pairs']>=.9

def main():
 rp=D/'dedup-native-direction-trace-211900-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_bounded_native_direction_trace' and len(r['results'])==7;assert all(sha(p)==h for p,h in r['pins'].items());mp=Path(r['source_manifest']);m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());assert sha(m['parent_manifest'])==m['parent_sha256'];fp=D/'dedup-directional-211900-211901-fixture-20261011.json';fixture=json.loads(fp.read_text());observations=[];values=[]
 for index,row in enumerate(r['results']):
  assert row['phase']==(['forward']+list(map(str,range(6))))[index] and row['status']=='observed' and row['returncode']==0;assert sha(row['log'])==row['log_sha256'] and Path(row['log']).read_text()==row['stdout']+row['stderr'];v=json.loads(row['stdout']);values.append(v);assert v['managed_used']==0;size_a,size_b=([2560,1920],[640,480]) if index==0 else ([640,480],[2560,1920]);assert v['source_tolerance']==(8 if index==0 else .5)
  for point in v['correspondences']:
   assert all(math.isfinite(x) for x in point['source']+point['target']);assert all(0<=x<limit for x,limit in zip(point['source'],size_a));assert all(0<=x<limit for x,limit in zip(point['target'],size_b))
  g=v['geometry'];joint=None
  if g:
   h=g['transform'];inv=inverse(h);admitted=[i for i,p in enumerate(v['correspondences']) if math.dist(apply(h,p['source']),p['target'])<=2];assert set(admitted)==set(g['inliers']);assert len(g['inliers'])>=10
   joint=sum(math.dist(apply(h,p['source']),p['target'])<=2 and math.dist(apply(inv,p['target']),p['source'])<=v['source_tolerance'] for p in v['correspondences'])
   if index==0:pixel_proof(v['anchor_result'])
   else:assert v['anchor_result']=={'error':'Rank(InsufficientGeometry)'} and joint<10
  else:assert 'anchor_result' not in v
  observations.append({'phase':row['phase'],'correspondences':len(v['correspondences']),'forward_residual_inliers':len(g['inliers']) if g else 0,'joint_residual_admissible_before_anchor_selection':joint,'anchor_result':v.get('anchor_result')})
 f=values[0];assert f['geometry']['transform']==fixture['forward']['result']['evidence']['transform'];assert f['anchor_result']==fixture['forward']['result']['evidence']['pixels'];pixel_proof(f['swapped_actual_correspondence_anchor_result']);assert f['swapped_actual_correspondence_anchor_result']['anchors']==[11,11];inv=f['inverse_transform'];h=f['geometry']['transform'];product=[[sum(h[i][k]*inv[k][j] for k in range(3)) for j in range(3)] for i in range(3)];scale=product[0][0];assert math.isfinite(scale) and abs(scale)>1e-12;assert max(abs(product[i][j]/scale-(1 if i==j else 0)) for i in range(3) for j in range(3))<1e-9
 swapped=[{'source':p['target'],'target':p['source']} for p in f['correspondences']];inverse_joint=sum(math.dist(apply(inv,p['source']),p['target'])<=2 and math.dist(apply(h,p['target']),p['source'])<=.5 for p in swapped);assert inverse_joint>=11
 a={(tuple(p['source']),tuple(p['target'])) for p in swapped};b={(tuple(p['source']),tuple(p['target'])) for p in values[1]['correspondences']};shared=len(a&b);assert shared==96 and len(a)==len(b)==113
 tp=D/'dedup-native-direction-tolerance-20261011.json';t=json.loads(tp.read_text());assert t['status']=='complete_bounded_native_direction_tolerance_control' and t['returncode']==0 and all(sha(p)==v for p,v in t['pins'].items());assert sha(t['log'])==t['log_sha256'] and Path(t['log']).read_text()==t['stdout']+t['stderr'];tm=json.loads(Path(t['source_manifest']).read_text());assert sha(tm['parent_manifest'])==tm['parent_sha256'] and all(sha(Path(tm['harness'])/p)==v for p,v in tm['file_hashes'].items());tv=json.loads(t['stdout']);assert tv['managed_used']==0 and tv['correspondences']==f['correspondences'] and tv['geometry']==f['geometry'] and tv['swapped_actual_correspondence_anchor_result']==f['swapped_actual_correspondence_anchor_result'];role=tv['role_swapped_original_policy_anchor_result'];assert tv['role_swapped_original_policy']=={'source_tolerance':2.,'target_tolerance':8.};pixel_proof(role);assert role['anchors']==list(reversed(f['anchor_result']['anchors'])) and role['directions']==list(reversed(f['anchor_result']['directions']))
 out=D/'dedup-native-direction-trace-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_bounded_opposite_proposal_inversion_support','results':observations,'actual_forward_correspondences':113,'actual_shared_swapped_correspondences':shared,'inverse_forward_geometry_joint_admissible':inverse_joint,'recorded_reverse_policy_anchors':f['swapped_actual_correspondence_anchor_result']['anchors'],'role_swapped_policy_anchors':role['anchors'],'pins':{str(p):sha(p) for p in [rp,mp,fp,tp,Path(t['source_manifest']),Path(__file__)]},'conclusion':'Recorded reverse thresholds can confirm this pair using actual opposite-direction proposals and inverse geometry. Three reverse fitted geometries have only5/4/7 jointly admissible points despite70/41/39 target-only inliers; other three have no geometry. Opposite point sets overlap96/113. Automatic bounded opposite-proposal fallback is not yet integrated.','scope':'One publisher pair, native source unchanged, seven recipe traces and one tolerance-role control. Not full corpus/negative precision/hard RSS/platform or automatic search fix. Role-swapped policy control is not a replacement for the stricter original replay policy.'},indent=2)+'\n');print('verified opposite-proposal inverse proof, unchanged reverse policy, anchors11/11')
if __name__=='__main__':main()
