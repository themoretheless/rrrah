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
 out=D/'dedup-joint-projective-native-audit-20261011.json';assert not out.exists();rp=D/'dedup-joint-projective-native-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_bounded_public_joint_geometry_native' and len(r['results'])==4;assert all(sha(p)==h for p,h in r['pins'].items());hp=Path(r['harness_manifest']);h=json.loads(hp.read_text());assert all(sha(Path(h['snapshot'])/p)==v for p,v in h['file_hashes'].items());m=json.loads(Path(h['parent_manifest']).read_text());assert all(sha(Path(m['snapshot'])/p)==v for p,v in m['file_hashes'].items());generic=D/'dedup-joint-projective-current-audit-20261011.json';g=json.loads(generic.read_text());assert g['status']=='complete_bounded_current_source_joint_projective_tests' and g['tests']==24;assert sha(Path(h['snapshot'])/'crates/rrrah-dedup/src/geometry.rs')==g['adopted_current']['crates/rrrah-dedup/src/geometry.rs'];pins={str(generic):sha(generic),str(rp):sha(rp),str(hp):sha(hp),str(Path(__file__)):sha(Path(__file__))};results=[]
 for row,result in zip(r['plan'],r['results']):
  assert row['case']==result['case'] and result['status']=='supported' and result['returncode']==0;assert sha(result['log'])==result['log_sha256'];assert Path(result['log']).read_text()==result['stdout']+result['stderr'];pins[result['log']]=result['log_sha256'];inp=json.loads(Path(row['input']).read_text());value=json.loads(result['stdout']);trace=json.loads(Path(row['trace_report']).read_text());v=json.loads(trace['results'][row['trace_index']]['stdout']);actual=v['correspondences'];actual=[{'source':p['target'],'target':p['source']} for p in actual] if row['trace_index']==0 else actual;assert len(actual)==value['points']==len(inp['points'])
  for original,p in zip(actual,inp['points']):
   for key in ['source','target']:assert p[key]==original[key] and p[key+'_bits']==[struct.unpack('>Q',struct.pack('>d',x))[0] for x in original[key]]
  assert not value['alias_pairs'] and value['managed_used']==0 and value['source_tolerance']==.5 and value['target_tolerance']==2.;best=value['best'];H=best['transform'];inv=inverse(H);joint=[];score=0.
  for i,p in enumerate(actual):
   f=math.dist(apply(H,p['source']),p['target']);b=math.dist(apply(inv,p['target']),p['source']);
   if f<=2. and b<=.5:joint.append(i);score+=f*f/4.+b*b/.25
  assert joint==best['joint_inliers'] and len(joint)>=10;assert math.isclose(score,best['normalized_bidirectional_error'],rel_tol=1e-9,abs_tol=1e-9);pixels=value['anchor_result'];assert pixels['supported'] is True and min(pixels['anchors'])>=10
  for d in pixels['directions']:assert d['informative_pairs']>=1000 and d['valid_sites']/d['sites']>=.3 and d['agreeing_pairs']/d['informative_pairs']>=.9
  results.append({'case':row['case'],'points':len(actual),'joint_inliers':len(joint),'anchors':pixels['anchors'],'decimal_parser_bit_differences':value['decimal_bit_differences'],'elapsed_seconds':result['elapsed_seconds']})
 out.write_text(json.dumps({'status':'verified_bounded_public_joint_projective_native','pins':pins,'results':results,'scope':'Public joint-score geometry primitive plus joint-admitted refinement on four exact-bit real point sets, independently checked strict residuals and original-pixel anchors. Current owned geometry source matches captured24-test generic gate. Frozen compatible decoder source, not concurrent root HDR edits. No automatic file-search integration, full corpus, geometry scratch managed credit, provider/RSS/platform qualification.'},indent=2)+'\n');print(json.dumps(results))
if __name__=='__main__':main()
