#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def proof(p):
 assert p['supported'] is True and all(n>=10 for n in p['anchors'])
 for v in p['directions']:assert v['informative_pairs']>=1000 and v['sites']>0 and v['valid_sites']/v['sites']>=.3 and v['agreeing_pairs']/v['informative_pairs']>=.9

def main():
 mp=D/'dedup-opposite-native-capacity-snapshot-20261011.json';m=json.loads(mp.read_text());src=Path(m['snapshot']);assert all(sha(src/f)==h for f,h in m['file_hashes'].items());assert sha(m['parent_manifest'])==m['parent_sha256'];parent=json.loads(Path(m['parent_manifest']).read_text());base=json.loads(Path(parent['parent_manifest']).read_text())
 rp=D/'dedup-opposite-native-20261011.json';r=json.loads(rp.read_text());assert r['status']=='complete_bounded_opposite_native_controls' and len(r['results'])==3;assert all(sha(p)==h for p,h in r['pins'].items());observed=[]
 for index,row in enumerate(r['results']):
  assert row['index']==index and sha(row['log'])==row['log_sha256'] and Path(row['log']).read_text()==row['stdout']+row['stderr'];plan=r['plan'][index];assert row['command'][5:]==[plan['left'],plan['right'],str(plan['source_tolerance'])]
  if row['returncode']==0:
   v=json.loads(row['stdout']);assert v['source_paths']==[plan['left'],plan['right']] and v['source_tolerance']==plan['source_tolerance'] and v['target_tolerance']==2 and v['managed_used']==0;assert 1<=v['attempted_recipes']<=6
   if row['status']=='supported':proof(v['pixels'])
   else:assert row['status']=='not_supported' and (v['pixels'] is None or v['pixels']['supported'] is False)
  else:assert row['status']=='inconclusive' and not row['stdout'] and 'Error: Rank(InsufficientGeometry)' in row['stderr']
  observed.append({'group_id':plan['group_id'],'status':row['status']})
 assert r['results'][0]['status']=='supported';v=json.loads(r['results'][0]['stdout']);trace_path=D/'dedup-native-direction-trace-211900-20261011.json';trace=json.loads(trace_path.read_text());forward=json.loads(trace['results'][0]['stdout']);assert v['points']==[{'source':p['target'],'target':p['source']} for p in forward['correspondences']];assert v['transform']==forward['inverse_transform'] and v['pixels']==forward['swapped_actual_correspondence_anchor_result'] and v['attempted_recipes']==1 and v['reflected_proposal_source'] is False
 log=D/'dedup-opposite-native-capacity-tests-20261011.log';assert '16 passed; 0 failed; 0 ignored' in log.read_text();artifacts=D/'dedup-opposite-native-capacity-test-build-20261011.jsonl';items=[json.loads(s) for s in artifacts.read_text().splitlines()];assert any(v.get('reason')=='compiler-artifact' and v.get('target',{}).get('name')=='rrrah_dedup' and v['profile']['opt_level']=='3' for v in items)
 files=['crates/rrrah-dedup/src/anchor_rank_file.rs','crates/rrrah-dedup/Cargo.toml','crates/rrrah-dedup/tests/opposite_native.rs','scripts/dedup-loftr-native-probe/src/bin/opposite_native_files.rs'];assert all(not (R/f).exists() for f in files[2:]);assert all(sha(R/f)==base['file_hashes'][f] for f in files[:2]);out=D/'dedup-opposite-native-adoption-20261011.json';assert not out.exists();before={f:sha(R/f) for f in files[:2]}
 for f in files:(R/f).parent.mkdir(parents=True,exist_ok=True);(R/f).write_bytes((src/f).read_bytes());assert sha(R/f)==m['file_hashes'][f]
 pinpaths=[mp,rp,trace_path,log,artifacts,Path(__file__),T/'rrrah-opposite-native-capacity-tests-qualified-20261011/opposite_native'];pins={str(p):sha(p) for p in pinpaths};out.write_text(json.dumps({'status':'adopted_bounded_explicit_opposite_native_api','pins':pins,'before':before,'adopted':{f:sha(R/f) for f in files},'tests':16,'new_tests':4,'native_controls':observed,'known_reverse_fixture_anchors':v['pixels']['anchors'],'scope':'Isolated frozen source and compatible baseline lock. Actual root Cargo.lock differs and two independent RAW decoder source changes are preserved; current-workspace gate separate. Explicit additional six recipes, not automatic portfolio/collection integration. Unsupported stage observations do not replace original unknowns or establish negative precision. No hard RSS/platform/full corpus/goal completion claim.'},indent=2)+'\n');print(json.dumps(observed))
if __name__=='__main__':main()
